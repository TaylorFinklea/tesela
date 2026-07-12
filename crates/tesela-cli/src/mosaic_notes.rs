//! Shared residency machinery for CLI data tools on a lazily-resident mosaic.
//!
//! The Loro engine only holds docs for notes that have flowed through it; the
//! canonical content for everything else is the engine's own materialized
//! `<mosaic>/notes/<slug>.md` files. Any tool that scans "the whole mosaic"
//! must therefore read BOTH — the engine render for resident notes, the
//! materialized file otherwise — and, before writing structured ops against a
//! non-resident note's blocks, hydrate it with the same op every editor save
//! and `reseed_from_disk` use: a `NoteUpsert` of the file content under the
//! system-wide stable id `blake3(slug)[..16]` (the server's
//! `stable_uuid_from_slug`). `NoteUpsert` is a non-destructive per-bid
//! reconcile on every engine (2026-06-10 semantics), so hydration cannot
//! clobber concurrent state.
//!
//! Extracted from `recover_logseq_dates` (c40ea34, live-proven on the real
//! mosaic) when `backfill_task` needed the same handling.

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
pub(crate) use tesela_core::stable_uuid_from_slug;
use tesela_sync::{Hlc, LoroEngine, OpPayload, SyncEngine};

use crate::backfill_task::{acquire_mosaic_lock, hex16, load_device_id};

/// One Loro-resident note: stable id, slug (display alias, falling back to
/// the hex id), and the engine's full render — the authority for residents.
pub(crate) struct ResidentNote {
    pub note_id: [u8; 16],
    pub slug: String,
    pub md: String,
}

/// Enumerate every Loro-resident note with its slug and full render.
pub(crate) async fn resident_notes(engine: &LoroEngine) -> Vec<ResidentNote> {
    let slugs: HashMap<String, String> = engine
        .index_entries()
        .await
        .into_iter()
        .map(|e| (e.note_id, e.slug))
        .collect();
    let mut out = Vec::new();
    for note_id in engine.note_ids().await {
        let Some(md) = engine.render_note_full(note_id).await else {
            continue;
        };
        let slug = slugs
            .get(&hex16(&note_id))
            .cloned()
            .unwrap_or_else(|| hex16(&note_id));
        out.push(ResidentNote { note_id, slug, md });
    }
    out
}

/// Read every materialized `<dir>/<slug>.md` whose slug is NOT resident,
/// in sorted order. Returns `(slug, file content)` pairs; empty when the
/// directory doesn't exist (e.g. in-memory test engines).
pub(crate) fn read_non_resident_notes(
    dir: &Path,
    resident_slugs: &HashSet<String>,
) -> Result<Vec<(String, String)>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("read notes dir {}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    paths.sort();
    let mut out = Vec::new();
    for path in paths {
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if resident_slugs.contains(stem) {
            continue; // engine version is authority
        }
        let content =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        out.push((stem.to_string(), content));
    }
    Ok(out)
}

/// `title:` from a YAML frontmatter block (mirrors the engine's reseed
/// title extraction); `None` if absent.
pub(crate) fn frontmatter_title(content: &str) -> Option<String> {
    let mut lines = content.lines();
    if lines.next()?.trim_end_matches('\r') != "---" {
        return None;
    }
    for line in lines {
        if line.trim_end_matches('\r') == "---" {
            return None;
        }
        if let Some(v) = line.strip_prefix("title:") {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// Hydrate a non-resident note into the engine: a `NoteUpsert` of its
/// materialized content — the per-bid-reconcile op every editor save
/// records — so subsequent structured ops can address its blocks.
pub(crate) async fn hydrate_note(
    engine: &LoroEngine,
    note_id: [u8; 16],
    slug: &str,
    content: &str,
) -> Result<()> {
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(slug.to_string()),
            title: frontmatter_title(content).unwrap_or_else(|| slug.to_string()),
            content: content.to_string(),
            created_at_millis: 0,
        })
        .await
        .map_err(|e| anyhow::anyhow!("hydrate note {slug}: {e}"))?;
    Ok(())
}

/// Parse `content`, stamp persistent block ids onto any unstamped bullets,
/// and return the canonical serialized form. Mirrors the server's
/// `stamp_block_ids` (routes/notes.rs `create_note`/`update_note`) — an
/// unstamped block parsed independently by a direct disk write and by the
/// engine's `NoteUpsert` apply would each mint a different id, so stamping
/// up front keeps the id embedded in the content consistent with whatever
/// the engine's own tree assigns. Returns `content` unchanged if every
/// bullet already has a bid.
pub(crate) fn stamp_block_ids(content: &str) -> String {
    let tree = tesela_core::note_tree::parse_note(content);
    if !tree.stamped_any {
        return content.to_string();
    }
    tesela_core::note_tree::serialize_note(&tree)
}

/// Turn a plain-text `--content` body into the bullet form the block-model
/// round trip actually preserves. Per the `note_tree` module docs, "Non-
/// bullet body content does NOT survive the round trip" — a heading or bare
/// prose paragraph is silently dropped by `parse_note`, so hydrating it
/// through the engine (which parses into blocks, then materializes FROM the
/// tree) would erase it. Mirrors the fix already applied to
/// `daily::daily_note_content` (seeding `- ` not `# {title}`): an empty body
/// becomes a single blank bullet, a body with no bullet lines gets each
/// non-blank line turned into its own top-level bullet, and a body that
/// already contains at least one bullet line is assumed outliner-shaped and
/// returned unchanged.
pub(crate) fn ensure_bulleted_body(body: &str) -> String {
    if body.is_empty() || body.lines().any(|l| l.trim_start().starts_with("- ")) {
        return if body.is_empty() {
            "- \n".to_string()
        } else {
            body.to_string()
        };
    }
    let mut out = String::new();
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        out.push_str("- ");
        out.push_str(line);
        out.push('\n');
    }
    if out.is_empty() {
        out.push_str("- \n");
    }
    out
}

/// Lock the mosaic (refuse while `tesela-server`/the desktop holds it) and
/// open the Loro engine over its snapshots + materialized notes dir — the
/// same LoroEngine-open pattern `repair_garbled_blocks` and `backfill_task`
/// use for their one-shot data tools, extended here to the CLI's day-to-day
/// `new`/`edit`/`daily` writes so they go through the engine instead of a
/// direct `FsNoteStore` write (engine-only-writes rule, 2026-06-09 — a
/// local-only write never syncs and gets reverted by the engine's next
/// materialize). The returned `File` guard must stay alive for the duration
/// of the write; dropping it releases the flock.
pub(crate) async fn open_locked_engine(mosaic: &Path) -> Result<(std::fs::File, LoroEngine)> {
    let lock = acquire_mosaic_lock(mosaic).context(
        "could not lock the mosaic — is tesela-server (or the desktop app) running on it? \
         Stop it before writing via the CLI, or make the change through the running server/app \
         instead (single-writer; the CLI refuses to bypass the lock).",
    )?;
    let device = load_device_id(mosaic);
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");
    let hlc = Arc::new(Hlc::new(device));
    let engine = LoroEngine::with_dirs(device, hlc, snapshot_dir, Some(notes_dir))
        .await
        .map_err(|e| anyhow::anyhow!("open loro engine: {e}"))?;
    Ok((lock, engine))
}
