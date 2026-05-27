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
    /// tree, building a `tesela_core::NoteTree`, and feeding it through
    /// the same `serialize_note` SqliteEngine uses on disk. This gives
    /// the divergence check a byte-identical comparison surface (modulo
    /// frontmatter, which lives on the on-disk file but not in the
    /// shadow).
    ///
    /// Returns `None` for unknown note ids. Order_key meta sorts
    /// children to match SqliteEngine's fractional-index file layout.
    pub async fn render_note(&self, note_id: [u8; 16]) -> Option<String> {
        let docs = self.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        let tree = doc.get_tree("blocks");
        let mut blocks: Vec<tesela_core::note_tree::FlatBlock> = Vec::new();
        let mut roots = tree.children(TreeParentId::Root).unwrap_or_default();
        sort_children_by_order_key(&tree, &mut roots);
        for root in roots {
            collect_blocks(&tree, root, None, &mut blocks);
        }
        let note_tree = tesela_core::note_tree::NoteTree {
            frontmatter: None,
            blocks,
            stamped_any: false,
        };
        Some(tesela_core::note_tree::serialize_note(&note_tree))
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

/// Walk a tree node + its descendants, appending a `FlatBlock` for each
/// to `out`. Children are walked sorted by their `order_key` meta so the
/// rendered order matches SqliteEngine's fractional-index file layout.
/// The collected blocks are handed to `tesela_core::serialize_note`,
/// which emits the canonical on-disk format (continuation indentation
/// for multi-line text, bid markers, etc.) so divergence checks compare
/// byte-identical strings.
fn collect_blocks(
    tree: &LoroTree,
    node: TreeID,
    parent_uuid: Option<uuid::Uuid>,
    out: &mut Vec<tesela_core::note_tree::FlatBlock>,
) {
    let meta = match tree.get_meta(node) {
        Ok(m) => m,
        Err(_) => return,
    };
    let indent = meta
        .get("indent_level")
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_i64().ok())
        .unwrap_or(0) as u16;
    let text = meta
        .get("text")
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| (*s).clone())
        .unwrap_or_default();
    let id_hex = meta
        .get("block_id")
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| (*s).clone())
        .unwrap_or_default();
    let id_uuid = parse_uuid_from_hex(&id_hex).unwrap_or_else(uuid::Uuid::nil);
    out.push(tesela_core::note_tree::FlatBlock {
        id: id_uuid,
        parent: parent_uuid,
        indent,
        text,
    });
    if let Some(mut children) = tree.children(node) {
        sort_children_by_order_key(tree, &mut children);
        for child in children {
            collect_blocks(tree, child, Some(id_uuid), out);
        }
    }
}

/// Seed a fresh tree from `tesela_core::FlatBlock`s parsed out of a
/// NoteUpsert's body content. Used when LoroEngine sees a note for the
/// first time and the only op is the NoteUpsert (no later BlockUpserts
/// to populate the tree).
///
/// Builds the parent UUID → TreeID map in order so child blocks find
/// their already-created parent. FlatBlocks come out of `parse_note`
/// in document order, so this is a single pass.
fn seed_tree_from_flatblocks(
    tree: &LoroTree,
    blocks: &[tesela_core::note_tree::FlatBlock],
) -> SyncResult<()> {
    use std::collections::HashMap;
    let mut uuid_to_node: HashMap<uuid::Uuid, TreeID> = HashMap::new();
    // Synthetic order_keys so render output is stable. Real BlockUpserts
    // will overwrite this when they arrive.
    for (idx, block) in blocks.iter().enumerate() {
        let parent_id = block
            .parent
            .and_then(|p| uuid_to_node.get(&p).copied())
            .map(TreeParentId::Node)
            .unwrap_or(TreeParentId::Root);
        let node = tree
            .create(parent_id)
            .map_err(|e| SyncError::Storage(format!("seed tree.create: {e}")))?;
        let meta = tree
            .get_meta(node)
            .map_err(|e| SyncError::Storage(format!("seed get_meta: {e}")))?;
        let block_hex = hex::encode(block.id.as_bytes());
        meta.insert("block_id", block_hex.as_str())
            .map_err(|e| SyncError::Storage(format!("seed meta insert: {e}")))?;
        meta.insert("text", block.text.as_str())
            .map_err(|e| SyncError::Storage(format!("seed meta insert: {e}")))?;
        // Pad idx so lexicographic compare matches numeric order up to
        // 999999 blocks; good enough for the shadow's render order.
        meta.insert("order_key", format!("{:06}", idx).as_str())
            .map_err(|e| SyncError::Storage(format!("seed meta insert: {e}")))?;
        meta.insert("indent_level", block.indent as i64)
            .map_err(|e| SyncError::Storage(format!("seed meta insert: {e}")))?;
        uuid_to_node.insert(block.id, node);
    }
    Ok(())
}

/// Parse a 32-char hex string back into a Uuid. Returns None on length
/// or hex-decode mismatch.
fn parse_uuid_from_hex(s: &str) -> Option<uuid::Uuid> {
    let bytes = hex::decode(s).ok()?;
    if bytes.len() != 16 {
        return None;
    }
    let mut arr = [0u8; 16];
    arr.copy_from_slice(&bytes);
    Some(uuid::Uuid::from_bytes(arr))
}

/// Sort `nodes` in place by the `order_key` meta on each. Nodes with no
/// `order_key` sort last with a stable identity tiebreaker.
fn sort_children_by_order_key(tree: &LoroTree, nodes: &mut [TreeID]) {
    nodes.sort_by(|a, b| {
        let ka = read_order_key(tree, *a).unwrap_or_else(|| "~~~".to_string());
        let kb = read_order_key(tree, *b).unwrap_or_else(|| "~~~".to_string());
        ka.cmp(&kb)
    });
}

fn read_order_key(tree: &LoroTree, node: TreeID) -> Option<String> {
    let meta = tree.get_meta(node).ok()?;
    let v = meta.get("order_key")?;
    let val = v.into_value().ok()?;
    let s = val.into_string().ok()?;
    Some((*s).clone())
}

/// Walk a tree to find the LIVE node whose `block_id` meta matches `target`.
/// `tree.nodes()` returns tombstoned nodes too, so we have to filter via
/// `is_node_deleted`. If a previously-created node was deleted (BlockDelete
/// in the history), subsequent BlockUpserts for the same block_id will get
/// `None` here and fall through to the create branch, building a fresh
/// node — without this filter, `tree.mov` on the tombstone errors with
/// "TreeID is deleted or does not exist" and the BlockUpsert is dropped.
fn find_node_by_block_id(tree: &LoroTree, target_hex: &str) -> Option<TreeID> {
    for node in tree.nodes() {
        if matches!(tree.is_node_deleted(&node), Ok(true)) {
            continue;
        }
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
    /// Per-payload mutation shared between `record_local`,
    /// `apply_changes`, and `DualEngine`'s startup oplog replay.
    /// Replays a single `OpPayload` against the per-note Loro doc/tree.
    /// Unknown block ids on Move/Delete are silent no-ops — SqliteEngine
    /// carries canonical state and the shadow catches up when the next
    /// BlockUpsert reseeds the block.
    pub async fn apply_payload(&self, payload: &OpPayload) -> SyncResult<()> {
        match payload {
            OpPayload::NoteUpsert { note_id, content, .. } => {
                let doc = self.doc_for_note_mut(*note_id).await;
                // Save the convenience content snapshot on root meta
                // (used by debugging; render_note ignores it).
                let root_meta = doc.get_map("root");
                root_meta
                    .insert("content", content.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro insert: {e}")))?;

                // If the tree is empty, this is the first time we've
                // seen the note. Parse the content into FlatBlocks and
                // seed the tree so render_note matches what's on disk
                // even when no BlockUpserts follow (legacy notes
                // created by the pre-engine FsNoteStore.write_note
                // path; auto-created dailies that only get NoteUpsert).
                //
                // Subsequent NoteUpserts for the same note skip the
                // parse — BlockUpsert/Move/Delete ops keep the tree in
                // sync from there. Without the skip, repeated
                // NoteUpserts would create duplicate nodes.
                let tree = doc.get_tree("blocks");
                let already_has_blocks = tree
                    .children(TreeParentId::Root)
                    .map(|c| !c.is_empty())
                    .unwrap_or(false);
                if !already_has_blocks {
                    let parsed = tesela_core::note_tree::parse_note(content);
                    seed_tree_from_flatblocks(&tree, &parsed.blocks)?;
                }
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
            OpPayload::NoteDelete { note_id, .. } => {
                // Drop the per-note doc entirely. SqliteEngine removes
                // the on-disk file in its materialize step; the shadow
                // needs to forget the doc so render_note returns None
                // and the divergence check matches PrimaryMissing.
                let mut docs = self.inner.docs.write().await;
                docs.remove(note_id);
            }
            OpPayload::AttachmentUpsert { .. } | OpPayload::AttachmentDelete { .. } => {
                // Attachments don't affect the rendered markdown body
                // (bytes flow out-of-band via the blob store; ops carry
                // metadata only). Divergence check compares rendered
                // markdown, so no shadow state change is needed. Kept
                // as an explicit arm rather than a wildcard so future
                // op types are caught by the compiler.
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
        assert_eq!(
            rendered,
            "- root block <!-- bid:0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a -->\n  \
             - child block <!-- bid:0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b -->\n"
        );
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
        assert_eq!(
            rendered,
            "- a <!-- bid:1e1e1e1e-1e1e-1e1e-1e1e-1e1e1e1e1e1e -->\n\
             - b <!-- bid:1f1f1f1f-1f1f-1f1f-1f1f-1f1f1f1f1f1f -->\n  \
             - c <!-- bid:20202020-2020-2020-2020-202020202020 -->\n"
        );
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
        assert_eq!(
            rendered,
            "- keep <!-- bid:28282828-2828-2828-2828-282828282828 -->\n"
        );
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
    async fn render_orders_by_order_key_not_insertion() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [70u8; 16];

        for (id, order, text) in [
            ([70u8; 16], "a5", "second by order"),
            ([71u8; 16], "a0", "first by order"),
            ([72u8; 16], "ar", "third by order"),
        ] {
            engine
                .record_local(OpPayload::BlockUpsert {
                    block_id: id,
                    note_id,
                    parent_block_id: None,
                    order_key: order.into(),
                    indent_level: 0,
                    text: text.into(),
                })
                .await
                .unwrap();
        }

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(
            rendered,
            "- first by order <!-- bid:47474747-4747-4747-4747-474747474747 -->\n\
             - second by order <!-- bid:46464646-4646-4646-4646-464646464646 -->\n\
             - third by order <!-- bid:48484848-4848-4848-4848-484848484848 -->\n"
        );
    }

    #[tokio::test]
    async fn block_upsert_after_delete_recreates_node() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [80u8; 16];
        let block = [81u8; 16];

        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: block,
                note_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: "before".into(),
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockDelete { block_id: block })
            .await
            .unwrap();
        // After delete, a re-upsert (e.g. peer revives the same block_id)
        // must create a fresh node — without the tombstone filter this
        // would error with "TreeID is deleted or does not exist".
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: block,
                note_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: "after".into(),
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(
            rendered,
            "- after <!-- bid:51515151-5151-5151-5151-515151515151 -->\n"
        );
    }

    #[tokio::test]
    async fn note_delete_drops_doc_from_shadow() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [60u8; 16];

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("doomed".into()),
                title: "Doomed".into(),
                content: "---\ntitle: Doomed\n---\n- bye\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        assert_eq!(engine.note_count().await, 1);

        engine
            .record_local(OpPayload::NoteDelete {
                note_id,
                display_alias: Some("doomed".into()),
            })
            .await
            .unwrap();
        assert_eq!(engine.note_count().await, 0);
        assert!(engine.render_note(note_id).await.is_none());
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
        assert_eq!(
            rendered,
            "- from peer <!-- bid:33333333-3333-3333-3333-333333333333 -->\n"
        );
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
        assert_eq!(
            rendered,
            "- second <!-- bid:14141414-1414-1414-1414-141414141414 -->\n"
        );
    }
}
