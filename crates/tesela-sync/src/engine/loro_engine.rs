//! Loro-backed [`SyncEngine`] implementation.
//!
//! **Status (Phase 4 — Loro migration, decisions.md 2026-05-27):** scaffold.
//! Currently implements only the subset needed for the dual-write smoke
//! test: `NoteUpsert` ops materialize through Loro alongside `SqliteEngine`.
//! Other op types and the full trait surface are stubbed and will be
//! filled in as the migration progresses.
//!
//! ## Schema
//!
//! One [`loro::LoroDoc`] per note, keyed by `note_id: [u8; 16]`. Each doc
//! holds a top-level [`loro::LoroTree`] called `"blocks"` representing the
//! block parent/order structure. Each tree node carries a meta map with:
//!
//! - `text: LoroText` — character-level concurrent edits (the bonus
//!   feature Loro unlocks vs the hand-rolled engine).
//! - `properties: LoroMap<String, String>` — per-block key/value props.
//! - `tags: LoroList<String>` — block-level tags.
//!
//! Frontmatter (note-level title, tags, created-at) lives on the doc's
//! root metadata. Implementation comes online as more op types are
//! ported; the scaffold only wires `NoteUpsert` end-to-end.
//!
//! ## Persistence
//!
//! For the dual-write smoke phase, docs are held in memory only — the
//! point is to compare materialized output against `SqliteEngine`, not
//! to persist anything yet. A real persistent store (per-note `.loro`
//! snapshots under `<mosaic>/sync-loro/`) lands once dual-write is
//! stable for a week per the cutover plan.

use crate::device::DeviceId;
use crate::engine::{
    applied::AppliedChanges, cursor::PeerCursor, LocalCursor, ParkedSummary, ProducedBatch,
    ReplayReport, SyncEngine,
};
use crate::error::{SyncError, SyncResult};
use crate::hlc::Hlc;
use crate::oplog::op::{ContentHash, EncodedOp, OpPayload};
use crate::oplog::parked::ParkReason;
use crate::wire::envelope::SyncEnvelope;
use async_trait::async_trait;
use loro::LoroDoc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Minimal Loro-backed engine for the dual-write scaffold. Most trait
/// methods are stubbed; only `record_local` for `NoteUpsert` does real
/// work. The stubs return defaults that match `SqliteEngine`'s shape
/// when there's nothing to do (empty batches, zero ops applied, etc.)
/// so the dual-write wrapper doesn't have to special-case them.
pub struct LoroEngine {
    inner: Arc<Inner>,
}

struct Inner {
    /// Per-note Loro documents. Key is the same `note_id` (`[u8; 16]`)
    /// `OpPayload::NoteUpsert` carries.
    docs: RwLock<HashMap<[u8; 16], LoroDoc>>,
    /// Local device id — must match the SqliteEngine's device id when
    /// dual-writing so the produced op streams identify the same peer.
    /// Currently held but unused; consumed once `apply_changes` /
    /// `produce_changes_since` start doing real work.
    #[allow(dead_code)]
    device: DeviceId,
    /// HLC clock. **Must be shared with SqliteEngine** in dual-write
    /// mode so both engines mint identical timestamps for the same op.
    /// The `DualEngine` wrapper enforces this by injecting one `Hlc` at
    /// construction.
    hlc: Arc<Hlc>,
}

impl LoroEngine {
    /// Construct a new in-memory Loro engine with the given device id +
    /// HLC. The `hlc` argument is `Arc<Hlc>` precisely to support
    /// shared-clock dual-write.
    pub fn new(device: DeviceId, hlc: Arc<Hlc>) -> Self {
        Self {
            inner: Arc::new(Inner {
                docs: RwLock::new(HashMap::new()),
                device,
                hlc,
            }),
        }
    }

    /// Number of distinct notes the engine has seen. Test/diagnostic
    /// hook — not part of the SyncEngine trait.
    pub async fn note_count(&self) -> usize {
        self.inner.docs.read().await.len()
    }

    /// Render a note's current state as markdown by walking its Loro
    /// tree. Mirrors what `SqliteEngine`'s materialize step would write
    /// to disk. Used by the dual-write wrapper to compare outputs.
    ///
    /// Returns `None` for unknown note ids.
    pub async fn render_note(&self, note_id: [u8; 16]) -> Option<String> {
        let docs = self.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        let tree = doc.get_tree("blocks");
        // For the scaffold, walk all tree nodes and emit one bullet per
        // node with its meta `"text"` value. Indent + parent-respecting
        // walk lands as more op types come online.
        let mut out = String::new();
        for node in tree.nodes() {
            if let Ok(meta) = tree.get_meta(node) {
                if let Some(text) = meta.get("text") {
                    // `meta.get` returns a `ValueOrContainer`. For the
                    // scaffold's plain-string values we just Debug-print;
                    // when we move to `LoroText` per block the render
                    // path swaps to `text.to_string()`.
                    out.push_str("- ");
                    out.push_str(&format!("{:?}", text));
                    out.push('\n');
                }
            }
        }
        Some(out)
    }

    /// Get-or-create the Loro doc for a given note id. Called from
    /// `record_local` when a NoteUpsert lands.
    async fn doc_for_note_mut(&self, note_id: [u8; 16]) -> LoroDoc {
        let mut docs = self.inner.docs.write().await;
        docs.entry(note_id).or_insert_with(LoroDoc::new).clone()
    }
}

#[async_trait]
impl SyncEngine for LoroEngine {
    fn device(&self) -> DeviceId {
        self.inner.device
    }

    /// Local-side mutation. For the scaffold we handle `NoteUpsert` by
    /// dropping the full body content onto the doc's root meta as a
    /// single text value. The "real" port (`BlockUpsert`/`Move`/`Delete`
    /// → tree operations) lands incrementally as we extract the markdown
    /// parser into block-level ops we can replay one at a time.
    async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash> {
        let hlc = self.inner.hlc.now();
        let op = EncodedOp::new(hlc, crate::SYNC_SCHEMA_VERSION, payload.clone(), None)?;
        let hash = op.content_hash;

        match &payload {
            OpPayload::NoteUpsert { note_id, content, .. } => {
                let doc = self.doc_for_note_mut(*note_id).await;
                // Scaffold approach: store the whole content on the doc's
                // root meta. Lossy compared to the eventual block-tree
                // shape, but enough to verify the wrapper round-trips.
                let root_meta = doc.get_map("root");
                root_meta
                    .insert("content", content.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro insert: {e}")))?;
                doc.commit();
            }
            _ => {
                // Other op types: scaffold no-op. They'll land op-by-op
                // during the rest of the migration phase. The
                // SqliteEngine in the dual-write pair handles them in
                // the meantime, so the system stays correct from the
                // user's perspective.
                tracing::debug!(
                    "tesela-sync/loro: scaffold no-op for {:?}",
                    std::mem::discriminant(&payload)
                );
            }
        }

        Ok(hash)
    }

    /// Apply incoming changes from a peer. Scaffold: no-op — the
    /// SqliteEngine in the dual-write pair handles real apply, and
    /// LoroEngine in this phase only needs to mirror local writes for
    /// comparison. Real apply lands when we start sending Loro updates
    /// over the wire (mid-migration).
    async fn apply_changes(
        &self,
        _peer: DeviceId,
        _envelope: SyncEnvelope,
    ) -> SyncResult<AppliedChanges> {
        Ok(AppliedChanges::default())
    }

    async fn produce_changes_since(
        &self,
        _peer: DeviceId,
        since: PeerCursor,
        _max_bytes: usize,
    ) -> SyncResult<ProducedBatch> {
        Ok(ProducedBatch {
            ops: Vec::new(),
            new_cursor: since,
        })
    }

    async fn produce_local_authored_since(
        &self,
        since: PeerCursor,
        _max_bytes: usize,
    ) -> SyncResult<ProducedBatch> {
        // Scaffold: LoroEngine doesn't yet emit ops over the wire — the
        // SqliteEngine in the dual-write pair carries that path. Empty
        // batches are correct: nothing for the relay to publish from
        // the shadow side.
        Ok(ProducedBatch {
            ops: Vec::new(),
            new_cursor: since,
        })
    }

    async fn local_cursor(&self) -> SyncResult<LocalCursor> {
        Ok(LocalCursor::Earliest)
    }

    async fn peer_cursor(&self, _peer: DeviceId) -> SyncResult<PeerCursor> {
        Ok(PeerCursor::Earliest)
    }

    async fn ack_peer(&self, _peer: DeviceId, _ack: PeerCursor) -> SyncResult<()> {
        Ok(())
    }

    async fn park_op(&self, _op: EncodedOp, _reason: ParkReason) -> SyncResult<()> {
        Ok(())
    }

    async fn replay_parked(&self) -> SyncResult<ReplayReport> {
        Ok(ReplayReport::default())
    }

    async fn parked_summary(&self) -> SyncResult<ParkedSummary> {
        Ok(ParkedSummary::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> DeviceId {
        DeviceId::from_bytes([1u8; 16])
    }

    #[tokio::test]
    async fn note_upsert_records_into_doc() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [9u8; 16];

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("smoke".into()),
                title: "Smoke".into(),
                content: "---\ntitle: Smoke\n---\n- Hello\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        assert_eq!(engine.note_count().await, 1);
        // doc exists; content stored on root meta. Detailed
        // materialization tests land as block ops come online.
    }

    #[tokio::test]
    async fn non_noteupsert_ops_are_silent_noops() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let result = engine
            .record_local(OpPayload::BlockDelete {
                block_id: [3u8; 16],
            })
            .await;
        assert!(result.is_ok());
        assert_eq!(engine.note_count().await, 0);
    }
}
