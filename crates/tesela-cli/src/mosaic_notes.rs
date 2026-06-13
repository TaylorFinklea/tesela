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
use tesela_sync::{LoroEngine, OpPayload, SyncEngine};

use crate::backfill_task::hex16;

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

/// System-wide stable note id: blake3(slug) truncated to 16 bytes —
/// mirrors the server's `stable_uuid_from_slug` (routes/notes.rs) and
/// the engine's `reseed_from_disk`, so the CLI addresses the SAME doc
/// every other surface does.
pub(crate) fn stable_uuid_from_slug(slug: &str) -> [u8; 16] {
    let hash = blake3::hash(slug.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&hash.as_bytes()[..16]);
    out
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
