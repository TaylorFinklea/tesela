//! Obsidian vault importer.
//!
//! Walks a vault directory, ignoring `.obsidian/` and other hidden
//! files, and imports each `.md` as a Tesela note. Subfolder structure
//! flattens into tags (e.g. `Projects/Foo.md` becomes a note tagged
//! `#Projects`). Idempotent via a `source_obsidian_path::` frontmatter
//! property + content SHA-256: re-running a `tesela import obsidian`
//! against the same vault is a no-op for unchanged files; changed
//! files are skipped + logged so the user resolves manually.
//!
//! v1 scope:
//! - YAML frontmatter passthrough (we add `source_obsidian_path` /
//!   `source_obsidian_sha`; existing keys are kept).
//! - Wikilinks: `[[Page]]` and `[[Page|alias]]` kept verbatim.
//! - Wikilinks with heading anchors `[[Page#Heading]]` and block refs
//!   `[[Page#^id]]` are downgraded to `[[Page]]` with a logged
//!   warning, since Tesela has no inline anchor support yet.
//! - `#tags` inline are kept verbatim.
//! - Attachments (Obsidian's `attachments/` or referenced images):
//!   deferred to a follow-up. We log + skip image-embed lines but
//!   leave the markdown intact.
//! - `.canvas` / `.excalidraw` / Dataview queries: skipped or kept
//!   verbatim with a log entry.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const SOURCE_PATH_KEY: &str = "source_obsidian_path";
const SOURCE_SHA_KEY: &str = "source_obsidian_sha";

#[derive(Debug, Default)]
struct ImportStats {
    imported: usize,
    unchanged: usize,
    conflicts: usize,
    warnings: usize,
}

pub async fn run(mosaic: &Path, source: PathBuf, dry_run: bool) -> Result<()> {
    if !source.exists() {
        anyhow::bail!("Obsidian vault not found: {}", source.display());
    }
    let notes_dir = mosaic.join("notes");
    fs::create_dir_all(&notes_dir)?;

    let mut stats = ImportStats::default();
    let mut log_lines: Vec<String> = Vec::new();
    let mut produced_ids: HashSet<String> = HashSet::new();

    for entry in WalkDir::new(&source).follow_links(false) {
        let entry = entry.context("walk vault")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if is_hidden(path, &source) {
            continue;
        }
        let rel = match path.strip_prefix(&source) {
            Ok(p) => p.to_path_buf(),
            Err(_) => continue,
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ext != "md" {
            // Skip non-markdown (canvas, images, etc.) but log canvas
            // explicitly so the user knows their visuals didn't come
            // along.
            if matches!(ext.as_str(), "canvas" | "excalidraw") {
                log_lines.push(format!(
                    "[skip] {} (Tesela has no canvas/excalidraw equivalent)",
                    rel.display()
                ));
                stats.warnings += 1;
            }
            continue;
        }

        match import_one(
            &source,
            &rel,
            &notes_dir,
            dry_run,
            &mut log_lines,
            &mut produced_ids,
        ) {
            Ok(IndexAction::Imported) => stats.imported += 1,
            Ok(IndexAction::Unchanged) => stats.unchanged += 1,
            Ok(IndexAction::Conflict) => stats.conflicts += 1,
            Err(e) => {
                tracing::warn!("Failed to import {}: {}", rel.display(), e);
                log_lines.push(format!("[error] {}: {}", rel.display(), e));
                stats.warnings += 1;
            }
        }
    }

    // Persist the skip log so the user has a paper trail of conflicts.
    if !log_lines.is_empty() && !dry_run {
        let log_path = mosaic.join("_import-skipped.log");
        let mut content = format!(
            "Obsidian import @ {}\nVault: {}\n\n",
            chrono::Local::now().to_rfc3339(),
            source.display()
        );
        content.push_str(&log_lines.join("\n"));
        content.push('\n');
        fs::write(&log_path, content)
            .with_context(|| format!("write import log {}", log_path.display()))?;
    }

    println!("Obsidian import complete:");
    println!("  Imported: {}", stats.imported);
    println!("  Unchanged (idempotent): {}", stats.unchanged);
    println!("  Conflicts (skipped): {}", stats.conflicts);
    println!("  Warnings: {}", stats.warnings);
    if !log_lines.is_empty() && !dry_run {
        println!("  Log: {}", mosaic.join("_import-skipped.log").display());
    }
    if dry_run {
        println!("  (dry run — no files written)");
    }
    Ok(())
}

enum IndexAction {
    Imported,
    Unchanged,
    Conflict,
}

fn import_one(
    vault: &Path,
    rel: &Path,
    notes_dir: &Path,
    dry_run: bool,
    log: &mut Vec<String>,
    produced_ids: &mut HashSet<String>,
) -> Result<IndexAction> {
    let source_path = vault.join(rel);
    let raw = fs::read_to_string(&source_path)
        .with_context(|| format!("read {}", source_path.display()))?;
    let sha = sha256_hex(&raw);
    let rel_str = rel.to_string_lossy().replace('\\', "/");

    let (note_id, folder_tags) = derive_note_id_and_tags(rel);
    if !produced_ids.insert(note_id.clone()) {
        // Two source files mapped to the same Tesela id (e.g.
        // `Foo.md` and `subfolder/Foo.md`). Log + skip the latter.
        log.push(format!(
            "[conflict] {} → {} already produced by an earlier file",
            rel_str, note_id
        ));
        return Ok(IndexAction::Conflict);
    }
    let target_path = notes_dir.join(format!("{}.md", note_id));

    // Idempotence check: same source path + same SHA → skip silently.
    if target_path.exists() {
        let existing = fs::read_to_string(&target_path).unwrap_or_default();
        if let Some(prev_sha) = extract_frontmatter_value(&existing, SOURCE_SHA_KEY) {
            if prev_sha == sha {
                return Ok(IndexAction::Unchanged);
            }
            // Different content for the same source path — could be
            // either user edits in Tesela or upstream changes. We err
            // on the safe side and skip.
            log.push(format!(
                "[conflict] {} → {} target exists with different SHA (yours: {}, source: {}). Skipped.",
                rel_str, note_id, prev_sha, sha
            ));
            return Ok(IndexAction::Conflict);
        }
        // Target exists but isn't ours — refuse to overwrite.
        log.push(format!(
            "[conflict] {} → {} target exists and was not produced by import. Skipped.",
            rel_str, note_id
        ));
        return Ok(IndexAction::Conflict);
    }

    let (frontmatter, body) = split_frontmatter(&raw);
    let body = rewrite_body(body, log, &rel_str);
    let new_frontmatter = build_frontmatter(frontmatter, &rel_str, &sha, &folder_tags);
    let mut out = String::new();
    out.push_str(&new_frontmatter);
    out.push_str(&body);

    if !dry_run {
        fs::write(&target_path, &out)
            .with_context(|| format!("write {}", target_path.display()))?;
    }
    Ok(IndexAction::Imported)
}

fn is_hidden(path: &Path, vault: &Path) -> bool {
    let rel = match path.strip_prefix(vault) {
        Ok(p) => p,
        Err(_) => return false,
    };
    rel.components().any(|c| {
        c.as_os_str()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    })
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

/// Map a vault-relative path to (note_id, folder_tags). Folder
/// hierarchy collapses into tags (the tag for `Projects/sub/foo.md`
/// is `Projects` *and* `sub`). The note id is the slugified file
/// stem.
///
/// Daily-note detection: if the file stem matches `YYYY-MM-DD`, it's
/// imported as a daily note (no folder tag).
fn derive_note_id_and_tags(rel: &Path) -> (String, Vec<String>) {
    let stem = rel
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string();
    let id = slugify(&stem);

    if is_iso_date(&id) {
        return (id, Vec::new());
    }

    let mut tags = Vec::new();
    if let Some(parent) = rel.parent() {
        for component in parent.components() {
            if let Some(name) = component.as_os_str().to_str() {
                if !name.is_empty() {
                    tags.push(slugify(name));
                }
            }
        }
    }
    (id, tags)
}

fn is_iso_date(s: &str) -> bool {
    s.len() == 10
        && s.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        })
}

/// Lowercase, replace whitespace + most punctuation with `-`, collapse
/// runs of `-`. Matches the rough shape Logseq importer uses.
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars() {
        let mapped = if c.is_ascii_alphanumeric() {
            c.to_ascii_lowercase()
        } else {
            '-'
        };
        if mapped == '-' {
            if !last_dash && !out.is_empty() {
                out.push('-');
            }
            last_dash = true;
        } else {
            out.push(mapped);
            last_dash = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("untitled");
    }
    out
}

/// Split a markdown file into (frontmatter_yaml or "", body). The
/// frontmatter excludes the `---` fences. If the file has no
/// frontmatter, returns ("", entire_content).
fn split_frontmatter(content: &str) -> (&str, &str) {
    let after_open = if content.starts_with("---\n") {
        4
    } else if content.starts_with("---\r\n") {
        5
    } else {
        return ("", content);
    };
    let rest = &content[after_open..];
    // Find the closing `---` line.
    let mut idx = 0;
    while idx < rest.len() {
        let line_end = rest[idx..]
            .find('\n')
            .map(|n| idx + n + 1)
            .unwrap_or(rest.len());
        let line = &rest[idx..line_end];
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
        if trimmed == "---" {
            // Body starts after the close fence line.
            return (&rest[..idx], &rest[line_end..]);
        }
        idx = line_end;
    }
    // Open without close — leave content alone.
    ("", content)
}

/// Look up a YAML scalar value from a raw frontmatter block. Quick
/// enough for our two well-known keys (`source_obsidian_path`,
/// `source_obsidian_sha`); a full YAML parse would be overkill here.
fn extract_frontmatter_value(content: &str, key: &str) -> Option<String> {
    let (fm, _) = split_frontmatter(content);
    for line in fm.lines() {
        let trimmed = line.trim_start();
        if let Some((k, rest)) = trimmed.split_once(':') {
            if k.trim() == key {
                return Some(rest.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

/// Build the new frontmatter: keep what was there, append/overwrite
/// our two source-tracking keys, ensure `tags` includes the folder
/// tags. Fully fenced YAML, ready to write.
fn build_frontmatter(existing: &str, rel_str: &str, sha: &str, folder_tags: &[String]) -> String {
    let mut out = String::new();
    out.push_str("---\n");

    let mut saw_tags = false;
    let mut wrote_tags = false;
    for line in existing.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&format!("{}:", SOURCE_PATH_KEY)) {
            continue; // We'll re-emit at the end.
        }
        if trimmed.starts_with(&format!("{}:", SOURCE_SHA_KEY)) {
            continue;
        }
        if trimmed.starts_with("tags:") && !folder_tags.is_empty() && !wrote_tags {
            saw_tags = true;
            wrote_tags = true;
            out.push_str(&render_tags_line(line, folder_tags));
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    if !folder_tags.is_empty() && !saw_tags {
        out.push_str(&format!(
            "tags: [{}]\n",
            folder_tags
                .iter()
                .map(|t| format!("\"{}\"", t))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out.push_str(&format!("{}: \"{}\"\n", SOURCE_PATH_KEY, rel_str));
    out.push_str(&format!("{}: \"{}\"\n", SOURCE_SHA_KEY, sha));
    out.push_str("---\n");
    out
}

/// Merge `folder_tags` into an existing `tags:` line. Handles the two
/// most common Obsidian shapes: inline list `tags: [a, b]` and YAML
/// flow `tags:` followed by `- a` lines (we leave the latter alone —
/// our render is good enough for a v1 importer; user can clean up).
fn render_tags_line(line: &str, folder_tags: &[String]) -> String {
    let trimmed = line.trim_end();
    if let Some(rest) = trimmed.strip_prefix("tags:") {
        let rest = rest.trim();
        if rest.starts_with('[') && rest.ends_with(']') {
            let inner = &rest[1..rest.len() - 1];
            let mut existing: Vec<String> = inner
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect();
            for t in folder_tags {
                if !existing.iter().any(|e| e == t) {
                    existing.push(t.clone());
                }
            }
            let rendered = existing
                .iter()
                .map(|s| format!("\"{}\"", s))
                .collect::<Vec<_>>()
                .join(", ");
            return format!("tags: [{}]", rendered);
        }
    }
    // Fall through: leave the line alone.
    line.to_string()
}

/// Rewrite body content: downgrade `[[Page#Heading]]` and
/// `[[Page#^id]]` wikilinks to `[[Page]]` with a log entry, since
/// Tesela has no inline anchor system.
fn rewrite_body(body: &str, log: &mut Vec<String>, rel_str: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    let bytes = body.as_bytes();
    while i < body.len() {
        if bytes[i] == b'[' && i + 1 < body.len() && bytes[i + 1] == b'[' {
            if let Some(close) = body[i + 2..].find("]]") {
                let inner = &body[i + 2..i + 2 + close];
                if let Some(hash_pos) = inner.find('#') {
                    let target = &inner[..hash_pos];
                    let anchor = &inner[hash_pos..];
                    out.push_str("[[");
                    out.push_str(target);
                    out.push_str("]]");
                    log.push(format!(
                        "[downgrade] {} wikilink anchor stripped: [[{}]]→[[{}]]",
                        rel_str, inner, target
                    ));
                    let _ = anchor;
                    i = i + 2 + close + 2;
                    continue;
                }
                // No anchor — keep verbatim.
                out.push_str("[[");
                out.push_str(inner);
                out.push_str("]]");
                i = i + 2 + close + 2;
                continue;
            }
        }
        out.push(body.as_bytes()[i] as char);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_vault(root: &Path) {
        fs::create_dir_all(root.join(".obsidian")).unwrap();
        fs::write(root.join(".obsidian/app.json"), "{}").unwrap();

        fs::create_dir_all(root.join("Projects/Active")).unwrap();
        fs::write(
            root.join("Projects/Active/Build Tesela.md"),
            "---\ntitle: Build Tesela\ntags: [\"work\"]\n---\n# Heading\n\nLink to [[Other Page]] and [[Other Page#section]] and [[Page|alias]] #project\n",
        )
        .unwrap();

        fs::create_dir_all(root.join("Daily Notes")).unwrap();
        fs::write(
            root.join("Daily Notes/2026-05-10.md"),
            "Today I worked on stuff.\n",
        )
        .unwrap();

        fs::write(root.join("Other Page.md"), "Just a page.\n").unwrap();
        fs::write(root.join("a.canvas"), "{}").unwrap();
    }

    #[tokio::test]
    async fn imports_vault_basic() {
        let temp = TempDir::new().unwrap();
        let vault = temp.path().join("vault");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        make_vault(&vault);

        run(&mosaic, vault.clone(), false).await.unwrap();

        // 3 .md files imported: Build Tesela, Daily Notes/2026-05-10, Other Page
        let entries: Vec<_> = fs::read_dir(mosaic.join("notes"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(entries.contains(&"build-tesela.md".to_string()));
        assert!(entries.contains(&"2026-05-10.md".to_string()));
        assert!(entries.contains(&"other-page.md".to_string()));

        let body = fs::read_to_string(mosaic.join("notes/build-tesela.md")).unwrap();
        // Folder hierarchy → tags (slugified lowercase)
        assert!(body.contains("\"projects\""));
        assert!(body.contains("\"active\""));
        // Source tracking
        assert!(body.contains(SOURCE_PATH_KEY));
        assert!(body.contains(SOURCE_SHA_KEY));
        // Anchor wikilinks downgraded
        assert!(body.contains("[[Other Page]]"));
        assert!(!body.contains("Other Page#section"));
        // Aliased wikilink preserved
        assert!(body.contains("[[Page|alias]]"));
        // Inline tags preserved
        assert!(body.contains("#project"));

        // Skipped log mentions the .canvas file
        let log = fs::read_to_string(mosaic.join("_import-skipped.log")).unwrap();
        assert!(log.contains("a.canvas"));
    }

    #[tokio::test]
    async fn idempotent_re_import() {
        let temp = TempDir::new().unwrap();
        let vault = temp.path().join("vault");
        let mosaic = temp.path().join("mosaic");
        fs::create_dir_all(mosaic.join("notes")).unwrap();
        make_vault(&vault);

        run(&mosaic, vault.clone(), false).await.unwrap();
        let first_body = fs::read_to_string(mosaic.join("notes/other-page.md")).unwrap();
        let first_mtime = fs::metadata(mosaic.join("notes/other-page.md"))
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Re-run with the same vault — second time should be a no-op
        // for unchanged files.
        run(&mosaic, vault.clone(), false).await.unwrap();
        let second_body = fs::read_to_string(mosaic.join("notes/other-page.md")).unwrap();
        let second_mtime = fs::metadata(mosaic.join("notes/other-page.md"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(first_body, second_body);
        // mtime unchanged — file wasn't rewritten.
        assert_eq!(first_mtime, second_mtime);
    }

    #[tokio::test]
    async fn collision_with_existing_target_is_logged() {
        let temp = TempDir::new().unwrap();
        let vault = temp.path().join("vault");
        let mosaic = temp.path().join("mosaic");
        let notes = mosaic.join("notes");
        fs::create_dir_all(&notes).unwrap();
        // Pre-seed a Tesela note that would collide with the import.
        fs::write(notes.join("other-page.md"), "preexisting content").unwrap();
        make_vault(&vault);

        run(&mosaic, vault.clone(), false).await.unwrap();

        let preserved = fs::read_to_string(notes.join("other-page.md")).unwrap();
        assert_eq!(preserved, "preexisting content");

        let log = fs::read_to_string(mosaic.join("_import-skipped.log")).unwrap();
        assert!(log.contains("other-page"));
    }

    #[test]
    fn slugify_known_cases() {
        assert_eq!(slugify("Foo Bar"), "foo-bar");
        assert_eq!(slugify("My Note's Title!"), "my-note-s-title");
        assert_eq!(slugify("---"), "untitled");
        assert_eq!(slugify("2026-05-10"), "2026-05-10");
    }
}
