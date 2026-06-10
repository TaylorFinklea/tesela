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
//! Dry-run by default (list affected blocks + a count, write nothing); only
//! `--apply` mutates. This is a DATA migration — it must never auto-run, and it
//! refuses to run while `tesela-server` holds the mosaic (single-writer).

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, PropOp, PropScalar, SyncEngine};

/// One block that carries a status but no Task tag.
pub struct Candidate {
    pub note_id: [u8; 16],
    pub block_id: [u8; 16],
    /// The block's first text line, for human-readable dry-run output.
    pub preview: String,
}

#[derive(Default)]
pub struct BackfillReport {
    pub candidates: Vec<Candidate>,
    pub applied: usize,
}

/// Core scan + (optional) apply over an open engine. Read-only first pass
/// collects candidates so enumeration never races our own writes; the apply
/// pass adds the tag via a structured `BlockPropertySet`. Pure over the engine
/// so it is unit-testable with an in-process `LoroEngine`.
pub async fn backfill(engine: &LoroEngine, apply: bool) -> Result<BackfillReport> {
    let mut report = BackfillReport::default();

    // Pass 1 — scan every note's blocks (read-only).
    for note_id in engine.note_ids().await {
        let Some(md) = engine.render_note_full(note_id).await else {
            continue;
        };
        let tree = tesela_core::note_tree::parse_note(&md);
        for block in &tree.blocks {
            if has_status(&block.text) && !has_task(&block.text) {
                report.candidates.push(Candidate {
                    note_id,
                    block_id: *block.id.as_bytes(),
                    preview: block.text.lines().next().unwrap_or("").trim().to_string(),
                });
            }
        }
    }

    // Pass 2 — apply the structured tag-add (union/idempotent).
    if apply {
        for c in &report.candidates {
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
/// the server's default), run the backfill, and print the report.
pub async fn run(mosaic: &Path, apply: bool) -> Result<()> {
    let _lock = acquire_mosaic_lock(mosaic).context(
        "could not lock the mosaic — is tesela-server (or the desktop app) running on it? \
         Stop it before running backfill-task (single-writer).",
    )?;

    let device = load_device_id(mosaic);
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");
    let hlc = Arc::new(Hlc::new(device));
    let engine = LoroEngine::with_dirs(device, hlc, snapshot_dir, Some(notes_dir))
        .await
        .map_err(|e| anyhow::anyhow!("open loro engine: {e}"))?;

    let report = backfill(&engine, apply).await?;

    if report.candidates.is_empty() {
        println!("backfill-task: no status-bearing blocks are missing #Task — nothing to do.");
        return Ok(());
    }

    if apply {
        println!("backfill-task: added #Task to {} block(s):", report.applied);
    } else {
        println!(
            "backfill-task (DRY RUN — re-run with --apply to write): {} block(s) would get #Task:",
            report.candidates.len()
        );
    }
    for c in &report.candidates {
        println!(
            "  {}:{}  {}",
            hex16(&c.note_id),
            hex16(&c.block_id),
            c.preview
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
fn line_property_key(line: &str) -> Option<String> {
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
fn load_device_id(mosaic: &Path) -> DeviceId {
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
fn acquire_mosaic_lock(mosaic: &Path) -> Result<std::fs::File> {
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

fn hex16(bytes: &[u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let report = backfill(&e, false).await.unwrap();

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

        let report = backfill(&e, true).await.unwrap();

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

        backfill(&e, true).await.unwrap();
        let second = backfill(&e, true).await.unwrap();

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

        let report = backfill(&e, false).await.unwrap();

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

        let report = backfill(&e, true).await.unwrap();

        assert_eq!(report.candidates.len(), 0);
        let md = e.render_note_full(NOTE_A).await.unwrap();
        assert!(!md.contains("tags:: Task"));
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

        let report = backfill(&e, false).await.unwrap();

        assert_eq!(
            report.candidates.len(),
            0,
            "a block that already has #Task is not a candidate"
        );
    }
}
