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
    cursor::PeerCursor, LocalCursor, ParkedSummary, ReplayReport, SyncEngine,
};
use crate::error::{SyncError, SyncResult};
use crate::hlc::Hlc;
use crate::oplog::op::{ContentHash, EncodedOp, OpPayload, PropOp};
use crate::oplog::parked::ParkReason;
use async_trait::async_trait;
use loro::{
    ExportMode, LoroDoc, LoroText, LoroTree, TreeID, TreeParentId, UpdateOptions, VersionVector,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Process-wide counter for unique snapshot temp-file names, so two
/// concurrent writers never collide on the same `.tmp` path and publish
/// a torn snapshot via rename (review finding [8]).
static SNAPSHOT_TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Build a unique temp path next to `path` for atomic write+rename.
fn unique_tmp(path: &Path) -> PathBuf {
    let n = SNAPSHOT_TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    path.with_extension(format!("tmp.{n}"))
}

fn hex_id(id: &[u8; 16]) -> String {
    hex::encode(id)
}

/// Schema version of the index doc's entry shape. Bump whenever the
/// per-entry fields OR their encoding change so a stale on-disk index is
/// rebuilt from the (self-describing) per-note docs on boot — no manual
/// cache clear. v1 = {title, slug}. v2 = + {tags, links} (comma-joined).
/// v3 = tags/links newline-joined (comma collided with link targets like
/// `[[Smith, John]]` — review finding [7]).
const INDEX_SCHEMA_VERSION: i64 = 3;

/// Delimiter for the multi-valued tags/links fields stored as a single
/// string in an index entry. Newline can't appear in a tag name
/// (`[A-Za-z0-9_/-]`) or a single-line `[[wiki-link]]` target, so it's
/// collision-free — unlike the comma it replaced.
const INDEX_LIST_SEP: char = '\n';

/// Join a multi-valued index field with the collision-free separator.
fn join_list(items: &[String]) -> String {
    items.join(&INDEX_LIST_SEP.to_string())
}

/// Build the block_id → note_id map from a set of loaded per-note docs
/// by reading each block node's `block_id` meta. Used at boot.
fn build_block_index(
    docs: &HashMap<[u8; 16], LoroDoc>,
) -> HashMap<[u8; 16], [u8; 16]> {
    let mut out = HashMap::new();
    for (note_id, doc) in docs.iter() {
        let tree = doc.get_tree("blocks");
        for node in tree.children(TreeParentId::Root).unwrap_or_default() {
            if matches!(tree.is_node_deleted(&node), Ok(true)) {
                continue;
            }
            if let Some(hex) = read_meta_str(&tree, node, "block_id") {
                if let Some(bid) = parse_note_id_from_hex(&hex) {
                    out.insert(bid, *note_id);
                }
            }
        }
    }
    out
}

/// Best-effort frontmatter `title:` extraction for index rebuild
/// fallback. Returns None if there's no frontmatter title.
fn frontmatter_title(content: &str) -> Option<String> {
    tesela_core::storage::markdown::parse_frontmatter(content)
        .ok()
        .and_then(|(meta, _)| meta.title)
        .filter(|t| !t.is_empty())
}

/// Derive a note's index metadata `(tags, links)` from its content +
/// parsed page properties. Tags come from three sources (frontmatter
/// `tags:`, the `tags::` page property, inline `#tags`); links are
/// `[[wiki-link]]` targets. Both deduped + sorted.
fn extract_index_metadata(
    content: &str,
    page_properties: &[(String, String)],
) -> (Vec<String>, Vec<String>) {
    use std::collections::BTreeSet;
    let mut tags: BTreeSet<String> = BTreeSet::new();

    // Frontmatter `tags:` (gray_matter via tesela_core).
    if let Ok((meta, _body)) = tesela_core::storage::markdown::parse_frontmatter(content) {
        for t in meta.tags {
            if !t.is_empty() {
                tags.insert(t);
            }
        }
    }
    // `tags::` page property (comma- or space-separated).
    for (k, v) in page_properties {
        if k == "tags" {
            for t in v.split(|c| c == ',' || c == ' ') {
                let t = t.trim().trim_start_matches('#');
                if !t.is_empty() {
                    tags.insert(t.to_string());
                }
            }
        }
    }
    // Inline `#tags`.
    for t in tesela_core::block::extract_tags(content) {
        if !t.is_empty() {
            tags.insert(t);
        }
    }

    let links: BTreeSet<String> = tesela_core::link::extract_wiki_links(content)
        .into_iter()
        .map(|l| l.target)
        .filter(|t| !t.is_empty())
        .collect();

    (tags.into_iter().collect(), links.into_iter().collect())
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
    /// Optional directory for per-note snapshots
    /// (`<dir>/<note-id-hex>.bin`). When `Some`, every successful
    /// `apply_payload` writes a fresh snapshot so the shadow survives
    /// process restart without re-replaying the oplog. When `None`,
    /// the shadow is in-memory only.
    snapshot_dir: Option<PathBuf>,
    /// Always-resident index doc (the hybrid model's spine; cutover spec
    /// Phase 2). A single small Loro doc holding a `"notes"` LoroMap of
    /// `hex(note_id) → {title, slug}` (tags + link graph land in step 2).
    /// Lets callers list notes / resolve refs without loading every
    /// per-note doc into memory. Persisted to `<dir>/_index.bin`.
    index: LoroDoc,
    /// Resident block_id → note_id map. Lets block-only ops
    /// (BlockMove/BlockDelete) resolve the owning note in O(1) instead
    /// of scanning every doc's tree, and is the prerequisite for
    /// lazy-load/evict (an evicted doc can't be scanned, but this map
    /// still points at the note so it can be loaded on demand). Derived
    /// state, rebuilt from the per-note docs at boot.
    block_index: RwLock<HashMap<[u8; 16], [u8; 16]>>,
    /// Per-note "last broadcast version vector" (encoded) for the relay
    /// broadcast model (Phase 5): each tick exports the updates a note
    /// has accrued since this marker and advances it. Idempotent imports
    /// on the receiving side mean transitive re-broadcast is harmless
    /// (bounded — each op is re-sent at most once per device that
    /// imports it). In-memory for now; the live relay wiring will
    /// persist it alongside relay_state.
    ///
    /// Persisted to `<snapshot_dir>/_broadcast.bin` so a restart doesn't
    /// re-broadcast every note's full state. Loaded in `with_dirs`, saved
    /// after each `produce_relay_updates`.
    broadcast_cursor: RwLock<HashMap<[u8; 16], Vec<u8>>>,
    /// When set (authoritative-writer mode), the `notes/` directory this
    /// engine materializes `<slug>.md` files into on every applied change
    /// — making LoroEngine the SOLE writer of the mosaic. `None` for the
    /// in-memory shadow / non-authoritative paths, which never touch disk
    /// beyond their `.bin` snapshots.
    materialize_dir: Option<PathBuf>,
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
                snapshot_dir: None,
                index: LoroDoc::new(),
                block_index: RwLock::new(HashMap::new()),
                broadcast_cursor: RwLock::new(HashMap::new()),
                materialize_dir: None,
            }),
        }
    }

    /// Construct a Loro engine that persists per-note snapshots under
    /// `snapshot_dir`. On construction, any existing snapshot files
    /// (`<dir>/<note-id-hex>.bin`) are loaded into memory so the shadow
    /// starts populated. Subsequent `apply_payload` calls write a fresh
    /// snapshot for the touched note synchronously.
    ///
    /// Falling back to oplog replay (`DualEngine::prepopulate_shadow_from_oplog`)
    /// is still valuable for notes whose snapshot is missing or corrupt
    /// — combine the two for first-boot coverage.
    pub async fn with_snapshot_dir(
        device: DeviceId,
        hlc: Arc<Hlc>,
        snapshot_dir: PathBuf,
    ) -> SyncResult<Self> {
        Self::with_dirs(device, hlc, snapshot_dir, None).await
    }

    /// Construct a Loro engine that persists snapshots under
    /// `snapshot_dir` and, when `materialize_dir` is `Some`, writes
    /// canonical `<slug>.md` files into it on every applied change
    /// (authoritative-writer mode — LoroEngine becomes the sole writer of
    /// the mosaic). `materialize_dir` is the `notes/` directory, matching
    /// the `<mosaic>/notes/<slug>.md` convention `FsNoteStore` reads from.
    pub async fn with_dirs(
        device: DeviceId,
        hlc: Arc<Hlc>,
        snapshot_dir: PathBuf,
        materialize_dir: Option<PathBuf>,
    ) -> SyncResult<Self> {
        tokio::fs::create_dir_all(&snapshot_dir)
            .await
            .map_err(|e| {
                SyncError::Storage(format!(
                    "create loro snapshot dir {}: {e}",
                    snapshot_dir.display()
                ))
            })?;
        if let Some(dir) = materialize_dir.as_ref() {
            tokio::fs::create_dir_all(dir).await.map_err(|e| {
                SyncError::Storage(format!(
                    "create loro materialize dir {}: {e}",
                    dir.display()
                ))
            })?;
        }
        let docs = load_snapshots_from_dir(&snapshot_dir).await?;
        // Load the index doc snapshot if present (best-effort: a missing
        // or corrupt index is rebuilt as NoteUpserts re-flow / re-seed).
        let index = LoroDoc::new();
        let index_path = snapshot_dir.join("_index.bin");
        match tokio::fs::read(&index_path).await {
            Ok(bytes) => {
                if let Err(e) = index.import(&bytes) {
                    tracing::warn!("tesela-sync/loro: import index snapshot: {e}");
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!("tesela-sync/loro: read index snapshot: {e}"),
        }
        // Self-heal the index if its schema is stale (or absent): rebuild
        // every entry from the self-describing per-note docs. Cheap —
        // in-memory over already-loaded docs — and removes the need to
        // ever hand-clear the cache when the index shape evolves.
        let stored_version = index
            .get_map("meta")
            .get("schema_version")
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_i64().ok())
            .unwrap_or(0);
        let needs_rebuild = stored_version != INDEX_SCHEMA_VERSION && !docs.is_empty();
        // Build the block_index from the loaded docs (block_id → note_id).
        let block_index = build_block_index(&docs);
        // Restore per-note broadcast cursors so a restart doesn't re-emit
        // every note's full state on the next relay tick (best-effort).
        let broadcast_cursor = load_broadcast_cursors(&snapshot_dir).await;
        let engine = Self {
            inner: Arc::new(Inner {
                docs: RwLock::new(docs),
                device,
                hlc,
                snapshot_dir: Some(snapshot_dir.clone()),
                index,
                block_index: RwLock::new(block_index),
                broadcast_cursor: RwLock::new(broadcast_cursor),
                materialize_dir,
            }),
        };
        if needs_rebuild {
            engine.rebuild_index_from_docs().await;
            engine.save_index_snapshot(&snapshot_dir).await;
            tracing::info!(
                "tesela-sync/loro: rebuilt index (schema {} → {})",
                stored_version,
                INDEX_SCHEMA_VERSION
            );
        }
        // Stamp this engine's PeerID on every loaded doc + the index so
        // future local ops are attributed to this device (Phase 4
        // convergence prerequisite).
        {
            let docs = engine.inner.docs.read().await;
            for doc in docs.values() {
                engine.set_doc_peer(doc);
            }
        }
        engine.set_doc_peer(&engine.inner.index);
        Ok(engine)
    }

    /// Encoded version vector of a note's doc — the relay cursor a peer
    /// sends so we export only updates newer than what it has. None if
    /// the doc isn't resident. (Phase 4.)
    pub async fn doc_version(&self, note_id: [u8; 16]) -> Option<Vec<u8>> {
        let docs = self.inner.docs.read().await;
        Some(docs.get(&note_id)?.oplog_vv().encode())
    }

    /// Export a note's Loro updates since the peer's (encoded) version
    /// vector. `since = None` exports full state — a fresh-device
    /// bootstrap. None if the doc isn't resident or export fails.
    /// (Phase 4.)
    pub async fn export_doc_update(
        &self,
        note_id: [u8; 16],
        since: Option<&[u8]>,
    ) -> Option<Vec<u8>> {
        let docs = self.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        // First broadcast (no cursor yet): ship a COMPACT snapshot, not the
        // full op history from an empty version vector. `updates(empty)` replays
        // every op ever applied — including content later deleted — so a note
        // whose history churned megabytes exports even more than its snapshot.
        // `ExportMode::Snapshot` is GC-compacted (the exact bytes `save_snapshot`
        // writes), so it is the smallest faithful representation, and the
        // receiver's `import` merges a snapshot identically to an update. This
        // still can't shrink a genuinely large note below the relay body cap
        // (e.g. ai-business ≈ 5 MB snapshot) — that relies on a large enough
        // `RELAY_MAX_BODY_BYTES` — but it never inflates it past the snapshot.
        // Once a cursor exists, send only the incremental delta since it.
        match since {
            Some(bytes) => {
                let vv = VersionVector::decode(bytes).ok()?;
                doc.export(ExportMode::updates(&vv)).ok()
            }
            None => doc.export(ExportMode::Snapshot).ok(),
        }
    }

    /// Apply a peer's Loro update bytes into the addressed note's doc —
    /// **the protected WS-apply path** (2026-06-02; LoroText era).
    ///
    /// Block text is now a nested [`LoroText`] sequence CRDT, not an LWW map
    /// register. On a SHARED Loro lineage (one node per block_id) a raw
    /// `doc.import` of the peer's frame MERGES the peer's text splices with the
    /// server's — it can neither revert the server nor drop the peer's edit. So
    /// shared-lineage blocks need no protection and defer entirely to Loro's
    /// merge.
    ///
    /// The ONE residual data-loss vector is the DISJOINT-TWIN case (the
    /// `project_multidevice_convergence` residue): when a block_id has >1 live
    /// node post-import, the twins hold two INDEPENDENT LoroTexts Loro can't
    /// merge, and the non-causal min-`TreeID` dedup picks one twin by peer (not
    /// recency) — so the import can keep the peer's STALE twin (reverting the
    /// server) OR keep the server's twin (dropping the peer's genuine edit).
    ///
    /// The fix has two halves when the server already holds this note:
    ///
    /// 1. **Plan (before mutating):** ask the discriminator
    ///    ([`peer_genuine_block_changes`]) for the RESOLVED target text each
    ///    disjoint-twin block must hold — the peer's value when it's a genuine
    ///    edit (differs from the server's current value AND was never in the
    ///    server's text history), else the server's current value (a stale
    ///    re-assertion). Empty plan ⇒ no twins ⇒ nothing to heal. Also snapshot
    ///    the server's current per-block text.
    /// 2. **Raw-import + heal:** raw-import the frame so genuinely-NEW blocks
    ///    arrive with Loro's native, convergent ordering (the multi-engine
    ///    relay path depends on this — re-authoring new blocks under the
    ///    server's peer would diverge across devices) and shared-lineage text
    ///    merges, then for each disjoint twin whose surviving node differs from
    ///    its resolved target, force the target via `record_local` (server
    ///    peer/lamport). Shared-lineage blocks are never touched.
    ///
    /// Bootstrap (the server has no doc for this note yet) is a plain raw
    /// import: there's nothing to clobber, and the peer's full state is exactly
    /// what the server should adopt.
    ///
    /// Idempotent + commutative: a re-applied frame finds every twin's value
    /// already at its target → no heal. A decode/fork failure logs + falls back
    /// to a plain raw import (never panics).
    pub async fn import_doc_update(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
    ) -> SyncResult<()> {
        let already_resident = self.inner.docs.read().await.contains_key(&note_id);
        let doc = self.doc_for_note_mut(note_id).await;

        // Compute the protection plan BEFORE mutating the auth doc: the
        // RESOLVED target text for every disjoint-twin block (the value its
        // surviving node must hold after the raw merge + dedup). Skipped on
        // bootstrap (nothing to protect) and on a decode/fork failure
        // (graceful: raw-import, never panic).
        let plan = if already_resident {
            match peer_genuine_block_changes(&doc, bytes) {
                Ok(changes) => Some(changes),
                Err(e) => {
                    tracing::warn!(
                        "tesela-sync/loro: WS-apply discriminator failed for {} ({e}); \
                         raw-importing without per-block protection",
                        hex_id(&note_id)
                    );
                    None
                }
            }
        } else {
            None
        };

        // Raw-import the frame. This brings in genuinely-new blocks with their
        // native, convergent ordering (the multi-engine relay path relies on
        // Loro's own concurrent-insert merge — re-emitting new blocks under
        // the server's peer would diverge across devices). The clobber it can
        // cause on EXISTING blocks is healed below.
        doc.import(bytes)
            .map_err(|e| SyncError::Storage(format!("loro import: {e}")))?;
        // A peer's snapshot can union same-bid twins minted on a disjoint
        // history (see `dedup_twins_by_block_id`). Tombstone the strays now —
        // before deriving the index/markdown from the tree — so the persisted
        // doc carries exactly one node per bid and later block-diff saves
        // can't update a ghost. Idempotent: a re-import finds nothing to drop.
        tombstone_duplicate_twins(&doc, note_id);

        // ── Disjoint-twin heal (the protection) ──────────────────────────
        // Block text is now a nested LoroText, so SHARED-lineage blocks need no
        // heal: the peer's frame carries only its own LoroText splices, which
        // MERGE with the server's (neither reverts the server nor drops the
        // peer's edit). Raw-import is final there — never override Loro's merge.
        //
        // What survives is the DISJOINT-twin case: the two twins hold two
        // INDEPENDENT LoroTexts Loro can't merge, and the non-causal min-
        // `TreeID` dedup picks one by peer (not recency) — so it can revert the
        // server (keep the peer's stale twin) OR drop the peer's genuine edit
        // (keep the server's twin). The discriminator already resolved the
        // correct target per twin; force it onto the survivor via record_local
        // (which routes through the LoroText write) whenever the post-import
        // value differs from it. Idempotent: a re-applied frame finds the
        // survivor already at the target → no heal.
        if let Some(changes) = plan {
            let post_texts = current_block_texts(&doc);
            let mut heals: Vec<(String, String, u16)> = Vec::new();
            for c in &changes {
                let bid_hex = hex_id(&c.block_id);
                if post_texts.get(&bid_hex).map(|s| s.as_str()) != Some(c.text.as_str()) {
                    heals.push((bid_hex, c.text.clone(), c.indent));
                }
            }
            // Drop the auth-doc handle's borrow expectations: record_local
            // re-acquires the docs lock + re-fetches the doc. Apply heals.
            for (bid_hex, text, indent) in heals {
                let Some(block_id) = parse_note_id_from_hex(&bid_hex) else {
                    continue;
                };
                self.record_local(OpPayload::BlockUpsert {
                    block_id,
                    note_id,
                    parent_block_id: None,
                    order_key: "00000000".to_string(),
                    indent_level: indent,
                    text,
                    after_block_id: None,
                })
                .await?;
            }
        }

        self.refresh_note_derived(note_id, &doc).await;
        if let Some(dir) = self.inner.snapshot_dir.as_ref() {
            self.save_snapshot(dir, note_id).await;
        }
        // Authoritative-writer mode: a peer's edit must land on disk too.
        if self.inner.materialize_dir.is_some() {
            self.materialize_note(note_id).await;
        }
        Ok(())
    }

    /// Apply the server's FULL snapshot as an AUTHORITATIVE re-base — the
    /// disjoint-device catch-up that `import_doc_update` can't do.
    ///
    /// A device that authored a note WITHOUT first importing the server's
    /// snapshot is on a DISJOINT Loro lineage: it minted its own `TreeID` for
    /// each block_id. Raw-importing the server snapshot UNIONS the lineages into
    /// same-bid twins, but [`tombstone_duplicate_twins`]'s min-`TreeID` dedup
    /// resolves the survivor by peer id, NOT by authority — so the device often
    /// KEEPS its own twin and stays on its disjoint lineage, never re-basing
    /// (its edits then keep clobbering the server's).
    ///
    /// This path re-bases SERVER-WINS instead: it identifies the device's
    /// pre-existing nodes BEFORE importing, then for each twin KEEPS the node
    /// that arrived FROM the snapshot (the server's lineage) and TOMBSTONES the
    /// device's stale node. After this the device's surviving node for each
    /// shared block_id is the server's — a shared lineage — so later concurrent
    /// edits MERGE through the block's `LoroText` instead of forking new twins.
    ///
    /// Block_ids the device has but the snapshot does NOT are KEPT untouched —
    /// those are the device's genuine unsynced new blocks.
    ///
    /// Otherwise mirrors [`import_doc_update`]'s tail: refresh derived
    /// projections, persist the snapshot, materialize the note to disk.
    pub async fn import_authoritative_snapshot(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
    ) -> SyncResult<()> {
        let doc = self.doc_for_note_mut(note_id).await;

        // Snapshot the device's pre-existing live nodes BEFORE importing, so we
        // can later tell a snapshot-origin node (server lineage) from a device
        // node (these). A `TreeID` not in this set after the import arrived from
        // the snapshot.
        let pre_nodes: std::collections::HashSet<TreeID> = {
            let tree = doc.get_tree("blocks");
            tree.children(TreeParentId::Root)
                .unwrap_or_default()
                .into_iter()
                .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
                .collect()
        };

        // Raw-import the server snapshot. This unions the disjoint lineages into
        // same-bid twins; the re-base below resolves each toward the server's.
        doc.import(bytes)
            .map_err(|e| SyncError::Storage(format!("loro import: {e}")))?;

        rebase_twins_onto_snapshot(&doc, &pre_nodes);

        self.refresh_note_derived(note_id, &doc).await;
        if let Some(dir) = self.inner.snapshot_dir.as_ref() {
            self.save_snapshot(dir, note_id).await;
        }
        if self.inner.materialize_dir.is_some() {
            self.materialize_note(note_id).await;
        }
        Ok(())
    }

    /// Like [`import_doc_update`](Self::import_doc_update) but RETURNS whether
    /// the imported update was left PENDING by Loro — i.e. it referenced ops the
    /// doc is missing (a causal gap / disjoint-lineage signal). `import_doc_update`
    /// discards loro's `ImportStatus`; this surfaces it so a caller (the iOS
    /// delta path) can trigger an authoritative-snapshot catch-up when a live
    /// delta can't fully integrate.
    ///
    /// Returns `Ok(true)` when `ImportStatus.pending` is non-empty. The apply
    /// itself runs the SAME protected path as `import_doc_update` (twin
    /// tombstone + disjoint-twin heal + derived refresh + persist), so behavior
    /// is identical; only the pending bool is additionally reported.
    pub async fn apply_doc_update_status(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
    ) -> SyncResult<bool> {
        let already_resident = self.inner.docs.read().await.contains_key(&note_id);
        let doc = self.doc_for_note_mut(note_id).await;

        let plan = if already_resident {
            match peer_genuine_block_changes(&doc, bytes) {
                Ok(changes) => Some(changes),
                Err(e) => {
                    tracing::warn!(
                        "tesela-sync/loro: WS-apply discriminator failed for {} ({e}); \
                         raw-importing without per-block protection",
                        hex_id(&note_id)
                    );
                    None
                }
            }
        } else {
            None
        };

        let status = doc
            .import(bytes)
            .map_err(|e| SyncError::Storage(format!("loro import: {e}")))?;
        let pending = status
            .pending
            .as_ref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);

        tombstone_duplicate_twins(&doc, note_id);

        if let Some(changes) = plan {
            let post_texts = current_block_texts(&doc);
            let mut heals: Vec<(String, String, u16)> = Vec::new();
            for c in &changes {
                let bid_hex = hex_id(&c.block_id);
                if post_texts.get(&bid_hex).map(|s| s.as_str()) != Some(c.text.as_str()) {
                    heals.push((bid_hex, c.text.clone(), c.indent));
                }
            }
            for (bid_hex, text, indent) in heals {
                let Some(block_id) = parse_note_id_from_hex(&bid_hex) else {
                    continue;
                };
                self.record_local(OpPayload::BlockUpsert {
                    block_id,
                    note_id,
                    parent_block_id: None,
                    order_key: "00000000".to_string(),
                    indent_level: indent,
                    text,
                    after_block_id: None,
                })
                .await?;
            }
        }

        self.refresh_note_derived(note_id, &doc).await;
        if let Some(dir) = self.inner.snapshot_dir.as_ref() {
            self.save_snapshot(dir, note_id).await;
        }
        if self.inner.materialize_dir.is_some() {
            self.materialize_note(note_id).await;
        }
        Ok(pending)
    }

    /// Apply a single CHARACTER-LEVEL splice to one block's text — the
    /// outbound foundation for cursor-accurate collaborative editing
    /// (collab editing C1). Instead of re-authoring the WHOLE block text
    /// (which a Myers-diff turns into DELETEs of a concurrent peer's
    /// characters → clobber), a client sends the user's actual keystroke:
    /// "delete `utf16_delete_len` UTF-16 code units at `utf16_offset`, then
    /// insert `insert`". The two operations at the same offset form a
    /// replace.
    ///
    /// Offsets are **UTF-16 code units**, matching iOS `NSRange` and
    /// JavaScript string indices, so a client passes the editor's native
    /// offset with no conversion. The splice goes through the block's nested
    /// `text_seq` [`LoroText`] sequence CRDT (`insert_utf16` /
    /// `delete_utf16`), so two replicas splicing the SAME block concurrently
    /// INTERLEAVE — neither side's characters are lost.
    ///
    /// The block node must already exist (a splice is an in-place edit, not
    /// a create): if no live node carries `block_id`, this is a no-op and
    /// returns `Ok(0)`. On a successful splice returns `Ok(1)`.
    ///
    /// Mirrors the [`BlockUpsert`](OpPayload::BlockUpsert) write tail so the
    /// change reaches disk + derived projections identically: `commit`,
    /// `refresh_note_derived`, persist the snapshot, materialize the note.
    pub async fn splice_block_text(
        &self,
        note_id: [u8; 16],
        block_id: [u8; 16],
        utf16_offset: u32,
        utf16_delete_len: u32,
        insert: &str,
    ) -> SyncResult<u32> {
        let doc = self.doc_for_note_mut(note_id).await;
        let tree = doc.get_tree("blocks");
        let block_hex = hex_id(&block_id);
        let Some(node) = find_node_by_block_id(&tree, &block_hex) else {
            // The block must already exist — a splice is an in-place edit.
            return Ok(0);
        };
        let meta = tree
            .get_meta(node)
            .map_err(|e| SyncError::Storage(format!("loro get_meta: {e}")))?;
        // Get-or-create the SAME `text_seq` container `write_block_text`
        // uses, so seed / whole-text upsert / splice all converge on ONE
        // sequence CRDT (a distinct container at the same key would
        // overwrite rather than merge).
        let text: LoroText = meta
            .get_or_create_container("text_seq", LoroText::new())
            .map_err(|e| SyncError::Storage(format!("loro text_seq get_or_create: {e}")))?;
        // Delete THEN insert at the same offset = a replace.
        if utf16_delete_len > 0 {
            text.delete_utf16(utf16_offset as usize, utf16_delete_len as usize)
                .map_err(|e| SyncError::Storage(format!("loro text_seq delete_utf16: {e}")))?;
        }
        if !insert.is_empty() {
            text.insert_utf16(utf16_offset as usize, insert)
                .map_err(|e| SyncError::Storage(format!("loro text_seq insert_utf16: {e}")))?;
        }
        doc.commit();
        self.register_note_blocks(note_id, &[block_id]).await;
        self.refresh_note_derived(note_id, &doc).await;
        if let Some(dir) = self.inner.snapshot_dir.as_ref() {
            self.save_snapshot(dir, note_id).await;
        }
        if self.inner.materialize_dir.is_some() {
            self.materialize_note(note_id).await;
        }
        Ok(1)
    }

    /// Read a single block's current text (the engine-exact `text_seq`
    /// content, falling back to a legacy `text` register) by note + block id.
    /// Read-only — the inbound live-apply path calls this AFTER a remote
    /// splice is applied to reconcile the open editor with the merged text.
    /// `None` for an unknown note/block or an empty block.
    pub async fn read_block_text(
        &self,
        note_id: [u8; 16],
        block_id: [u8; 16],
    ) -> Option<String> {
        let docs = self.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        let tree = doc.get_tree("blocks");
        let block_hex = hex_id(&block_id);
        let node = find_node_by_block_id(&tree, &block_hex)?;
        read_block_text(&tree, node)
    }

    /// Compute the per-note Loro updates to broadcast on a relay tick:
    /// for every note that has accrued ops since its last broadcast,
    /// export the delta. Returns `(note_id, update_bytes, captured_vv)`
    /// where `captured_vv` is the doc's version at capture time.
    ///
    /// **This does NOT advance the broadcast cursor.** The caller advances
    /// it via [`commit_broadcast_cursors`](Self::commit_broadcast_cursors)
    /// ONLY after the relay PUT is confirmed, so a failed send is retried
    /// on the next tick instead of being silently dropped (the delta would
    /// otherwise be lost forever — review finding, 2026-05-29). The method
    /// is therefore idempotent: called twice with no commit between, it
    /// returns the same set. This is the relay BROADCAST model (Phase 5):
    /// we emit our deltas and let every receiver import idempotently.
    pub async fn produce_relay_updates(&self) -> Vec<([u8; 16], Vec<u8>, Vec<u8>)> {
        let note_ids: Vec<[u8; 16]> =
            self.inner.docs.read().await.keys().copied().collect();
        let mut out = Vec::new();
        for note_id in note_ids {
            let current = match self.doc_version(note_id).await {
                Some(v) => v,
                None => continue,
            };
            let since = self.inner.broadcast_cursor.read().await.get(&note_id).cloned();
            // Nothing new since last broadcast → skip.
            if since.as_deref() == Some(current.as_slice()) {
                continue;
            }
            if let Some(bytes) = self.export_doc_update(note_id, since.as_deref()).await {
                out.push((note_id, bytes, current));
            }
        }
        out
    }

    /// Advance + persist the broadcast cursor for notes whose updates were
    /// confirmed sent. Call ONLY after a successful relay PUT (paired with
    /// [`produce_relay_updates`](Self::produce_relay_updates), passing each
    /// note's `captured_vv`). On send failure, skip this so the same delta
    /// is re-produced next tick.
    pub async fn commit_broadcast_cursors(&self, committed: &[([u8; 16], Vec<u8>)]) {
        if committed.is_empty() {
            return;
        }
        {
            let mut cur = self.inner.broadcast_cursor.write().await;
            for (note_id, vv) in committed {
                cur.insert(*note_id, vv.clone());
            }
        }
        // Persist so a restart doesn't re-broadcast every note's full
        // state (best-effort).
        self.save_broadcast_cursors().await;
    }

    /// Apply a batch of broadcast per-note Loro updates (the inbound
    /// relay tick). Idempotent + commutative — duplicate / out-of-order
    /// batches are safe. Returns the count applied.
    pub async fn apply_relay_updates(&self, updates: &[([u8; 16], Vec<u8>)]) -> usize {
        let mut applied = 0;
        for (note_id, bytes) in updates {
            // Fully-qualified call: `import_doc_update` now also exists on the
            // `SyncEngine` trait, so the unqualified `self.import_doc_update`
            // would be ambiguous-by-convention here (and a recursion trap if
            // this body were ever reached through `dyn SyncEngine`). Pin it to
            // the inherent method like every other call site in this file.
            if LoroEngine::import_doc_update(self, *note_id, bytes).await.is_ok() {
                applied += 1;
            }
        }
        applied
    }

    /// Refresh derived state for one note after its doc changed via an
    /// import: re-register its live blocks in block_index and rebuild
    /// its index entry from the doc's root content.
    async fn refresh_note_derived(&self, note_id: [u8; 16], doc: &LoroDoc) {
        let tree = doc.get_tree("blocks");
        let mut ids = Vec::new();
        for node in tree.children(TreeParentId::Root).unwrap_or_default() {
            if matches!(tree.is_node_deleted(&node), Ok(true)) {
                continue;
            }
            if let Some(hex) = read_meta_str(&tree, node, "block_id") {
                if let Some(b) = parse_note_id_from_hex(&hex) {
                    ids.push(b);
                }
            }
        }
        self.register_note_blocks(note_id, &ids).await;
        let root = doc.get_map("root");
        let read = |k: &str| -> String {
            root.get(k)
                .and_then(|v| v.into_value().ok())
                .and_then(|v| v.into_string().ok())
                .map(|s| (*s).clone())
                .unwrap_or_default()
        };
        let content = doc_full_markdown(doc);
        let slug = read("slug");
        let title = read("title");
        let parsed = tesela_core::note_tree::parse_note(&content);
        self.index_upsert(
            note_id,
            Some(slug.as_str()).filter(|s| !s.is_empty()),
            &title,
            &content,
            &parsed.page_properties,
        );
    }

    /// Rebuild every index entry from the loaded per-note docs. Each doc's
    /// full markdown is reconstructed via `doc_full_markdown` (frontmatter +
    /// rendered body, or the legacy root `content` for pre-dedup docs), and
    /// slug + title come from root meta, so the index is a derived
    /// projection. tags/links are always re-derived from that markdown.
    /// title/slug prefer the doc's root meta, then fall back to the
    /// existing index entry (so a rebuild against docs written by an
    /// older engine — which lack slug/title on root meta — doesn't lose
    /// the slugs the prior index already had), then to a frontmatter
    /// title. Stamps the current schema version.
    async fn rebuild_index_from_docs(&self) {
        // Snapshot existing index title/slug as fallback.
        let existing: std::collections::HashMap<String, (String, String)> = self
            .index_entries()
            .await
            .into_iter()
            .map(|e| (e.note_id, (e.title, e.slug)))
            .collect();

        let docs = self.inner.docs.read().await;

        // Prune index entries that have no backing doc, so the rebuild is
        // a TRUE projection of the loaded docs — not an upsert-merge that
        // leaves ghost entries (review finding [6]). A doc can be absent
        // because its snapshot was corrupt/unreadable on load; its index
        // entry must not survive as a phantom note.
        let live: std::collections::HashSet<String> =
            docs.keys().map(hex_id).collect();
        let notes_map = self.inner.index.get_map("notes");
        let stale: Vec<String> = existing
            .keys()
            .filter(|k| !live.contains(*k))
            .cloned()
            .collect();
        for key in stale {
            let _ = notes_map.delete(&key);
        }

        for (note_id, doc) in docs.iter() {
            let root = doc.get_map("root");
            let read = |k: &str| -> String {
                root.get(k)
                    .and_then(|v| v.into_value().ok())
                    .and_then(|v| v.into_string().ok())
                    .map(|s| (*s).clone())
                    .unwrap_or_default()
            };
            let content = doc_full_markdown(doc);
            let key = hex_id(note_id);
            let prior = existing.get(&key);
            let mut slug = read("slug");
            if slug.is_empty() {
                slug = prior.map(|(_, s)| s.clone()).unwrap_or_default();
            }
            let mut title = read("title");
            if title.is_empty() {
                title = prior
                    .map(|(t, _)| t.clone())
                    .filter(|t| !t.is_empty())
                    .or_else(|| frontmatter_title(&content))
                    .unwrap_or_else(|| slug.clone());
            }
            let parsed = tesela_core::note_tree::parse_note(&content);
            self.index_upsert(
                *note_id,
                Some(slug.as_str()).filter(|s| !s.is_empty()),
                &title,
                &content,
                &parsed.page_properties,
            );
        }
        // Stamp schema version (index_upsert already stamps, but ensure
        // it's set even when there are zero docs).
        let _ = self
            .inner
            .index
            .get_map("meta")
            .insert("schema_version", INDEX_SCHEMA_VERSION);
        self.inner.index.commit();
    }

    /// Update the index entry for a note. Called on NoteUpsert. Stores
    /// title + slug + tags + outbound link targets — all derived from
    /// the note content and overwritten wholesale (the index is a
    /// derived projection of the notes).
    fn index_upsert(
        &self,
        note_id: [u8; 16],
        slug: Option<&str>,
        title: &str,
        content: &str,
        page_properties: &[(String, String)],
    ) {
        let (tags, links) = extract_index_metadata(content, page_properties);
        let notes = self.inner.index.get_map("notes");
        let key = hex_id(&note_id);
        let entry = match notes.get(&key) {
            Some(loro::ValueOrContainer::Container(loro::Container::Map(m))) => m,
            _ => match notes.insert_container(&key, loro::LoroMap::new()) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("tesela-sync/loro: index insert_container: {e}");
                    return;
                }
            },
        };
        let _ = entry.insert("title", title);
        let _ = entry.insert("slug", slug.unwrap_or(""));
        // Tags + links as comma-joined strings (derived, overwritten
        // wholesale; structured per-tag containers can come if granular
        // tag merge is ever needed).
        let _ = entry.insert("tags", join_list(&tags));
        let _ = entry.insert("links", join_list(&links));
        // Stamp the schema version so a freshly-built index (e.g. from
        // disk-seed) is recognized as current and not needlessly rebuilt
        // on the next boot.
        let _ = self
            .inner
            .index
            .get_map("meta")
            .insert("schema_version", INDEX_SCHEMA_VERSION);
        self.inner.index.commit();
    }

    /// Remove a note's index entry (NoteDelete).
    fn index_remove(&self, note_id: [u8; 16]) {
        let notes = self.inner.index.get_map("notes");
        let _ = notes.delete(&hex_id(&note_id));
        self.inner.index.commit();
    }

    /// List all index entries. The hybrid model's note list — sourced
    /// from the always-resident index, no per-note docs loaded.
    pub async fn index_entries(&self) -> Vec<crate::engine::IndexEntry> {
        let notes = self.inner.index.get_map("notes");
        let value = notes.get_deep_value();
        let mut out = Vec::new();
        if let loro::LoroValue::Map(m) = value {
            for (key, v) in m.iter() {
                if let loro::LoroValue::Map(entry) = v {
                    let get = |k: &str| {
                        entry.get(k).and_then(|x| {
                            if let loro::LoroValue::String(s) = x {
                                Some((**s).to_string())
                            } else {
                                None
                            }
                        })
                    };
                    let split = |k: &str| -> Vec<String> {
                        get(k)
                            .filter(|s| !s.is_empty())
                            .map(|s| {
                                s.split(INDEX_LIST_SEP).map(|t| t.to_string()).collect()
                            })
                            .unwrap_or_default()
                    };
                    out.push(crate::engine::IndexEntry {
                        note_id: key.to_string(),
                        title: get("title").unwrap_or_default(),
                        slug: get("slug").unwrap_or_default(),
                        tags: split("tags"),
                        links: split("links"),
                    });
                }
            }
        }
        out.sort_by(|a, b| a.note_id.cmp(&b.note_id));
        out
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
    /// tree and feeding `tesela_core::serialize_note`, the same renderer
    /// SqliteEngine uses on disk. Gives the divergence check a
    /// byte-identical comparison surface (modulo frontmatter, which is
    /// on the file but not the shadow).
    ///
    /// **Ordering model matches SqliteEngine exactly:** a flat list in
    /// insertion (document) order, each block rendered at its stored
    /// `indent_level`. SqliteEngine never reorders by `order_key` and
    /// keeps document position stable across moves (a move only changes
    /// indent), so the shadow does the same — all blocks live directly
    /// under root in creation order, and `tree.children(Root)` returns
    /// them in that order.
    ///
    /// Returns `None` for unknown note ids.
    pub async fn render_note(&self, note_id: [u8; 16]) -> Option<String> {
        let docs = self.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        Some(tesela_core::note_tree::serialize_note(&note_tree_from_doc(
            doc, None,
        )))
    }

    /// Render the *complete* `.md` file the engine writes to disk as the
    /// authoritative writer: verbatim frontmatter (root `frontmatter` meta)
    /// + page properties + blocks. Identical to
    /// [`render_note`](Self::render_note) except the frontmatter is
    /// included, so this is the exact byte stream materialization emits.
    /// Delegates to [`doc_full_markdown`], which also handles pre-dedup docs
    /// that still carry the full markdown on root `content`.
    ///
    /// A note whose frontmatter never reached the doc materializes
    /// body-only.
    ///
    /// Returns `None` for unknown note ids.
    pub async fn render_note_full(&self, note_id: [u8; 16]) -> Option<String> {
        let docs = self.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        Some(doc_full_markdown(doc))
    }

    /// This engine's Loro PeerID, derived deterministically from its
    /// 16-byte DeviceId (first 8 bytes, top bit cleared to stay in
    /// Loro's valid PeerID range). Stable across restarts so a device's
    /// ops are always attributed to it — the prerequisite for two
    /// engines' per-note docs merging cleanly (Phase 4).
    fn peer_id(&self) -> u64 {
        let b = self.inner.device.as_bytes();
        let raw = u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]);
        let masked = raw & 0x7FFF_FFFF_FFFF_FFFF;
        if masked == 0 {
            1
        } else {
            masked
        }
    }

    /// Stamp this engine's PeerID on a doc so its subsequent local ops
    /// are attributed to this device. Idempotent; safe on a loaded or
    /// imported doc (sets the peer for FUTURE ops only).
    fn set_doc_peer(&self, doc: &LoroDoc) {
        let _ = doc.set_peer_id(self.peer_id());
    }

    /// Get-or-create the Loro doc for a given note id, with this engine's
    /// PeerID stamped. Called from `record_local` when a NoteUpsert or
    /// BlockUpsert lands.
    async fn doc_for_note_mut(&self, note_id: [u8; 16]) -> LoroDoc {
        let mut docs = self.inner.docs.write().await;
        if !docs.contains_key(&note_id) {
            let doc = LoroDoc::new();
            self.set_doc_peer(&doc);
            docs.insert(note_id, doc);
        }
        docs.get(&note_id).expect("just inserted").clone()
    }

    /// Locate the doc + tree node hosting a given block id by walking
    /// every doc the engine has seen. `BlockMove` / `BlockDelete` ops
    /// carry only the block id (not the owning note), so this lookup
    /// has to be a scan. For the scaffold it's fine — typical mosaics
    /// have a few hundred notes. Replace with an index once profiling
    /// flags it.
    ///
    /// Returns the note_id alongside the doc+node so the outer
    /// `apply_payload` wrapper knows which snapshot to refresh.
    ///
    /// Resolves via the resident `block_index` (block_id → note_id)
    /// rather than scanning every doc's tree. Besides being O(1), this
    /// is a prerequisite for lazy-load/evict (Phase 3/6): once docs can
    /// be evicted, a scan can't see them, but the block_index always can
    /// point at the owning note so its doc can be loaded on demand.
    /// Stale entries (note deleted) self-correct: the docs lookup misses
    /// and we return None, matching "unknown block → no-op".
    async fn find_doc_for_block(
        &self,
        block_id: &[u8; 16],
    ) -> Option<([u8; 16], LoroDoc, TreeID)> {
        let note_id = *self.inner.block_index.read().await.get(block_id)?;
        let block_hex = hex_id(block_id);
        let docs = self.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        let tree = doc.get_tree("blocks");
        let node = find_node_by_block_id(&tree, &block_hex)?;
        Some((note_id, doc.clone(), node))
    }

    /// Register every block in a note as owned by it (block_id →
    /// note_id), so block-only ops (BlockMove/BlockDelete) and lazy-load
    /// can resolve the owning note without scanning all docs.
    async fn register_note_blocks(&self, note_id: [u8; 16], block_ids: &[[u8; 16]]) {
        let mut idx = self.inner.block_index.write().await;
        for b in block_ids {
            idx.insert(*b, note_id);
        }
    }

    /// Write the per-note snapshot to disk, or delete the snapshot
    /// file if the note's doc has been removed (NoteDelete). Best-effort
    /// — failures warn but don't propagate.
    async fn save_snapshot(&self, dir: &Path, note_id: [u8; 16]) {
        let path = dir.join(format!("{}.bin", hex_id(&note_id)));
        let docs = self.inner.docs.read().await;
        match docs.get(&note_id) {
            Some(doc) => {
                let bytes = match doc.export(ExportMode::Snapshot) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(
                            "tesela-sync/loro: snapshot export for {}: {e}",
                            hex_id(&note_id)
                        );
                        return;
                    }
                };
                let tmp = unique_tmp(&path);
                if let Err(e) = tokio::fs::write(&tmp, &bytes).await {
                    tracing::warn!(
                        "tesela-sync/loro: snapshot write {}: {e}",
                        tmp.display()
                    );
                    return;
                }
                if let Err(e) = tokio::fs::rename(&tmp, &path).await {
                    tracing::warn!(
                        "tesela-sync/loro: snapshot rename {}: {e}",
                        path.display()
                    );
                    let _ = tokio::fs::remove_file(&tmp).await;
                }
            }
            None => {
                // Doc gone (NoteDelete). Remove the snapshot if present.
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        tracing::warn!(
                            "tesela-sync/loro: snapshot delete {}: {e}",
                            path.display()
                        );
                    }
                }
            }
        }
    }

    /// Resolve a note's filename slug. Reads the doc's `root.slug` meta
    /// (set on every NoteUpsert), falling back to the index entry. Used
    /// to name the materialized `<slug>.md` file.
    async fn slug_for_note(&self, note_id: [u8; 16]) -> Option<String> {
        {
            let docs = self.inner.docs.read().await;
            if let Some(doc) = docs.get(&note_id) {
                let slug = doc
                    .get_map("root")
                    .get("slug")
                    .and_then(|v| v.into_value().ok())
                    .and_then(|v| v.into_string().ok())
                    .map(|s| (*s).clone())
                    .unwrap_or_default();
                if !slug.is_empty() {
                    return Some(slug);
                }
            }
        }
        let key = hex_id(&note_id);
        self.index_entries()
            .await
            .into_iter()
            .find(|e| e.note_id == key)
            .map(|e| e.slug)
            .filter(|s| !s.is_empty())
    }

    /// Write the note's canonical full `.md` (frontmatter + body) to
    /// `<materialize_dir>/<slug>.md` via atomic tmp+rename. No-op when
    /// `materialize_dir` is unset (non-authoritative) or the slug can't
    /// be resolved. This is what makes LoroEngine the sole writer of the
    /// mosaic in authoritative mode.
    async fn materialize_note(&self, note_id: [u8; 16]) {
        let Some(dir) = self.inner.materialize_dir.as_ref() else {
            return;
        };
        let Some(full) = self.render_note_full(note_id).await else {
            return;
        };
        let Some(slug) = self.slug_for_note(note_id).await else {
            tracing::warn!(
                "tesela-sync/loro: cannot materialize {} — no slug",
                hex_id(&note_id)
            );
            return;
        };
        let path = dir.join(format!("{slug}.md"));
        let tmp = unique_tmp(&path);
        if let Err(e) = tokio::fs::write(&tmp, full.as_bytes()).await {
            tracing::warn!("tesela-sync/loro: materialize write {}: {e}", tmp.display());
            return;
        }
        if let Err(e) = tokio::fs::rename(&tmp, &path).await {
            tracing::warn!("tesela-sync/loro: materialize rename {}: {e}", path.display());
            let _ = tokio::fs::remove_file(&tmp).await;
        }
    }

    /// Remove a materialized `<slug>.md` (authoritative NoteDelete). No-op
    /// when `materialize_dir` is unset or the file is already gone.
    async fn remove_materialized(&self, slug: &str) {
        let Some(dir) = self.inner.materialize_dir.as_ref() else {
            return;
        };
        let path = dir.join(format!("{slug}.md"));
        if let Err(e) = tokio::fs::remove_file(&path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("tesela-sync/loro: materialize delete {}: {e}", path.display());
            }
        }
    }

    /// Persist the per-note broadcast cursors to `<snapshot_dir>/_broadcast.bin`
    /// (postcard of `Vec<(note_id, encoded_vv)>`). Best-effort; a lost
    /// cursor only costs a redundant (idempotent) full re-broadcast.
    async fn save_broadcast_cursors(&self) {
        let Some(dir) = self.inner.snapshot_dir.as_ref() else {
            return;
        };
        let entries: Vec<([u8; 16], Vec<u8>)> = self
            .inner
            .broadcast_cursor
            .read()
            .await
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let bytes = match postcard::to_allocvec(&entries) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("tesela-sync/loro: broadcast cursor encode: {e}");
                return;
            }
        };
        let path = dir.join("_broadcast.bin");
        let tmp = unique_tmp(&path);
        if tokio::fs::write(&tmp, &bytes).await.is_ok() {
            if tokio::fs::rename(&tmp, &path).await.is_err() {
                let _ = tokio::fs::remove_file(&tmp).await;
            }
        } else {
            let _ = tokio::fs::remove_file(&tmp).await;
        }
    }

    /// Reseed every note's Loro doc from the authoritative `.md` files in
    /// `notes_dir` by replaying a `NoteUpsert` per file. For notes already
    /// resident, `apply_payload`'s NoteUpsert tree-reconcile corrects a
    /// drifted/stale doc to match disk (the fix for the stale-shadow
    /// divergences the materialization dry-run found). For new notes it
    /// seeds them. This is the canonical-device bootstrap for the cutover
    /// — the source of truth on first authoritative boot is DISK, not the
    /// frozen oplog/snapshots. Returns the number of files processed.
    ///
    /// NOTE: independent disk-reseed on multiple devices mints
    /// non-merging Loro nodes; only the designated canonical device
    /// reseeds, the rest bootstrap by importing from the relay.
    pub async fn reseed_from_disk(&self, notes_dir: &Path) -> SyncResult<usize> {
        let mut entries = match tokio::fs::read_dir(notes_dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => {
                return Err(SyncError::Storage(format!(
                    "reseed read dir {}: {e}",
                    notes_dir.display()
                )))
            }
        };
        let mut count = 0usize;
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            SyncError::Storage(format!("reseed read_dir {}: {e}", notes_dir.display()))
        })? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("tesela-sync/loro: reseed read {}: {e}", path.display());
                    continue;
                }
            };
            let hash = blake3::hash(stem.as_bytes());
            let mut note_id = [0u8; 16];
            note_id.copy_from_slice(&hash.as_bytes()[..16]);
            let title = frontmatter_title(&content).unwrap_or_else(|| stem.to_string());
            let payload = OpPayload::NoteUpsert {
                note_id,
                display_alias: Some(stem.to_string()),
                title,
                content,
                created_at_millis: 0,
            };
            if let Err(e) = self.apply_payload(&payload).await {
                tracing::warn!("tesela-sync/loro: reseed apply {stem}: {e}");
                continue;
            }
            count += 1;
        }
        Ok(count)
    }
}

/// Load per-note broadcast cursors persisted by
/// `LoroEngine::save_broadcast_cursors`. Missing/corrupt → empty map
/// (a full re-broadcast on the next tick is idempotent).
async fn load_broadcast_cursors(dir: &Path) -> HashMap<[u8; 16], Vec<u8>> {
    let path = dir.join("_broadcast.bin");
    match tokio::fs::read(&path).await {
        Ok(bytes) => match postcard::from_bytes::<Vec<([u8; 16], Vec<u8>)>>(&bytes) {
            Ok(entries) => entries.into_iter().collect(),
            Err(e) => {
                tracing::warn!("tesela-sync/loro: broadcast cursor decode: {e}");
                HashMap::new()
            }
        },
        Err(_) => HashMap::new(),
    }
}

/// Scan a snapshot directory for `<note-id-hex>.bin` files and import
/// each into a `LoroDoc`. Used by `LoroEngine::with_snapshot_dir` at
/// boot so the shadow starts with the state it had at shutdown,
/// without re-replaying the entire oplog.
///
/// Files with malformed names or corrupt snapshot bytes are warned
/// about and skipped — the caller's prepopulate-from-oplog path covers
/// them.
async fn load_snapshots_from_dir(
    dir: &Path,
) -> SyncResult<HashMap<[u8; 16], LoroDoc>> {
    let mut docs: HashMap<[u8; 16], LoroDoc> = HashMap::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(docs),
        Err(e) => {
            return Err(SyncError::Storage(format!(
                "read snapshot dir {}: {e}",
                dir.display()
            )))
        }
    };
    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        SyncError::Storage(format!("read_dir {}: {e}", dir.display()))
    })? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("bin") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // The index doc (`_index.bin`) is loaded separately, not as a
        // per-note snapshot.
        if stem == "_index" {
            continue;
        }
        let Some(note_id) = parse_note_id_from_hex(stem) else {
            tracing::warn!(
                "tesela-sync/loro: snapshot filename not a hex note id: {}",
                path.display()
            );
            continue;
        };
        let bytes = match tokio::fs::read(&path).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    "tesela-sync/loro: read snapshot {}: {e}",
                    path.display()
                );
                continue;
            }
        };
        let doc = LoroDoc::new();
        if let Err(e) = doc.import(&bytes) {
            tracing::warn!(
                "tesela-sync/loro: import snapshot {}: {e}",
                path.display()
            );
            continue;
        }
        docs.insert(note_id, doc);
    }
    Ok(docs)
}

fn parse_note_id_from_hex(s: &str) -> Option<[u8; 16]> {
    let bytes = hex::decode(s).ok()?;
    if bytes.len() != 16 {
        return None;
    }
    let mut arr = [0u8; 16];
    arr.copy_from_slice(&bytes);
    Some(arr)
}

/// Build a `FlatBlock` from a single tree node's meta. Returns `None`
/// if the node's meta can't be read. The block's `parent` field is left
/// `None` because `serialize_note` renders purely off `indent` —
/// matching SqliteEngine, which also derives the on-disk shape from
/// document order + indent, not from the parent pointer.
fn flatblock_from_node(
    tree: &LoroTree,
    node: TreeID,
) -> Option<tesela_core::note_tree::FlatBlock> {
    let meta = tree.get_meta(node).ok()?;
    let indent = meta
        .get("indent_level")
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_i64().ok())
        .unwrap_or(0) as u16;
    let text = read_block_text(tree, node).unwrap_or_default();
    let id_hex = meta
        .get("block_id")
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| (*s).clone())
        .unwrap_or_default();
    let id_uuid = parse_uuid_from_hex(&id_hex).unwrap_or_else(uuid::Uuid::nil);
    // Container properties (P1.5): read the block node's `props`/`prop_keys`
    // NON-MUTATINGLY and materialize them in canonical order. Absent → empty.
    let properties = prop_containers::read_node_prop_containers(&meta)
        .map(|(props, prop_keys)| prop_containers::materialize_props(&props, &prop_keys))
        .unwrap_or_default();
    Some(tesela_core::note_tree::FlatBlock {
        id: id_uuid,
        parent: None,
        indent,
        text,
        properties,
    })
}

/// Read a string meta field off a tree node, or None if absent/empty.
fn read_meta_str(tree: &LoroTree, node: TreeID, key: &str) -> Option<String> {
    let meta = tree.get_meta(node).ok()?;
    let v = meta.get(key)?;
    let val = v.into_value().ok()?;
    let s = val.into_string().ok()?;
    let s = (*s).clone();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Write a block's whole text into its node's nested `text_seq`
/// [`LoroText`] container — the sequence CRDT that lets concurrent
/// same-block edits INTERLEAVE instead of clobbering (the LWW map
/// register `text` did). `get_or_create_container` is idempotent and
/// returns the existing handler if present, so seed + upsert + heal all
/// converge on ONE container at the stable `text_seq` key; `update`
/// Myers-diffs the whole string into minimal splices against the
/// container's CURRENT value. We never write the legacy `text` register
/// again — concurrently minting a different container at the same key
/// can overwrite rather than merge, so a distinct key + get-or-create is
/// the safe path. The legacy `text` register stays readable for old
/// snapshots via [`read_block_text`].
fn write_block_text(meta: &loro::LoroMap, text: &str) -> SyncResult<()> {
    let text_c: LoroText = meta
        .get_or_create_container("text_seq", LoroText::new())
        .map_err(|e| SyncError::Storage(format!("loro text_seq get_or_create: {e}")))?;
    text_c
        .update(text, UpdateOptions::default())
        .map_err(|e| SyncError::Storage(format!("loro text_seq update: {e}")))?;
    Ok(())
}

/// Read a block's whole text, PREFERRING the nested `text_seq`
/// [`LoroText`] container and FALLING BACK to the legacy `text` map
/// register for snapshots written before the LoroText migration. The
/// read is NON-MUTATING: it only inspects whether the container exists
/// (via `meta.get`), never minting one on a pure read path (which would
/// dirty the doc and grow its history). Empty text → `None`, matching
/// `read_meta_str`.
fn read_block_text(tree: &LoroTree, node: TreeID) -> Option<String> {
    let meta = tree.get_meta(node).ok()?;
    let from_seq = meta
        .get("text_seq")
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .map(|t| t.to_string());
    let s = match from_seq {
        Some(s) => s,
        None => {
            let v = meta.get("text")?;
            let val = v.into_value().ok()?;
            (*val.into_string().ok()?).clone()
        }
    };
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Read a node's `indent_level` meta. Used by BlockMove to recompute a
/// moved block's indent as parent.indent + 1, mirroring SqliteEngine.
fn read_indent_level(tree: &LoroTree, node: TreeID) -> Option<u16> {
    let meta = tree.get_meta(node).ok()?;
    let v = meta.get("indent_level")?;
    let val = v.into_value().ok()?;
    Some(val.into_i64().ok()? as u16)
}

// ---------------------------------------------------------------------------
// Typed property containers (Phase-1 foundation).
//
// Each owner (a block node's meta map, or the doc root for page properties)
// carries a `props` LoroMap keyed by property name plus a sibling `prop_keys`
// LoroList recording first-seen key order — LoroMap iteration order is NOT
// guaranteed, so the materializer/index walk `prop_keys`, never the map.
// Scalars are stored as primitive LoroValues (zero sub-containers); free text
// as a nested LoroText; multi-value as a nested LoroList (concurrent add/remove
// union-merges). Every nested write goes through `get_or_create_container` at a
// stable key — never `insert_container` at an existing key — so concurrent
// first-touch converges on ONE container (same discipline as `write_block_text`).
// ---------------------------------------------------------------------------

mod prop_containers {
use super::*;
use tesela_core::property::PropScalar;

/// Get-or-create the `props` + `prop_keys` containers on a block node's meta map.
pub(super) fn node_prop_containers(meta: &loro::LoroMap) -> SyncResult<(loro::LoroMap, loro::LoroList)> {
    let props = meta
        .get_or_create_container("props", loro::LoroMap::new())
        .map_err(|e| SyncError::Storage(format!("loro props get_or_create: {e}")))?;
    let prop_keys = meta
        .get_or_create_container("prop_keys", loro::LoroList::new())
        .map_err(|e| SyncError::Storage(format!("loro prop_keys get_or_create: {e}")))?;
    Ok((props, prop_keys))
}

/// The page-level `props` + `prop_keys` containers live at the doc root.
pub(super) fn page_prop_containers(doc: &LoroDoc) -> (loro::LoroMap, loro::LoroList) {
    (doc.get_map("props"), doc.get_list("prop_keys"))
}

/// NON-MUTATING read of a block node's `props` + `prop_keys` containers,
/// returning `None` when either has never been created. The materializer
/// reads through here — `node_prop_containers` would `get_or_create` the
/// nested containers and dirty the doc on a pure render (same discipline as
/// `read_block_text` inspecting `text_seq` via `meta.get`, never minting).
pub(super) fn read_node_prop_containers(
    meta: &loro::LoroMap,
) -> Option<(loro::LoroMap, loro::LoroList)> {
    let props = meta
        .get("props")?
        .into_container()
        .ok()?
        .into_map()
        .ok()?;
    let prop_keys = meta
        .get("prop_keys")?
        .into_container()
        .ok()?
        .into_list()
        .ok()?;
    Some((props, prop_keys))
}

/// The keys in `prop_keys` insertion order (raw — the dedup / drop-missing
/// reconcile shared by the materializer and index lands in P1.3).
fn prop_keys_ordered(prop_keys: &loro::LoroList) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..prop_keys.len() {
        if let Some(v) = prop_keys.get(i) {
            if let Ok(val) = v.into_value() {
                if let Ok(s) = val.into_string() {
                    out.push(s.to_string());
                }
            }
        }
    }
    out
}

/// All keys present in the `props` map (primitive values AND nested
/// containers), in the map's (unspecified) iteration order.
fn props_map_keys(props: &loro::LoroMap) -> Vec<String> {
    props.keys().map(|k| k.to_string()).collect()
}

/// The canonical ordered key set shared by the materializer + index + chips.
///
/// Walk `prop_keys` keeping the FIRST occurrence of each key and DROPPING any
/// key absent from `props`; then APPEND any key present in `props` but missing
/// from `prop_keys`, in lexicographic (byte) order. `prop_keys` is the sole
/// ordering authority — the `props` map's iteration order is never trusted for
/// order, only consulted for membership and for the lexicographic tail.
fn prop_keys_resolved(props: &loro::LoroMap, prop_keys: &loro::LoroList) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for key in prop_keys_ordered(prop_keys) {
        if seen.contains(&key) {
            continue;
        }
        if props.get(&key).is_none() {
            continue;
        }
        seen.insert(key.clone());
        out.push(key);
    }
    let mut tail: Vec<String> = props_map_keys(props)
        .into_iter()
        .filter(|k| !seen.contains(k))
        .collect();
    tail.sort();
    out.extend(tail);
    out
}

/// Append `key` to `prop_keys` once (first-seen order); no-op if present.
fn prop_keys_ensure(prop_keys: &loro::LoroList, key: &str) -> SyncResult<()> {
    if prop_keys_ordered(prop_keys).iter().any(|k| k.as_str() == key) {
        return Ok(());
    }
    prop_keys
        .push(key)
        .map_err(|e| SyncError::Storage(format!("loro prop_keys push: {e}")))
}

/// Remove every occurrence of `key` from `prop_keys`.
fn prop_keys_remove(prop_keys: &loro::LoroList, key: &str) -> SyncResult<()> {
    let mut i = 0;
    while i < prop_keys.len() {
        let matches = prop_keys
            .get(i)
            .and_then(|v| v.into_value().ok())
            .and_then(|val| val.into_string().ok())
            .map(|s| s.as_str() == key)
            .unwrap_or(false);
        if matches {
            prop_keys
                .delete(i, 1)
                .map_err(|e| SyncError::Storage(format!("loro prop_keys delete: {e}")))?;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Convert a primitive LoroValue back into a `PropScalar` (containers → None).
fn loro_value_to_scalar(v: loro::LoroValue) -> Option<PropScalar> {
    match v {
        loro::LoroValue::String(s) => Some(PropScalar::Text(s.to_string())),
        loro::LoroValue::I64(i) => Some(PropScalar::Int(i)),
        loro::LoroValue::Double(f) => Some(PropScalar::Float(f)),
        loro::LoroValue::Bool(b) => Some(PropScalar::Bool(b)),
        _ => None,
    }
}

fn map_insert_scalar(props: &loro::LoroMap, key: &str, value: &PropScalar) -> SyncResult<()> {
    let r = match value {
        PropScalar::Text(s) => props.insert(key, s.as_str()),
        PropScalar::Int(i) => props.insert(key, *i),
        PropScalar::Float(f) => props.insert(key, *f),
        PropScalar::Bool(b) => props.insert(key, *b),
    };
    r.map_err(|e| SyncError::Storage(format!("loro props insert: {e}")))
}

/// Set a single-value scalar property (primitive LoroValue; concurrent set = LWW).
pub(super) fn prop_set_scalar(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
    value: &PropScalar,
) -> SyncResult<()> {
    map_insert_scalar(props, key, value)?;
    prop_keys_ensure(prop_keys, key)
}

/// Set a free-text property as a nested LoroText (concurrent char-merge).
pub(super) fn prop_set_text(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
    text: &str,
) -> SyncResult<()> {
    let t: LoroText = props
        .get_or_create_container(key, LoroText::new())
        .map_err(|e| SyncError::Storage(format!("loro prop text get_or_create: {e}")))?;
    t.update(text, UpdateOptions::default())
        .map_err(|e| SyncError::Storage(format!("loro prop text update: {e}")))?;
    prop_keys_ensure(prop_keys, key)
}

fn list_push_scalar(list: &loro::LoroList, value: &PropScalar) -> SyncResult<()> {
    let r = match value {
        PropScalar::Text(s) => list.push(s.as_str()),
        PropScalar::Int(i) => list.push(*i),
        PropScalar::Float(f) => list.push(*f),
        PropScalar::Bool(b) => list.push(*b),
    };
    r.map_err(|e| SyncError::Storage(format!("loro prop list push: {e}")))
}

/// Index of `value` in a list, or None.
fn list_position(list: &loro::LoroList, value: &PropScalar) -> Option<usize> {
    for i in 0..list.len() {
        if let Some(v) = list.get(i) {
            if let Ok(val) = v.into_value() {
                if loro_value_to_scalar(val).as_ref() == Some(value) {
                    return Some(i);
                }
            }
        }
    }
    None
}

fn get_list_container(props: &loro::LoroMap, key: &str) -> Option<loro::LoroList> {
    props
        .get(key)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_list().ok())
}

/// Add a value to a multi-value property's nested LoroList, union semantics
/// (a value already present is a no-op).
pub(super) fn prop_add_to_list(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
    value: &PropScalar,
) -> SyncResult<()> {
    let list: loro::LoroList = props
        .get_or_create_container(key, loro::LoroList::new())
        .map_err(|e| SyncError::Storage(format!("loro prop list get_or_create: {e}")))?;
    if list_position(&list, value).is_none() {
        list_push_scalar(&list, value)?;
    }
    prop_keys_ensure(prop_keys, key)
}

/// Remove a value from a multi-value property's list (no-op if absent or the
/// property isn't a list). `prop_keys` is left intact — an emptied list keeps
/// its key until an explicit `prop_clear`.
pub(super) fn prop_remove_from_list(
    props: &loro::LoroMap,
    key: &str,
    value: &PropScalar,
) -> SyncResult<()> {
    let Some(list) = get_list_container(props, key) else {
        return Ok(());
    };
    if let Some(i) = list_position(&list, value) {
        list.delete(i, 1)
            .map_err(|e| SyncError::Storage(format!("loro prop list delete: {e}")))?;
    }
    Ok(())
}

/// Remove a property entirely: from `props` AND `prop_keys`.
pub(super) fn prop_clear(props: &loro::LoroMap, prop_keys: &loro::LoroList, key: &str) -> SyncResult<()> {
    props
        .delete(key)
        .map_err(|e| SyncError::Storage(format!("loro props delete: {e}")))?;
    prop_keys_remove(prop_keys, key)
}

/// Read a scalar property (primitive value under `key`); None if absent or a container.
#[allow(dead_code)] // scalar reader wired by the materializer in P1.5; tests read it now
pub(super) fn prop_get_scalar(props: &loro::LoroMap, key: &str) -> Option<PropScalar> {
    let val = props.get(key)?.into_value().ok()?;
    loro_value_to_scalar(val)
}

/// Read a text property (nested LoroText); None if absent or not text.
fn prop_get_text(props: &loro::LoroMap, key: &str) -> Option<String> {
    props
        .get(key)?
        .into_container()
        .ok()?
        .into_text()
        .ok()
        .map(|t| t.to_string())
}

/// Read a multi-value property as scalars in list order; empty if absent.
pub(super) fn prop_get_list(props: &loro::LoroMap, key: &str) -> Vec<PropScalar> {
    let Some(list) = get_list_container(props, key) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0..list.len() {
        if let Some(s) = list
            .get(i)
            .and_then(|v| v.into_value().ok())
            .and_then(loro_value_to_scalar)
        {
            out.push(s);
        }
    }
    out
}

/// Read a multi-value property with STABLE first-occurrence dedup over the
/// merged list order. A concurrent union-merge across replicas can leave the
/// same value at more than one position; the materializer/index/chips read
/// through here so the view is deterministic (same CRDT state → same bytes).
fn prop_get_list_dedup(props: &loro::LoroMap, key: &str) -> Vec<PropScalar> {
    let mut seen: Vec<PropScalar> = Vec::new();
    for v in prop_get_list(props, key) {
        if !seen.contains(&v) {
            seen.push(v);
        }
    }
    seen
}

/// Materialize a `(props, prop_keys)` owner into the ordered, canonical
/// `(key, value)` pairs the `note_tree` serializer renders as `key:: value`
/// continuation lines. The ONE shared read path for the materializer (block
/// and page). Walks `prop_keys_resolved` order, rendering each stored kind:
/// a scalar via `format_scalar`; a multi-value list via stable-dedup
/// comma-join (the `tags::` convention); free text via the nested LoroText
/// string. Deterministic: same CRDT state always yields the same bytes.
pub(super) fn materialize_props(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
) -> Vec<(String, String)> {
    use tesela_core::property::format_scalar;
    let mut out: Vec<(String, String)> = Vec::new();
    for key in prop_keys_resolved(props, prop_keys) {
        let value = if let Some(scalar) = prop_get_scalar(props, &key) {
            format_scalar(&scalar)
        } else if get_list_container(props, &key).is_some() {
            prop_get_list_dedup(props, &key)
                .iter()
                .map(format_scalar)
                .collect::<Vec<_>>()
                .join(", ")
        } else if let Some(text) = prop_get_text(props, &key) {
            text
        } else {
            // A key in `prop_keys` whose `props` entry is absent is already
            // dropped by `prop_keys_resolved`; an unreadable container is
            // skipped rather than rendered as an empty line.
            continue;
        };
        out.push((key, value));
    }
    out
}

#[cfg(test)]
mod prop_helper_tests {
    use super::*;
    use tesela_core::property::PropScalar;

    #[test]
    fn prop_helpers_set_get_clear_on_block_node() {
        let doc = loro::LoroDoc::new();
        let tree = doc.get_tree("blocks");
        let node = tree.create(loro::TreeParentId::Root).unwrap();
        let meta = tree.get_meta(node).unwrap();
        let (props, prop_keys) = node_prop_containers(&meta).unwrap();

        // scalar (string)
        prop_set_scalar(&props, &prop_keys, "status", &PropScalar::Text("doing".into())).unwrap();
        assert_eq!(prop_get_scalar(&props, "status"), Some(PropScalar::Text("doing".into())));

        // scalar (int)
        prop_set_scalar(&props, &prop_keys, "priority", &PropScalar::Int(3)).unwrap();
        assert_eq!(prop_get_scalar(&props, "priority"), Some(PropScalar::Int(3)));

        // text (nested LoroText)
        prop_set_text(&props, &prop_keys, "note", "hello world").unwrap();
        assert_eq!(prop_get_text(&props, "note").as_deref(), Some("hello world"));

        // multi-value: add → union (dup is a no-op), then remove
        prop_add_to_list(&props, &prop_keys, "tags", &PropScalar::Text("Task".into())).unwrap();
        prop_add_to_list(&props, &prop_keys, "tags", &PropScalar::Text("Urgent".into())).unwrap();
        prop_add_to_list(&props, &prop_keys, "tags", &PropScalar::Text("Task".into())).unwrap();
        assert_eq!(
            prop_get_list(&props, "tags"),
            vec![
                PropScalar::Text("Task".into()),
                PropScalar::Text("Urgent".into())
            ]
        );
        prop_remove_from_list(&props, "tags", &PropScalar::Text("Task".into())).unwrap();
        assert_eq!(prop_get_list(&props, "tags"), vec![PropScalar::Text("Urgent".into())]);

        // prop_keys preserves first-seen order
        assert_eq!(prop_keys_ordered(&prop_keys).join(","), "status,priority,note,tags");

        // clear removes from BOTH props and prop_keys
        prop_clear(&props, &prop_keys, "priority").unwrap();
        assert_eq!(prop_get_scalar(&props, "priority"), None);
        assert_eq!(prop_keys_ordered(&prop_keys).join(","), "status,note,tags");
    }

    #[test]
    fn prop_helpers_on_page_root() {
        let doc = loro::LoroDoc::new();
        let (props, prop_keys) = page_prop_containers(&doc);
        prop_set_scalar(&props, &prop_keys, "type", &PropScalar::Text("Tag".into())).unwrap();
        assert_eq!(prop_get_scalar(&props, "type"), Some(PropScalar::Text("Tag".into())));
        assert_eq!(prop_keys_ordered(&prop_keys).join(","), "type");
    }

    #[test]
    fn prop_keys_resolved_dedups_drops_missing_appends_lexicographically() {
        let doc = loro::LoroDoc::new();
        let (props, prop_keys) = page_prop_containers(&doc);

        // props gets three keys via the set helpers (which also push to prop_keys);
        // then we hand-build a messy prop_keys list to exercise the reconcile:
        //   - "status"  : present in props, listed
        //   - "ghost"   : NOT in props, listed (must be DROPPED)
        //   - "status"  : duplicate (must keep FIRST occurrence only)
        //   - "priority": present in props, listed
        // and "zeta" is present in props but absent from prop_keys (must be
        // APPENDED in lexicographic order relative to other props-only keys).
        prop_set_scalar(&props, &prop_keys, "status", &PropScalar::Text("doing".into())).unwrap();
        prop_set_scalar(&props, &prop_keys, "priority", &PropScalar::Int(3)).unwrap();
        // props-only keys (NOT pushed to prop_keys): inserted directly so they
        // exercise the lexicographic append. "alpha" sorts before "zeta".
        props.insert("zeta", "z").unwrap();
        props.insert("alpha", "a").unwrap();

        // Overwrite prop_keys with the messy ordering described above.
        prop_keys.delete(0, prop_keys.len()).unwrap();
        for k in ["status", "ghost", "status", "priority"] {
            prop_keys.push(k).unwrap();
        }

        // Expected: listed-and-present in first-seen order (status, priority,
        // dup status dropped, ghost dropped), then props-only keys appended in
        // byte order (alpha, zeta).
        assert_eq!(
            prop_keys_resolved(&props, &prop_keys),
            vec![
                "status".to_string(),
                "priority".to_string(),
                "alpha".to_string(),
                "zeta".to_string(),
            ]
        );
    }

    #[test]
    fn prop_get_list_dedup_is_stable_first_occurrence() {
        let doc = loro::LoroDoc::new();
        let (props, prop_keys) = page_prop_containers(&doc);

        // Simulate a post-merge list where a value appears at more than one
        // position: [A, B, A, C, B]. The stable dedup keeps the FIRST sighting
        // of each value, preserving merged order: [A, B, C].
        let list: loro::LoroList = props.get_or_create_container("tags", loro::LoroList::new()).unwrap();
        for v in ["A", "B", "A", "C", "B"] {
            list.push(v).unwrap();
        }
        prop_keys.push("tags").unwrap();

        assert_eq!(
            prop_get_list_dedup(&props, "tags"),
            vec![
                PropScalar::Text("A".into()),
                PropScalar::Text("B".into()),
                PropScalar::Text("C".into()),
            ]
        );
    }
}
}

/// Page-property storage: an ordered `LoroList` named "page_props" on
/// the note doc, holding key, value, key, value, … (interleaved). Page
/// properties arrive wholesale via NoteUpsert (full-content reparse),
/// so we rewrite the whole list each time — clear + repush. Ordered so
/// render reproduces on-disk order deterministically. (When granular
/// per-property merge lands — the deferred multi-value work — this
/// becomes a map/movable-list with per-key updates.)
fn set_page_properties(
    doc: &LoroDoc,
    props: &[(String, String)],
) -> SyncResult<()> {
    let list = doc.get_list("page_props");
    let len = list.len();
    if len > 0 {
        list.delete(0, len)
            .map_err(|e| SyncError::Storage(format!("loro page_props clear: {e}")))?;
    }
    for (k, v) in props {
        list.push(k.as_str())
            .map_err(|e| SyncError::Storage(format!("loro page_props push: {e}")))?;
        list.push(v.as_str())
            .map_err(|e| SyncError::Storage(format!("loro page_props push: {e}")))?;
    }
    Ok(())
}

/// Walk a doc's `blocks` tree into a `NoteTree` — flat blocks in document
/// (insertion) order at their stored indent, plus the ordered page
/// properties — attaching the given `frontmatter`. Shared renderer behind
/// both [`LoroEngine::render_note`] (frontmatter `None`, the shadow
/// comparison surface) and [`LoroEngine::render_note_full`] (frontmatter
/// from the doc's stored content, the exact bytes materialization emits).
fn note_tree_from_doc(
    doc: &LoroDoc,
    frontmatter: Option<String>,
) -> tesela_core::note_tree::NoteTree {
    let tree = doc.get_tree("blocks");
    let mut blocks: Vec<tesela_core::note_tree::FlatBlock> = Vec::new();
    // Live root children in walk order, mirroring the `is_node_deleted`
    // filtering used elsewhere, then collapse any duplicate-bid twins to a
    // single canonical node (Loro unions same-bid nodes minted on disjoint
    // histories — see `dedup_twins_by_block_id`). Render-side heal so an
    // already-corrupted on-disk doc shows each block exactly once.
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    for node in dedup_twins_by_block_id(&tree, live) {
        if let Some(fb) = flatblock_from_node(&tree, node) {
            // NOTE: blank (empty) bullets are KEPT. They are the editing
            // surface — the web outliner relies on a trailing empty bullet
            // existing so an "empty" day has a focusable row to type into
            // (`JournalView.ensureTrailingEmpty`). Dropping them made empty
            // days zero-block and un-editable (keyboard + mouse), so the
            // 2026-05-29 "drop blank blocks" experiment is reverted.
            // Headings / non-bullet body lines are still absent — the
            // flat-block model never captured them (that's the intended
            // heading drop).
            blocks.push(fb);
        }
    }
    tesela_core::note_tree::NoteTree {
        frontmatter,
        page_properties: page_properties_materialized(doc),
        blocks,
        stamped_any: false,
    }
}

/// Page-level properties for materialization: container `props`/`prop_keys`
/// at the doc root FIRST (canonical order), then any LEGACY `page_props`
/// entry whose key the container hasn't already supplied. Container props
/// and legacy `page_props` are disjoint stores at this stage (P1.5);
/// migrate-on-write that folds legacy into the container and CLEARS it is
/// P1.6, so for now we surface both without double-emitting a shared key.
fn page_properties_materialized(doc: &LoroDoc) -> Vec<(String, String)> {
    let (props, prop_keys) = prop_containers::page_prop_containers(doc);
    let mut out = prop_containers::materialize_props(&props, &prop_keys);
    let seen: std::collections::HashSet<String> =
        out.iter().map(|(k, _)| k.clone()).collect();
    for (k, v) in read_page_properties(doc) {
        if !seen.contains(&k) {
            out.push((k, v));
        }
    }
    out
}

/// Read the ordered page properties back out of the "page_props" list.
fn read_page_properties(doc: &LoroDoc) -> Vec<(String, String)> {
    let list = doc.get_list("page_props");
    let len = list.len();
    let mut out = Vec::with_capacity(len / 2);
    let mut i = 0;
    while i + 1 < len {
        let k = list
            .get(i)
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| (*s).clone());
        let v = list
            .get(i + 1)
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| (*s).clone());
        if let (Some(k), Some(v)) = (k, v) {
            out.push((k, v));
        }
        i += 2;
    }
    out
}

/// Read a per-note doc's verbatim frontmatter. Current-version docs store
/// it directly on root `frontmatter` (the lean schema — the body lives in
/// the tree, so the full markdown is never duplicated on root meta).
/// Pre-dedup docs instead stored the full markdown on root `content`; fall
/// back to parsing that so their frontmatter still renders until a reseed
/// rebuilds them lean. Returns `None` when neither is present (body-only).
fn doc_frontmatter(doc: &LoroDoc) -> Option<String> {
    let root = doc.get_map("root");
    let read = |k: &str| -> String {
        root.get(k)
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| (*s).clone())
            .unwrap_or_default()
    };
    let fm = read("frontmatter");
    if !fm.is_empty() {
        return Some(fm);
    }
    let content = read("content");
    if !content.is_empty() {
        return tesela_core::note_tree::parse_note(&content).frontmatter;
    }
    None
}

/// Reconstruct the full `.md` for a per-note doc — frontmatter + rendered
/// body — which equals what materialization writes to disk and what the
/// index derives tags/links from. Lean (current-version) docs reconstruct
/// from the tree; pre-dedup docs that still carry the full markdown on root
/// `content` return it verbatim (matching the old derivation exactly until
/// a reseed converts them).
fn doc_full_markdown(doc: &LoroDoc) -> String {
    let content = doc
        .get_map("root")
        .get("content")
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| (*s).clone())
        .unwrap_or_default();
    if !content.is_empty() {
        return content;
    }
    tesela_core::note_tree::serialize_note(&note_tree_from_doc(doc, doc_frontmatter(doc)))
}

/// Seed a flat tree from `tesela_core::FlatBlock`s parsed out of a
/// NoteUpsert's body content. Used when LoroEngine sees a note for the
/// first time and the only op is the NoteUpsert.
///
/// All blocks are created directly under root in document order so
/// `tree.children(Root)` later returns them in that order — matching
/// SqliteEngine's flat-document-order model. `indent_level` carries the
/// visual hierarchy; the tree is intentionally flat.
/// True if the tree's live blocks (in render order) match `blocks` by
/// id + text + indent. Used to decide whether a NoteUpsert needs to
/// reconcile the tree (no-op when they already agree, preserving block
/// identity on ordinary re-saves).
fn tree_matches_blocks(
    tree: &LoroTree,
    blocks: &[tesela_core::note_tree::FlatBlock],
) -> bool {
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    if live.len() != blocks.len() {
        return false;
    }
    for (node, block) in live.iter().zip(blocks.iter()) {
        let id_ok = read_meta_str(tree, *node, "block_id").as_deref()
            == Some(hex::encode(block.id.as_bytes()).as_str());
        let text_ok = read_block_text(tree, *node).unwrap_or_default() == block.text;
        let indent_ok = read_indent_level(tree, *node).unwrap_or(0) == block.indent;
        if !(id_ok && text_ok && indent_ok) {
            return false;
        }
    }
    true
}

/// Delete every live block node from the tree (tombstones them). Used
/// before a reseed when a NoteUpsert body differs from the current tree.
fn clear_block_tree(tree: &LoroTree) {
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    for node in live {
        let _ = tree.delete(node);
    }
}

fn seed_tree_from_flatblocks(
    tree: &LoroTree,
    blocks: &[tesela_core::note_tree::FlatBlock],
) -> SyncResult<()> {
    for block in blocks {
        let node = tree
            .create(TreeParentId::Root)
            .map_err(|e| SyncError::Storage(format!("seed tree.create: {e}")))?;
        let meta = tree
            .get_meta(node)
            .map_err(|e| SyncError::Storage(format!("seed get_meta: {e}")))?;
        let block_hex = hex::encode(block.id.as_bytes());
        meta.insert("block_id", block_hex.as_str())
            .map_err(|e| SyncError::Storage(format!("seed meta insert: {e}")))?;
        write_block_text(&meta, block.text.as_str())?;
        meta.insert("indent_level", block.indent as i64)
            .map_err(|e| SyncError::Storage(format!("seed meta insert: {e}")))?;
        meta.insert(
            "parent",
            block
                .parent
                .map(|p| hex::encode(p.as_bytes()))
                .unwrap_or_default(),
        )
        .map_err(|e| SyncError::Storage(format!("seed meta insert: {e}")))?;
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

#[allow(dead_code)]
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

/// Create a fresh `blocks`-tree node under root, honoring an optional
/// positional `after_block_id` hint.
///
/// - `None` hint → `tree.create(Root)`: append at document end (the
///   historical behavior; what every receive-only path and every pre-hint
///   producer relies on).
/// - `Some(pred)` whose hex resolves to a LIVE root child → insert the new
///   node IMMEDIATELY AFTER it via `tree.create_at(Root, idx + 1)`, where
///   `idx` is the predecessor's position in `tree.children(Root)` (the same
///   live-child list `create_at` indexes into). This makes a mid-note
///   split's new half render adjacent to its sibling.
/// - `Some(pred)` that ISN'T a live node (already deleted, or never seen
///   on this replica) → fall back to append. Loss-free: the block is still
///   created and rendered; only its position degrades to end-of-document,
///   which is exactly today's behavior.
///
/// Determinism: `create_at` is a Loro movable-tree op. Two replicas that
/// apply the same `BlockUpsert` resolve `pred` to the same index (the tree
/// state is shared CRDT state) and call the same `create_at`, and Loro
/// merges two concurrent adjacent positional inserts to the same order on
/// every replica (verified by `positional_insert_*` tests). Fractional
/// index is enabled by default on a Loro tree (jitter 0), so `create_at`
/// needs no explicit `enable_fractional_index` call.
fn create_block_node_positioned(
    tree: &LoroTree,
    after_block_id: Option<&[u8; 16]>,
) -> SyncResult<TreeID> {
    let append = |tree: &LoroTree| {
        tree.create(TreeParentId::Root)
            .map_err(|e| SyncError::Storage(format!("loro tree create: {e}")))
    };
    let Some(pred_bytes) = after_block_id else {
        return append(tree);
    };
    let pred_hex = hex_id(pred_bytes);
    // Index of the predecessor among the LIVE root children — the same list
    // `create_at` indexes into (it counts live children). A tombstoned or
    // unknown predecessor yields `None` → append.
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    let pred_idx = live
        .iter()
        .position(|n| read_meta_str(tree, *n, "block_id").as_deref() == Some(pred_hex.as_str()));
    match pred_idx {
        Some(idx) => {
            // Insert immediately after the predecessor. `idx + 1` is in
            // bounds: idx < live.len(), and create_at accepts index ==
            // children_num (append). Should the count race (it can't here —
            // single-threaded apply under the doc lock), fall back to append.
            tree.create_at(TreeParentId::Root, idx + 1)
                .or_else(|_| append(tree))
        }
        None => append(tree),
    }
}

/// Collapse duplicate-`block_id` twins to a single canonical node, returning
/// the survivors in their original `nodes` walk order.
///
/// **Why this exists (the bug):** Loro tree node identity is the internal
/// `TreeID` (peer + counter), NOT the `block_id` meta. When two engines that
/// never shared a Loro base both author the same bid (e.g. the Mac server
/// seeds a note from disk while iOS re-authors blocks from its own markdown),
/// each mints a DIFFERENT `TreeID` for that bid. On merge Loro UNIONS the
/// nodes, so two live nodes carry the same `block_id` meta → the renderer
/// emits the block twice and block-diff saves update only one twin (leaving a
/// stale ghost = "my web edit reverted on refresh"). This dedups them.
///
/// **Tie-break rule — lexicographically-min `TreeID` (lower `peer`, then lower
/// `counter`).** loro 1.12's `TreeID` exposes public `peer: u64` / `counter:
/// i32` fields and derives `Ord` over `(peer, counter)`, so this is a stable,
/// process-restart-independent comparator. We deliberately do NOT use a
/// "most-recently-edited" rule: the `text` meta is a plain LWW map-register
/// (`meta.insert("text", ...)`), and loro 1.12's `LoroMap` only exposes
/// `get_last_editor(key) -> PeerID` — the *peer* that last wrote a key, not a
/// comparable per-update lamport/timestamp. `LoroTree::get_last_move_id`
/// reflects the last STRUCTURAL (create/move) op, not text-meta updates, so it
/// can't order twins by text recency either. With no reliable cross-peer
/// recency signal available, min-`TreeID` is the deterministic choice. It is
/// NOT recency-aware: in a disjoint merge it may keep a stale twin's text — so
/// the *true* convergence fix is giving the device the server's doc as a shared
/// base before it authors (then both sides resolve to the same `TreeID`). This
/// helper only guarantees no duplicate render + a deterministic survivor.
fn dedup_twins_by_block_id(tree: &LoroTree, nodes: Vec<TreeID>) -> Vec<TreeID> {
    // First pass: for each block_id, find the canonical (min-TreeID) survivor.
    let mut canonical: HashMap<String, TreeID> = HashMap::new();
    for node in &nodes {
        if let Some(hex) = read_meta_str(tree, *node, "block_id") {
            canonical
                .entry(hex)
                .and_modify(|kept| {
                    if node < kept {
                        *kept = *node;
                    }
                })
                .or_insert(*node);
        }
    }
    // Second pass: keep nodes in original walk order, emitting each block_id's
    // canonical survivor exactly once. Nodes with no block_id meta are kept
    // (they can't be twins by bid; preserve existing behavior).
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match read_meta_str(tree, node, "block_id") {
            Some(hex) => {
                if canonical.get(&hex) == Some(&node) {
                    out.push(node);
                }
            }
            None => out.push(node),
        }
    }
    out
}

/// Permanently tombstone every non-canonical duplicate-`block_id` twin in a
/// doc's `blocks` tree, committing if anything was deleted. This is the
/// persistent counterpart to the render-side heal in `note_tree_from_doc`:
/// after a peer's snapshot is imported (which unions same-bid twins), it
/// removes the strays from the doc itself so later block-diff saves can't
/// resurrect or update a ghost.
///
/// Uses the same min-`TreeID` survivor rule as `dedup_twins_by_block_id`, so
/// the survivor a render shows is the one that stays in the doc. Idempotent:
/// after one pass each bid has exactly one live node, so a re-import (which
/// merges identical state) finds nothing to delete and returns `false`
/// without committing. `note_id` is accepted for log/parity with the other
/// per-note helpers (the doc is already addressed).
fn tombstone_duplicate_twins(doc: &LoroDoc, _note_id: [u8; 16]) -> bool {
    let tree = doc.get_tree("blocks");
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    let kept = dedup_twins_by_block_id(&tree, live.clone());
    let mut deleted_any = false;
    for node in live {
        if !kept.contains(&node) {
            // Already-deleted nodes were filtered out above, so this only
            // hits live non-canonical twins; delete is safe (no double-free).
            if tree.delete(node).is_ok() {
                deleted_any = true;
            }
        }
    }
    if deleted_any {
        doc.commit();
    }
    deleted_any
}

/// Re-base same-bid twins onto the SERVER's lineage after an authoritative
/// snapshot import (the disjoint-device catch-up). Unlike
/// [`tombstone_duplicate_twins`], which resolves a twin by min-`TreeID` (peer
/// id, NOT authority), this KEEPS the twin that arrived FROM the snapshot — a
/// `TreeID` NOT in `pre_nodes`, the device's pre-import node set — and
/// TOMBSTONES the device's stale node, so the device truly re-bases onto the
/// server's lineage instead of clinging to its own.
///
/// Per twin block_id:
/// - keep the snapshot-origin node (min-`TreeID` among the nodes absent from
///   `pre_nodes`, if more than one arrived);
/// - if a twin bid has ONLY pre-existing nodes (the snapshot didn't include it,
///   so there's nothing to re-base onto), fall back to keeping the min-`TreeID`
///   among them (matching the dedup's deterministic survivor).
///
/// Block_ids with a single live node are untouched (no twin); block_ids only
/// the device has (no snapshot-origin node and no twin) are likewise untouched —
/// the device's genuine unsynced new blocks. Commits if anything was deleted.
fn rebase_twins_onto_snapshot(doc: &LoroDoc, pre_nodes: &std::collections::HashSet<TreeID>) {
    let tree = doc.get_tree("blocks");
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();

    // Group live nodes by block_id.
    let mut by_bid: HashMap<String, Vec<TreeID>> = HashMap::new();
    for node in &live {
        if let Some(hex) = read_meta_str(&tree, *node, "block_id") {
            by_bid.entry(hex).or_default().push(*node);
        }
    }

    let mut deleted_any = false;
    for (_bid, nodes) in by_bid {
        if nodes.len() < 2 {
            continue; // not a twin → nothing to re-base.
        }
        // Snapshot-origin nodes = those NOT present pre-import.
        let snapshot_origin: Vec<TreeID> = nodes
            .iter()
            .copied()
            .filter(|n| !pre_nodes.contains(n))
            .collect();
        let keep = if let Some(min) = snapshot_origin.iter().min().copied() {
            // Re-base onto the server's lineage: keep the snapshot-origin node.
            min
        } else {
            // No snapshot-origin node (snapshot didn't carry this bid): fall
            // back to the deterministic min-`TreeID` survivor.
            nodes.iter().min().copied().expect("len >= 2")
        };
        for node in nodes {
            if node != keep && tree.delete(node).is_ok() {
                deleted_any = true;
            }
        }
    }
    if deleted_any {
        doc.commit();
    }
}

/// A DISJOINT-twin block whose surviving node the import heal must force to a
/// resolved `text`, because the non-causal min-`TreeID` dedup can otherwise
/// keep the wrong twin. `text` is the RESOLVED correct value:
///
/// - the peer's GENUINE edit (a value the server never authored), when the
///   peer's twin holds a real new edit the dedup might discard; or
/// - the server's CURRENT (pre-import) value, when the peer's twin is a STALE
///   re-assertion (a value the server already moved past) that the dedup might
///   pick — restoring the server's edit the import reverted.
///
/// Only DISJOINT twins (>1 live node for the block_id) are emitted. A
/// SHARED-register block (single node) defers entirely to Loro's own LoroText
/// merge and is never force-healed.
struct PeerBlockChange {
    block_id: [u8; 16],
    text: String,
    indent: u16,
}

/// The server's per-`block_id` text HISTORY — every value the server ever held
/// for each block, across its own oplog. Used by the disjoint-twin
/// discriminator to tell a peer's GENUINE edit (a value the server never
/// authored) from a STALE re-assertion (a value the server already moved past,
/// e.g. the seed text the peer re-ships in a full snapshot).
///
/// Block text is now a [`LoroText`], so the per-write values can't be scraped
/// from `JsonMapOp::Insert` ops, and consecutive same-peer writes coalesce into
/// ONE change — so change-granular replay misses intermediate states. We replay
/// at OP granularity instead: export each single-counter id-span of the server
/// oplog as a binary update, import them in counter order into one scratch doc,
/// and snapshot every block's text after each step, accumulating the distinct
/// values seen. Bounded by the server's op count for this one note, on the
/// WS-apply path that already forks + diffs.
fn server_block_text_history(
    auth: &LoroDoc,
) -> HashMap<String, std::collections::HashSet<String>> {
    let mut history: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    let server_vv = auth.oplog_vv();
    let scratch = LoroDoc::new();
    // Walk every (peer, counter) the server holds, importing one op-step at a
    // time so each intermediate LoroText state (seed, then edit) is observed.
    // Collect the spans first so the order is per-peer ascending by counter.
    let mut spans: Vec<(u64, i32)> = server_vv
        .iter()
        .map(|(peer, counter)| (*peer, *counter))
        .collect();
    spans.sort();
    for (peer, end_counter) in spans {
        for c in 0..end_counter {
            let span = loro::IdSpan::new(peer, c, c + 1);
            let Ok(bytes) = auth.export(ExportMode::updates_in_range(&[span][..])) else {
                continue;
            };
            if scratch.import(&bytes).is_err() {
                continue;
            }
            for (bid, text) in current_block_texts(&scratch) {
                history.entry(bid).or_default().insert(text);
            }
        }
    }
    history
}

/// **The disjoint-twin heal discriminator** (2026-06-02, LoroText era). Given
/// the server's current authoritative doc + a peer's frame bytes, return for
/// every DISJOINT-twin block the RESOLVED text its surviving node must hold —
/// so the import heal can override the non-causal min-`TreeID` dedup, which
/// picks a twin by peer, not by recency.
///
/// ## Why this is scoped to disjoint twins
/// Block text is now a nested [`LoroText`] sequence CRDT, not an LWW map
/// register. On a SHARED Loro lineage (one node per block_id) the peer's frame
/// carries only its own LoroText splices, which MERGE with the server's — the
/// import neither reverts the server nor drops the peer's edit. So shared-
/// lineage blocks need NO heal and are skipped entirely (never override Loro's
/// merge).
///
/// The DISJOINT-twin case (the `project_multidevice_convergence` residue)
/// survives: when a block_id has >1 live node post-import the twins hold two
/// INDEPENDENT LoroTexts that Loro CANNOT merge. The dedup
/// ([`tombstone_duplicate_twins`] / [`dedup_twins_by_block_id`]) keeps one twin
/// arbitrarily, so it can BOTH:
///   - revert the server (keep the peer's STALE twin — the wire incident: the
///     device re-ships seed text "Awesome" after the server moved that block
///     to "Awesome sweet"), and
///   - drop the peer's GENUINE edit (keep the server's twin when the peer
///     authored a real new value).
/// The resolved target below covers both.
///
/// ## The discriminator
/// 1. `server_vv = auth.oplog_vv()`. Fork the auth doc, import the frame into
///    the fork (full-history clone — never touches the auth doc). Nothing
///    causally new → idempotent no-op.
/// 2. `twin_bids` = block_ids with >1 live node in the fork (disjoint twins).
///    None → nothing to heal.
/// 3. Per twin block_id, read its live twin nodes' texts (LoroText container,
///    fallback legacy register) and resolve the target against the server's
///    CURRENT value + its text HISTORY ([`server_block_text_history`]):
///    - a twin value that differs from the server's current AND is NOT in the
///      server's history → GENUINE peer edit the dedup might drop → target =
///      that peer value;
///    - otherwise (the peer re-asserted a value the server already authored and
///      moved past, or didn't diverge) → STALE → target = the server's current
///      value (restoring it if the dedup reverted the server to a stale twin).
///
/// Conservative + idempotent: the heal forces the target only when the post-
/// import survivor differs from it, so a re-applied frame is a no-op.
fn peer_genuine_block_changes(
    auth: &LoroDoc,
    frame: &[u8],
) -> SyncResult<Vec<PeerBlockChange>> {
    let server_vv = auth.oplog_vv();
    // Fork (full-history clone) so the import never touches the auth doc.
    let fork = auth.fork();
    fork.import(frame)
        .map_err(|e| SyncError::Storage(format!("fork import: {e}")))?;
    let fork_vv = fork.oplog_vv();
    // Nothing causally new from the peer → idempotent no-op.
    if fork_vv == server_vv {
        return Ok(Vec::new());
    }

    // block_ids with >1 live node in the imported fork = DISJOINT twins (the
    // peer authored on a separate lineage). ONLY these need a force-heal;
    // shared-lineage blocks defer entirely to Loro's LoroText merge.
    let twin_bids = duplicate_block_ids(&fork);
    if twin_bids.is_empty() {
        return Ok(Vec::new());
    }

    // Server's CURRENT value per block_id (the live tree, pre-import) — the
    // keep value — and its full text HISTORY, so a stale re-assertion of an old
    // server value keeps the server's edit instead of healing to the stale one.
    let server_current = current_block_texts(auth);
    let server_history = server_block_text_history(auth);

    // Collect each disjoint twin's candidate texts from the fork's twin nodes
    // (read via the LoroText container — no JsonOp scan; text is no longer an
    // LWW map register), keyed by block_id, with the node's indent.
    let fork_tree = fork.get_tree("blocks");
    let live: Vec<TreeID> = fork_tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(fork_tree.is_node_deleted(n), Ok(true)))
        .collect();
    let mut twin_texts: HashMap<String, Vec<(String, u16)>> = HashMap::new();
    for node in live {
        let Some(bid_hex) = read_meta_str(&fork_tree, node, "block_id") else {
            continue;
        };
        if !twin_bids.contains(&bid_hex) {
            continue; // shared-register block → Loro already merged it.
        }
        let text = read_block_text(&fork_tree, node).unwrap_or_default();
        let indent = read_indent_level(&fork_tree, node).unwrap_or(0);
        twin_texts.entry(bid_hex).or_default().push((text, indent));
    }

    // Resolve a single target per twin block_id.
    let mut out: Vec<PeerBlockChange> = Vec::new();
    for (bid_hex, candidates) in twin_texts {
        let Some(bid) = parse_note_id_from_hex(&bid_hex) else {
            continue;
        };
        let server_text = server_current.get(&bid_hex).map(|s| s.as_str());
        // A GENUINE peer edit = a twin value that differs from the server's
        // current value AND was never a value the server itself authored.
        let genuine = candidates.iter().find(|(t, _)| {
            server_text != Some(t.as_str())
                && !server_history
                    .get(&bid_hex)
                    .is_some_and(|h| h.contains(t))
        });
        let (text, indent) = match genuine {
            // The peer's genuine edit the dedup might drop → force the peer value.
            Some((t, ind)) => (t.clone(), *ind),
            // Stale re-assertion or no peer divergence → keep the server's value
            // (restores it if the dedup reverted the server to a stale twin).
            None => {
                let keep = server_current.get(&bid_hex).cloned().unwrap_or_default();
                let ind = candidates
                    .iter()
                    .find(|(t, _)| Some(t.as_str()) == server_text)
                    .or_else(|| candidates.first())
                    .map(|(_, i)| *i)
                    .unwrap_or(0);
                (keep, ind)
            }
        };
        out.push(PeerBlockChange {
            block_id: bid,
            text,
            indent,
        });
    }
    Ok(out)
}

/// The set of block_ids (hex) that have MORE THAN ONE live node in a doc's
/// `blocks` tree — disjoint-lineage twins. A block_id with a single live node
/// is a SHARED Loro register whose value Loro's LWW resolves authoritatively.
fn duplicate_block_ids(doc: &LoroDoc) -> std::collections::HashSet<String> {
    let tree = doc.get_tree("blocks");
    let mut counts: HashMap<String, u32> = HashMap::new();
    for node in tree.children(TreeParentId::Root).unwrap_or_default() {
        if matches!(tree.is_node_deleted(&node), Ok(true)) {
            continue;
        }
        if let Some(bid) = read_meta_str(&tree, node, "block_id") {
            *counts.entry(bid).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(b, _)| b)
        .collect()
}

/// Map each live block_id (hex) in a doc's `blocks` tree to its current text,
/// applying the same twin dedup the renderer uses so a corrupted-on-disk doc
/// still reports one value per block.
fn current_block_texts(doc: &LoroDoc) -> HashMap<String, String> {
    let tree = doc.get_tree("blocks");
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|n| !matches!(tree.is_node_deleted(n), Ok(true)))
        .collect();
    let mut out = HashMap::new();
    for node in dedup_twins_by_block_id(&tree, live) {
        if let Some(bid) = read_meta_str(&tree, node, "block_id") {
            let text = read_block_text(&tree, node).unwrap_or_default();
            out.insert(bid, text);
        }
    }
    out
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

    /// Trait-level override that forwards to the inherent
    /// `LoroEngine::render_note`. Lets `Arc<dyn SyncEngine>` callers
    /// (the server's HTTP routes) inspect the shadow without
    /// downcasting.
    async fn render_note(&self, note_id: [u8; 16]) -> Option<String> {
        LoroEngine::render_note(self, note_id).await
    }

    /// Trait-level override forwarding to the inherent
    /// `LoroEngine::render_note_full` (the full-file materialization
    /// dry-run surface).
    async fn render_note_full(&self, note_id: [u8; 16]) -> Option<String> {
        LoroEngine::render_note_full(self, note_id).await
    }

    async fn produce_relay_updates(&self) -> Vec<([u8; 16], Vec<u8>, Vec<u8>)> {
        LoroEngine::produce_relay_updates(self).await
    }

    async fn commit_broadcast_cursors(&self, committed: &[([u8; 16], Vec<u8>)]) {
        LoroEngine::commit_broadcast_cursors(self, committed).await
    }

    async fn apply_relay_updates(&self, updates: &[([u8; 16], Vec<u8>)]) -> usize {
        LoroEngine::apply_relay_updates(self, updates).await
    }

    /// Trait-level override forwarding to the inherent
    /// `LoroEngine::doc_version`. The live WS path (holding `dyn
    /// SyncEngine`) uses this to capture a note's pre-edit version vector.
    async fn doc_version(&self, note_id: [u8; 16]) -> Option<Vec<u8>> {
        LoroEngine::doc_version(self, note_id).await
    }

    /// Trait-level override forwarding to the inherent
    /// `LoroEngine::export_doc_update` — the cursor-free delta export the
    /// live WS path uses (does NOT touch the relay broadcast cursor).
    async fn export_doc_update(
        &self,
        note_id: [u8; 16],
        since: Option<&[u8]>,
    ) -> Option<Vec<u8>> {
        LoroEngine::export_doc_update(self, note_id, since).await
    }

    /// Trait-level override forwarding to the inherent
    /// `LoroEngine::import_doc_update` — applies one received delta.
    async fn import_doc_update(&self, note_id: [u8; 16], bytes: &[u8]) -> SyncResult<()> {
        LoroEngine::import_doc_update(self, note_id, bytes).await
    }

    /// Trait-level override forwarding to the inherent
    /// `LoroEngine::import_authoritative_snapshot` — the server-wins re-base.
    async fn import_authoritative_snapshot(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
    ) -> SyncResult<()> {
        LoroEngine::import_authoritative_snapshot(self, note_id, bytes).await
    }

    /// Trait-level override forwarding to the inherent
    /// `LoroEngine::apply_doc_update_status` — applies one received delta and
    /// reports whether Loro left it pending (causal gap).
    async fn apply_doc_update_status(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
    ) -> SyncResult<bool> {
        LoroEngine::apply_doc_update_status(self, note_id, bytes).await
    }

    /// Trait-level override forwarding to the inherent
    /// `LoroEngine::splice_block_text` — the character-level splice the FFI
    /// (holding `Arc<dyn SyncEngine>`) applies to one block's `text_seq`.
    async fn splice_block_text(
        &self,
        note_id: [u8; 16],
        block_id: [u8; 16],
        utf16_offset: u32,
        utf16_delete_len: u32,
        insert: &str,
    ) -> SyncResult<u32> {
        LoroEngine::splice_block_text(
            self,
            note_id,
            block_id,
            utf16_offset,
            utf16_delete_len,
            insert,
        )
        .await
    }

    /// `LoroEngine::read_block_text` — the inbound counterpart the FFI calls
    /// to read a block's merged text after applying a remote splice.
    async fn read_block_text(&self, note_id: [u8; 16], block_id: [u8; 16]) -> Option<String> {
        LoroEngine::read_block_text(self, note_id, block_id).await
    }

    async fn tracked_note_ids(&self) -> Vec<[u8; 16]> {
        self.note_ids().await
    }

    async fn index_entries(&self) -> Vec<crate::engine::IndexEntry> {
        LoroEngine::index_entries(self).await
    }
}

impl LoroEngine {
    /// Per-payload mutation shared between `record_local`,
    /// `apply_changes`, and `DualEngine`'s startup oplog replay.
    /// Replays a single `OpPayload` against the per-note Loro doc/tree.
    /// Unknown block ids on Move/Delete are silent no-ops — SqliteEngine
    /// carries canonical state and the shadow catches up when the next
    /// BlockUpsert reseeds the block.
    ///
    /// On successful apply, writes a per-note snapshot to disk if the
    /// engine was constructed with `with_snapshot_dir` — so the shadow
    /// survives process restart without re-replaying the oplog.
    pub async fn apply_payload(&self, payload: &OpPayload) -> SyncResult<()> {
        // For an authoritative NoteDelete, resolve the slug BEFORE the
        // inner apply drops the doc + index entry — afterwards
        // `slug_for_note` can't find it, so a NoteDelete whose op carries
        // no `display_alias` would orphan the `.md` file (review finding,
        // 2026-05-29). Prefer the op's alias; fall back to the resident
        // doc/index slug.
        let delete_slug: Option<String> = if self.inner.materialize_dir.is_some() {
            match payload {
                OpPayload::NoteDelete {
                    note_id,
                    display_alias,
                } => display_alias
                    .clone()
                    .or(self.slug_for_note(*note_id).await),
                _ => None,
            }
        } else {
            None
        };
        let touched_note = self.apply_payload_inner(payload).await?;
        if let (Some(dir), Some(note_id)) = (self.inner.snapshot_dir.as_ref(), touched_note) {
            self.save_snapshot(dir, note_id).await;
            // The index only changes on note create/delete; persist it
            // then (cheap, infrequent).
            if matches!(
                payload,
                OpPayload::NoteUpsert { .. } | OpPayload::NoteDelete { .. }
            ) {
                self.save_index_snapshot(dir).await;
            }
        }
        // Authoritative-writer materialization: write (or delete) the
        // `<slug>.md` file so disk reflects the CRDT. No-op unless
        // `materialize_dir` is set. NoteDelete removes the file (its doc
        // is already gone, so render returns None) using the slug the op
        // carries; all other ops re-render the touched note.
        if self.inner.materialize_dir.is_some() {
            match payload {
                OpPayload::NoteDelete { .. } => {
                    if let Some(slug) = delete_slug {
                        self.remove_materialized(&slug).await;
                    }
                }
                _ => {
                    if let Some(note_id) = touched_note {
                        self.materialize_note(note_id).await;
                    }
                }
            }
        }
        Ok(())
    }

    /// Persist the index doc to `<dir>/_index.bin`. Best-effort.
    async fn save_index_snapshot(&self, dir: &Path) {
        let bytes = match self.inner.index.export(ExportMode::Snapshot) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("tesela-sync/loro: index snapshot export: {e}");
                return;
            }
        };
        let path = dir.join("_index.bin");
        let tmp = unique_tmp(&path);
        if tokio::fs::write(&tmp, &bytes).await.is_ok() {
            if tokio::fs::rename(&tmp, &path).await.is_err() {
                let _ = tokio::fs::remove_file(&tmp).await;
            }
        } else {
            let _ = tokio::fs::remove_file(&tmp).await;
        }
    }

    /// Inner per-payload apply that returns the affected note_id (so
    /// the public wrapper knows which snapshot to refresh). Returns
    /// `None` for ops that don't touch a single note (AttachmentUpsert,
    /// no-op cases) — those don't trigger a snapshot write.
    async fn apply_payload_inner(&self, payload: &OpPayload) -> SyncResult<Option<[u8; 16]>> {
        let touched = match payload {
            OpPayload::NoteUpsert {
                note_id,
                content,
                title,
                display_alias,
                ..
            } => {
                let doc = self.doc_for_note_mut(*note_id).await;
                // Root meta makes the per-note doc SELF-DESCRIBING:
                // frontmatter (verbatim), slug, title. The body is NOT
                // duplicated here — it lives in the "blocks" tree, and the
                // full markdown is reconstructed on demand
                // (`doc_full_markdown`). Storing the whole content on root
                // meta doubled every snapshot (a 1.3 MB page → +1.3 MB of
                // redundant history that pushed it past the relay's body
                // limit); the lean schema keeps snapshots ~half the size.
                // This lets the index still be rebuilt purely from per-note
                // docs (no dependence on a prior index) — what makes the
                // index self-healing across schema changes.
                let root_meta = doc.get_map("root");
                let frontmatter = tesela_core::note_tree::parse_note(content)
                    .frontmatter
                    .unwrap_or_default();
                root_meta
                    .insert("frontmatter", frontmatter.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro insert: {e}")))?;
                root_meta
                    .insert("slug", display_alias.as_deref().unwrap_or(""))
                    .map_err(|e| SyncError::Storage(format!("loro insert: {e}")))?;
                root_meta
                    .insert("title", title.as_str())
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
                let parsed = tesela_core::note_tree::parse_note(content);
                // Index spine: note_id → {title, slug, tags, links},
                // derived from content + page properties.
                self.index_upsert(
                    *note_id,
                    display_alias.as_deref(),
                    title,
                    content,
                    &parsed.page_properties,
                );
                // Page properties are authoritative from the full
                // content and overwritten wholesale on every NoteUpsert
                // (they only arrive via full-content ops, never block
                // ops). Stored as an ordered list so render preserves
                // their on-disk order deterministically.
                set_page_properties(&doc, &parsed.page_properties)?;
                // Reconcile the block tree to the parsed body. A
                // full-content NoteUpsert is authoritative for the whole
                // note (SqliteEngine overwrites the entire file —
                // sqlite_engine.rs materialize). If the current tree
                // already matches the body (the common no-op re-save),
                // this is a fast no-op that PRESERVES block identity.
                // When they differ — drift recovery, or a full-content
                // rewrite the block-granular diff didn't capture — we
                // reseed so the shadow matches the body exactly, instead
                // of leaving stale blocks. Without this a drifted shadow
                // never self-heals even when the user re-saves the whole
                // note (review finding [2]).
                let tree = doc.get_tree("blocks");
                if !tree_matches_blocks(&tree, &parsed.blocks) {
                    clear_block_tree(&tree);
                    seed_tree_from_flatblocks(&tree, &parsed.blocks)?;
                }
                doc.commit();
                // Register this note's blocks in the block_index.
                let block_ids: Vec<[u8; 16]> =
                    parsed.blocks.iter().map(|b| *b.id.as_bytes()).collect();
                self.register_note_blocks(*note_id, &block_ids).await;
                Some(*note_id)
            }
            OpPayload::BlockUpsert {
                block_id,
                note_id,
                parent_block_id,
                order_key: _,
                indent_level,
                text,
                after_block_id,
            } => {
                // Flat model: every block is a direct child of root in
                // document (render) order. `indent_level` (from the op)
                // carries the visual hierarchy; `order_key` is ignored for
                // placement. Existing blocks update text/indent in place
                // WITHOUT moving (an upsert never reorders).
                //
                // New blocks: when the op carries an `after_block_id`
                // positional hint, the new node is created IMMEDIATELY
                // AFTER that predecessor via `create_at(Root, idx + 1)`, so
                // a mid-note split's new half lands adjacent to its sibling
                // instead of at document end (the historical
                // append-at-end behavior that scattered mid-note inserts
                // and stranded trailing empties). `after_block_id == None`,
                // or a predecessor that isn't a live node, falls back to
                // `create(Root)` (append) — exactly the old behavior, so
                // every pre-hint producer and every receive-only path is
                // unchanged. `create_at` is a Loro movable-tree op: two
                // devices applying the same positional insert (and two
                // concurrent adjacent inserts) merge to the same
                // deterministic order on every replica.
                //
                // `parent_block_id` is recorded in meta (NOT used for tree
                // placement) so BlockDelete can reparent a deleted block's
                // direct children, matching SqliteEngine (review finding
                // [1]).
                let doc = self.doc_for_note_mut(*note_id).await;
                let tree = doc.get_tree("blocks");
                let block_hex = hex_id(block_id);
                let node = match find_node_by_block_id(&tree, &block_hex) {
                    Some(existing) => existing,
                    None => create_block_node_positioned(&tree, after_block_id.as_ref())?,
                };
                let meta = tree
                    .get_meta(node)
                    .map_err(|e| SyncError::Storage(format!("loro get_meta: {e}")))?;
                meta.insert("block_id", block_hex.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                write_block_text(&meta, text.as_str())?;
                meta.insert("indent_level", *indent_level as i64)
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                meta.insert(
                    "parent",
                    parent_block_id.map(|p| hex_id(&p)).unwrap_or_default(),
                )
                .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                doc.commit();
                self.register_note_blocks(*note_id, &[*block_id]).await;
                Some(*note_id)
            }
            OpPayload::BlockMove {
                block_id,
                new_parent,
                new_order_key: _,
            } => {
                let Some((note_id, doc, node)) = self.find_doc_for_block(block_id).await
                else {
                    // We never saw the prior BlockUpsert (e.g. the
                    // engine started after the block was created).
                    // SqliteEngine handles it; LoroEngine catches up
                    // when the next BlockUpsert for this block lands.
                    tracing::debug!(
                        "tesela-sync/loro: BlockMove for unknown block {}",
                        hex_id(block_id)
                    );
                    return Ok(None);
                };
                // Flat model: a move only changes the block's indent, NOT
                // its document position — exactly what SqliteEngine's
                // apply_block_move does (it recomputes indent =
                // parent.indent + 1 and leaves the block at its file
                // position). So we DON'T reparent the tree node; we just
                // recompute and update the indent_level meta.
                let tree = doc.get_tree("blocks");
                let new_indent = match new_parent {
                    None => 0u16,
                    Some(p) => find_node_by_block_id(&tree, &hex_id(p))
                        .and_then(|pn| read_indent_level(&tree, pn))
                        .map(|i| i + 1)
                        .unwrap_or(0),
                };
                let meta = tree
                    .get_meta(node)
                    .map_err(|e| SyncError::Storage(format!("loro get_meta: {e}")))?;
                meta.insert("indent_level", new_indent as i64)
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                meta.insert(
                    "parent",
                    new_parent.map(|p| hex_id(&p)).unwrap_or_default(),
                )
                .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                doc.commit();
                Some(note_id)
            }
            OpPayload::BlockDelete { block_id } => {
                let Some((note_id, doc, node)) = self.find_doc_for_block(block_id).await
                else {
                    tracing::debug!(
                        "tesela-sync/loro: BlockDelete for unknown block {}",
                        hex_id(block_id)
                    );
                    return Ok(None);
                };
                let tree = doc.get_tree("blocks");
                // Match SqliteEngine::apply_block_delete: reparent the
                // deleted block's DIRECT children to top level
                // (parent=none, indent=0) before removing it. Grandchildren
                // keep their indent. Without this the flat-model children
                // keep their deeper indent and the shadow diverges from
                // disk on every parent-with-children delete (finding [1]).
                let deleted_hex = hex_id(block_id);
                for sib in tree.nodes() {
                    if matches!(tree.is_node_deleted(&sib), Ok(true)) {
                        continue;
                    }
                    if read_meta_str(&tree, sib, "parent").as_deref() == Some(&deleted_hex) {
                        if let Ok(m) = tree.get_meta(sib) {
                            let _ = m.insert("indent_level", 0i64);
                            let _ = m.insert("parent", "");
                        }
                    }
                }
                tree.delete(node)
                    .map_err(|e| SyncError::Storage(format!("loro tree delete: {e}")))?;
                doc.commit();
                Some(note_id)
            }
            OpPayload::NoteDelete { note_id, .. } => {
                // Drop the per-note doc entirely. SqliteEngine removes
                // the on-disk file in its materialize step; the shadow
                // needs to forget the doc so render_note returns None
                // and the divergence check matches PrimaryMissing.
                // The outer wrapper sees `save_snapshot(note_id)` find
                // the doc missing and removes the .bin file too.
                self.index_remove(*note_id);
                let mut docs = self.inner.docs.write().await;
                docs.remove(note_id);
                Some(*note_id)
            }
            OpPayload::AttachmentUpsert { .. } | OpPayload::AttachmentDelete { .. } => {
                // Attachments don't affect the rendered markdown body
                // (bytes flow out-of-band via the blob store; ops carry
                // metadata only). Divergence check compares rendered
                // markdown, so no shadow state change is needed. Kept
                // as an explicit arm rather than a wildcard so future
                // op types are caught by the compiler.
                None
            }
            OpPayload::BlockPropertySet {
                note_id,
                block_id,
                key,
                value,
            } => {
                // Dedicated property op: properties live in their OWN `props`
                // + ordered `prop_keys` containers on the block node's meta,
                // so they merge INDEPENDENTLY of the block's `text_seq` prose
                // (a concurrent prose splice and a property set don't clobber
                // each other). Resolve the per-note doc, find the node; a
                // property set on a block we've never seen is a SAFE NO-OP
                // (matches BlockMove/BlockDelete on a missing block —
                // SqliteEngine carries canonical state and the shadow catches
                // up when the next BlockUpsert reseeds the block).
                let doc = self.doc_for_note_mut(*note_id).await;
                let tree = doc.get_tree("blocks");
                let block_hex = hex_id(block_id);
                let Some(node) = find_node_by_block_id(&tree, &block_hex) else {
                    tracing::debug!(
                        "tesela-sync/loro: BlockPropertySet for unknown block {block_hex}"
                    );
                    return Ok(None);
                };
                let meta = tree
                    .get_meta(node)
                    .map_err(|e| SyncError::Storage(format!("loro get_meta: {e}")))?;
                let (props, prop_keys) = prop_containers::node_prop_containers(&meta)?;
                apply_prop_op(&props, &prop_keys, key, value)?;
                doc.commit();
                self.register_note_blocks(*note_id, &[*block_id]).await;
                Some(*note_id)
            }
            OpPayload::PagePropertySet {
                note_id,
                key,
                value,
            } => {
                // Page-level property: lives in `props`/`prop_keys` at the doc
                // ROOT. Same independent-merge guarantee as block props.
                let doc = self.doc_for_note_mut(*note_id).await;
                let (props, prop_keys) = prop_containers::page_prop_containers(&doc);
                apply_prop_op(&props, &prop_keys, key, value)?;
                doc.commit();
                Some(*note_id)
            }
        };

        Ok(touched)
    }
}

/// Dispatch a [`PropOp`] onto a resolved (`props`, `prop_keys`) container
/// pair via the `prop_containers` helpers. `prop_keys` maintenance lives in
/// the helpers (the apply arm, never the wire).
fn apply_prop_op(
    props: &loro::LoroMap,
    prop_keys: &loro::LoroList,
    key: &str,
    value: &PropOp,
) -> SyncResult<()> {
    match value {
        PropOp::SetScalar(s) => prop_containers::prop_set_scalar(props, prop_keys, key, s),
        PropOp::SetText(t) => prop_containers::prop_set_text(props, prop_keys, key, t),
        PropOp::AddToList(s) => prop_containers::prop_add_to_list(props, prop_keys, key, s),
        PropOp::RemoveFromList(s) => prop_containers::prop_remove_from_list(props, key, s),
        PropOp::Clear => prop_containers::prop_clear(props, prop_keys, key),
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
                after_block_id: None,
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
                after_block_id: None,
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

    /// Helper: record a top-level BlockUpsert with an optional positional
    /// hint. Returns nothing — the caller renders to assert order.
    async fn upsert_block(
        engine: &LoroEngine,
        note_id: [u8; 16],
        block_id: [u8; 16],
        text: &str,
        after_block_id: Option<[u8; 16]>,
    ) {
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id,
                note_id,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: text.into(),
                after_block_id,
            })
            .await
            .unwrap();
    }

    /// Render a note's blocks as their texts in document (render) order —
    /// strips the bid markers so order is the only thing under test.
    async fn block_texts(engine: &LoroEngine, note_id: [u8; 16]) -> Vec<String> {
        let rendered = engine.render_note(note_id).await.unwrap();
        rendered
            .lines()
            .filter_map(|l| {
                let t = l.trim_start().trim_start_matches("- ");
                let t = t.split(" <!-- bid:").next().unwrap_or(t).trim();
                (!t.is_empty()).then(|| t.to_string())
            })
            .collect()
    }

    // A new block with an `after_block_id` hint lands ADJACENT to its
    // predecessor (between it and the old next block), NOT at document end.
    // This is the headline fix: a mid-note split's new half renders in
    // place instead of scattering to the bottom.
    #[tokio::test]
    async fn positional_insert_lands_adjacent() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [40u8; 16];
        let a = [41u8; 16];
        let c = [42u8; 16];
        let b = [43u8; 16];

        // Seed A, C (append).
        upsert_block(&engine, note_id, a, "A", None).await;
        upsert_block(&engine, note_id, c, "C", None).await;
        assert_eq!(block_texts(&engine, note_id).await, vec!["A", "C"]);

        // Insert B AFTER A → expect A, B, C (not A, C, B).
        upsert_block(&engine, note_id, b, "B", Some(a)).await;
        assert_eq!(
            block_texts(&engine, note_id).await,
            vec!["A", "B", "C"],
            "new block with after-hint must land adjacent, not at end"
        );
    }

    // Backward compatibility: a BlockUpsert with NO positional hint appends
    // at document end — exactly today's behavior. Receive-only devices and
    // pre-hint producers depend on this.
    #[tokio::test]
    async fn positional_insert_no_hint_appends() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [44u8; 16];
        let a = [45u8; 16];
        let c = [46u8; 16];
        let b = [47u8; 16];

        upsert_block(&engine, note_id, a, "A", None).await;
        upsert_block(&engine, note_id, c, "C", None).await;
        // No hint → append at end.
        upsert_block(&engine, note_id, b, "B", None).await;
        assert_eq!(block_texts(&engine, note_id).await, vec!["A", "C", "B"]);
    }

    // An `after_block_id` that doesn't resolve to a live node (the engine
    // never saw the predecessor, or it was deleted) falls back to append.
    // Loss-free: the block is still created and rendered; only its position
    // degrades to end-of-document (today's behavior).
    #[tokio::test]
    async fn positional_insert_unknown_predecessor_appends() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [48u8; 16];
        let a = [49u8; 16];
        let b = [50u8; 16];
        let ghost = [99u8; 16]; // never created

        upsert_block(&engine, note_id, a, "A", None).await;
        upsert_block(&engine, note_id, b, "B", Some(ghost)).await;
        // Ghost predecessor → append; B still present, at end.
        assert_eq!(block_texts(&engine, note_id).await, vec!["A", "B"]);
    }

    // Insert-at-top: `after_block_id == None` appends, but a hint pointing
    // at the FIRST block puts the new block second. (Top-of-document insert
    // is exercised by the diff path's pos==0 → None = append for a fresh
    // note; an explicit top insert in an existing note is rare and falls to
    // append, which is loss-free.)
    #[tokio::test]
    async fn positional_insert_after_first_is_second() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [51u8; 16];
        let a = [52u8; 16];
        let b = [53u8; 16];
        let x = [54u8; 16];

        upsert_block(&engine, note_id, a, "A", None).await;
        upsert_block(&engine, note_id, b, "B", None).await;
        upsert_block(&engine, note_id, x, "X", Some(a)).await; // after A
        assert_eq!(block_texts(&engine, note_id).await, vec!["A", "X", "B"]);
    }

    // CONVERGENCE: two engines that share a base (A, C) each insert a
    // DIFFERENT new block at the SAME adjacent position (after A),
    // concurrently. Cross-importing their updates must converge to the SAME
    // deterministic order on BOTH engines — no divergence, no panic. This
    // is the load-bearing CRDT invariant for `create_at`.
    #[tokio::test]
    async fn positional_insert_concurrent_converges() {
        let note_id = [55u8; 16];
        let a = [56u8; 16];
        let c = [57u8; 16];
        let b1 = [58u8; 16];
        let b2 = [59u8; 16];

        // Engine 1 builds the shared base A, C.
        let dev1 = DeviceId::from_bytes([0xd1; 16]);
        let e1 = LoroEngine::new(dev1, Arc::new(Hlc::new(dev1)));
        upsert_block(&e1, note_id, a, "A", None).await;
        upsert_block(&e1, note_id, c, "C", None).await;

        // Engine 2 imports the base so both share history (same TreeIDs for
        // A and C — the convergence precondition the cutover relies on).
        let dev2 = DeviceId::from_bytes([0xd2; 16]);
        let e2 = LoroEngine::new(dev2, Arc::new(Hlc::new(dev2)));
        let base = e1.export_doc_update(note_id, None).await.unwrap();
        e2.import_doc_update(note_id, &base).await.unwrap();
        assert_eq!(block_texts(&e2, note_id).await, vec!["A", "C"]);

        // Concurrent adjacent inserts: e1 inserts B1 after A, e2 inserts B2
        // after A — neither has seen the other yet.
        upsert_block(&e1, note_id, b1, "B1", Some(a)).await;
        upsert_block(&e2, note_id, b2, "B2", Some(a)).await;

        // Cross-import both directions.
        let u1 = e1.export_doc_update(note_id, None).await.unwrap();
        let u2 = e2.export_doc_update(note_id, None).await.unwrap();
        e2.import_doc_update(note_id, &u1).await.unwrap();
        e1.import_doc_update(note_id, &u2).await.unwrap();

        let t1 = block_texts(&e1, note_id).await;
        let t2 = block_texts(&e2, note_id).await;
        assert_eq!(t1, t2, "engines diverged after concurrent positional insert");
        // Both new blocks survive, A first and C last (the inserts went
        // between them).
        assert_eq!(t1.first().map(String::as_str), Some("A"));
        assert_eq!(t1.last().map(String::as_str), Some("C"));
        assert!(t1.contains(&"B1".to_string()) && t1.contains(&"B2".to_string()));
        assert_eq!(t1.len(), 4);
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
                    after_block_id: None,
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
                    after_block_id: None,
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
    async fn render_uses_insertion_order_ignoring_order_key() {
        // SqliteEngine renders by document/insertion order and ignores
        // order_key entirely (apply_block_move's new_order_key param is
        // unused). The shadow must match: blocks render in creation
        // order regardless of the order_key carried on the op.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [70u8; 16];

        for (id, order, text) in [
            ([70u8; 16], "a5", "created first"),
            ([71u8; 16], "a0", "created second"),
            ([72u8; 16], "ar", "created third"),
        ] {
            engine
                .record_local(OpPayload::BlockUpsert {
                    block_id: id,
                    note_id,
                    parent_block_id: None,
                    order_key: order.into(),
                    indent_level: 0,
                    text: text.into(),
                    after_block_id: None,
                })
                .await
                .unwrap();
        }

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(
            rendered,
            "- created first <!-- bid:46464646-4646-4646-4646-464646464646 -->\n\
             - created second <!-- bid:47474747-4747-4747-4747-474747474747 -->\n\
             - created third <!-- bid:48484848-4848-4848-4848-484848484848 -->\n"
        );
    }

    #[tokio::test]
    async fn block_move_changes_indent_not_position() {
        // Reproduces the 2026-05-28 nursery-rhyme divergence: a move
        // must change only the block's indent, never its document
        // position — matching SqliteEngine.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [73u8; 16];
        let a = [0xa0; 16];
        let b = [0xb0; 16];
        let c = [0xc0; 16];

        // Create three flat top-level blocks: a, b, c.
        for (id, text) in [(a, "a"), (b, "b"), (c, "c")] {
            engine
                .record_local(OpPayload::BlockUpsert {
                    block_id: id,
                    note_id,
                    parent_block_id: None,
                    order_key: "x".into(),
                    indent_level: 0,
                    text: text.into(),
                    after_block_id: None,
                })
                .await
                .unwrap();
        }
        // Move c under a. SqliteEngine would set c.indent = a.indent+1
        // = 1 and leave c at document position 3 (last). Order stays
        // a, b, c; only c's indent changes.
        engine
            .record_local(OpPayload::BlockMove {
                block_id: c,
                new_parent: Some(a),
                new_order_key: "y".into(),
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(
            rendered,
            "- a <!-- bid:a0a0a0a0-a0a0-a0a0-a0a0-a0a0a0a0a0a0 -->\n\
             - b <!-- bid:b0b0b0b0-b0b0-b0b0-b0b0-b0b0b0b0b0b0 -->\n  \
             - c <!-- bid:c0c0c0c0-c0c0-c0c0-c0c0-c0c0c0c0c0c0 -->\n"
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
                after_block_id: None,
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
                after_block_id: None,
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
    async fn snapshot_round_trip_survives_reload() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");

        // First engine — write a block + verify snapshot file lands.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine =
            LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
                .await
                .unwrap();
        let note_id = [0xee; 16];
        let block = [0xff; 16];
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: block,
                note_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: "persisted".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        drop(engine);

        // Second engine — points at the same dir, loads snapshot,
        // render should match without replaying any oplog ops.
        let hlc2 = Arc::new(Hlc::new(test_device()));
        let reloaded =
            LoroEngine::with_snapshot_dir(test_device(), hlc2, dir.clone())
                .await
                .unwrap();
        assert_eq!(reloaded.note_count().await, 1);
        let rendered = reloaded.render_note(note_id).await.unwrap();
        assert_eq!(
            rendered,
            "- persisted <!-- bid:ffffffff-ffff-ffff-ffff-ffffffffffff -->\n"
        );
    }

    #[tokio::test]
    async fn note_upsert_after_snapshot_load_does_not_duplicate_blocks() {
        // Regression: a NoteUpsert re-save of the SAME (stamped) content
        // after a snapshot reload must be a no-op — no duplicate nodes
        // AND stable block identity (the tree_matches_blocks fast path).
        // Content carries bid markers, as a real note file does after
        // its first write (unstamped content would mint fresh ids each
        // parse, which is not a realistic re-save).
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");

        let hlc = Arc::new(Hlc::new(test_device()));
        let engine =
            LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
                .await
                .unwrap();
        let note_id = [0x10; 16];
        let content = "---\ntitle: T\n---\n- a <!-- bid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa -->\n- b <!-- bid:bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb -->\n";

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("t".into()),
                title: "T".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let after_first = engine.render_note(note_id).await.unwrap();
        drop(engine);

        // Reload from snapshot, then re-fire NoteUpsert with same body.
        let hlc2 = Arc::new(Hlc::new(test_device()));
        let reloaded =
            LoroEngine::with_snapshot_dir(test_device(), hlc2, dir)
                .await
                .unwrap();
        reloaded
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("t".into()),
                title: "T".into(),
                content: content.into(),
                created_at_millis: 2,
            })
            .await
            .unwrap();
        let after_second = reloaded.render_note(note_id).await.unwrap();

        assert_eq!(
            after_first, after_second,
            "second NoteUpsert after snapshot load must not duplicate blocks"
        );
    }

    #[tokio::test]
    async fn corrupt_snapshot_skipped_on_load() {
        // Write a garbage .bin file with a valid-looking hex name.
        // Load should warn + skip without panicking, and the engine
        // should still be functional.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let bad_id = [0xab; 16];
        let bad_path = dir.join(format!("{}.bin", hex::encode(bad_id)));
        tokio::fs::write(&bad_path, b"this is not a Loro snapshot")
            .await
            .unwrap();

        let hlc = Arc::new(Hlc::new(test_device()));
        let engine =
            LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
                .await
                .unwrap();
        // Corrupt note didn't load; engine works for a fresh note.
        assert_eq!(engine.note_count().await, 0);

        let good_id = [0xcd; 16];
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: [0xef; 16],
                note_id: good_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: "still works".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        assert!(engine.render_note(good_id).await.is_some());
    }

    #[tokio::test]
    async fn snapshot_dir_created_when_missing() {
        // Construct with a path that doesn't exist yet — should be
        // created, not error.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro").join("nested").join("path");
        assert!(!dir.exists());

        let hlc = Arc::new(Hlc::new(test_device()));
        let engine =
            LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
                .await
                .unwrap();
        assert!(dir.exists());
        assert_eq!(engine.note_count().await, 0);
    }

    #[tokio::test]
    async fn snapshot_deleted_on_note_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine =
            LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
                .await
                .unwrap();
        let note_id = [0xdd; 16];

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
        let path = dir.join(format!("{}.bin", hex::encode(note_id)));
        assert!(path.exists(), "snapshot should land for new note");

        engine
            .record_local(OpPayload::NoteDelete {
                note_id,
                display_alias: Some("doomed".into()),
            })
            .await
            .unwrap();
        assert!(!path.exists(), "snapshot should be removed on NoteDelete");
    }

    #[tokio::test]
    async fn note_upsert_renders_page_properties() {
        // A page-property-only note (query page) must round-trip its
        // properties through the shadow — previously rendered empty.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x5a; 16];
        let content = "---\ntitle: Saved\n---\n\nquery:: kind:page\nsort:: modified desc\n";

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("saved".into()),
                title: "Saved".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        // render_note omits frontmatter (lives on disk, not the shadow);
        // page properties render in order.
        assert_eq!(rendered, "query:: kind:page\nsort:: modified desc\n");
    }

    #[tokio::test]
    async fn render_note_full_includes_frontmatter() {
        // The cutover dry-run surface: render_note_full must reproduce the
        // verbatim frontmatter (from the doc's stored content) + body, so
        // it equals what materialization would write to disk. For a note
        // whose source is itself canonical, this round-trips byte-for-byte.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x7f; 16];
        let content = "---\ntitle: Full\n---\n\n- hello <!-- bid:00000000-0000-0000-0000-000000000001 -->\n";

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("full".into()),
                title: "Full".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // render_note (body only) drops the frontmatter…
        let body = engine.render_note(note_id).await.unwrap();
        assert!(
            !body.starts_with("---"),
            "render_note must omit frontmatter, got: {body:?}"
        );
        // …render_note_full prepends it back, byte-identical to the source.
        let full = engine.render_note_full(note_id).await.unwrap();
        assert_eq!(full, content, "render_note_full should reproduce source");
        assert!(
            full.starts_with("---\ntitle: Full\n---\n"),
            "render_note_full must carry frontmatter, got: {full:?}"
        );
    }

    #[tokio::test]
    async fn note_upsert_stores_lean_frontmatter_not_full_content() {
        // The dedup invariant: a NoteUpsert must NOT duplicate the body onto
        // root meta. Storing the full markdown there doubled every snapshot
        // (a 1.3 MB page → +1.3 MB of redundant history past the relay's body
        // limit). The body lives only in the tree; root carries just the
        // verbatim frontmatter, and the full markdown is reconstructed on
        // demand. tags (frontmatter) + links (body) must still index from the
        // reconstruction — proving nothing was lost by not storing content.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x6c; 16];
        let content =
            "---\ntitle: Lean\ntags: [alpha]\n---\n\n- see [[target]] #beta <!-- bid:00000000-0000-0000-0000-00000000000a -->\n";

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("lean".into()),
                title: "Lean".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        {
            let docs = engine.inner.docs.read().await;
            let root = docs.get(&note_id).unwrap().get_map("root");
            assert!(
                root.get("content").is_none(),
                "lean schema must not store full content on root meta"
            );
            assert_eq!(
                root.get("frontmatter")
                    .and_then(|v| v.into_value().ok())
                    .and_then(|v| v.into_string().ok())
                    .map(|s| (*s).clone()),
                Some("---\ntitle: Lean\ntags: [alpha]\n---\n".to_string()),
                "verbatim frontmatter stored on root meta"
            );
        }

        // Reconstruction round-trips the source byte-for-byte…
        let full = engine.render_note_full(note_id).await.unwrap();
        assert_eq!(full, content, "render_note_full reconstructs from the tree");

        // …and the index still derives the frontmatter tag + body tag/link
        // from the reconstruction (not from a stored copy of content).
        let entry = engine
            .index_entries()
            .await
            .into_iter()
            .find(|e| e.note_id == hex_id(&note_id))
            .unwrap();
        assert!(entry.tags.contains(&"alpha".to_string()), "frontmatter tag: {:?}", entry.tags);
        assert!(entry.tags.contains(&"beta".to_string()), "inline body tag: {:?}", entry.tags);
        assert_eq!(entry.links, vec!["target".to_string()], "body link indexed");
    }

    #[tokio::test]
    async fn blank_blocks_are_kept_as_editing_surface() {
        // Blank bullets are KEPT (reverted the 2026-05-29 drop): the web
        // outliner needs a trailing empty bullet as the focusable editing
        // surface for "empty" days. A note with a real block + a blank one
        // round-trips BOTH.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x4b; 16];
        let content = "- real <!-- bid:aaaaaaaa-0000-0000-0000-000000000001 -->\n-  <!-- bid:aaaaaaaa-0000-0000-0000-000000000002 -->\n";
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("b".into()),
                title: "B".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let rendered = engine.render_note(note_id).await.unwrap();
        assert!(
            rendered.contains("- real ") && rendered.contains("000000000002"),
            "both real and blank blocks kept: {rendered:?}"
        );
    }

    #[tokio::test]
    async fn render_note_full_body_only_when_no_frontmatter() {
        // A note whose content never carried frontmatter materializes
        // body-only — render_note_full == render_note in that case.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x80; 16];
        let content = "- bare <!-- bid:00000000-0000-0000-0000-000000000002 -->\n";

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("bare".into()),
                title: "bare".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        let body = engine.render_note(note_id).await.unwrap();
        let full = engine.render_note_full(note_id).await.unwrap();
        assert_eq!(full, body, "no frontmatter → full equals body");
        assert_eq!(full, content);
    }

    #[tokio::test]
    async fn index_doc_tracks_notes() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: [1u8; 16],
                display_alias: Some("alpha".into()),
                title: "Alpha".into(),
                content: "- a\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: [2u8; 16],
                display_alias: Some("beta".into()),
                title: "Beta".into(),
                content: "- b\n".into(),
                created_at_millis: 2,
            })
            .await
            .unwrap();

        let entries = engine.index_entries().await;
        assert_eq!(entries.len(), 2);
        let titles: Vec<_> = entries.iter().map(|e| e.title.as_str()).collect();
        assert!(titles.contains(&"Alpha"));
        assert!(titles.contains(&"Beta"));
        let slugs: Vec<_> = entries.iter().map(|e| e.slug.as_str()).collect();
        assert!(slugs.contains(&"alpha"));

        // Delete removes the index entry.
        engine
            .record_local(OpPayload::NoteDelete {
                note_id: [1u8; 16],
                display_alias: Some("alpha".into()),
            })
            .await
            .unwrap();
        let entries = engine.index_entries().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Beta");
    }

    #[tokio::test]
    async fn note_upsert_reconciles_drifted_tree() {
        // Review finding [2]: a full-content NoteUpsert must re-sync an
        // already-populated tree that has drifted from the body, instead
        // of skipping. Simulate drift by injecting a stale extra block,
        // then re-upsert the canonical body and assert the render matches.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x55; 16];
        let body = "- one <!-- bid:11111111-1111-1111-1111-111111111111 -->\n- two <!-- bid:22222222-2222-2222-2222-222222222222 -->\n";
        let up = |content: String| OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("n".into()),
            title: "N".into(),
            content,
            created_at_millis: 1,
        };
        engine.record_local(up(body.to_string())).await.unwrap();
        assert_eq!(engine.render_note(note_id).await.unwrap(), body);

        // Drift the shadow tree out of band: append a stale block.
        {
            let doc = engine.doc_for_note_mut(note_id).await;
            let tree = doc.get_tree("blocks");
            let n = tree.create(TreeParentId::Root).unwrap();
            let m = tree.get_meta(n).unwrap();
            m.insert("block_id", "33333333-3333-3333-3333-333333333333").unwrap();
            m.insert("text", "STALE").unwrap();
            m.insert("indent_level", 0i64).unwrap();
            doc.commit();
        }
        assert!(engine.render_note(note_id).await.unwrap().contains("STALE"));

        // Re-save the canonical body — should reconcile away the drift.
        engine.record_local(up(body.to_string())).await.unwrap();
        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(rendered, body, "full-content re-save heals drift");
        assert!(!rendered.contains("STALE"));
    }

    #[tokio::test]
    async fn three_engines_converge_via_broadcast_relay() {
        // PHASE 5: the relay BROADCAST cursor model (the recon's #1
        // flagged risk). Three engines edit the same note; each tick
        // every engine broadcasts its per-note deltas and every other
        // engine imports them idempotently. Assert all three converge,
        // and that a steady-state tick produces nothing (bounded
        // re-broadcast, no infinite loop).
        let mk = |seed: u8| {
            let d = DeviceId::from_bytes([seed; 16]);
            LoroEngine::new(d, Arc::new(Hlc::new(d)))
        };
        let a = mk(0xa1);
        let b = mk(0xb2);
        let c = mk(0xc3);
        let note = [0x88; 16];

        // A creates the note; one broadcast round seeds B and C.
        a.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("n".into()),
            title: "N".into(),
            content: "- base <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

        // Helper: one full relay round — everyone broadcasts, everyone
        // else imports.
        async fn relay_round(engines: &[&LoroEngine]) {
            let mut bus: Vec<([u8; 16], Vec<u8>)> = Vec::new();
            for e in engines {
                let produced = e.produce_relay_updates().await;
                let committed: Vec<([u8; 16], Vec<u8>)> =
                    produced.iter().map(|(d, _, vv)| (*d, vv.clone())).collect();
                for (d, b, _) in &produced {
                    bus.push((*d, b.clone()));
                }
                // Simulate a confirmed send → advance the cursor.
                e.commit_broadcast_cursors(&committed).await;
            }
            for e in engines {
                e.apply_relay_updates(&bus).await;
            }
        }
        let all = [&a, &b, &c];
        relay_round(&all).await;
        relay_round(&all).await; // second round propagates any transitive deltas

        // Concurrent edits on all three.
        a.record_local(OpPayload::BlockUpsert {
            block_id: [0xaa; 16], note_id: note, parent_block_id: None,
            order_key: "a".into(), indent_level: 0, text: "A edit".into(),
            after_block_id: None,
        }).await.unwrap();
        b.record_local(OpPayload::BlockUpsert {
            block_id: [0xbb; 16], note_id: note, parent_block_id: None,
            order_key: "b".into(), indent_level: 0, text: "B edit".into(),
            after_block_id: None,
        }).await.unwrap();
        c.record_local(OpPayload::BlockUpsert {
            block_id: [0xcc; 16], note_id: note, parent_block_id: None,
            order_key: "c".into(), indent_level: 0, text: "C edit".into(),
            after_block_id: None,
        }).await.unwrap();

        // A couple of relay rounds to fully propagate.
        relay_round(&all).await;
        relay_round(&all).await;

        let ra = a.render_note(note).await.unwrap();
        let rb = b.render_note(note).await.unwrap();
        let rc = c.render_note(note).await.unwrap();
        assert_eq!(ra, rb, "A and B converge");
        assert_eq!(rb, rc, "B and C converge");
        for needle in ["base", "A edit", "B edit", "C edit"] {
            assert!(ra.contains(needle), "converged state has {needle}: {ra}");
        }

        // Steady state: a further round broadcasts nothing new.
        let nothing: usize = {
            let mut n = 0;
            for e in &all {
                n += e.produce_relay_updates().await.len();
            }
            n
        };
        assert_eq!(nothing, 0, "no new broadcasts at steady state (bounded re-broadcast)");
    }

    #[tokio::test]
    async fn trait_level_delta_methods_converge_cursor_free() {
        // INSTANT-MULTIDEVICE PHASE 0: the live WS path holds `dyn SyncEngine`
        // and exchanges deltas via the NEW trait-level doc_version /
        // export_doc_update / import_doc_update. This proves (1) those methods
        // are reachable + correct through the trait object (the FFI/server
        // holder shape), and (2) the live export is CURSOR-FREE — it must NOT
        // advance the relay's broadcast cursor, so the relay path still sees
        // the note as pending (spec finding #3: no WS/relay cursor contention).
        let a_concrete =
            LoroEngine::new(DeviceId::from_bytes([0xc1; 16]), Arc::new(Hlc::new(DeviceId::from_bytes([0xc1; 16]))));
        let b_concrete =
            LoroEngine::new(DeviceId::from_bytes([0xd2; 16]), Arc::new(Hlc::new(DeviceId::from_bytes([0xd2; 16]))));
        let note = [0x88; 16];

        a_concrete
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("shared".into()),
                title: "Shared".into(),
                content: "- base <!-- bid:02020202-0202-0202-0202-020202020202 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // Drive the exchange THROUGH the trait object, exactly as the live WS
        // path (and the FFI) will.
        let a: &dyn SyncEngine = &a_concrete;
        let b: &dyn SyncEngine = &b_concrete;

        let bootstrap = a.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &bootstrap).await.unwrap();
        assert_eq!(
            a.render_note(note).await,
            b.render_note(note).await,
            "bootstrap via trait methods converges"
        );

        // Cursor-free invariant: the live export above did NOT advance the
        // broadcast cursor, so the relay producer still owes this note. (If
        // export had consumed the cursor, produce_relay_updates would return
        // nothing — the finding-#3 bug.)
        let pending = a.produce_relay_updates().await;
        assert!(
            pending.iter().any(|(nid, _, _)| *nid == note),
            "cursor-free export must leave the note pending for the relay path"
        );

        // Concurrent edits, exchanged both ways via the trait object.
        a_concrete
            .record_local(OpPayload::BlockUpsert {
                block_id: [0xca; 16], note_id: note, parent_block_id: None,
                order_key: "a".into(), indent_level: 0, text: "from A".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        b_concrete
            .record_local(OpPayload::BlockUpsert {
                block_id: [0xcb; 16], note_id: note, parent_block_id: None,
                order_key: "b".into(), indent_level: 0, text: "from B".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        let b_vv = b.doc_version(note).await;
        let a_upd = a.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_upd).await.unwrap();
        let a_vv = a.doc_version(note).await;
        let b_upd = b.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        a.import_doc_update(note, &b_upd).await.unwrap();

        let ra = a.render_note(note).await.unwrap();
        let rb = b.render_note(note).await.unwrap();
        assert_eq!(ra, rb, "trait-level exchange converges — no flashing");
        assert!(ra.contains("base") && ra.contains("from A") && ra.contains("from B"));
    }

    #[tokio::test]
    async fn two_engines_converge_on_concurrent_edits_no_flashing() {
        // PHASE 4 KEYSTONE: the flashing fix at the engine level. Two
        // LoroEngines (distinct devices/PeerIDs) edit the SAME note
        // concurrently, exchange Loro updates, and converge to one
        // deterministic state on both sides — stable across repeated
        // exchange (no ping-pong). The hand-rolled engine could not do
        // this; that's the whole reason for the migration.
        let a = LoroEngine::new(DeviceId::from_bytes([0xa1; 16]), Arc::new(Hlc::new(DeviceId::from_bytes([0xa1; 16]))));
        let b = LoroEngine::new(DeviceId::from_bytes([0xb2; 16]), Arc::new(Hlc::new(DeviceId::from_bytes([0xb2; 16]))));
        assert_ne!(a.peer_id(), b.peer_id(), "devices must have distinct peer ids");
        let note = [0x77; 16];

        // A creates the note with one stamped block; B bootstraps from
        // A's full state (the new-device-join path).
        a.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("shared".into()),
            title: "Shared".into(),
            content: "- base <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
        let bootstrap = a.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &bootstrap).await.unwrap();
        assert_eq!(a.render_note(note).await, b.render_note(note).await, "bootstrapped equal");

        // Concurrent edits: A appends a block, B appends a different one.
        a.record_local(OpPayload::BlockUpsert {
            block_id: [0xaa; 16], note_id: note, parent_block_id: None,
            order_key: "a".into(), indent_level: 0, text: "from A".into(),
            after_block_id: None,
        }).await.unwrap();
        b.record_local(OpPayload::BlockUpsert {
            block_id: [0xbb; 16], note_id: note, parent_block_id: None,
            order_key: "b".into(), indent_level: 0, text: "from B".into(),
            after_block_id: None,
        }).await.unwrap();

        // Exchange updates both ways (two relay ticks), using each peer's
        // version vector as the cursor.
        let b_vv = b.doc_version(note).await;
        let a_upd = a.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_upd).await.unwrap();
        let a_vv = a.doc_version(note).await;
        let b_upd = b.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        a.import_doc_update(note, &b_upd).await.unwrap();

        let ra = a.render_note(note).await.unwrap();
        let rb = b.render_note(note).await.unwrap();
        assert_eq!(ra, rb, "engines converge to identical state — no flashing");
        assert!(ra.contains("base") && ra.contains("from A") && ra.contains("from B"));

        // Re-exchange must be a stable no-op (no oscillation).
        let b_vv2 = b.doc_version(note).await;
        if let Some(u) = a.export_doc_update(note, b_vv2.as_deref()).await {
            if !u.is_empty() { b.import_doc_update(note, &u).await.unwrap(); }
        }
        assert_eq!(a.render_note(note).await.unwrap(), ra, "stable after re-exchange");
        assert_eq!(b.render_note(note).await.unwrap(), rb, "stable after re-exchange");
    }

    #[tokio::test]
    async fn block_op_resolves_seeded_block_via_index() {
        // A block created via NoteUpsert seed (not BlockUpsert) must be
        // resolvable by a later block-only op through the block_index.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x66; 16];
        // Seed two stamped blocks via NoteUpsert.
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: "- keep <!-- bid:10101010-1010-1010-1010-101010101010 -->\n- drop <!-- bid:20202020-2020-2020-2020-202020202020 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        // BlockDelete the second block by id — only resolvable via the
        // block_index (the op carries no note_id).
        let drop_id = [0x20; 16];
        engine
            .record_local(OpPayload::BlockDelete { block_id: drop_id })
            .await
            .unwrap();
        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(
            rendered,
            "- keep <!-- bid:10101010-1010-1010-1010-101010101010 -->\n",
            "seeded block resolved + deleted via block_index"
        );
    }

    #[tokio::test]
    async fn block_delete_reparents_direct_children_to_indent_0() {
        // Review finding [1]/[9]: deleting a parent must flatten its
        // DIRECT children to indent 0 (matching SqliteEngine), while
        // grandchildren keep their indent.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x44; 16];
        let a = [0xa1; 16];
        let b = [0xb1; 16]; // direct child of a (indent 1)
        let c = [0xc1; 16]; // child of b (indent 2, grandchild of a)

        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: a, note_id, parent_block_id: None,
                order_key: "a".into(), indent_level: 0, text: "A".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: b, note_id, parent_block_id: Some(a),
                order_key: "b".into(), indent_level: 1, text: "B".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: c, note_id, parent_block_id: Some(b),
                order_key: "c".into(), indent_level: 2, text: "C".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        // Delete A (the parent with a direct child B).
        engine
            .record_local(OpPayload::BlockDelete { block_id: a })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        // B (direct child) flattened to indent 0; C (grandchild) keeps
        // indent 2 — exactly SqliteEngine's apply_block_delete behavior.
        assert_eq!(
            rendered,
            "- B <!-- bid:b1b1b1b1-b1b1-b1b1-b1b1-b1b1b1b1b1b1 -->\n    - C <!-- bid:c1c1c1c1-c1c1-c1c1-c1c1-c1c1c1c1c1c1 -->\n"
        );
    }

    #[tokio::test]
    async fn index_link_with_comma_is_one_edge() {
        // Review finding [7]: a wiki-link target containing a comma must
        // remain a single link, not fragment into two.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: [8u8; 16],
                display_alias: Some("c".into()),
                title: "C".into(),
                content: "- see [[Smith, John]] and [[plain]]\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let entries = engine.index_entries().await;
        assert_eq!(entries.len(), 1);
        let mut links = entries[0].links.clone();
        links.sort();
        assert_eq!(links, vec!["Smith, John".to_string(), "plain".to_string()]);
    }

    #[tokio::test]
    async fn index_rebuild_prunes_ghost_entries() {
        // Review finding [6]: rebuild must drop index entries that have
        // no backing per-note doc, not leave them as phantoms.
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        // One real note with a doc.
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: [1u8; 16],
                display_alias: Some("real".into()),
                title: "Real".into(),
                content: "- x\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        // Inject a ghost index entry with no backing doc.
        {
            let notes = engine.inner.index.get_map("notes");
            let ghost = notes
                .insert_container(&hex_id(&[0x99u8; 16]), loro::LoroMap::new())
                .unwrap();
            ghost.insert("title", "Ghost").unwrap();
            ghost.insert("slug", "ghost").unwrap();
            engine.inner.index.commit();
        }
        assert_eq!(engine.index_entries().await.len(), 2, "ghost present pre-rebuild");

        engine.rebuild_index_from_docs().await;
        let entries = engine.index_entries().await;
        assert_eq!(entries.len(), 1, "ghost pruned");
        assert_eq!(entries[0].title, "Real");
    }

    #[tokio::test]
    async fn index_self_heals_when_schema_stale() {
        // Simulate a stale on-disk index: write notes, then hand-corrupt
        // the persisted index's schema_version to 1 (pre-tags/links) and
        // strip the tags field, then reload — the boot rebuild should
        // restore tags/links from the self-describing per-note docs.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
            .await
            .unwrap();
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: [3u8; 16],
                display_alias: Some("n".into()),
                title: "N".into(),
                content: "---\ntitle: N\ntags: [alpha]\n---\n\n- see [[target]]\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        drop(engine);

        // Downgrade the persisted index schema marker to force a rebuild.
        let idx_path = dir.join("_index.bin");
        let idx = LoroDoc::new();
        idx.import(&tokio::fs::read(&idx_path).await.unwrap()).unwrap();
        idx.get_map("meta").insert("schema_version", 1i64).unwrap();
        idx.commit();
        tokio::fs::write(&idx_path, idx.export(ExportMode::Snapshot).unwrap())
            .await
            .unwrap();

        // Reload: boot rebuild should fire and restore tags/links.
        let hlc2 = Arc::new(Hlc::new(test_device()));
        let reloaded = LoroEngine::with_snapshot_dir(test_device(), hlc2, dir)
            .await
            .unwrap();
        let entries = reloaded.index_entries().await;
        assert_eq!(entries.len(), 1);
        assert!(entries[0].tags.contains(&"alpha".to_string()), "tags: {:?}", entries[0].tags);
        assert_eq!(entries[0].links, vec!["target".to_string()]);
    }

    #[tokio::test]
    async fn index_rebuild_preserves_slug_when_doc_lacks_it() {
        // The live upgrade scenario: per-note docs written by an older
        // engine carry "content" but NOT slug/title on root meta, while
        // the prior index DOES have the slug. Rebuild must keep the slug
        // (from the prior index) rather than blanking it.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
            .await
            .unwrap();
        let note_id = [4u8; 16];
        // Build a per-note doc WITHOUT slug/title on root (simulate old
        // engine): only content. Then a prior index entry with the slug.
        {
            let doc = engine.doc_for_note_mut(note_id).await;
            doc.get_map("root")
                .insert("content", "---\ntitle: Kept\ntags: [z]\n---\n\n- body\n")
                .unwrap();
            doc.commit();
        }
        // Prior index entry (title+slug only, no tags) — like step 1.
        {
            let notes = engine.inner.index.get_map("notes");
            let entry = notes.insert_container(&hex_id(&note_id), loro::LoroMap::new()).unwrap();
            entry.insert("title", "Kept").unwrap();
            entry.insert("slug", "kept-slug").unwrap();
            engine.inner.index.commit();
        }

        engine.rebuild_index_from_docs().await;
        let entries = engine.index_entries().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].slug, "kept-slug", "slug preserved from prior index");
        assert_eq!(entries[0].title, "Kept");
        assert!(entries[0].tags.contains(&"z".to_string()), "tags derived: {:?}", entries[0].tags);
    }

    #[tokio::test]
    async fn index_doc_captures_tags_and_links() {
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let content = "---\ntitle: T\ntags: [daily]\n---\n\ntags:: project\n- see [[other-note]] and #urgent stuff\n";

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: [7u8; 16],
                display_alias: Some("t".into()),
                title: "T".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        let entries = engine.index_entries().await;
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        // tags from frontmatter (daily), page property (project), inline (#urgent)
        assert!(e.tags.contains(&"daily".to_string()), "frontmatter tag: {:?}", e.tags);
        assert!(e.tags.contains(&"project".to_string()), "page-prop tag: {:?}", e.tags);
        assert!(e.tags.contains(&"urgent".to_string()), "inline tag: {:?}", e.tags);
        // link target
        assert_eq!(e.links, vec!["other-note".to_string()]);
    }

    #[tokio::test]
    async fn index_doc_survives_reload() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
            .await
            .unwrap();
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: [9u8; 16],
                display_alias: Some("kept".into()),
                title: "Kept".into(),
                content: "- x\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        drop(engine);

        let hlc2 = Arc::new(Hlc::new(test_device()));
        let reloaded = LoroEngine::with_snapshot_dir(test_device(), hlc2, dir)
            .await
            .unwrap();
        let entries = reloaded.index_entries().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Kept");
        assert_eq!(entries[0].slug, "kept");
    }

    #[tokio::test]
    async fn note_upsert_overwrites_page_properties() {
        // A second NoteUpsert with different props replaces the first
        // wholesale (no stale leftovers).
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::new(test_device(), hlc);
        let note_id = [0x5b; 16];

        for content in [
            "query:: kind:page\nsort:: modified desc\n",
            "query:: kind:block\n",
        ] {
            engine
                .record_local(OpPayload::NoteUpsert {
                    note_id,
                    display_alias: Some("q".into()),
                    title: "Q".into(),
                    content: content.into(),
                    created_at_millis: 1,
                })
                .await
                .unwrap();
        }

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(rendered, "query:: kind:block\n", "wholesale overwrite");
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
                after_block_id: None,
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
                after_block_id: None,
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note_id).await.unwrap();
        assert_eq!(
            rendered,
            "- second <!-- bid:14141414-1414-1414-1414-141414141414 -->\n"
        );
    }

    // ── Authoritative-writer cutover ─────────────────────────────────

    #[tokio::test]
    async fn authoritative_engine_materializes_and_deletes_md_files() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("loro");
        let notes = tmp.path().join("notes");
        let dev = test_device();
        let engine = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            snap,
            Some(notes.clone()),
        )
        .await
        .unwrap();
        let note_id = blake3_note_id("daily");
        let content =
            "---\ntitle: Daily\n---\n\n- one <!-- bid:30303030-3030-3030-3030-303030303030 -->\n";
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("daily".into()),
                title: "Daily".into(),
                content: content.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let path = notes.join("daily.md");
        let on_disk = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(on_disk, content, "NoteUpsert materializes the full file");

        // A block append rewrites the file with both bullets.
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: [0x31; 16],
                note_id,
                parent_block_id: None,
                order_key: "b".into(),
                indent_level: 0,
                text: "two".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let on_disk = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(on_disk.contains("- one ") && on_disk.contains("- two "), "block append materialized: {on_disk:?}");
        assert!(on_disk.starts_with("---\ntitle: Daily\n---\n"), "frontmatter preserved");

        // NoteDelete removes the file.
        engine
            .record_local(OpPayload::NoteDelete {
                note_id,
                display_alias: Some("daily".into()),
            })
            .await
            .unwrap();
        assert!(!path.exists(), "NoteDelete removes the materialized file");
    }

    #[tokio::test]
    async fn note_delete_without_alias_still_removes_file() {
        // Review finding: a NoteDelete whose op carries no display_alias
        // (op.rs: "None means the producer did not know the slug") must
        // still remove the materialized file — the slug is resolved from
        // the resident doc/index BEFORE the inner apply drops them.
        let tmp = tempfile::tempdir().unwrap();
        let dev = test_device();
        let engine = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            tmp.path().join("loro"),
            Some(tmp.path().join("notes")),
        )
        .await
        .unwrap();
        let note_id = blake3_note_id("orphan");
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("orphan".into()),
                title: "Orphan".into(),
                content: "- x <!-- bid:33333333-3333-3333-3333-333333333333 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let path = tmp.path().join("notes").join("orphan.md");
        assert!(path.exists(), "materialized");
        engine
            .record_local(OpPayload::NoteDelete {
                note_id,
                display_alias: None,
            })
            .await
            .unwrap();
        assert!(
            !path.exists(),
            "NoteDelete with display_alias=None must still remove the file"
        );
    }

    #[tokio::test]
    async fn non_authoritative_engine_writes_no_md_files() {
        // Without materialize_dir, the engine must not touch the notes dir.
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("loro");
        let dev = test_device();
        let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, None)
            .await
            .unwrap();
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: blake3_note_id("x"),
                display_alias: Some("x".into()),
                title: "X".into(),
                content: "- hi <!-- bid:32323232-3232-3232-3232-323232323232 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        // Only the snapshot dir should exist; no notes/ dir was created.
        assert!(!tmp.path().join("notes").exists(), "no .md materialization when non-authoritative");
    }

    #[tokio::test]
    async fn reseed_from_disk_tracks_and_canonicalizes() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("loro");
        let notes = tmp.path().join("notes");
        tokio::fs::create_dir_all(&notes).await.unwrap();
        // A canonical note and a non-canonical one (bullet missing its bid
        // — reseed will stamp + re-render canonically).
        tokio::fs::write(
            notes.join("alpha.md"),
            "---\ntitle: Alpha\n---\n\n- a1 <!-- bid:40404040-4040-4040-4040-404040404040 -->\n",
        )
        .await
        .unwrap();
        tokio::fs::write(notes.join("beta.md"), "- just text\n")
            .await
            .unwrap();
        let dev = test_device();
        let engine = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            snap,
            Some(notes.clone()),
        )
        .await
        .unwrap();
        let n = engine.reseed_from_disk(&notes).await.unwrap();
        assert_eq!(n, 2, "both .md files reseeded");
        // Both notes are now tracked + render their content.
        let alpha = blake3_note_id("alpha");
        let rendered = engine.render_note(alpha).await.unwrap();
        assert!(rendered.contains("a1"), "alpha block present: {rendered:?}");
        // beta got a canonical bid stamped on its bullet (was bare).
        let beta = blake3_note_id("beta");
        let rb = engine.render_note(beta).await.unwrap();
        assert!(rb.contains("- just text") && rb.contains("<!-- bid:"), "beta canonicalized: {rb:?}");
    }

    #[tokio::test]
    async fn two_authoritative_engines_converge_through_wire_codec() {
        // The real relay-payload path minus HTTP: A produces relay
        // updates → encode_loro_relay_payload (the TLR2 v2 wire) → decode
        // → B.apply_relay_updates. Both materialize identical files and
        // converge with no flashing, exactly as two Macs over the relay.
        use crate::wire::{decode_loro_relay_payload, encode_loro_relay_payload, LoroDocUpdate};

        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let a = LoroEngine::with_dirs(
            dev_a,
            Arc::new(Hlc::new(dev_a)),
            tmp_a.path().join("loro"),
            Some(tmp_a.path().join("notes")),
        )
        .await
        .unwrap();
        let b = LoroEngine::with_dirs(
            dev_b,
            Arc::new(Hlc::new(dev_b)),
            tmp_b.path().join("loro"),
            Some(tmp_b.path().join("notes")),
        )
        .await
        .unwrap();

        // Helper: ship A's produced updates to B through the wire codec,
        // then commit A's cursor (simulating a confirmed send).
        async fn ship(from: &LoroEngine, to: &LoroEngine) -> usize {
            let updates = from.produce_relay_updates().await;
            if updates.is_empty() {
                return 0;
            }
            let payload: Vec<LoroDocUpdate> = updates
                .iter()
                .map(|(doc, update_bytes, _vv)| LoroDocUpdate {
                    doc: *doc,
                    update_bytes: update_bytes.clone(),
                })
                .collect();
            let committed: Vec<([u8; 16], Vec<u8>)> =
                updates.into_iter().map(|(doc, _b, vv)| (doc, vv)).collect();
            let wire = encode_loro_relay_payload(&payload).unwrap();
            let decoded = decode_loro_relay_payload(&wire).unwrap().expect("v2 payload");
            let pairs: Vec<([u8; 16], Vec<u8>)> =
                decoded.into_iter().map(|u| (u.doc, u.update_bytes)).collect();
            let n = to.apply_relay_updates(&pairs).await;
            from.commit_broadcast_cursors(&committed).await;
            n
        }

        let note = blake3_note_id("shared");
        a.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("shared".into()),
            title: "Shared".into(),
            content: "- base <!-- bid:50505050-5050-5050-5050-505050505050 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
        // A → B bootstrap.
        assert!(ship(&a, &b).await >= 1, "B received the note");
        assert_eq!(
            a.render_note(note).await,
            b.render_note(note).await,
            "bootstrapped equal"
        );
        // Both have materialized the file.
        let fa = tmp_a.path().join("notes").join("shared.md");
        let fb = tmp_b.path().join("notes").join("shared.md");
        assert!(fa.exists() && fb.exists(), "both materialized shared.md");
        assert_eq!(
            tokio::fs::read_to_string(&fa).await.unwrap(),
            tokio::fs::read_to_string(&fb).await.unwrap(),
            "materialized files identical"
        );

        // Concurrent edits, exchanged both ways.
        a.record_local(OpPayload::BlockUpsert {
            block_id: [0x5a; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "a".into(),
            indent_level: 0,
            text: "from A".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
        b.record_local(OpPayload::BlockUpsert {
            block_id: [0x5b; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "b".into(),
            indent_level: 0,
            text: "from B".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
        // Two ticks each direction to fully exchange.
        ship(&a, &b).await;
        ship(&b, &a).await;
        ship(&a, &b).await;
        ship(&b, &a).await;

        let ra = a.render_note(note).await.unwrap();
        let rb = b.render_note(note).await.unwrap();
        assert_eq!(ra, rb, "engines converge — no flashing");
        assert!(ra.contains("base") && ra.contains("from A") && ra.contains("from B"));
        assert_eq!(
            tokio::fs::read_to_string(&fa).await.unwrap(),
            tokio::fs::read_to_string(&fb).await.unwrap(),
            "materialized files converge"
        );

        // Steady state: another exchange ships nothing (bounded broadcast).
        assert_eq!(ship(&a, &b).await, 0, "no re-broadcast at steady state");
    }

    #[tokio::test]
    async fn broadcast_cursors_persist_across_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("loro");
        let notes = tmp.path().join("notes");
        let dev = test_device();
        let note = blake3_note_id("persist");
        {
            let engine = LoroEngine::with_dirs(
                dev,
                Arc::new(Hlc::new(dev)),
                snap.clone(),
                Some(notes.clone()),
            )
            .await
            .unwrap();
            engine
                .record_local(OpPayload::NoteUpsert {
                    note_id: note,
                    display_alias: Some("persist".into()),
                    title: "P".into(),
                    content: "- x <!-- bid:60606060-6060-6060-6060-606060606060 -->\n".into(),
                    created_at_millis: 1,
                })
                .await
                .unwrap();
            let first = engine.produce_relay_updates().await;
            assert_eq!(first.len(), 1, "first produce emits the note");
            // Commit (confirmed send) advances + persists the cursor.
            let committed: Vec<([u8; 16], Vec<u8>)> =
                first.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
            engine.commit_broadcast_cursors(&committed).await;
        }
        // Reopen: cursor was persisted, so produce emits nothing new.
        let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes))
            .await
            .unwrap();
        let again = engine.produce_relay_updates().await;
        assert!(again.is_empty(), "persisted cursor suppresses re-broadcast after restart");
    }

    #[tokio::test]
    async fn produce_without_commit_re_emits_delta_on_failed_send() {
        // Review finding #1: produce_relay_updates must NOT advance the
        // cursor — only commit_broadcast_cursors does. So a failed relay
        // send (no commit) re-emits the same delta next tick instead of
        // losing it forever.
        let tmp = tempfile::tempdir().unwrap();
        let dev = test_device();
        let engine = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            tmp.path().join("loro"),
            Some(tmp.path().join("notes")),
        )
        .await
        .unwrap();
        let note = blake3_note_id("retry");
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("retry".into()),
                title: "R".into(),
                content: "- x <!-- bid:90909090-9090-9090-9090-909090909090 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        // First produce: one delta.
        let first = engine.produce_relay_updates().await;
        assert_eq!(first.len(), 1, "produce emits the note");
        // Simulate a FAILED send: do NOT commit. Next produce must still
        // emit the same delta (not lost).
        let retry = engine.produce_relay_updates().await;
        assert_eq!(retry.len(), 1, "failed send re-emits the delta — not dropped");
        assert_eq!(retry[0].0, note);
        // Now commit (confirmed send). Subsequent produce is empty.
        let committed: Vec<([u8; 16], Vec<u8>)> =
            retry.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
        engine.commit_broadcast_cursors(&committed).await;
        assert!(
            engine.produce_relay_updates().await.is_empty(),
            "committed cursor suppresses re-broadcast"
        );
    }

    #[tokio::test]
    async fn since_vv_delta_is_smaller_than_snapshot_and_converges() {
        // iOS #150 (block-granular-writes spec, Stage 4): the live WS frame
        // ships a DELTA relative to the last-pushed VV, not a full snapshot
        // every keystroke. This proves the two properties the iOS change
        // relies on: (1) `export_doc_update(note, Some(vv_before_edit))` after
        // a single edit is byte-SMALLER than the full snapshot, and (2) a peer
        // that already holds `vv_before_edit` converges after importing only
        // that delta. Together: the steady-state WS frame shrinks AND stays
        // loss-free.
        let author = LoroEngine::new(
            DeviceId::from_bytes([0xe1; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xe1; 16]))),
        );
        let note = [0x91; 16];

        // Seed a multi-block base so the snapshot has real heft.
        author
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("shared".into()),
                title: "Shared".into(),
                content: "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n\
                          - beta <!-- bid:02020202-0202-0202-0202-020202020202 -->\n\
                          - gamma <!-- bid:03030303-0303-0303-0303-030303030303 -->\n"
                    .into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // A peer bootstraps from the full snapshot — this is the base the
        // delta will be relative to (mirrors iOS's `lastPushedVV[slug]`
        // tracking the VV as of the last push the peer received).
        let peer = LoroEngine::new(
            DeviceId::from_bytes([0xf2; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xf2; 16]))),
        );
        let snapshot = author.export_doc_update(note, None).await.unwrap();
        peer.import_doc_update(note, &snapshot).await.unwrap();
        assert_eq!(
            author.render_note(note).await,
            peer.render_note(note).await,
            "peer bootstrapped to the same base"
        );

        // Capture the VV AS OF the last push (the value iOS records as
        // `lastPushedVV[slug]` after `recordAndPush`), then author one edit.
        let vv_before_edit = author.doc_version(note).await.expect("vv before edit");
        author
            .record_local(OpPayload::BlockUpsert {
                block_id: [0x02; 16],
                note_id: note,
                parent_block_id: None,
                order_key: "b".into(),
                indent_level: 0,
                text: "beta EDITED".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        // The steady-state WS frame (delta) vs. the full snapshot iOS used to
        // ship every keystroke.
        let delta = author
            .export_doc_update(note, Some(&vv_before_edit))
            .await
            .expect("delta export");
        let full_snapshot = author.export_doc_update(note, None).await.expect("snapshot");
        assert!(
            delta.len() < full_snapshot.len(),
            "since_vv delta ({} bytes) must be smaller than the full snapshot ({} bytes)",
            delta.len(),
            full_snapshot.len(),
        );

        // The peer holding `vv_before_edit` applies ONLY the delta and
        // converges — no full-snapshot resend needed (loss-free).
        peer.import_doc_update(note, &delta).await.unwrap();
        let rendered = peer.render_note(note).await.unwrap();
        assert!(
            rendered.contains("beta EDITED"),
            "peer converges from the delta alone; got: {rendered:?}"
        );
        assert_eq!(
            author.render_note(note).await,
            peer.render_note(note).await,
            "author + peer converge after the delta-only exchange"
        );
    }

    fn blake3_note_id(slug: &str) -> [u8; 16] {
        let h = blake3::hash(slug.as_bytes());
        let mut id = [0u8; 16];
        id.copy_from_slice(&h.as_bytes()[..16]);
        id
    }

    // ── WS-push clobber guard (2026-06-02) ───────────────────────────
    //
    // The FINAL data-loss vector: a device ships a WHOLE-NOTE SNAPSHOT
    // carrying its STALE value for a block another peer (the server, via
    // HTTP) just edited. The stale op is CONCURRENT with the server's
    // edit and WINS the LWW tiebreak → a raw `doc.import` reverts the
    // server's edit on the authoritative doc. `import_doc_update` must
    // apply ONLY the blocks the peer GENUINELY (causally) re-authored,
    // never a stale re-assertion the peer merely re-shipped.

    /// Read a single block's current text off a note's tree by block_id
    /// bytes (matching the dashless hex the engine stores in meta).
    async fn block_text(engine: &LoroEngine, note_id: [u8; 16], block: [u8; 16]) -> Option<String> {
        let docs = engine.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        let tree = doc.get_tree("blocks");
        let node = find_node_by_block_id(&tree, &hex_id(&block))?;
        read_block_text(&tree, node)
    }

    /// Construct the EXACT wire incident (the wire-captured DISJOINT-lineage
    /// case): the server and the device each author block_id A / B
    /// INDEPENDENTLY (no shared Loro import), so each mints its OWN `TreeID`
    /// for the same `block_id` — the residual disjoint lineage these daily
    /// blocks carry (pre-shared-base, or `recordNoteDiff` re-authoring from
    /// stale markdown). The server then edits A→"Awesome sweet" via HTTP. The
    /// device, holding its own stale A="Awesome" twin, genuinely edits B→"B
    /// device" and exports a FULL SNAPSHOT. On a raw `doc.import` the device's
    /// A-twin unions with the server's, and the non-causal min-`TreeID` dedup
    /// can keep the STALE twin → the server's edit reverts. The protected
    /// path must keep "Awesome sweet" (A) AND apply "B device" (B).
    const A_BID: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    const B_BID: &str = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";
    const A_BID_BYTES: [u8; 16] = [0x0a; 16];
    const B_BID_BYTES: [u8; 16] = [0x0b; 16];

    async fn seed_disjoint(server: &LoroEngine, device: &LoroEngine, note: [u8; 16]) {
        // BOTH author the same note body independently — disjoint Loro
        // lineages (distinct TreeIDs for the same block_ids).
        let content =
            format!("- Awesome <!-- bid:{A_BID} -->\n- B base <!-- bid:{B_BID} -->\n");
        for e in [server, device] {
            e.record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("daily".into()),
                title: "Daily".into(),
                content: content.clone(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn ws_apply_stale_snapshot_does_not_revert_peer_edit() {
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let ddev = DeviceId::from_bytes([0x7f; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let note = blake3_note_id("daily");

        seed_disjoint(&server, &device, note).await;

        // Server edits A via HTTP-style block op (the protected, correct value).
        server
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "Awesome sweet".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        // Device (stale: never saw the server edit) re-authors A back to the
        // stale value AND genuinely edits B. Then exports a FULL SNAPSHOT —
        // the cold-launch first-push frame that triggered the incident.
        device
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "Awesome".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        device
            .record_local(OpPayload::BlockUpsert {
                block_id: B_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "B device".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let snapshot = device.export_doc_update(note, None).await.unwrap();

        // Server applies the device's stale snapshot via the WS-apply path.
        server.import_doc_update(note, &snapshot).await.unwrap();

        let a = block_text(&server, note, A_BID_BYTES).await.unwrap_or_default();
        let b = block_text(&server, note, B_BID_BYTES).await.unwrap_or_default();
        assert_eq!(
            a, "Awesome sweet",
            "A must NOT be reverted by the device's stale snapshot (got {a:?})"
        );
        assert_eq!(
            b, "B device",
            "B (the device's genuine edit) must apply (got {b:?})"
        );
    }

    #[tokio::test]
    async fn ws_apply_genuine_edit_applies() {
        // No competing server edit on B: the device's genuine B edit must
        // land on the server (don't invert the bug into "always keep server").
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let ddev = DeviceId::from_bytes([0x7f; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let note = blake3_note_id("daily");

        seed_disjoint(&server, &device, note).await;

        device
            .record_local(OpPayload::BlockUpsert {
                block_id: B_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "B device".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        // Device ships a since_vv DELTA of just B (the steady-state frame).
        let delta = device.export_doc_update(note, None).await.unwrap();
        server.import_doc_update(note, &delta).await.unwrap();

        assert_eq!(
            block_text(&server, note, A_BID_BYTES).await.as_deref(),
            Some("Awesome"),
            "A unchanged"
        );
        assert_eq!(
            block_text(&server, note, B_BID_BYTES).await.as_deref(),
            Some("B device"),
            "genuine B edit applied"
        );
    }

    #[tokio::test]
    async fn ws_apply_stale_snapshot_is_idempotent() {
        // Applying the same stale snapshot twice must not corrupt state; the
        // second apply is a no-op (A kept, B applied, both stable).
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let ddev = DeviceId::from_bytes([0x7f; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let note = blake3_note_id("daily");

        seed_disjoint(&server, &device, note).await;

        server
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "Awesome sweet".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        device
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "Awesome".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        device
            .record_local(OpPayload::BlockUpsert {
                block_id: B_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "B device".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let snapshot = device.export_doc_update(note, None).await.unwrap();

        server.import_doc_update(note, &snapshot).await.unwrap();
        let a1 = block_text(&server, note, A_BID_BYTES).await.unwrap_or_default();
        let b1 = block_text(&server, note, B_BID_BYTES).await.unwrap_or_default();
        // Second apply of the identical frame.
        server.import_doc_update(note, &snapshot).await.unwrap();
        let a2 = block_text(&server, note, A_BID_BYTES).await.unwrap_or_default();
        let b2 = block_text(&server, note, B_BID_BYTES).await.unwrap_or_default();

        assert_eq!(a1, "Awesome sweet");
        assert_eq!(b1, "B device");
        assert_eq!(a1, a2, "A stable across re-apply");
        assert_eq!(b1, b2, "B stable across re-apply");
    }

    #[tokio::test]
    async fn ws_apply_shared_register_concurrent_edit_merges_via_loro_text() {
        // When the server + device SHARE the Loro lineage for a block (one
        // LoroText) and BOTH edit it concurrently, the protected apply must
        // DEFER to Loro's own LoroText merge — NOT force one side's whole value
        // and NOT restore the other. Block text is a sequence CRDT now, so the
        // two whole-text edits INTERLEAVE: both sides converge to the SAME
        // merged value, both contributions survive, and re-apply is stable (no
        // oscillation, no clobber). (Pre-LoroText this was an LWW whole-string
        // pick; the merge is the deepest fix.)
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let ddev = DeviceId::from_bytes([0x7f; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let note = blake3_note_id("daily");

        // SHARED base: device imports the server's snapshot (same TreeIDs).
        server
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("daily".into()),
                title: "Daily".into(),
                content: format!("- base <!-- bid:{A_BID} -->\n"),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let base = server.export_doc_update(note, None).await.unwrap();
        device.import_doc_update(note, &base).await.unwrap();

        // Capture the device's pre-edit VV so it can ship a true since-vv
        // DELTA of just its own concurrent edit on the SHARED register.
        let dev_vv = device.doc_version(note).await;
        // Concurrent edits to the SAME shared block.
        server
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES, note_id: note, parent_block_id: None,
                order_key: "00000000".into(), indent_level: 0,
                text: "server edit".into(), after_block_id: None,
            })
            .await
            .unwrap();
        device
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES, note_id: note, parent_block_id: None,
                order_key: "00000000".into(), indent_level: 0,
                text: "device edit".into(), after_block_id: None,
            })
            .await
            .unwrap();
        let delta = device.export_doc_update(note, dev_vv.as_deref()).await.unwrap();

        // Server applies the device's delta. Loro's LoroText merge picks the
        // SAME converged value on both sides — and re-applying must be stable.
        server.import_doc_update(note, &delta).await.unwrap();
        // Round-trip the server's state back to the device to converge.
        let dev_vv2 = device.doc_version(note).await;
        let srv_delta = server.export_doc_update(note, dev_vv2.as_deref()).await.unwrap();
        device.import_doc_update(note, &srv_delta).await.unwrap();

        let sa = block_text(&server, note, A_BID_BYTES).await.unwrap_or_default();
        let da = block_text(&device, note, A_BID_BYTES).await.unwrap_or_default();
        assert_eq!(sa, da, "shared-register concurrent edit converges on both sides");
        // The LoroText merge INTERLEAVES both whole-text edits rather than
        // LWW-picking one: the result is NEITHER whole string (no clobber) and
        // is longer than either input — both sides contributed characters.
        assert_ne!(sa, "server edit", "not an LWW pick of the server's whole edit");
        assert_ne!(sa, "device edit", "not an LWW pick of the device's whole edit");
        assert!(
            sa.len() > "server edit".len() && sa.contains("device"),
            "both concurrent edits' contributions survive the merge: {sa:?}"
        );

        // Stable: re-applying the same delta does not flip the value.
        server.import_doc_update(note, &delta).await.unwrap();
        let sa2 = block_text(&server, note, A_BID_BYTES).await.unwrap_or_default();
        assert_eq!(sa, sa2, "no oscillation on re-apply");
    }

    // ── Same-block concurrent text MERGE (2026-06-02 LoroText fix) ────
    //
    // The DEEPEST data-loss vector: two replicas on a SHARED Loro lineage
    // each apply a DIFFERENT whole-text BlockUpsert to the SAME block,
    // concurrently. With the legacy LWW map register one side's typing
    // vanished. With block text stored as a nested `LoroText`, each
    // replica's whole-text `update()` Myers-diffs into the minimal
    // splices against the shared sequence, so cross-import INTERLEAVES
    // both contributions instead of clobbering.
    #[tokio::test]
    async fn concurrent_same_block_text_merges_not_clobbers() {
        let note = blake3_note_id("merge");

        // Replica A builds the shared base for block X.
        let dev_a = DeviceId::from_bytes([0xa7; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        a.record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "The quick fox".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

        // Replica B imports the base so both share the same TreeID lineage
        // for X (the merge precondition — NOT disjoint twins).
        let dev_b = DeviceId::from_bytes([0xb7; 16]);
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        let base = a.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &base).await.unwrap();
        assert_eq!(
            block_text(&b, note, A_BID_BYTES).await.as_deref(),
            Some("The quick fox"),
            "shared base seeded on B"
        );

        // Capture each replica's pre-edit VV so each ships only its own
        // concurrent edit as a since-vv delta.
        let a_vv = a.doc_version(note).await;
        let b_vv = b.doc_version(note).await;

        // Concurrent whole-text edits to the SAME shared block X.
        a.record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "The quick brown fox".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
        b.record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "The quick red fox jumps".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

        // Cross-import each replica's delta into the other, then converge.
        let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_delta).await.unwrap();
        a.import_doc_update(note, &b_delta).await.unwrap();

        let ta = block_text(&a, note, A_BID_BYTES).await.unwrap_or_default();
        let tb = block_text(&b, note, A_BID_BYTES).await.unwrap_or_default();

        // Byte-identical on both replicas.
        assert_eq!(ta, tb, "replicas converge to the same merged text");
        // NOT the LWW whole-string pick: neither whole edit wholly won.
        assert_ne!(
            ta, "The quick brown fox",
            "must not be A's whole-string LWW pick"
        );
        assert_ne!(
            ta, "The quick red fox jumps",
            "must not be B's whole-string LWW pick"
        );
        // INTERLEAVED merge: both sides' contributions survive — A added
        // "brown", B added "red" and "jumps". Neither was wholly dropped.
        assert!(
            ta.contains("brown"),
            "A's edit (\"brown\") must survive the merge: {ta:?}"
        );
        assert!(
            ta.contains("red"),
            "B's edit (\"red\") must survive the merge: {ta:?}"
        );
        assert!(
            ta.contains("jumps"),
            "B's edit (\"jumps\") must survive the merge: {ta:?}"
        );
    }

    // ── Character-level splice API (collab editing C1 foundation) ─────
    //
    // `splice_block_text` lets a client send the user's ACTUAL keystroke
    // (insert at offset / delete a range) instead of re-authoring the whole
    // block text. Re-authoring Myers-diffs into DELETEs of a concurrent
    // peer's characters → clobber; a splice is a single insert/delete on the
    // block's `text_seq` LoroText, so concurrent splices INTERLEAVE.

    /// Seed a shared base for `note` with one block `A_BID_BYTES` holding
    /// `text`, then return a second replica that has imported the base — so
    /// both share the SAME `text_seq` lineage (the merge precondition, NOT
    /// disjoint twins).
    async fn splice_shared_base(
        note: [u8; 16],
        text: &str,
    ) -> (LoroEngine, LoroEngine) {
        let dev_a = DeviceId::from_bytes([0xa7; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        a.record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: text.into(),
            after_block_id: None,
        })
        .await
        .unwrap();

        let dev_b = DeviceId::from_bytes([0xb7; 16]);
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        let base = a.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &base).await.unwrap();
        // `read_block_text` maps empty text → None, so compare against the
        // expected `None` when seeding an empty block.
        let expect = if text.is_empty() { None } else { Some(text) };
        assert_eq!(
            block_text(&b, note, A_BID_BYTES).await.as_deref(),
            expect,
            "shared base seeded on B"
        );
        (a, b)
    }

    #[tokio::test]
    async fn splice_block_text_concurrent_inserts_interleave() {
        // Two replicas on a SHARED text_seq lineage each splice an insert at
        // offset 0 of an EMPTY block concurrently. Cross-importing each
        // other's since-vv delta must INTERLEAVE both inserts — both replicas
        // byte-identical, both "AAA" and "BBB" present (neither overwritten).
        let note = blake3_note_id("splice-interleave");
        // Start from an empty block so offset 0 is unambiguous on both sides.
        let (a, b) = splice_shared_base(note, "").await;

        // Capture each replica's pre-edit VV so each ships only its own splice.
        let a_vv = a.doc_version(note).await;
        let b_vv = b.doc_version(note).await;

        // Concurrent splices: A inserts "AAA" at 0, B inserts "BBB" at 0.
        let na = a
            .splice_block_text(note, A_BID_BYTES, 0, 0, "AAA")
            .await
            .unwrap();
        let nb = b
            .splice_block_text(note, A_BID_BYTES, 0, 0, "BBB")
            .await
            .unwrap();
        assert_eq!(na, 1, "A's splice applied");
        assert_eq!(nb, 1, "B's splice applied");

        // Cross-import each replica's delta into the other, then converge.
        let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_delta).await.unwrap();
        a.import_doc_update(note, &b_delta).await.unwrap();

        let ta = block_text(&a, note, A_BID_BYTES).await.unwrap_or_default();
        let tb = block_text(&b, note, A_BID_BYTES).await.unwrap_or_default();

        assert_eq!(ta, tb, "replicas converge to the same merged text");
        assert!(
            ta.contains("AAA"),
            "A's splice (\"AAA\") must survive the interleave: {ta:?}"
        );
        assert!(
            ta.contains("BBB"),
            "B's splice (\"BBB\") must survive the interleave: {ta:?}"
        );
        // A real interleave: both 3-char inserts land, so the merged text is
        // 6 chars — neither side OVERWROTE the other (that would be 3 chars).
        assert_eq!(
            ta.chars().count(),
            6,
            "both inserts present, neither overwritten: {ta:?}"
        );
    }

    #[tokio::test]
    async fn splice_block_text_utf16_offsets_handle_multibyte() {
        // The block holds "a😀b". The emoji is 2 UTF-16 code units, so the
        // offset JUST AFTER it is 3 (a=1, 😀=2 → 1+2). Splicing an insert at
        // UTF-16 offset 3 must land between 😀 and "b" — proving the offset is
        // UTF-16, not a Unicode-scalar index (which would be 2) or a byte
        // index (which would be 5).
        let note = blake3_note_id("splice-utf16");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "a😀b".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        let n = engine
            .splice_block_text(note, A_BID_BYTES, 3, 0, "X")
            .await
            .unwrap();
        assert_eq!(n, 1, "splice applied");

        let got = block_text(&engine, note, A_BID_BYTES)
            .await
            .unwrap_or_default();
        assert_eq!(
            got, "a😀Xb",
            "insert at UTF-16 offset 3 lands after the 2-unit emoji: {got:?}"
        );
    }

    #[tokio::test]
    async fn splice_block_text_delete_then_insert_replaces() {
        // A single splice with delete_len>0 AND a non-empty insert at the
        // same offset replaces the range: "hello world" → delete "world"
        // (offset 6, len 5) + insert "there" → "hello there".
        let note = blake3_note_id("splice-replace");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "hello world".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        let n = engine
            .splice_block_text(note, A_BID_BYTES, 6, 5, "there")
            .await
            .unwrap();
        assert_eq!(n, 1, "replace splice applied");

        let got = block_text(&engine, note, A_BID_BYTES)
            .await
            .unwrap_or_default();
        assert_eq!(got, "hello there", "the range was replaced: {got:?}");
    }

    #[tokio::test]
    async fn read_block_text_returns_merged_text_after_splice() {
        // The inbound live-apply read (C1-inbound): after a splice mutates a
        // block's text_seq, the public read_block_text(note, block) returns the
        // current merged text — this is what iOS reads to reconcile the open
        // editor with a remote peer's concurrent edit. Unknown note/block → None.
        let note = blake3_note_id("read-block-text");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "hello".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        engine
            .splice_block_text(note, A_BID_BYTES, 5, 0, " world")
            .await
            .unwrap();

        let got = engine.read_block_text(note, A_BID_BYTES).await;
        assert_eq!(
            got.as_deref(),
            Some("hello world"),
            "reads the merged text_seq content"
        );

        assert_eq!(
            engine.read_block_text(note, [0xcc; 16]).await,
            None,
            "unknown block → None"
        );
        assert_eq!(
            engine
                .read_block_text(blake3_note_id("nope"), A_BID_BYTES)
                .await,
            None,
            "unknown note → None"
        );
    }

    #[tokio::test]
    async fn splice_block_text_unknown_block_is_noop() {
        // A splice targeting a block_id that has no live node is a no-op and
        // returns Ok(0) — a splice is an in-place edit, the block must exist.
        let note = blake3_note_id("splice-missing");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "present".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        // B_BID_BYTES was never created in this note.
        let n = engine
            .splice_block_text(note, B_BID_BYTES, 0, 0, "X")
            .await
            .unwrap();
        assert_eq!(n, 0, "missing block → no-op Ok(0)");
        // The existing block is untouched.
        assert_eq!(
            block_text(&engine, note, A_BID_BYTES).await.as_deref(),
            Some("present"),
            "the present block is unaffected"
        );
    }

    // ---- P1.4 property ops ----

    use tesela_core::property::PropScalar;

    /// Read a block's `props` scalar by note + block id, navigating the doc
    /// the way the apply arm writes it. Mirrors `block_text`.
    async fn block_prop_scalar(
        engine: &LoroEngine,
        note_id: [u8; 16],
        block: [u8; 16],
        key: &str,
    ) -> Option<PropScalar> {
        let docs = engine.inner.docs.read().await;
        let doc = docs.get(&note_id)?;
        let tree = doc.get_tree("blocks");
        let node = find_node_by_block_id(&tree, &hex_id(&block))?;
        let meta = tree.get_meta(node).ok()?;
        let (props, _keys) = prop_containers::node_prop_containers(&meta).ok()?;
        prop_containers::prop_get_scalar(&props, key)
    }

    /// Read a block's `props` multi-value list by note + block id.
    async fn block_prop_list(
        engine: &LoroEngine,
        note_id: [u8; 16],
        block: [u8; 16],
        key: &str,
    ) -> Vec<PropScalar> {
        let docs = engine.inner.docs.read().await;
        let Some(doc) = docs.get(&note_id) else {
            return Vec::new();
        };
        let tree = doc.get_tree("blocks");
        let Some(node) = find_node_by_block_id(&tree, &hex_id(&block)) else {
            return Vec::new();
        };
        let Ok(meta) = tree.get_meta(node) else {
            return Vec::new();
        };
        let Ok((props, _keys)) = prop_containers::node_prop_containers(&meta) else {
            return Vec::new();
        };
        prop_containers::prop_get_list(&props, key)
    }

    // (a) BlockPropertySet SetScalar on a block → read it back via the engine.
    #[tokio::test]
    async fn block_property_set_scalar_round_trips() {
        let note = blake3_note_id("prop-scalar");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        upsert_block(&engine, note, A_BID_BYTES, "a block", None).await;

        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();

        assert_eq!(
            block_prop_scalar(&engine, note, A_BID_BYTES, "status").await,
            Some(PropScalar::Text("doing".into())),
            "scalar property reads back after BlockPropertySet"
        );
    }

    // A property set on a block that doesn't exist is a safe no-op, NOT a crash.
    #[tokio::test]
    async fn block_property_set_on_missing_block_is_noop() {
        let note = blake3_note_id("prop-missing-block");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        // B_BID_BYTES is never created in this note.
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: B_BID_BYTES,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("doing".into())),
            })
            .await
            .expect("property set on a missing block must not error");
        assert_eq!(
            block_prop_scalar(&engine, note, B_BID_BYTES, "status").await,
            None,
            "no node was created for the missing block"
        );
    }

    // (b) ⭐ Shared base: A splices prose on block X, B sets a property on the
    // SAME block X. Exchange both ways → BOTH survive (prose carries A's edit
    // AND the property is set — neither clobbers the other).
    #[tokio::test]
    async fn concurrent_prose_splice_and_property_set_both_survive() {
        let note = blake3_note_id("prose-vs-prop");
        let block = A_BID_BYTES;

        // Engine A builds the shared base (one block, text "Hello").
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        upsert_block(&a, note, block, "Hello", None).await;

        // Engine B imports the base so both share Loro history (same TreeID).
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        let base = a.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &base).await.unwrap();
        assert_eq!(block_text(&b, note, block).await.as_deref(), Some("Hello"));

        // Concurrent, neither has seen the other:
        //   A appends " world" to the SAME block's prose.
        //   B sets a `status` property on the SAME block.
        a.splice_block_text(note, block, 5, 0, " world").await.unwrap();
        b.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();

        // Exchange updates both ways.
        let ua = a.export_doc_update(note, None).await.unwrap();
        let ub = b.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &ua).await.unwrap();
        a.import_doc_update(note, &ub).await.unwrap();

        // Both survive on BOTH replicas: the prose carries A's edit AND the
        // property is set — neither clobbers the other.
        for (label, e) in [("A", &a), ("B", &b)] {
            assert_eq!(
                block_text(e, note, block).await.as_deref(),
                Some("Hello world"),
                "{label}: prose edit must survive the concurrent property set"
            );
            assert_eq!(
                block_prop_scalar(e, note, block, "status").await,
                Some(PropScalar::Text("doing".into())),
                "{label}: property must survive the concurrent prose edit"
            );
        }
    }

    // (c) AddToList of two DISTINCT values on the same block's "tags" from two
    // engines on a shared base → union after merge.
    //
    // The "tags" LoroList must exist in SHARED history before the two engines
    // diverge: Loro derives a child container's id from the op that created
    // it, so two peers each minting the list for the FIRST time concurrently
    // produce rival containers and one branch is overwritten (documented
    // "Container ID And Overwrite Hazards"). The realistic product path tags
    // an EXISTING block, so we seed the list once on the base (one initial
    // AddToList, imported by B) and THEN add distinct values concurrently —
    // which unions correctly because both push into the same shared container.
    #[tokio::test]
    async fn concurrent_add_to_list_unions() {
        let note = blake3_note_id("tags-union");
        let block = A_BID_BYTES;

        let dev_a = DeviceId::from_bytes([0xc1; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        upsert_block(&a, note, block, "a block", None).await;
        // Seed the shared "tags" list on the base so both replicas share its
        // container id (see the doc comment above).
        a.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "tags".into(),
            value: PropOp::AddToList(PropScalar::Text("Base".into())),
        })
        .await
        .unwrap();

        let dev_b = DeviceId::from_bytes([0xc2; 16]);
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        let base = a.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &base).await.unwrap();

        // Concurrent AddToList of DISTINCT values to the same (shared) "tags" list.
        a.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "tags".into(),
            value: PropOp::AddToList(PropScalar::Text("Task".into())),
        })
        .await
        .unwrap();
        b.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "tags".into(),
            value: PropOp::AddToList(PropScalar::Text("Urgent".into())),
        })
        .await
        .unwrap();

        let ua = a.export_doc_update(note, None).await.unwrap();
        let ub = b.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &ua).await.unwrap();
        a.import_doc_update(note, &ub).await.unwrap();

        // Union on both replicas — both distinct values present.
        for (label, e) in [("A", &a), ("B", &b)] {
            let mut tags: Vec<String> = block_prop_list(e, note, block, "tags")
                .await
                .into_iter()
                .map(|s| match s {
                    PropScalar::Text(t) => t,
                    other => format!("{other:?}"),
                })
                .collect();
            tags.sort();
            assert_eq!(
                tags,
                vec!["Base".to_string(), "Task".to_string(), "Urgent".to_string()],
                "{label}: concurrent AddToList must union both distinct values \
                 (alongside the shared base value)"
            );
        }
    }

    // ---- P1.5 container-property materialization ----

    // A scalar block property materializes as a `key:: value` continuation
    // line AFTER the block's prose in the rendered markdown.
    #[tokio::test]
    async fn render_materializes_block_scalar_property() {
        let note = blake3_note_id("mat-scalar");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        upsert_block(&engine, note, A_BID_BYTES, "Task", None).await;

        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();

        let full = engine.render_note_full(note).await.unwrap();
        assert_eq!(
            full,
            format!(
                "- Task <!-- bid:{} -->\n  status:: doing\n",
                uuid::Uuid::from_bytes(A_BID_BYTES),
            ),
            "scalar prop renders as a continuation line after the prose"
        );
    }

    // A multi-value (list) property materializes as a single comma-joined
    // `key:: a, b` line (the `tags::` join convention), stable-deduped.
    #[tokio::test]
    async fn render_materializes_block_multi_value_property() {
        let note = blake3_note_id("mat-multi");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        upsert_block(&engine, note, A_BID_BYTES, "Task", None).await;

        for v in ["Task", "Urgent"] {
            engine
                .record_local(OpPayload::BlockPropertySet {
                    note_id: note,
                    block_id: A_BID_BYTES,
                    key: "tags".into(),
                    value: PropOp::AddToList(PropScalar::Text(v.into())),
                })
                .await
                .unwrap();
        }

        let full = engine.render_note_full(note).await.unwrap();
        assert_eq!(
            full,
            format!(
                "- Task <!-- bid:{} -->\n  tags:: Task, Urgent\n",
                uuid::Uuid::from_bytes(A_BID_BYTES),
            ),
            "multi-value prop renders comma-joined in list order"
        );
    }

    // A page-level scalar property materializes at the body top, per the
    // `split_page_properties` convention.
    #[tokio::test]
    async fn render_materializes_page_property() {
        let note = blake3_note_id("mat-page");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        upsert_block(&engine, note, A_BID_BYTES, "Body", None).await;

        engine
            .record_local(OpPayload::PagePropertySet {
                note_id: note,
                key: "type".into(),
                value: PropOp::SetScalar(PropScalar::Text("Tag".into())),
            })
            .await
            .unwrap();

        let full = engine.render_note_full(note).await.unwrap();
        assert_eq!(
            full,
            format!(
                "type:: Tag\n- Body <!-- bid:{} -->\n",
                uuid::Uuid::from_bytes(A_BID_BYTES),
            ),
            "page prop renders at the body top before the bullets"
        );
    }

    // ⭐ REVIEW-GATE determinism test: the SAME set of property ops applied
    // in DIFFERENT orders to two FRESH engines, converged via export/import,
    // must render BYTE-IDENTICAL markdown. Determinism is the whole point of
    // `prop_keys` + canonical formatting + stable-dedup.
    #[tokio::test]
    async fn render_is_byte_identical_regardless_of_prop_op_order() {
        let note = blake3_note_id("mat-determinism");

        // A shared base both replicas import, so block + list containers
        // share Loro ids (the union-merge precondition the engine relies on).
        let dev_seed = DeviceId::from_bytes([0xd0; 16]);
        let seed = LoroEngine::new(dev_seed, Arc::new(Hlc::new(dev_seed)));
        upsert_block(&seed, note, A_BID_BYTES, "Task", None).await;
        // Seed the shared "tags" list (one initial value) so concurrent
        // AddToList unions instead of minting rival containers.
        seed.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "tags".into(),
            value: PropOp::AddToList(PropScalar::Text("Base".into())),
        })
        .await
        .unwrap();
        let base = seed.export_doc_update(note, None).await.unwrap();

        // The SAME logical op set, in two different orders.
        let ops_order_1 = vec![
            ("status", PropOp::SetScalar(PropScalar::Text("doing".into()))),
            ("priority", PropOp::SetScalar(PropScalar::Int(3))),
            ("tags", PropOp::AddToList(PropScalar::Text("Task".into()))),
            ("tags", PropOp::AddToList(PropScalar::Text("Urgent".into()))),
            ("note", PropOp::SetText("freeform".into())),
        ];
        let ops_order_2 = vec![
            ("tags", PropOp::AddToList(PropScalar::Text("Urgent".into()))),
            ("note", PropOp::SetText("freeform".into())),
            ("priority", PropOp::SetScalar(PropScalar::Int(3))),
            ("status", PropOp::SetScalar(PropScalar::Text("doing".into()))),
            ("tags", PropOp::AddToList(PropScalar::Text("Task".into()))),
        ];

        async fn build(
            note: [u8; 16],
            base: &[u8],
            peer: u8,
            ops: &[(&str, PropOp)],
        ) -> LoroEngine {
            let dev = DeviceId::from_bytes([peer; 16]);
            let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
            engine.import_doc_update(note, base).await.unwrap();
            for (key, value) in ops {
                engine
                    .record_local(OpPayload::BlockPropertySet {
                        note_id: note,
                        block_id: A_BID_BYTES,
                        key: (*key).into(),
                        value: value.clone(),
                    })
                    .await
                    .unwrap();
            }
            engine
        }

        let a = build(note, &base, 0xa1, &ops_order_1).await;
        let b = build(note, &base, 0xb2, &ops_order_2).await;

        // Converge: exchange full updates both ways.
        let ua = a.export_doc_update(note, None).await.unwrap();
        let ub = b.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &ua).await.unwrap();
        a.import_doc_update(note, &ub).await.unwrap();

        let ra = a.render_note_full(note).await.unwrap();
        let rb = b.render_note_full(note).await.unwrap();
        assert_eq!(
            ra, rb,
            "converged replicas must render byte-identical markdown \
             regardless of the order property ops were applied"
        );
        // And the rendered form must actually carry every property (guards
        // against the trivial both-empty pass).
        for needle in [
            "status:: doing",
            "priority:: 3",
            "tags:: Base, Task, Urgent",
            "note:: freeform",
        ] {
            assert!(ra.contains(needle), "converged render missing {needle}: {ra}");
        }
    }

    // A legacy `key:: value` line embedded in a block's TEXT (the pre-P1.6
    // form, before migrate-on-write lifts it into `props`) round-trips
    // unchanged — container props and legacy-in-text props are DISJOINT at
    // this stage, so the materializer must NOT double-emit.
    #[tokio::test]
    async fn legacy_in_text_property_round_trips_without_double_emit() {
        let note = blake3_note_id("mat-legacy");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        let bid = uuid::Uuid::from_bytes(A_BID_BYTES);
        // The legacy form: the property lives INSIDE the block text (folded
        // continuation), with NO container `props` set.
        let content = format!("- Task <!-- bid:{} -->\n  status:: doing\n", bid);

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("legacy".into()),
                title: "legacy".into(),
                content: content.clone(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        let full = engine.render_note_full(note).await.unwrap();
        assert_eq!(
            full, content,
            "legacy in-text property round-trips unchanged — no container, no double-emit"
        );
    }
}
