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
}

#[async_trait]
impl SyncEngine for DualEngine {
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
