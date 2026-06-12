//! `repair-daily-tags` subcommand.
//!
//! Finds canonical `YYYY-MM-DD.md` daily notes in the mosaic whose YAML
//! frontmatter does NOT carry the `daily` tag, and (with `--apply`) adds it
//! in place. Dry-run by default; `--apply` is the only mutating flag.
//!
//! The repair is intentionally a *pure filesystem* operation — it walks
//! `<mosaic>/notes/`, parses each candidate's frontmatter, and rewrites
//! the file with `tesela_core::storage::markdown::add_tag_to_frontmatter`.
//! It does NOT touch the Loro engine, so it does not need to take the
//! `tesela-server` flock; the underlying `tesela-server` re-reads the
//! files on its next pass.
//!
//! Idempotency: a second `--apply` reports zero changes because the
//! first run put `daily` in the canonical parsed form.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use tesela_core::storage::markdown::add_tag_to_frontmatter;

/// One canonical date-slug note that needs the `daily` tag added.
pub struct DailyCandidate {
    pub path: PathBuf,
    pub slug: String,
}

/// CLI entry: walk the mosaic, list / apply the daily-tag repair.
pub async fn run(mosaic: &Path, apply: bool) -> Result<()> {
    let notes_dir = mosaic.join("notes");
    if !notes_dir.exists() {
        anyhow::bail!(
            "Notes directory not found at {} — is {} a tesela mosaic?",
            notes_dir.display(),
            mosaic.display()
        );
    }

    let candidates = find_daily_slug_candidates(&notes_dir)?;

    if candidates.is_empty() {
        println!(
            "repair-daily-tags: no date-slug notes are missing the `daily` tag — nothing to do."
        );
        return Ok(());
    }

    if !apply {
        println!(
            "repair-daily-tags (DRY RUN — re-run with --apply to write): {} date-slug note(s) would get `tags: [\"daily\"]` added:",
            candidates.len()
        );
        for c in &candidates {
            println!("  {}", c.path.display());
        }
        println!("\nRun the same command with --apply to write.");
        return Ok(());
    }

    let mut applied = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for c in &candidates {
        match apply_to_file(&c.path) {
            Ok(true) => applied += 1,
            Ok(false) => {
                // Race: someone else added `daily` between the scan and the
                // apply, or the file's content changed such that the helper
                // now sees the tag as already present. Treat as a no-op.
            }
            Err(e) => errors.push(format!("{}: {}", c.path.display(), e)),
        }
    }

    println!("repair-daily-tags: added `daily` to {} note(s).", applied);
    if !errors.is_empty() {
        eprintln!("\n{} file(s) failed:", errors.len());
        for line in &errors {
            eprintln!("  {line}");
        }
        anyhow::bail!("{} file(s) failed to repair", errors.len());
    }

    Ok(())
}

/// Walk `notes_dir` recursively. Return every `YYYY-MM-DD.md` file whose
/// frontmatter does NOT already carry the `daily` tag.
pub fn find_daily_slug_candidates(notes_dir: &Path) -> Result<Vec<DailyCandidate>> {
    let mut out = Vec::new();
    if !notes_dir.exists() {
        return Ok(out);
    }
    for entry in WalkDir::new(notes_dir).follow_links(true).into_iter() {
        let entry = entry.with_context(|| format!("walk failed under {}", notes_dir.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        // Canonical date-slug files end in `.md` and have a stem that
        // parses as `YYYY-MM-DD`. The FsNoteStore config can list other
        // extensions, but Tesela's daily-note generator only writes `.md`.
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if NaiveDate::parse_from_str(stem, "%Y-%m-%d").is_err() {
            continue;
        }
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("  warn: cannot read {}: {}", path.display(), e);
                continue;
            }
        };
        let (meta, _) = match tesela_core::storage::markdown::parse_frontmatter(&content) {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "  warn: cannot parse frontmatter in {}: {}",
                    path.display(),
                    e
                );
                continue;
            }
        };
        if meta.tags.iter().any(|t| t == "daily") {
            continue;
        }
        out.push(DailyCandidate {
            path: path.to_path_buf(),
            slug: stem.to_string(),
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

/// Apply the tag-add to one file. Returns `Ok(true)` if the file was
/// actually modified, `Ok(false)` if the helper decided no change was
/// needed (e.g. someone else added the tag between the scan and the write).
fn apply_to_file(path: &Path) -> Result<bool> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let updated = match add_tag_to_frontmatter(&content, "daily") {
        Some(s) => s,
        None => return Ok(false),
    };
    if updated == content {
        return Ok(false);
    }
    std::fs::write(path, &updated).with_context(|| format!("write {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Build a tiny mosaic with notes that mix: a date-slug without `daily`,
    /// a date-slug with `daily`, a regular note, and a malformed-date slug.
    /// Returns the `notes/` dir for direct manipulation.
    fn fixture() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let notes = tmp.path().join("notes");
        fs::create_dir_all(&notes).unwrap();

        // Date slug, no daily tag → candidate.
        fs::write(
            notes.join("2026-06-10.md"),
            "---\ntitle: 2026-06-10\n---\n\n- visible journal block\n",
        )
        .unwrap();
        // Date slug, already daily → NOT a candidate.
        fs::write(
            notes.join("2026-06-11.md"),
            "---\ntitle: 2026-06-11\ntags: [\"daily\"]\n---\n\n- tagged daily block\n",
        )
        .unwrap();
        // Regular note → NOT a candidate.
        fs::write(
            notes.join("regular-note.md"),
            "---\ntitle: regular-note\n---\n\n- not daily\n",
        )
        .unwrap();
        // Date-slug-shaped but invalid date → NOT a candidate.
        fs::write(
            notes.join("2026-13-45.md"),
            "---\ntitle: 2026-13-45\n---\n\n- invalid date\n",
        )
        .unwrap();
        // Date slug in a nested dir (defensive: WalkDir recurses) → candidate.
        let nested = notes.join("archive");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("2025-01-02.md"),
            "---\ntitle: 2025-01-02\ncreated: 2025-01-02T00:00:00Z\n---\n\n- archive block\n",
        )
        .unwrap();

        (tmp, notes)
    }

    #[test]
    fn repair_daily_tags_finds_only_date_slugs_missing_daily() {
        let (_tmp, notes) = fixture();
        let cands = find_daily_slug_candidates(&notes).unwrap();
        let slugs: Vec<&str> = cands.iter().map(|c| c.slug.as_str()).collect();
        assert_eq!(
            slugs,
            vec!["2025-01-02", "2026-06-10"],
            "exactly the two date-slug notes without `daily`"
        );
    }

    #[test]
    fn repair_daily_tags_apply_preserves_body_and_other_frontmatter_fields() {
        let (_tmp, notes) = fixture();
        let target = notes.join("2026-06-10.md");
        let before = fs::read_to_string(&target).unwrap();

        // Apply via the same code path the CLI uses.
        assert!(apply_to_file(&target).unwrap());

        let after = fs::read_to_string(&target).unwrap();
        // The body is byte-for-byte identical.
        let body_start_before = before.find("- visible journal block").unwrap();
        let body_start_after = after.find("- visible journal block").unwrap();
        assert_eq!(&before[body_start_before..], &after[body_start_after..]);
        // The new `tags: ["daily"]` line is now in the frontmatter.
        assert!(after.contains("tags: [\"daily\"]"));
        // The new run lists zero candidates for the touched file.
        let cands = find_daily_slug_candidates(&notes).unwrap();
        assert!(
            !cands.iter().any(|c| c.slug == "2026-06-10"),
            "touched file is no longer a candidate: {:?}",
            cands.iter().map(|c| &c.slug).collect::<Vec<_>>()
        );
    }

    #[test]
    fn repair_daily_tags_apply_preserves_existing_fields_and_aliases() {
        let (_tmp, notes) = fixture();
        // The nested file has a `created:` field and a body line — make
        // sure the surgery does not drop them.
        let target = notes.join("archive").join("2025-01-02.md");
        let before = fs::read_to_string(&target).unwrap();
        assert!(before.contains("created: 2025-01-02T00:00:00Z"));
        assert!(before.contains("- archive block"));

        assert!(apply_to_file(&target).unwrap());

        let after = fs::read_to_string(&target).unwrap();
        assert!(after.contains("title: 2025-01-02"));
        assert!(after.contains("created: 2025-01-02T00:00:00Z"));
        assert!(after.contains("tags: [\"daily\"]"));
        assert!(after.contains("- archive block"));
    }

    #[test]
    fn repair_daily_tags_apply_is_idempotent() {
        let (_tmp, notes) = fixture();

        // Apply to EVERY candidate (the CLI's `run` loops over them).
        let cands = find_daily_slug_candidates(&notes).unwrap();
        for c in &cands {
            assert!(
                apply_to_file(&c.path).unwrap(),
                "first apply mutated {}",
                c.path.display()
            );
        }

        // A second pass must be a no-op for every file (the helper sees
        // the tag as already present and returns false).
        for c in &cands {
            assert!(
                !apply_to_file(&c.path).unwrap(),
                "second apply is a no-op for {}",
                c.path.display()
            );
        }

        // File contents are byte-for-byte stable between the first and
        // second apply — captured against one of the repaired files.
        let target = notes.join("2026-06-10.md");
        let once = fs::read_to_string(&target).unwrap();
        let _ = apply_to_file(&target).unwrap();
        let twice = fs::read_to_string(&target).unwrap();
        assert_eq!(once, twice, "second apply must not rewrite the file");

        // The full mosaic scan now reports zero candidates.
        let after = find_daily_slug_candidates(&notes).unwrap();
        assert!(
            after.is_empty(),
            "all candidates repaired; second run sees none: {:?}",
            after.iter().map(|c| &c.slug).collect::<Vec<_>>()
        );
    }

    #[test]
    fn repair_daily_tags_handles_existing_canonical_quoted_tags_form() {
        // A file whose frontmatter tags use the canonical quoted form must
        // round-trip through the parser without being misidentified as a
        // candidate.
        let tmp = TempDir::new().unwrap();
        let notes = tmp.path().join("notes");
        fs::create_dir_all(&notes).unwrap();
        fs::write(
            notes.join("2026-06-12.md"),
            "---\ntitle: 2026-06-12\ntags: [\"journal\", \"daily\"]\n---\n\n- body\n",
        )
        .unwrap();

        let cands = find_daily_slug_candidates(&notes).unwrap();
        assert!(cands.is_empty(), "already has daily, no candidate");

        // apply is a no-op too.
        assert!(!apply_to_file(&notes.join("2026-06-12.md")).unwrap());
    }
}
