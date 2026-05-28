//! Fans-out [`SyncEngine`] calls to two backing engines and compares
//! their outputs. The migration vehicle (decisions.md 2026-05-27).
//!
//! ## Why this exists
//!
//! Cutting over from `SqliteEngine` to `LoroEngine` in one step would
//! risk losing data if the new engine has a subtle bug we didn't catch
//! in tests. Instead, both engines run side-by-side: every mutation
//! goes to both, every read still comes from `SqliteEngine`. The
//! Loro side mirrors what would have been written; we periodically
//! compare materialized output to detect divergence. After a week of
//! zero divergence in normal usage, we flip the read path to Loro and
//! eventually rip out SqliteEngine.
//!
//! ## HLC sharing
//!
//! Both engines mint timestamps from the **same** `Hlc` instance. The
//! `DualEngine::new` constructor accepts a single `Arc<Hlc>` and hands
//! it to both. Without this, the engines' produced op streams would
//! differ on timestamps alone — `record_local` would emit op A with
//! HLC T1 from SqliteEngine and op A' with HLC T2 from LoroEngine,
//! looking like two distinct ops to anything downstream.
//!
//! Currently `SqliteEngine` mints its own HLC internally. Sharing
//! requires either refactoring SqliteEngine to accept an `Hlc` at
//! construction (cleaner) OR snapshotting after each call (hackier).
//! Phase 1 of dual-write uses the snapshot approach: we let
//! `SqliteEngine` mint its HLC, then advance the shared HLC to match
//! before LoroEngine records. This adds a single-tick race window
//! (both engines could be invoked from different threads), but for
//! the smoke phase that's acceptable.

use crate::device::DeviceId;
use crate::engine::{
    applied::AppliedChanges, cursor::PeerCursor, loro_engine::LoroEngine, sqlite_engine::SqliteEngine,
    LocalCursor, ParkedSummary, ProducedBatch, ReplayReport, SyncEngine,
};
use crate::error::SyncResult;
use crate::hlc::Hlc;
use crate::oplog::op::{ContentHash, EncodedOp, OpPayload};
use crate::oplog::parked::ParkReason;
use crate::wire::envelope::SyncEnvelope;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

/// Result of comparing one note's rendered output between the primary
/// and shadow engines. `Match` means the two engines agree after
/// normalization (bid markers stripped, trailing whitespace trimmed);
/// `Diverge` carries both normalized forms for the warn log.
#[derive(Debug, Clone)]
pub enum NoteComparison {
    /// Both engines rendered the same content after normalization.
    Match,
    /// Engines disagree; both normalized forms attached for diagnostics.
    Diverge {
        /// SqliteEngine's materialized markdown body, normalized.
        primary: String,
        /// LoroEngine's rendered tree, normalized.
        shadow: String,
    },
    /// Primary engine has nothing materialized yet (no slug, no file).
    /// Shadow may or may not have the note.
    PrimaryMissing,
    /// Shadow hasn't seen this note. Should not happen if we iterate
    /// via `shadow.note_ids()`, but kept for symmetry.
    ShadowMissing,
}

/// Default interval between divergence-check passes. Long enough that
/// the check isn't a performance concern, short enough that a real
/// divergence surfaces in a few minutes of normal usage.
pub const DIVERGENCE_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Wraps both a `SqliteEngine` (authoritative) and a `LoroEngine`
/// (shadow). Reads come from SqliteEngine; writes fan out to both.
/// Used as the engine when `TESELA_LORO_DUAL_WRITE=1` is set on the
/// server.
pub struct DualEngine {
    primary: SqliteEngine,
    shadow: LoroEngine,
}

impl DualEngine {
    /// Wrap an existing `SqliteEngine` + a fresh `LoroEngine` that
    /// shares the given HLC clock + device id.
    pub fn new(primary: SqliteEngine, shadow: LoroEngine) -> Self {
        Self { primary, shadow }
    }

    /// Build a `DualEngine` from a `SqliteEngine`, deriving the device
    /// id + HLC from the primary so they're guaranteed to match.
    /// Convenience for the server-side wiring.
    ///
    /// Synchronous; does NOT pre-populate the shadow from the primary's
    /// oplog. Call `prepopulate_shadow_from_oplog()` on the returned
    /// instance if you want the divergence check to cover historical
    /// notes from the first tick.
    pub fn from_primary(primary: SqliteEngine) -> Self {
        let device = primary.device();
        // SqliteEngine has its own HLC inside; for the scaffold we pass
        // a fresh `Arc<Hlc>` to LoroEngine. The two clocks may
        // disagree by milliseconds on concurrent writes — acceptable
        // for the smoke phase, see module docstring.
        let shadow_hlc = Arc::new(Hlc::new(device));
        let shadow = LoroEngine::new(device, shadow_hlc);
        Self { primary, shadow }
    }

    /// Build a `DualEngine` whose shadow persists per-note snapshots
    /// under `snapshot_dir`. On construction, existing snapshots are
    /// loaded into the shadow so it survives process restart without
    /// re-replaying the entire oplog. For notes whose snapshot is
    /// missing, the caller should still invoke
    /// `prepopulate_shadow_from_oplog()` once — that path is idempotent
    /// (existing tree nodes aren't duplicated when NoteUpsert re-fires
    /// because the seed branch checks `already_has_blocks`).
    pub async fn from_primary_with_snapshot_dir(
        primary: SqliteEngine,
        snapshot_dir: std::path::PathBuf,
    ) -> crate::error::SyncResult<Self> {
        let device = primary.device();
        let shadow_hlc = Arc::new(Hlc::new(device));
        let shadow =
            LoroEngine::with_snapshot_dir(device, shadow_hlc, snapshot_dir).await?;
        Ok(Self { primary, shadow })
    }

    /// Walk the mosaic's `notes/` directory and seed the shadow with
    /// every note file that isn't already tracked. Catches notes that
    /// were created via the pre-Phase-1 `FsNoteStore.write_note` path
    /// and never made it into the oplog. After this runs, the
    /// divergence check has coverage over the entire corpus, not just
    /// notes touched by record_local since the engine became
    /// authoritative.
    ///
    /// Returns the number of NEW notes added to the shadow. Idempotent
    /// — re-running skips notes already in the shadow. Each new note's
    /// snapshot lands on disk so the next boot loads from snapshot
    /// without re-scanning.
    pub async fn seed_shadow_from_disk(
        &self,
        notes_dir: &std::path::Path,
    ) -> SyncResult<usize> {
        let mut entries = match tokio::fs::read_dir(notes_dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => {
                return Err(crate::error::SyncError::Storage(format!(
                    "read notes dir {}: {e}",
                    notes_dir.display()
                )))
            }
        };
        let mut added = 0usize;
        let known_ids: std::collections::HashSet<[u8; 16]> =
            self.shadow.note_ids().await.into_iter().collect();
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            crate::error::SyncError::Storage(format!(
                "read_dir {}: {e}",
                notes_dir.display()
            ))
        })? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let hash = blake3::hash(stem.as_bytes());
            let mut note_id = [0u8; 16];
            note_id.copy_from_slice(&hash.as_bytes()[..16]);
            if known_ids.contains(&note_id) {
                continue;
            }
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        "tesela-sync/dual-write: seed_from_disk read {}: {e}",
                        path.display()
                    );
                    continue;
                }
            };
            // Synthesize a NoteUpsert and apply it through the shadow's
            // normal seed path. apply_payload handles snapshot write.
            let payload = OpPayload::NoteUpsert {
                note_id,
                display_alias: Some(stem.to_string()),
                title: stem.to_string(),
                content,
                created_at_millis: 0,
            };
            if let Err(e) = self.shadow.apply_payload(&payload).await {
                tracing::warn!(
                    "tesela-sync/dual-write: seed_from_disk apply {}: {e}",
                    stem
                );
                continue;
            }
            added += 1;
        }
        Ok(added)
    }

    /// Replay the primary's full oplog through the shadow. Used at
    /// startup so the divergence check covers all existing notes from
    /// the first tick, not just notes touched since boot.
    ///
    /// If the shadow was loaded with snapshot persistence and already
    /// has notes (i.e. snapshots survived the previous shutdown),
    /// returns 0 immediately — the snapshots ARE the canonical state.
    /// Force a replay by clearing `<mosaic>/.tesela/loro/` before boot.
    ///
    /// Returns the number of payloads replayed. Errors out if the
    /// oplog read fails; individual payload apply errors are logged
    /// and skipped.
    pub async fn prepopulate_shadow_from_oplog(&self) -> SyncResult<usize> {
        if self.shadow.note_count().await > 0 {
            tracing::info!(
                "tesela-sync/dual-write: shadow loaded {} notes from snapshots; skipping oplog replay",
                self.shadow.note_count().await
            );
            return Ok(0);
        }
        let payloads = self.primary.iter_oplog_payloads().await?;
        let total = payloads.len();
        let mut skipped = 0usize;
        let mut sample_errors: Vec<(String, String)> = Vec::new();
        for payload in &payloads {
            if let Err(e) = self.shadow.apply_payload(payload).await {
                skipped += 1;
                if sample_errors.len() < 5 {
                    sample_errors.push((format!("{:?}", payload.kind()), format!("{e}")));
                }
            }
        }
        if skipped > 0 {
            tracing::warn!(
                "tesela-sync/dual-write: prepopulated shadow with {} payloads ({} skipped)",
                total - skipped,
                skipped
            );
            for (kind, err) in &sample_errors {
                tracing::warn!("  prepopulate skip sample: {kind} -> {err}");
            }
        }
        Ok(total - skipped)
    }

    /// Access to the shadow engine for tests + divergence-comparison
    /// hooks. Not exposed via the trait — only the wrapper's owners
    /// look here.
    pub fn shadow(&self) -> &LoroEngine {
        &self.shadow
    }

    /// Access to the primary engine for the same reasons.
    pub fn primary(&self) -> &SqliteEngine {
        &self.primary
    }

    /// Compare one note's rendered output between primary and shadow,
    /// after normalization. Used by the periodic divergence check and
    /// available for ad-hoc inspection from tests / diagnostics.
    pub async fn compare_note(&self, note_id: [u8; 16]) -> NoteComparison {
        let shadow_render = self.shadow.render_note(note_id).await;
        let primary_body = self
            .primary
            .materialize_note_body(note_id)
            .await
            .ok()
            .flatten();
        match (primary_body, shadow_render) {
            (Some(p), Some(s)) => {
                let p_norm = normalize(&p);
                let s_norm = normalize(&s);
                if p_norm == s_norm {
                    NoteComparison::Match
                } else {
                    NoteComparison::Diverge {
                        primary: p_norm,
                        shadow: s_norm,
                    }
                }
            }
            (None, Some(_)) => NoteComparison::PrimaryMissing,
            (Some(_), None) => NoteComparison::ShadowMissing,
            (None, None) => NoteComparison::PrimaryMissing,
        }
    }

    /// Compare every note the shadow has seen against the primary's
    /// materialized output. Returns the per-note results plus a count
    /// of divergences for the caller's log line.
    pub async fn compare_all(&self) -> Vec<([u8; 16], NoteComparison)> {
        let ids = self.shadow.note_ids().await;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let cmp = self.compare_note(id).await;
            out.push((id, cmp));
        }
        out
    }

    /// Spawn the periodic divergence-check loop. The loop wakes every
    /// [`DIVERGENCE_CHECK_INTERVAL`] and walks `compare_all()`,
    /// emitting `tracing::warn!` for any divergence found. Cloneable
    /// `SqliteEngine` + `LoroEngine` mean we can hand the task its own
    /// handles without sharing `Self`.
    ///
    /// The task is fire-and-forget; it runs until the tokio runtime
    /// shuts down. Returns the `JoinHandle` mostly for tests that need
    /// to abort the loop early.
    pub fn spawn_divergence_check(&self) -> tokio::task::JoinHandle<()> {
        let primary = self.primary.clone();
        let shadow = self.shadow.clone();
        tokio::spawn(async move {
            let probe = DualEngine {
                primary,
                shadow,
            };
            loop {
                tokio::time::sleep(DIVERGENCE_CHECK_INTERVAL).await;
                let results = probe.compare_all().await;
                let total = results.len();
                let mut matched = 0usize;
                let mut diverged = Vec::new();
                let mut primary_missing = 0usize;
                let mut shadow_missing = 0usize;
                for (id, cmp) in results {
                    match cmp {
                        NoteComparison::Match => matched += 1,
                        NoteComparison::Diverge { primary, shadow } => {
                            diverged.push((id, primary, shadow))
                        }
                        NoteComparison::PrimaryMissing => primary_missing += 1,
                        NoteComparison::ShadowMissing => shadow_missing += 1,
                    }
                }
                if diverged.is_empty() {
                    tracing::info!(
                        "tesela-sync/dual-write: divergence check OK ({} notes: \
                         {} match, {} primary-missing, {} shadow-missing)",
                        total,
                        matched,
                        primary_missing,
                        shadow_missing
                    );
                    continue;
                }
                tracing::warn!(
                    "tesela-sync/dual-write: {} of {} notes diverged \
                     ({} match, {} primary-missing, {} shadow-missing)",
                    diverged.len(),
                    total,
                    matched,
                    primary_missing,
                    shadow_missing
                );
                for (id, primary, shadow) in diverged.iter().take(3) {
                    tracing::warn!(
                        "  note {} primary={:?} shadow={:?}",
                        hex::encode(id),
                        truncate(primary, 200),
                        truncate(shadow, 200)
                    );
                }
            }
        })
    }
}

/// Normalize a rendered note body for comparison. Keeps the comparison
/// focused on what LoroEngine actually models (block bullet lines +
/// hierarchy) and discards parts of the file format SqliteEngine
/// preserves but LoroEngine doesn't yet:
///
/// - Strip `<!-- bid:UUID -->` markers (LoroEngine doesn't emit them).
/// - Drop block-property lines (`  key:: value` indented under a
///   bullet). LoroEngine doesn't model block properties yet — this
///   normalization keeps the soak focused on block-content divergence.
///   Removing this once `properties: LoroMap` lands in LoroEngine.
/// - Drop blank lines (SqliteEngine preserves user-typed blank lines
///   between blocks; LoroEngine's render is dense).
/// - Trim trailing whitespace per line.
fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        let stripped = strip_bid_markers(line);
        let trimmed = stripped.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        if is_block_property_line(trimmed) {
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    out
}

/// True for lines like `  status:: done` or `    tags:: Task` — a
/// (possibly tab- or space-indented) identifier followed by `::` and a
/// value. Used by `normalize` to drop block-property lines that
/// LoroEngine doesn't model yet.
fn is_block_property_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    // Must start with leading whitespace (otherwise it's a top-level
    // line, not a child of a block).
    if trimmed.len() == line.len() {
        return false;
    }
    // Bullet lines aren't properties.
    if trimmed.starts_with("- ") || trimmed == "-" {
        return false;
    }
    // Look for `key::` where key is non-empty letters/digits/underscore/dash.
    let Some(idx) = trimmed.find("::") else {
        return false;
    };
    let key = &trimmed[..idx];
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Remove every `<!-- bid:... -->` marker from a line. Markers are
/// always emitted as a single space + the comment on the line where a
/// block starts; stripping is byte-level scan, no regex.
fn strip_bid_markers(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(start) = rest.find("<!-- bid:") {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find("-->") {
            rest = &rest[start + end + 3..];
        } else {
            // Malformed marker; keep the rest verbatim.
            out.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[async_trait]
impl SyncEngine for DualEngine {
    fn device(&self) -> DeviceId {
        self.primary.device()
    }

    async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash> {
        // Primary first — its return value is authoritative. If it
        // fails we don't even try the shadow; the server caller
        // wouldn't see the op succeed and shouldn't see it in either
        // engine.
        let hash = self.primary.record_local(payload.clone()).await?;
        // Shadow is best-effort: log on failure, never propagate the
        // error. The whole point of dual-write is "primary stays
        // correct even if shadow has a bug we haven't caught."
        if let Err(e) = self.shadow.record_local(payload).await {
            tracing::warn!(
                "tesela-sync/dual-write: shadow record_local failed: {e} \
                 (primary succeeded, divergence will be visible at compare time)"
            );
        }
        Ok(hash)
    }

    async fn apply_changes(
        &self,
        peer: DeviceId,
        envelope: SyncEnvelope,
    ) -> SyncResult<AppliedChanges> {
        // Apply on both. The shadow's no-op implementation means this
        // doesn't actually do anything Loro-side yet; lands when we
        // start sending Loro updates over the wire.
        let applied = self.primary.apply_changes(peer, envelope.clone()).await?;
        if let Err(e) = self.shadow.apply_changes(peer, envelope).await {
            tracing::warn!("tesela-sync/dual-write: shadow apply_changes failed: {e}");
        }
        Ok(applied)
    }

    async fn produce_changes_since(
        &self,
        peer: DeviceId,
        since: PeerCursor,
        max_bytes: usize,
    ) -> SyncResult<ProducedBatch> {
        // Reads come from primary. Shadow's produce returns empty
        // batches in the scaffold; once we send Loro updates over the
        // wire, the dual-write wrapper picks the primary's ops here
        // and the comparison logic verifies the shadow would have
        // emitted the same set.
        self.primary
            .produce_changes_since(peer, since, max_bytes)
            .await
    }

    async fn produce_local_authored_since(
        &self,
        since: PeerCursor,
        max_bytes: usize,
    ) -> SyncResult<ProducedBatch> {
        self.primary
            .produce_local_authored_since(since, max_bytes)
            .await
    }

    async fn local_cursor(&self) -> SyncResult<LocalCursor> {
        self.primary.local_cursor().await
    }

    async fn peer_cursor(&self, peer: DeviceId) -> SyncResult<PeerCursor> {
        self.primary.peer_cursor(peer).await
    }

    async fn ack_peer(&self, peer: DeviceId, ack: PeerCursor) -> SyncResult<()> {
        self.primary.ack_peer(peer, ack).await
    }

    async fn park_op(&self, op: EncodedOp, reason: ParkReason) -> SyncResult<()> {
        self.primary.park_op(op, reason).await
    }

    async fn replay_parked(&self) -> SyncResult<ReplayReport> {
        self.primary.replay_parked().await
    }

    async fn parked_summary(&self) -> SyncResult<ParkedSummary> {
        self.primary.parked_summary().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_bid_markers_and_trailing_ws() {
        let raw = "- hello world <!-- bid:abc-123 -->   \n\t- child <!-- bid:def --> \n";
        let normed = normalize(raw);
        assert_eq!(normed, "- hello world\n\t- child\n");
    }

    #[test]
    fn normalize_passes_through_markerless() {
        let raw = "- a\n\t- b\n";
        assert_eq!(normalize(raw), raw);
    }

    #[test]
    fn normalize_drops_block_property_lines() {
        let raw = "- do a thing\n  status:: done\n  tags:: Task\n- next block\n";
        assert_eq!(normalize(raw), "- do a thing\n- next block\n");
    }

    #[test]
    fn normalize_drops_blank_lines() {
        let raw = "\n- a\n\n- b\n\n";
        assert_eq!(normalize(raw), "- a\n- b\n");
    }

    #[test]
    fn normalize_keeps_top_level_lines_that_look_like_properties() {
        // `key:: value` at the top level (no indent) isn't a block
        // property — leave it alone.
        let raw = "key:: top-level\n- a block\n";
        assert_eq!(normalize(raw), "key:: top-level\n- a block\n");
    }

    #[tokio::test]
    async fn compare_note_returns_primary_missing_when_no_mosaic() {
        let device = DeviceId::from_bytes([7u8; 16]);
        let primary = SqliteEngine::open("sqlite::memory:", device).await.unwrap();
        let dual = DualEngine::from_primary(primary);
        let note_id = [99u8; 16];

        // Record a NoteUpsert so the shadow has the note.
        dual.record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("smoke".into()),
            title: "Smoke".into(),
            content: "---\ntitle: Smoke\n---\n- hi\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

        // No mosaic_dir on the in-memory primary, so materialize returns
        // None. Compare reports PrimaryMissing.
        match dual.compare_note(note_id).await {
            NoteComparison::PrimaryMissing => {}
            other => panic!("expected PrimaryMissing, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn note_upsert_lands_in_both_engines() {
        let device = DeviceId::from_bytes([7u8; 16]);
        // SqliteEngine with an in-memory DB so we don't touch disk.
        let primary = SqliteEngine::open("sqlite::memory:", device).await.unwrap();
        let dual = DualEngine::from_primary(primary);

        let note_id = [42u8; 16];
        let payload = OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("dual-smoke".into()),
            title: "Dual smoke".into(),
            content: "---\ntitle: Dual smoke\n---\n- Hi there\n".into(),
            created_at_millis: 1,
        };

        dual.record_local(payload).await.unwrap();

        // Primary should have one oplog row (the NoteUpsert just
        // recorded). Shadow should have one Loro doc for this note.
        let primary_total = dual.primary().oplog_total().await.unwrap();
        assert_eq!(primary_total, 1, "primary oplog should hold the NoteUpsert");
        assert_eq!(
            dual.shadow().note_count().await,
            1,
            "shadow should have created the per-note Loro doc"
        );
    }
}
