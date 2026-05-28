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
use loro::{ExportMode, LoroDoc, LoroTree, TreeID, TreeParentId};
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
        tokio::fs::create_dir_all(&snapshot_dir)
            .await
            .map_err(|e| {
                SyncError::Storage(format!(
                    "create loro snapshot dir {}: {e}",
                    snapshot_dir.display()
                ))
            })?;
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
        let engine = Self {
            inner: Arc::new(Inner {
                docs: RwLock::new(docs),
                device,
                hlc,
                snapshot_dir: Some(snapshot_dir.clone()),
                index,
                block_index: RwLock::new(block_index),
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
        Ok(engine)
    }

    /// Rebuild every index entry from the loaded per-note docs. Each doc
    /// stores its content on root meta (and, once written by a
    /// current-version engine, its slug + title too), so the index is a
    /// derived projection. tags/links are always re-derived from content.
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
            let content = read("content");
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
        let tree = doc.get_tree("blocks");
        let mut blocks: Vec<tesela_core::note_tree::FlatBlock> = Vec::new();
        for node in tree.children(TreeParentId::Root).unwrap_or_default() {
            if matches!(tree.is_node_deleted(&node), Ok(true)) {
                continue;
            }
            if let Some(fb) = flatblock_from_node(&tree, node) {
                blocks.push(fb);
            }
        }
        let note_tree = tesela_core::note_tree::NoteTree {
            frontmatter: None,
            page_properties: read_page_properties(doc),
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
    Some(tesela_core::note_tree::FlatBlock {
        id: id_uuid,
        parent: None,
        indent,
        text,
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

/// Read a node's `indent_level` meta. Used by BlockMove to recompute a
/// moved block's indent as parent.indent + 1, mirroring SqliteEngine.
fn read_indent_level(tree: &LoroTree, node: TreeID) -> Option<u16> {
    let meta = tree.get_meta(node).ok()?;
    let v = meta.get("indent_level")?;
    let val = v.into_value().ok()?;
    Some(val.into_i64().ok()? as u16)
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
        let text_ok = read_meta_str(tree, *node, "text").unwrap_or_default() == block.text;
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
        meta.insert("text", block.text.as_str())
            .map_err(|e| SyncError::Storage(format!("seed meta insert: {e}")))?;
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

    /// Trait-level override that forwards to the inherent
    /// `LoroEngine::render_note`. Lets `Arc<dyn SyncEngine>` callers
    /// (the server's HTTP routes) inspect the shadow without
    /// downcasting.
    async fn render_note(&self, note_id: [u8; 16]) -> Option<String> {
        LoroEngine::render_note(self, note_id).await
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
                // content (full markdown), slug, title. This lets the
                // index be rebuilt purely from per-note docs (no
                // dependence on a prior index), which is what makes the
                // index self-healing across schema changes.
                let root_meta = doc.get_map("root");
                root_meta
                    .insert("content", content.as_str())
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
            } => {
                // Flat model: every block is a direct child of root in
                // creation order. `indent_level` (from the op) carries
                // the visual hierarchy; `order_key` is ignored for
                // placement because SqliteEngine ignores it too — it
                // appends new blocks at the end of the document and
                // renders by document-order + indent. New blocks
                // `create` under root (append); existing blocks update
                // text/indent in place without moving. `parent_block_id`
                // is recorded in meta (NOT used for tree placement) so
                // BlockDelete can reparent a deleted block's direct
                // children, matching SqliteEngine (review finding [1]).
                let doc = self.doc_for_note_mut(*note_id).await;
                let tree = doc.get_tree("blocks");
                let block_hex = hex_id(block_id);
                let node = match find_node_by_block_id(&tree, &block_hex) {
                    Some(existing) => existing,
                    None => tree
                        .create(TreeParentId::Root)
                        .map_err(|e| SyncError::Storage(format!("loro tree create: {e}")))?,
                };
                let meta = tree
                    .get_meta(node)
                    .map_err(|e| SyncError::Storage(format!("loro get_meta: {e}")))?;
                meta.insert("block_id", block_hex.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                meta.insert("text", text.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
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
        };

        Ok(touched)
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
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: b, note_id, parent_block_id: Some(a),
                order_key: "b".into(), indent_level: 1, text: "B".into(),
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: c, note_id, parent_block_id: Some(b),
                order_key: "c".into(), indent_level: 2, text: "C".into(),
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
