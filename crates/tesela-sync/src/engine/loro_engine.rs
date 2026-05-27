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
use loro::{LoroDoc, LoroTree, TreeID, TreeParentId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn hex_id(id: &[u8; 16]) -> String {
    hex::encode(id)
}

/// Minimal Loro-backed engine for the dual-write scaffold. Most trait
/// methods are stubbed; only `record_local` for `NoteUpsert` does real
/// work. The stubs return defaults that match `SqliteEngine`'s shape
/// when there's nothing to do (empty batches, zero ops applied, etc.)
/// so the dual-write wrapper doesn't have to special-case them.
///
/// Cloneable so the divergence-check background task can hold its own
/// handle while the wrapper keeps another. `Inner` is Arc-wrapped, so
/// clones share the same docs map and HLC.
#[derive(Clone)]
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

    /// All note ids the engine has seen. Used by the divergence-check
    /// background task to iterate over notes for comparison.
    pub async fn note_ids(&self) -> Vec<[u8; 16]> {
        self.inner.docs.read().await.keys().copied().collect()
    }

    /// Render a note's current state as markdown by walking its Loro
    /// tree. Mirrors what `SqliteEngine`'s materialize step would write
    /// to disk. Used by the dual-write wrapper to compare outputs.
    ///
    /// Returns `None` for unknown note ids. The walk is parent-respecting:
    /// each root node and its descendants emit indented `- text` lines.
    /// `indent_level` from the BlockUpsert op is used directly so the
    /// output matches SqliteEngine's materialization indent for indent.
    pub async fn render_note(&self, note_id: [u8; 16]) -> Option<String> {
        let docs = self.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        let tree = doc.get_tree("blocks");
        let mut out = String::new();
        for root in tree.children(TreeParentId::Root).unwrap_or_default() {
            render_node(&tree, root, &mut out);
        }
        Some(out)
    }

    /// Get-or-create the Loro doc for a given note id. Called from
    /// `record_local` when a NoteUpsert or BlockUpsert lands.
    async fn doc_for_note_mut(&self, note_id: [u8; 16]) -> LoroDoc {
        let mut docs = self.inner.docs.write().await;
        docs.entry(note_id).or_insert_with(LoroDoc::new).clone()
    }

    /// Locate the doc + tree node hosting a given block id by walking
    /// every doc the engine has seen. `BlockMove` / `BlockDelete` ops
    /// carry only the block id (not the owning note), so this lookup
    /// has to be a scan. For the scaffold it's fine — typical mosaics
    /// have a few hundred notes. Replace with an index once profiling
    /// flags it.
    async fn find_doc_for_block(&self, block_id: &[u8; 16]) -> Option<(LoroDoc, TreeID)> {
        let block_hex = hex_id(block_id);
        let docs = self.inner.docs.read().await;
        for doc in docs.values() {
            let tree = doc.get_tree("blocks");
            if let Some(node) = find_node_by_block_id(&tree, &block_hex) {
                return Some((doc.clone(), node));
            }
        }
        None
    }
}

/// Walk a tree node + its descendants, emitting `- text` lines indented
/// per the meta `indent_level`. Free function (not a method on Inner)
/// so it can recurse cleanly without re-locking `docs`.
fn render_node(tree: &LoroTree, node: TreeID, out: &mut String) {
    if let Ok(meta) = tree.get_meta(node) {
        let indent = meta
            .get("indent_level")
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_i64().ok())
            .unwrap_or(0) as usize;
        let text = meta
            .get("text")
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| (*s).clone())
            .unwrap_or_default();
        for _ in 0..indent {
            out.push('\t');
        }
        out.push_str("- ");
        out.push_str(&text);
        out.push('\n');
    }
    if let Some(children) = tree.children(node) {
        for child in children {
            render_node(tree, child, out);
        }
    }
}

/// Walk a tree to find the node whose `block_id` meta matches `target`.
/// O(n) over nodes in the doc; n is small (typical note < 100 blocks)
/// for the scaffold. Replace with an index if profiling needs it.
fn find_node_by_block_id(tree: &LoroTree, target_hex: &str) -> Option<TreeID> {
    for node in tree.nodes() {
        if let Ok(meta) = tree.get_meta(node) {
            if let Some(v) = meta.get("block_id") {
                if let Ok(val) = v.into_value() {
                    if let Ok(s) = val.into_string() {
                        if *s == target_hex {
                            return Some(node);
                        }
                    }
                }
            }
        }
    }
    None
}

#[async_trait]
impl SyncEngine for LoroEngine {
    fn device(&self) -> DeviceId {
        self.inner.device
    }

    /// Local-side mutation. Stamps a fresh HLC + content hash, then
    /// runs the payload through the same per-op logic that
    /// `apply_changes` uses for peer-originated ops.
    async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash> {
        let hlc = self.inner.hlc.now();
        let op = EncodedOp::new(hlc, crate::SYNC_SCHEMA_VERSION, payload.clone(), None)?;
        let hash = op.content_hash;
        self.apply_payload(&payload).await?;
        Ok(hash)
    }

    /// Apply incoming changes from a peer. The envelope's `ciphertext`
    /// at this layer is a postcard-encoded `Vec<EncodedOp>` (transport
    /// encryption already stripped). We replay each payload through the
    /// shadow tree using the same logic as local writes so the divergence
    /// check also catches peer-applied ops.
    ///
    /// Returns an `AppliedChanges` with the per-op counts; this isn't
    /// authoritative (SqliteEngine's count is what callers act on) but
    /// keeps the trait shape uniform.
    async fn apply_changes(
        &self,
        _peer: DeviceId,
        envelope: SyncEnvelope,
    ) -> SyncResult<AppliedChanges> {
        let ops = crate::wire::decode_op_batch(&envelope.ciphertext)?;
        let mut applied = AppliedChanges::default();
        for op in ops {
            self.apply_payload(&op.payload).await?;
            applied.applied += 1;
        }
        Ok(applied)
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

impl LoroEngine {
    /// Per-payload mutation shared between `record_local` and
    /// `apply_changes`. Replays a single `OpPayload` against the
    /// per-note Loro doc/tree. Unknown block ids on Move/Delete are
    /// silent no-ops — SqliteEngine carries canonical state and the
    /// shadow catches up when the next BlockUpsert reseeds the block.
    async fn apply_payload(&self, payload: &OpPayload) -> SyncResult<()> {
        match payload {
            OpPayload::NoteUpsert { note_id, content, .. } => {
                let doc = self.doc_for_note_mut(*note_id).await;
                // Frontmatter + raw content snapshot lives on the doc's
                // root meta until block ops fully replace it. When the
                // server emits BlockUpsert ops alongside NoteUpsert,
                // this branch just keeps the convenience copy; the
                // tree under `"blocks"` is the source of truth.
                let root_meta = doc.get_map("root");
                root_meta
                    .insert("content", content.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro insert: {e}")))?;
                doc.commit();
            }
            OpPayload::BlockUpsert {
                block_id,
                note_id,
                parent_block_id,
                order_key,
                indent_level,
                text,
            } => {
                let doc = self.doc_for_note_mut(*note_id).await;
                let tree = doc.get_tree("blocks");
                let block_hex = hex_id(block_id);
                let parent_id = match parent_block_id {
                    Some(p) => find_node_by_block_id(&tree, &hex_id(p))
                        .map(TreeParentId::Node)
                        .unwrap_or(TreeParentId::Root),
                    None => TreeParentId::Root,
                };
                let node = match find_node_by_block_id(&tree, &block_hex) {
                    Some(existing) => {
                        if tree.parent(existing) != Some(parent_id) {
                            tree.mov(existing, parent_id).map_err(|e| {
                                SyncError::Storage(format!("loro tree mov: {e}"))
                            })?;
                        }
                        existing
                    }
                    None => tree
                        .create(parent_id)
                        .map_err(|e| SyncError::Storage(format!("loro tree create: {e}")))?,
                };
                let meta = tree
                    .get_meta(node)
                    .map_err(|e| SyncError::Storage(format!("loro get_meta: {e}")))?;
                meta.insert("block_id", block_hex.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                meta.insert("text", text.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                meta.insert("order_key", order_key.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                meta.insert("indent_level", *indent_level as i64)
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                doc.commit();
            }
            OpPayload::BlockMove {
                block_id,
                new_parent,
                new_order_key,
            } => {
                let Some((doc, node)) = self.find_doc_for_block(block_id).await else {
                    // We never saw the prior BlockUpsert (e.g. the
                    // engine started after the block was created).
                    // SqliteEngine handles it; LoroEngine catches up
                    // when the next BlockUpsert for this block lands.
                    tracing::debug!(
                        "tesela-sync/loro: BlockMove for unknown block {}",
                        hex_id(block_id)
                    );
                    return Ok(());
                };
                let tree = doc.get_tree("blocks");
                let parent_id = match new_parent {
                    Some(p) => find_node_by_block_id(&tree, &hex_id(p))
                        .map(TreeParentId::Node)
                        .unwrap_or(TreeParentId::Root),
                    None => TreeParentId::Root,
                };
                tree.mov(node, parent_id)
                    .map_err(|e| SyncError::Storage(format!("loro tree mov: {e}")))?;
                let meta = tree
                    .get_meta(node)
                    .map_err(|e| SyncError::Storage(format!("loro get_meta: {e}")))?;
                meta.insert("order_key", new_order_key.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                doc.commit();
            }
            OpPayload::BlockDelete { block_id } => {
                let Some((doc, node)) = self.find_doc_for_block(block_id).await else {
                    tracing::debug!(
                        "tesela-sync/loro: BlockDelete for unknown block {}",
                        hex_id(block_id)
                    );
                    return Ok(());
                };
                let tree = doc.get_tree("blocks");
                tree.delete(node)
                    .map_err(|e| SyncError::Storage(format!("loro tree delete: {e}")))?;
                doc.commit();
            }
            _ => {
                // Other op types: scaffold no-op. NoteDelete / Attachment*
                // land op-by-op as the migration progresses. SqliteEngine
                // in the dual-write pair handles them in the meantime, so
                // the system stays correct from the user's perspective.
                tracing::debug!("tesela-sync/loro: scaffold no-op for {:?}", payload.kind());
            }
        }

        Ok(())
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

    #[tokio::test]
    async fn block_upsert_builds_indented_tree() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [1u8; 16];
        let root_block = [10u8; 16];
        let child_block = [11u8; 16];

        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: root_block,
                note_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: "root block".into(),
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: child_block,
                note_id,
                parent_block_id: Some(root_block),
                order_key: "a0a".into(),
                indent_level: 1,
                text: "child block".into(),
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(rendered, "- root block\n\t- child block\n");
    }

    #[tokio::test]
    async fn block_move_reparents_in_tree() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [3u8; 16];
        let a = [30u8; 16];
        let b = [31u8; 16];
        let c = [32u8; 16];

        // Set up: a (root), b (root), c child of a → render = "a / b / \tc"
        for (id, parent, indent, text) in [
            (a, None, 0u16, "a"),
            (b, None, 0u16, "b"),
            (c, Some(a), 1u16, "c"),
        ] {
            engine
                .record_local(OpPayload::BlockUpsert {
                    block_id: id,
                    note_id,
                    parent_block_id: parent,
                    order_key: "a0".into(),
                    indent_level: indent,
                    text: text.into(),
                })
                .await
                .unwrap();
        }

        engine
            .record_local(OpPayload::BlockMove {
                block_id: c,
                new_parent: Some(b),
                new_order_key: "b0".into(),
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(rendered, "- a\n- b\n\t- c\n");
    }

    #[tokio::test]
    async fn block_delete_removes_from_render() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [4u8; 16];
        let a = [40u8; 16];
        let b = [41u8; 16];

        for (id, text) in [(a, "keep"), (b, "delete me")] {
            engine
                .record_local(OpPayload::BlockUpsert {
                    block_id: id,
                    note_id,
                    parent_block_id: None,
                    order_key: "a0".into(),
                    indent_level: 0,
                    text: text.into(),
                })
                .await
                .unwrap();
        }

        engine
            .record_local(OpPayload::BlockDelete { block_id: b })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(rendered, "- keep\n");
    }

    #[tokio::test]
    async fn block_move_or_delete_for_unknown_block_is_noop() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);

        let res = engine
            .record_local(OpPayload::BlockMove {
                block_id: [99u8; 16],
                new_parent: None,
                new_order_key: "z".into(),
            })
            .await;
        assert!(res.is_ok());

        let res = engine
            .record_local(OpPayload::BlockDelete {
                block_id: [99u8; 16],
            })
            .await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn apply_changes_populates_shadow_from_peer_envelope() {
        use crate::wire::envelope::SyncEnvelope;

        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc.clone());
        let note_id = [50u8; 16];
        let block = [51u8; 16];
        let peer_device = DeviceId::from_bytes([99u8; 16]);

        let payload = OpPayload::BlockUpsert {
            block_id: block,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "from peer".into(),
        };
        let op = EncodedOp::new(hlc.now(), crate::SYNC_SCHEMA_VERSION, payload, None).unwrap();
        let ciphertext = postcard::to_allocvec(&vec![op]).unwrap();

        let envelope = SyncEnvelope {
            from_device: peer_device,
            to_group: crate::group::GroupId([0u8; 16]),
            nonce: [0u8; 24],
            ciphertext,
        };

        let applied = engine.apply_changes(peer_device, envelope).await.unwrap();
        assert_eq!(applied.applied, 1);

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(rendered, "- from peer\n");
    }

    #[tokio::test]
    async fn block_upsert_with_same_block_id_updates_text() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [2u8; 16];
        let block = [20u8; 16];

        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: block,
                note_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: "first".into(),
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: block,
                note_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: "second".into(),
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(rendered, "- second\n");
    }
}
