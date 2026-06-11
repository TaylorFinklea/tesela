//! `backfill-task` subcommand.
//!
//! Scans the mosaic and, for every block that carries a status (an in-text
//! `status:: <value>` line OR a container `status` property — both materialize
//! to a `status::` line) but lacks a Task tag, adds the `Task` tag through the
//! STRUCTURED property path (a `BlockPropertySet` `AddToList` on the block's
//! `tags`). Going through the structured path means the materializer emits one
//! `tags:: Task` line — it does NOT write a duplicate in-text line that would
//! later clash with the container (the dual-write dup this whole property
//! milestone avoids). `AddToList` is union/idempotent, so re-running is safe.
//!
//! Dry-run by default (summary + per-note rollup, write nothing; `--verbose`
//! for the full block list); only `--apply` mutates. This is a DATA migration —
//! it must never auto-run, and it refuses to run while `tesela-server` holds
//! the mosaic (single-writer).
//!
//! Residency: the Loro engine only holds docs for notes that have flowed
//! through it; on a lazily-resident mosaic the canonical content for
//! everything else is the engine's own materialized `<mosaic>/notes/<slug>.md`
//! files. The scan therefore reads BOTH — engine render for resident notes,
//! the materialized file otherwise — and `--apply` hydrates a non-resident
//! note first (`mosaic_notes::hydrate_note`, the same `NoteUpsert` per-bid
//! reconcile every editor save records) before the structured tag-add.

use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, PropOp, PropScalar, SyncEngine};

use crate::mosaic_notes::{
    hydrate_note, read_non_resident_notes, resident_notes, stable_uuid_from_slug,
};

/// One block that carries a status but no Task tag.
pub struct Candidate {
    pub note_id: [u8; 16],
    pub block_id: [u8; 16],
    /// Note slug (display alias) for human-readable output.
    pub slug: String,
    /// The block's first text line, for human-readable dry-run output.
    pub preview: String,
    /// The note is not Loro-resident — apply records a `NoteUpsert` of the
    /// materialized file first (per-bid reconcile, the op every save uses).
    pub needs_hydration: bool,
}

#[derive(Default)]
pub struct BackfillReport {
    pub candidates: Vec<Candidate>,
    pub applied: usize,
    /// Notes hydrated into the engine (NoteUpsert) during apply.
    pub hydrated_notes: usize,
}

/// Core scan + (optional) apply over an open engine. Read-only first pass
/// collects candidates so enumeration never races our own writes; the apply
/// pass adds the tag via a structured `BlockPropertySet`. `notes_dir` is the
/// engine's own materialized notes directory — blocks in notes the engine
/// doesn't hold resident are scanned from their `.md` files and hydrated on
/// apply (see module docs). Pure over the engine so it is unit-testable with
/// an in-process `LoroEngine`.
pub async fn backfill(
    engine: &LoroEngine,
    notes_dir: Option<&Path>,
    apply: bool,
) -> Result<BackfillReport> {
    let mut report = BackfillReport::default();

    // Pass 1a — scan every Loro-resident note's blocks (read-only).
    let mut resident_slugs: HashSet<String> = HashSet::new();
    for note in resident_notes(engine).await {
        resident_slugs.insert(note.slug.clone());
        let tree = tesela_core::note_tree::parse_note(&note.md);
        for block in &tree.blocks {
            if has_status(&block.text) && !has_task(&block.text) {
                report.candidates.push(Candidate {
                    note_id: note.note_id,
                    block_id: *block.id.as_bytes(),
                    slug: note.slug.clone(),
                    preview: block.text.lines().next().unwrap_or("").trim().to_string(),
                    needs_hydration: false,
                });
            }
        }
    }

    // Pass 1b — non-resident notes: the engine's materialized `.md` files
    // are the canonical content; scan from disk, hydrate on apply.
    let mut hydrate_by_note: HashMap<[u8; 16], (String, String)> = HashMap::new();
    if let Some(dir) = notes_dir {
        for (stem, content) in read_non_resident_notes(dir, &resident_slugs)? {
            let note_id = stable_uuid_from_slug(&stem);
            let tree = tesela_core::note_tree::parse_note(&content);
            let mut hit = false;
            for block in &tree.blocks {
                if has_status(&block.text) && !has_task(&block.text) {
                    report.candidates.push(Candidate {
                        note_id,
                        block_id: *block.id.as_bytes(),
                        slug: stem.clone(),
                        preview: block.text.lines().next().unwrap_or("").trim().to_string(),
                        needs_hydration: true,
                    });
                    hit = true;
                }
            }
            if hit {
                hydrate_by_note.insert(note_id, (stem, content));
            }
        }
    }

    // Pass 2 — apply: hydrate non-resident notes once, then the structured
    // tag-add (union/idempotent).
    if apply {
        let mut hydrated: HashSet<[u8; 16]> = HashSet::new();
        for c in &report.candidates {
            if c.needs_hydration && !hydrated.contains(&c.note_id) {
                let (slug, content) = hydrate_by_note
                    .get(&c.note_id)
                    .expect("needs_hydration implies recorded content");
                hydrate_note(engine, c.note_id, slug, content).await?;
                hydrated.insert(c.note_id);
                report.hydrated_notes += 1;
            }
            engine
                .record_local(OpPayload::BlockPropertySet {
                    note_id: c.note_id,
                    block_id: c.block_id,
                    key: "tags".to_string(),
                    value: PropOp::AddToList(PropScalar::Text("Task".to_string())),
                })
                .await
                .map_err(|e| anyhow::anyhow!("add #Task tag: {e}"))?;
            report.applied += 1;
        }
    }

    Ok(report)
}

/// CLI entry: lock the mosaic (refuse if the server holds it), open the Loro
/// engine over the mosaic's existing snapshots (no reseed — load only, matching
/// the server's default), run the backfill, and print the report —
/// summary-first (counts + per-note rollup); the full per-block list can run
/// to thousands of lines on a big mosaic, so it hides behind `--verbose`.
pub async fn run(mosaic: &Path, apply: bool, verbose: bool) -> Result<()> {
    let _lock = acquire_mosaic_lock(mosaic).context(
        "could not lock the mosaic — is tesela-server (or the desktop app) running on it? \
         Stop it before running backfill-task (single-writer).",
    )?;

    let device = load_device_id(mosaic);
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");
    let hlc = Arc::new(Hlc::new(device));
    let engine = LoroEngine::with_dirs(device, hlc, snapshot_dir, Some(notes_dir.clone()))
        .await
        .map_err(|e| anyhow::anyhow!("open loro engine: {e}"))?;

    let report = backfill(&engine, Some(&notes_dir), apply).await?;

    if report.candidates.is_empty() {
        println!("backfill-task: no status-bearing blocks are missing #Task — nothing to do.");
        return Ok(());
    }

    // Summary first.
    let mut by_note: BTreeMap<&str, usize> = BTreeMap::new();
    for c in &report.candidates {
        *by_note.entry(c.slug.as_str()).or_default() += 1;
    }
    let hydrating: HashSet<&str> = report
        .candidates
        .iter()
        .filter(|c| c.needs_hydration)
        .map(|c| c.slug.as_str())
        .collect();
    if apply {
        println!(
            "backfill-task: added #Task to {} block(s) across {} note(s) ({} note(s) hydrated).",
            report.applied,
            by_note.len(),
            report.hydrated_notes
        );
    } else {
        println!(
            "backfill-task (DRY RUN — re-run with --apply to write): {} block(s) across {} note(s) would get #Task ({} non-resident note(s) would be hydrated).",
            report.candidates.len(),
            by_note.len(),
            hydrating.len()
        );
    }

    println!("\nPer-note rollup:");
    for (slug, n) in &by_note {
        let hydrate = if hydrating.contains(slug) {
            "  [hydrates]"
        } else {
            ""
        };
        println!("  {n:>5}  {slug}{hydrate}");
    }

    if verbose {
        println!("\nBlocks:");
        for c in &report.candidates {
            println!("  {}:{}  {}", c.slug, hex16(&c.block_id), c.preview);
        }
    } else {
        println!(
            "\n({} block(s) total — re-run with --verbose for the full per-block list.)",
            report.candidates.len()
        );
    }
    if !apply {
        println!("\nRun the same command with --apply to write (stop the server first).");
    }
    Ok(())
}

/// Lowercased identifier key of a SOLELY `key:: value` line (matching the
/// engine's materialized continuation-line form), or `None`. Conservative: the
/// whole trimmed line must be `key:: value` with a bare-identifier key and a
/// non-empty value — mirrors the engine's `solely_property_line`.
pub(crate) fn line_property_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let (key, value) = trimmed.split_once(":: ")?;
    if value.is_empty() {
        return None;
    }
    let mut chars = key.chars();
    let first = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    Some(key.to_ascii_lowercase())
}

/// True if the block carries a `status` property (in-text or container — both
/// materialize to a `status::` line that `parse_note` folds into the text).
fn has_status(text: &str) -> bool {
    text.lines()
        .any(|l| line_property_key(l).as_deref() == Some("status"))
}

/// True if the block already carries the `Task` tag — via a `tags::` list line
/// (any case, comma-separated) or an inline `#Task` token.
fn has_task(text: &str) -> bool {
    for line in text.lines() {
        if line_property_key(line).as_deref() == Some("tags") {
            if let Some((_, value)) = line.trim().split_once(":: ") {
                if value
                    .split(',')
                    .any(|t| t.trim().eq_ignore_ascii_case("Task"))
                {
                    return true;
                }
            }
        }
    }
    // Inline `#Task` (a whole token, case-insensitive) — not `#TaskList` etc.
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '#'))
        .any(|tok| tok.eq_ignore_ascii_case("#Task"))
}

/// Read the mosaic's existing device id (no write — the backfill must not mint
/// a `device_id.hex`); fall back to a random id if absent/malformed (harmless
/// for the union/idempotent tag-add).
pub(crate) fn load_device_id(mosaic: &Path) -> DeviceId {
    let path = mosaic.join(".tesela").join("device_id.hex");
    if let Ok(bytes) = std::fs::read(&path) {
        let s = String::from_utf8_lossy(&bytes);
        let s = s.trim();
        if s.len() == 32 {
            let mut arr = [0u8; 16];
            let mut ok = true;
            for i in 0..16 {
                match u8::from_str_radix(&s[i * 2..i * 2 + 2], 16) {
                    Ok(b) => arr[i] = b,
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
            }
            if ok {
                return DeviceId::from_bytes(arr);
            }
        }
    }
    DeviceId::new_random()
}

/// Exclusive, non-blocking flock on `<mosaic>/.tesela/server.lock` — the SAME
/// lock the server holds, so the backfill refuses to run while a server is up
/// (the CLI engine has no lock of its own). Mirrors the server's
/// `acquire_mosaic_lock`. The returned `File` must stay alive for the duration.
pub(crate) fn acquire_mosaic_lock(mosaic: &Path) -> Result<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let tesela_dir = mosaic.join(".tesela");
    std::fs::create_dir_all(&tesela_dir)?;
    let lock_path = tesela_dir.join("server.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        anyhow::bail!("lock held (EWOULDBLOCK)");
    }
    Ok(file)
}

pub(crate) fn hex16(bytes: &[u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn engine() -> LoroEngine {
        let dev = DeviceId::from_bytes([7u8; 16]);
        LoroEngine::new(dev, Arc::new(Hlc::new(dev)))
    }

    async fn upsert(engine: &LoroEngine, note_id: [u8; 16], content: &str) {
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("t".into()),
                title: "t".into(),
                content: content.to_string(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
    }

    const NOTE_A: [u8; 16] = [0xa1; 16];
    const BID: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";

    #[tokio::test]
    async fn dry_run_finds_status_block_without_task_and_writes_nothing() {
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- buy milk <!-- bid:{BID} -->\n  status:: doing\n"),
        )
        .await;

        let report = backfill(&e, None, false).await.unwrap();

        assert_eq!(report.candidates.len(), 1, "one status block lacks #Task");
        assert_eq!(report.applied, 0, "dry-run writes nothing");
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert!(!md.contains("tags:: Task"), "dry-run added no tag: {md:?}");
    }

    #[tokio::test]
    async fn apply_adds_task_tag_through_structured_path() {
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- buy milk <!-- bid:{BID} -->\n  status:: doing\n"),
        )
        .await;

        let report = backfill(&e, None, true).await.unwrap();

        assert_eq!(report.applied, 1);
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert_eq!(
            md.matches("tags:: Task").count(),
            1,
            "exactly one tags:: Task line (structured, materialized once): {md:?}"
        );
        assert_eq!(
            md.matches("status::").count(),
            1,
            "status preserved, no dup: {md:?}"
        );
    }

    #[tokio::test]
    async fn apply_is_idempotent() {
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- buy milk <!-- bid:{BID} -->\n  status:: doing\n"),
        )
        .await;

        backfill(&e, None, true).await.unwrap();
        let second = backfill(&e, None, true).await.unwrap();

        assert_eq!(
            second.candidates.len(),
            0,
            "after the first apply the block carries #Task → no longer a candidate"
        );
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert_eq!(
            md.matches("tags:: Task").count(),
            1,
            "still exactly one tags:: Task after a second run: {md:?}"
        );
    }

    #[tokio::test]
    async fn block_with_unioned_tags_line_is_untouched() {
        // The importers can merge Task into an existing tags continuation
        // line (`tags:: errand, Task`) — backfill must not re-add it.
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- buy milk <!-- bid:{BID} -->\n  status:: todo\n  tags:: errand, Task\n"),
        )
        .await;

        let report = backfill(&e, None, false).await.unwrap();

        assert_eq!(
            report.candidates.len(),
            0,
            "unioned tags line already carries Task"
        );
    }

    #[tokio::test]
    async fn block_without_status_is_untouched() {
        let e = engine();
        upsert(&e, NOTE_A, &format!("- just a note <!-- bid:{BID} -->\n")).await;

        let report = backfill(&e, None, true).await.unwrap();

        assert_eq!(report.candidates.len(), 0);
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert!(!md.contains("tags:: Task"));
    }

    #[tokio::test]
    async fn materialized_non_resident_status_block_is_found() {
        // THE BUG (2026-06-10): on a lazily-resident mosaic the engine only
        // holds docs for notes that have flowed through it — the canonical
        // content for everything else is the materialized notes/<slug>.md.
        // The live run printed "nothing to do" on ~3,951 task blocks because
        // the scan only saw the ~9 resident docs. The scan must also read
        // the materialized files.
        let temp = TempDir::new().unwrap();
        let notes = temp.path().join("notes");
        fs::create_dir_all(&notes).unwrap();
        fs::write(
            notes.join("2026-01-02.md"),
            format!(
                "---\ntitle: \"2026-01-02\"\n---\n\n- buy milk <!-- bid:{BID} -->\n  status:: doing\n"
            ),
        )
        .unwrap();
        let e = engine(); // fresh — NOTHING resident

        let report = backfill(&e, Some(&notes), false).await.unwrap();

        assert_eq!(
            report.candidates.len(),
            1,
            "a status block in a materialized-but-not-resident note must be found"
        );
        assert!(report.candidates[0].needs_hydration);
        assert_eq!(report.candidates[0].slug, "2026-01-02");
        assert_eq!(report.applied, 0, "dry-run writes nothing");
        assert_eq!(report.hydrated_notes, 0, "dry-run hydrates nothing");
    }

    #[tokio::test]
    async fn non_resident_note_hydrates_on_apply_preserves_content_and_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let notes = temp.path().join("notes");
        fs::create_dir_all(&notes).unwrap();
        fs::write(
            notes.join("2026-01-02.md"),
            format!(
                "---\ntitle: \"2026-01-02\"\n---\n\n- buy milk <!-- bid:{BID} -->\n  status:: doing\n"
            ),
        )
        .unwrap();
        let e = engine();

        // Apply hydrates under the stable blake3(slug) id, then tags.
        let report = backfill(&e, Some(&notes), true).await.unwrap();
        assert_eq!(report.applied, 1);
        assert_eq!(report.hydrated_notes, 1);
        let note_id = crate::mosaic_notes::stable_uuid_from_slug("2026-01-02");
        let md = e.render_note_full(note_id).await.unwrap();
        assert!(md.contains("buy milk"), "content preserved: {md:?}");
        assert!(md.contains("status:: doing"), "props preserved: {md:?}");
        assert_eq!(
            md.matches("tags:: Task").count(),
            1,
            "exactly one tags:: Task line: {md:?}"
        );

        // Now the note is resident — a re-run scans the ENGINE copy (no
        // resident+disk double match) and finds nothing to do.
        let second = backfill(&e, Some(&notes), true).await.unwrap();
        assert_eq!(second.candidates.len(), 0, "idempotent");
        assert_eq!(second.hydrated_notes, 0);
        let md = e.render_note_full(note_id).await.unwrap();
        assert_eq!(md.matches("tags:: Task").count(), 1, "{md:?}");
    }

    #[tokio::test]
    async fn block_already_tagged_is_untouched() {
        let e = engine();
        upsert(
            &e,
            NOTE_A,
            &format!("- buy milk <!-- bid:{BID} -->\n  status:: doing\n  tags:: Task\n"),
        )
        .await;

        let report = backfill(&e, None, false).await.unwrap();

        assert_eq!(
            report.candidates.len(),
            0,
            "a block that already has #Task is not a candidate"
        );
    }
}
