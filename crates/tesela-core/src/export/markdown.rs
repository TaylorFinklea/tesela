//! Mosaic-level markdown export — Phase 13.B.
//!
//! Two modes share most of the code; portable adds a stripping pass.
//!
//! - **Full**: byte-exact copy of `notes/`, `attachments/`, and
//!   `.tesela/config.toml`. Drop the SQLite cache (markdown is canonical;
//!   `tesela reindex --mosaic <out>` rebuilds it). Re-importing into
//!   Tesela reproduces the original mosaic.
//!
//! - **Portable**: same shape, but each note runs through a stripping
//!   pass that removes Tesela-internal properties (Apple Reminders sync
//!   state, source-* import markers, etc) so the output opens cleanly
//!   in Obsidian or Logseq. Wikilinks and #tags are kept verbatim —
//!   they're universal.

use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::error::{Result, TeselaError};

/// Which export shape to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarkdownMode {
    /// Byte-exact, round-trippable.
    #[default]
    Full,
    /// Lossy, opens cleanly in Obsidian/Logseq.
    Portable,
}

#[derive(Debug, Default, Clone)]
pub struct ExportOptions {
    pub mode: MarkdownMode,
    pub include_attachments: bool,
}

#[derive(Debug, Default, Clone)]
pub struct ExportOutcome {
    pub note_count: usize,
    pub attachment_count: usize,
    pub stripped_property_count: usize,
}

/// Walk a mosaic and write a portable copy to `out_root`. The output
/// has the same `notes/` + `attachments/` layout as the source. In
/// portable mode we also write a `README.md` at the export root so a
/// human opening the directory understands what was stripped.
pub fn export_mosaic(
    mosaic_root: &Path,
    out_root: &Path,
    opts: &ExportOptions,
) -> Result<ExportOutcome> {
    if !mosaic_root.exists() {
        return Err(TeselaError::NoteNotFound {
            identifier: format!("mosaic {}", mosaic_root.display()),
        });
    }
    fs::create_dir_all(out_root)?;

    let mut outcome = ExportOutcome::default();

    let notes_src = mosaic_root.join("notes");
    let notes_out = out_root.join("notes");
    if notes_src.exists() {
        for entry in WalkDir::new(&notes_src) {
            let entry = entry.map_err(walk_err)?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&notes_src)
                .expect("walk under notes_src");
            let dst = notes_out.join(rel);
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            match opts.mode {
                MarkdownMode::Full => {
                    fs::copy(entry.path(), &dst)?;
                }
                MarkdownMode::Portable => {
                    let raw = fs::read_to_string(entry.path())?;
                    let (stripped, removed) = strip_for_portable(&raw);
                    outcome.stripped_property_count += removed;
                    fs::write(&dst, stripped)?;
                }
            }
            outcome.note_count += 1;
        }
    }

    if opts.include_attachments {
        let attach_src = mosaic_root.join("attachments");
        let attach_out = out_root.join("attachments");
        if attach_src.exists() {
            for entry in WalkDir::new(&attach_src) {
                let entry = entry.map_err(walk_err)?;
                if !entry.file_type().is_file() {
                    continue;
                }
                let rel = entry
                    .path()
                    .strip_prefix(&attach_src)
                    .expect("walk under attach_src");
                let dst = attach_out.join(rel);
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(entry.path(), &dst)?;
                outcome.attachment_count += 1;
            }
        }
    }

    if matches!(opts.mode, MarkdownMode::Full) {
        // Carry over `.tesela/config.toml` so a `tesela reindex` against
        // the export reproduces the original mosaic. The SQLite DB itself
        // is intentionally NOT exported — markdown is canonical.
        let config_src = mosaic_root.join(".tesela/config.toml");
        if config_src.exists() {
            let dst = out_root.join(".tesela/config.toml");
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&config_src, &dst)?;
        }
    } else {
        write_portable_readme(out_root, &outcome)?;
    }

    Ok(outcome)
}

fn walk_err(e: walkdir::Error) -> TeselaError {
    e.into_io_error()
        .map(TeselaError::from)
        .unwrap_or_else(|| TeselaError::Other("walkdir error without underlying io".to_string()))
}

/// Strip Tesela-internal properties from a markdown note. Returns the
/// stripped text plus the count of properties removed.
///
/// Removed:
/// - Inline `key:: value` lines whose key matches one of the
///   well-known Tesela-internal property names.
/// - Inline properties whose key starts with an underscore (Tesela
///   convention for "private" / draft properties).
/// - Frontmatter keys: `created`, `modified` (Obsidian users will get
///   filesystem timestamps anyway), and any key matching the same
///   patterns as the inline strippers.
///
/// Kept:
/// - All other inline properties (status, deadline, scheduled,
///   recurring, priority, user-defined tag properties, etc).
/// - All wikilinks and #tags.
/// - All other frontmatter keys.
pub fn strip_for_portable(content: &str) -> (String, usize) {
    let (frontmatter, body) = split_frontmatter(content);
    let mut removed = 0;
    let mut out = String::with_capacity(content.len());

    if let Some(fm) = frontmatter {
        let (filtered, fm_removed) = strip_frontmatter(fm);
        removed += fm_removed;
        out.push_str("---\n");
        out.push_str(&filtered);
        out.push_str("---\n");
    }

    for line in body.split_inclusive('\n') {
        if let Some((key, _)) = parse_property_line(line) {
            if is_tesela_internal_key(key) {
                removed += 1;
                continue;
            }
        }
        out.push_str(line);
    }

    (out, removed)
}

/// Split content into (Some(frontmatter_yaml), body) when the file
/// starts with a `---\n ... \n---\n` block, else (None, full_content).
/// The frontmatter slice excludes both fences. The body slice is what
/// follows the closing fence (no leading newline beyond what was in
/// the file).
fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    let after_open = if content.starts_with("---\n") {
        4
    } else if content.starts_with("---\r\n") {
        5
    } else {
        return (None, content);
    };
    let rest = &content[after_open..];
    match find_frontmatter_close(rest) {
        Some(close_end) => {
            // close_end points just past `---\n`. The frontmatter body
            // is everything before the line that holds the close fence.
            // Walk backwards from close_end until we drop the close
            // fence line.
            let close_line_start = rest[..close_end]
                .rfind('\n')
                .and_then(|n| rest[..n].rfind('\n').map(|p| p + 1))
                .unwrap_or(0);
            let fm = &rest[..close_line_start];
            let body = &rest[close_end..];
            (Some(fm), body)
        }
        None => (None, content),
    }
}

/// Find the byte index *just past* the closing `---\n` (or `---\r\n`)
/// of a frontmatter block, where `s` is the text *after* the opening
/// fence. Returns None if no closing fence exists.
fn find_frontmatter_close(s: &str) -> Option<usize> {
    // Search line-by-line for a line that is exactly `---`.
    let mut idx = 0;
    while idx < s.len() {
        let line_end = s[idx..].find('\n').map(|n| idx + n + 1).unwrap_or(s.len());
        let line = &s[idx..line_end];
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
        if trimmed == "---" {
            return Some(line_end);
        }
        idx = line_end;
    }
    None
}

/// Filter a frontmatter block (raw YAML between the fences) by
/// removing internal keys. We do a lightweight per-line filter — we
/// don't reparse YAML because that would normalize comments,
/// indentation, and ordering, which the user might depend on for full
/// round-trip fidelity (Tesela's portable mode is lossy but we still
/// want the output to look like what was written).
fn strip_frontmatter(frontmatter: &str) -> (String, usize) {
    let mut removed = 0;
    let mut out = String::with_capacity(frontmatter.len());
    let mut skipping_block = false;
    for line in frontmatter.split_inclusive('\n') {
        if skipping_block {
            // Skip continuation lines (indented or list items) that
            // belong to a previously-removed top-level key.
            let trimmed = line.trim_start();
            if trimmed.is_empty()
                || (line.starts_with(' ') || line.starts_with('\t') || trimmed.starts_with('-'))
            {
                continue;
            }
            skipping_block = false;
        }
        if let Some((key, _val)) = parse_yaml_line(line) {
            if is_tesela_internal_key(key) {
                removed += 1;
                // If this key opens a block (no inline value), keep
                // skipping until we hit a sibling top-level key.
                if !line.contains(':') || line.trim_end().ends_with(':') {
                    skipping_block = true;
                }
                continue;
            }
        }
        out.push_str(line);
    }
    (out, removed)
}

/// `key:: value` pattern (Logseq/Tesela inline property). Matches a
/// line whose first non-whitespace run is `<ident> ::`.
fn parse_property_line(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    let colon_pos = trimmed.find("::")?;
    let key = trimmed[..colon_pos].trim();
    if key.is_empty()
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    let value = trimmed[colon_pos + 2..].trim_end_matches('\n').trim();
    Some((key, value))
}

/// `key: value` pattern (YAML frontmatter). Returns None for nested
/// indented lines (those are handled by `skipping_block` upstream).
fn parse_yaml_line(line: &str) -> Option<(&str, &str)> {
    if line.starts_with(' ') || line.starts_with('\t') {
        return None;
    }
    let colon_pos = line.find(':')?;
    let key = line[..colon_pos].trim();
    if key.is_empty()
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    let value = line[colon_pos + 1..].trim_end_matches('\n').trim();
    Some((key, value))
}

/// Hard-coded list of Tesela-internal property/frontmatter keys we
/// strip in portable mode. Keep this list narrow — false positives
/// here destroy real user data.
fn is_tesela_internal_key(key: &str) -> bool {
    if key.starts_with('_') {
        return true;
    }
    matches!(
        key,
        // Apple Reminders sync state (Phase 12.1)
        "apple_reminder_id"
        | "apple_reminder_synced_at"
        | "apple_reminder_orphan"
        | "apple_reminder_list_id"
        // Importer markers (Phase 13.C/D/E)
        | "source_obsidian_path"
        | "source_logseq_path"
        | "source_org_path"
        // Phase 12.2 recurring last-completed audit trail
        | "last_completed"
        // Tesela's persisted internal page identity
        | "tesela_page_id"
        // Tesela's tracking / housekeeping
        | "checksum"
        // Frontmatter timestamps — Obsidian/Logseq use file mtime, not
        // explicit fields. Including them in portable output is noise.
        | "created"
        | "modified"
    )
}

fn write_portable_readme(out_root: &Path, outcome: &ExportOutcome) -> Result<()> {
    let body = format!(
        "# Tesela portable export\n\
         \n\
         This directory was produced by `tesela export --mode portable`. \
         It contains a markdown-only copy of a Tesela mosaic that opens \
         cleanly in Obsidian, Logseq, or any plain-text editor.\n\
         \n\
         - {} note{} exported into `notes/`\n\
         - {} attachment{} included\n\
         - {} Tesela-internal propert{} stripped (Apple Reminders sync state, \
           import markers, recurring `last_completed` audit trail, etc.)\n\
         \n\
         Wiki-style links (`[[Page]]`) and inline `#tags` are kept verbatim. \
         All status/deadline/scheduled/recurring properties are preserved.\n\
         \n\
         To round-trip back into Tesela, prefer `tesela export --mode full` \
         (lossless) or restore from a `.tesela/backups/` archive.\n",
        outcome.note_count,
        if outcome.note_count == 1 { "" } else { "s" },
        outcome.attachment_count,
        if outcome.attachment_count == 1 {
            ""
        } else {
            "s"
        },
        outcome.stripped_property_count,
        if outcome.stripped_property_count == 1 {
            "y"
        } else {
            "ies"
        },
    );
    fs::write(out_root.join("README.md"), body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_fixture(root: &Path) {
        fs::create_dir_all(root.join("notes")).unwrap();
        fs::create_dir_all(root.join("attachments")).unwrap();
        fs::create_dir_all(root.join(".tesela")).unwrap();
        fs::write(
            root.join("notes/2026-05-10.md"),
            "---\ntitle: 2026-05-10\ncreated: 2026-05-10T00:00:00Z\nmodified: 2026-05-10T00:00:00Z\n---\n- Pay rent\n  status:: todo\n  deadline:: [[2026-05-15]]\n  apple_reminder_id:: ABC-123\n  apple_reminder_synced_at:: 2026-05-10T00:00:00Z\n  last_completed:: 2026-05-09\n  recurring:: monthly\n",
        )
        .unwrap();
        fs::write(
            root.join("notes/task.md"),
            "---\ntitle: Task\ntype: Tag\n---\n- Tag page\n",
        )
        .unwrap();
        fs::write(root.join("attachments/foo.png"), b"\x89PNG\r\n").unwrap();
        fs::write(root.join(".tesela/config.toml"), "[general]\n").unwrap();
    }

    #[test]
    fn full_mode_is_byte_exact() {
        let temp = TempDir::new().unwrap();
        let mosaic = temp.path().join("source");
        let out = temp.path().join("export-full");
        make_fixture(&mosaic);

        let outcome = export_mosaic(
            &mosaic,
            &out,
            &ExportOptions {
                mode: MarkdownMode::Full,
                include_attachments: true,
            },
        )
        .unwrap();
        assert_eq!(outcome.note_count, 2);
        assert_eq!(outcome.attachment_count, 1);
        assert_eq!(outcome.stripped_property_count, 0);

        let original = fs::read(mosaic.join("notes/2026-05-10.md")).unwrap();
        let exported = fs::read(out.join("notes/2026-05-10.md")).unwrap();
        assert_eq!(original, exported, "full mode must be byte-exact");

        // .tesela/config.toml carried over; SQLite DB is not.
        assert!(out.join(".tesela/config.toml").exists());
        assert!(!out.join(".tesela/tesela.db").exists());
    }

    #[test]
    fn portable_mode_strips_internal_properties() {
        let temp = TempDir::new().unwrap();
        let mosaic = temp.path().join("source");
        let out = temp.path().join("export-portable");
        make_fixture(&mosaic);

        let outcome = export_mosaic(
            &mosaic,
            &out,
            &ExportOptions {
                mode: MarkdownMode::Portable,
                include_attachments: false,
            },
        )
        .unwrap();
        assert!(outcome.stripped_property_count >= 5);

        let stripped = fs::read_to_string(out.join("notes/2026-05-10.md")).unwrap();
        // Tesela-internal properties: gone.
        assert!(!stripped.contains("apple_reminder_id"));
        assert!(!stripped.contains("apple_reminder_synced_at"));
        assert!(!stripped.contains("last_completed"));
        // Frontmatter timestamps (created/modified) are also internal.
        assert!(!stripped.contains("created:"));
        assert!(!stripped.contains("modified:"));
        // User-facing properties: preserved.
        assert!(stripped.contains("status:: todo"));
        assert!(stripped.contains("deadline:: [[2026-05-15]]"));
        assert!(stripped.contains("recurring:: monthly"));
        // README dropped at export root.
        assert!(out.join("README.md").exists());
    }

    #[test]
    fn portable_strips_underscore_keys() {
        let (out, removed) =
            strip_for_portable("- Test\n  status:: todo\n  _draft:: true\n  _internal:: 42\n");
        assert_eq!(removed, 2);
        assert!(out.contains("status:: todo"));
        assert!(!out.contains("_draft"));
        assert!(!out.contains("_internal"));
    }

    #[test]
    fn portable_keeps_wikilinks_and_tags() {
        let (out, _) = strip_for_portable(
            "- See [[Other Page]] and [[Aliased|alias]] #project #task\n  status:: todo\n",
        );
        assert!(out.contains("[[Other Page]]"));
        assert!(out.contains("[[Aliased|alias]]"));
        assert!(out.contains("#project"));
        assert!(out.contains("#task"));
    }
}
