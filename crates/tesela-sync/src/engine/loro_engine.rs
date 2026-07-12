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
    cursor::PeerCursor, LocalCursor, PendingImport, RelayApplyReport, SyncEngine,
    CATCHUP_BACKOFF_SHIFT_CAP, MAX_CATCHUP_ATTEMPTS,
};
use crate::error::{SyncError, SyncResult};
use crate::hlc::Hlc;
use crate::oplog::op::{ContentHash, EncodedOp, OpPayload, PropOp};
use crate::PropScalar;
use async_trait::async_trait;
use loro::{
    ExportMode, LoroDoc, LoroText, LoroTree, TreeID, TreeParentId, UpdateOptions, VersionVector,
};
use loro::cursor::{Cursor, Side};
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

/// Best-effort list of the Loro PeerIDs whose ops a relay update frame
/// carries, decoded from the frame's blob metadata (tesela-c7s item 2 — the
/// "from_peer" a pending causal-gap ledger entry records). Returns empty on a
/// frame whose metadata won't decode; never fails. Checksum verification is
/// skipped (`false`) — this is metadata for a log/ledger, not a trust
/// boundary, and the bytes already imported into Loro before we ask.
fn peers_of_update(bytes: &[u8]) -> Vec<u64> {
    match LoroDoc::decode_import_blob_meta(bytes, false) {
        Ok(meta) => meta.partial_end_vv.iter().map(|(peer, _)| *peer).collect(),
        Err(_) => Vec::new(),
    }
}

/// Is a DIRTY note's outbound broadcast cursor stranded — i.e. would shipping
/// `updates(cursor)` produce an empty / no-op frame instead of a real delta?
/// (tesela-c7s item 3.) The caller establishes the note is dirty (cursor bytes
/// != `current_enc`); this classifies WHY the incremental export can't carry
/// content:
/// - undecodable cursor (2026-06-25 class) → stranded;
/// - cursor whose VV COVERS the doc's current version (stale-ahead, 2026-06-29
///   class — e.g. an authoritative import rebased current 'backward') →
///   stranded;
/// - a cursor that is behind or DISJOINT from current → NOT stranded (it ships
///   a genuine delta).
/// `since == None` (never broadcast) is a first-snapshot, not a strand.
fn outbound_cursor_stranded(since: Option<&[u8]>, current_enc: &[u8]) -> bool {
    let Some(since_bytes) = since else {
        return false;
    };
    let Ok(current_vv) = VersionVector::decode(current_enc) else {
        return false;
    };
    match VersionVector::decode(since_bytes) {
        Err(_) => true,
        Ok(since_vv) => since_vv.includes_vv(&current_vv),
    }
}

/// Should the outbound cursor be REWOUND to a just-deposited snapshot's
/// version (tesela-c7s item 4)? Only when the current cursor is genuinely
/// STRANDED relative to that snapshot — undecodable, or covering (at-or-ahead
/// of) the snapshot version so the next incremental export would be empty.
/// A cursor equal to, behind, or disjoint from the snapshot is NOT stranded:
/// repairing it would rewind a healthy cursor and force a redundant
/// re-broadcast, so it is left alone. `None` (never broadcast) is never
/// repaired — the first real broadcast establishes the cursor.
fn broadcast_cursor_needs_repair(existing: Option<&[u8]>, snap_vv_enc: &[u8]) -> bool {
    let Some(existing_bytes) = existing else {
        return false;
    };
    let Ok(snap_vv) = VersionVector::decode(snap_vv_enc) else {
        return false;
    };
    match VersionVector::decode(existing_bytes) {
        Err(_) => true,
        // Cursor covers the snapshot and isn't identical to it → stale-ahead
        // strand → rewind. Identical → already anchored, nothing to heal.
        Ok(cur_vv) => cur_vv != snap_vv && cur_vv.includes_vv(&snap_vv),
    }
}

/// Well-known doc id of the SYNCED VIEWS REGISTRY doc (saved-views spec,
/// 2026-06-10): the 16 ASCII bytes `tesela.views.reg`
/// (hex `746573656c612e76696577732e726567`). Chosen over a random UUID so
/// the id is self-describing in snapshot filenames / relay logs, and over
/// a nil/derived id so it can never collide with a real note id — note ids
/// are blake3-of-slug truncations (or minted UUIDs), and a slug whose hash
/// lands on this exact ASCII string is astronomically unlikely.
///
/// The views doc lives IN the engine's `docs` map under this id, which is
/// precisely what makes it sync like a note doc with zero wire changes:
/// `produce_relay_updates` / `apply_relay_updates` / snapshot deposits /
/// bootstrap all iterate or address `docs` by `[u8; 16]` id. The id is
/// instead EXCLUDED from every note-shaped projection (materialization,
/// index, render, twin heal, note-op apply) via explicit guards.
pub const VIEWS_DOC_ID: [u8; 16] = *b"tesela.views.reg";

/// Fixed id of the built-in Inbox view. A CONSTANT (not a minted UUID) so
/// two devices seeding concurrently write the SAME registry entry with the
/// same field values — whichever insert wins the map-key LWW, the group
/// converges to one identical Inbox.
pub const INBOX_VIEW_ID: &str = "builtin-inbox";

/// Default DSL of the built-in Inbox view (Taylor's definition: backlog or
/// todo status, no scheduled or deadline date). User-editable after seed.
/// Re-exports the canonical const from `tesela-core` (landed in c3afb69
/// alongside the comma-OR DSL support that makes it parse).
pub const INBOX_DEFAULT_DSL: &str = tesela_core::query::INBOX_VIEW_DSL;

/// Schema version stamped on the views registry doc's `meta` map, for
/// future shape evolution (mirrors `INDEX_SCHEMA_VERSION`'s role —
/// though the views doc is authoritative CRDT state, never rebuilt).
const VIEWS_SCHEMA_VERSION: i64 = 1;

/// The Loro PeerID every device uses to author the deterministic
/// builtin-views seed ops (`builtin_views_seed_update`). Reserved: a real
/// engine can never collide — `LoroEngine::peer_id` masks the top bit AND
/// maps a masked 0 to 1, so no device doc ever writes as peer 0. And at
/// equal lamport Loro's map LWW picks the GREATER `(lamport, peer)`
/// (`MapValue::cmp`), so a seed container also LOSES any tie against a
/// pre-fix randomly-peered container — the migration-safe direction (the
/// container that may carry user edits wins).
const BUILTIN_VIEWS_SEED_PEER: u64 = 0;

/// The builtin-views seed as ONE deterministic Loro update: a scratch doc
/// with the reserved seed peer (and timestamp recording off) authors the
/// Inbox entry + `meta.schema_version`, byte-identical on every device.
/// Two devices that seed independently therefore author the SAME op IDs —
/// there is no same-key `insert_container` race for a later merge to
/// resolve by dropping one container (and its user edits) wholesale; the
/// adversarial-review fresh-device-clobber vector (2026-06-10).
fn builtin_views_seed_update() -> SyncResult<Vec<u8>> {
    let doc = LoroDoc::new();
    // Default is already false; pinned explicitly because a recorded
    // wall-clock timestamp would make the seed ops differ per device.
    doc.set_record_timestamp(false);
    doc.set_peer_id(BUILTIN_VIEWS_SEED_PEER)
        .map_err(|e| SyncError::Storage(format!("views seed peer: {e}")))?;
    let views = doc.get_map("views");
    let entry = views
        .insert_container(INBOX_VIEW_ID, loro::LoroMap::new())
        .map_err(|e| SyncError::Storage(format!("views seed insert_container: {e}")))?;
    let ins = |e: loro::LoroError| SyncError::Storage(format!("views seed insert: {e}"));
    entry.insert("id", INBOX_VIEW_ID).map_err(ins)?;
    entry.insert("name", "Views").map_err(ins)?;
    entry.insert("dsl", INBOX_DEFAULT_DSL).map_err(ins)?;
    entry.insert("order", 0i64).map_err(ins)?;
    entry.insert("builtin", true).map_err(ins)?;
    entry.insert("display_mode", "list").map_err(ins)?;
    doc.get_map("meta")
        .insert("schema_version", VIEWS_SCHEMA_VERSION)
        .map_err(ins)?;
    doc.commit();
    doc.export(ExportMode::all_updates())
        .map_err(|e| SyncError::Storage(format!("views seed export: {e}")))
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
    /// Migrate-on-apply (P1.6) toggle. When `true`, the `BlockUpsert` apply
    /// arm lifts recognized in-text `key:: value` continuation lines out of the
    /// incoming prose into the typed `props`/`prop_keys` container (prose-only
    /// `text_seq`, one commit, idempotent). DEFAULT-OFF — resolved ONCE at
    /// construction from `TESELA_LORO_MIGRATE_IN_TEXT` (mirrors how
    /// `TESELA_LORO_RESEED` is read once at boot). The flag stays off until the
    /// WHOLE fleet (incl. iOS old FFI) is props-read-capable; an old reader that
    /// can't read the lifted container would render property-less and could
    /// re-broadcast a fleet-wide erase. The rendered VIEW keeps emitting
    /// `key:: value` lines regardless (from `FlatBlock.properties`), so an old
    /// reader still SEES the property as text.
    migrate_in_text: bool,
    /// Per-note apply serialization (tesela-4ju). `apply_import`'s
    /// plan→import→tombstone sequence reads the pre-import doc, mutates it,
    /// then resolves disjoint twins from post-import state — none of that is
    /// atomic against a CONCURRENT `apply_import` for the SAME note (the docs
    /// map's write lock, taken only inside `doc_for_note_mut`, is released
    /// before the sequence starts). Without this, two racing applies for one
    /// note could interleave: a second import lands between the first's plan
    /// fork and its tombstone pass, so the tombstone — sized to the FIRST
    /// import's twin set — can delete a block the second import just
    /// legitimately created or edited. One `tokio::sync::Mutex` per note_id,
    /// held for the entire `apply_import` body, closes the window. Lazily
    /// created; never removed (bounded by note count, matches `docs`).
    ///
    /// Widened post-review (tesela-4ju REVIEW REJECT, 2026-07-02): the SAME
    /// interleave is reachable via `record_local` (a local edit racing an
    /// inbound `apply_import` for the same note) and via `heal_disjoint_twins`
    /// (its own plan/tombstone/reassert sequence, run frameless). Both now
    /// acquire this lock for the same note before mutating — see
    /// `record_local`'s trait impl and `heal_disjoint_twins`.
    ///
    /// **Lock ordering rule**: `apply_locks` is always acquired (via
    /// `apply_lock_for_note`) and its per-note `Mutex` guard established
    /// BEFORE any subsequent read/write of `docs` (or `block_index`) within
    /// the same call — never the reverse, and never hold `docs`'s lock across
    /// an `.await` that then tries to acquire an `apply_locks` guard. Every
    /// caller that resolves a note-scoped payload's target note_id first
    /// (`note_id_for_payload`, a `block_index` read released before the
    /// guard is taken) upholds this. `apply_locks` itself is never held
    /// across an inner acquisition of `apply_locks` for a DIFFERENT note_id —
    /// only one per-note guard is ever live per call stack, so the per-note
    /// `Mutex` is never reentered: internal helpers invoked from inside an
    /// already-guarded body (`reassert_prop_heals`) call the lock-free
    /// `record_local_locked` rather than the public `record_local`, which
    /// would deadlock trying to re-acquire the SAME note's non-reentrant
    /// guard.
    apply_locks: RwLock<HashMap<[u8; 16], Arc<tokio::sync::Mutex<()>>>>,
    /// Durable causal-gap ledger (tesela-c7s item 2): note_id → the
    /// [`PendingImport`] record for a note whose inbound relay update Loro
    /// left PENDING (referenced ops the doc is missing). Recorded here
    /// STRUCTURALLY by `apply_relay_updates` — not merely `tracing::warn`'d —
    /// so the strand is observable ([`pending_import_notes`]) and self-healing
    /// ([`notes_needing_snapshot_catchup`] drives an auto snapshot catch-up).
    /// Cleared when a later delta OR an authoritative snapshot fully
    /// integrates the note. Persisted to `<snapshot_dir>/_pending_imports.bin`
    /// (mirrors the broadcast cursor) so a restart doesn't forget an
    /// unresolved gap.
    ///
    /// [`pending_import_notes`]: LoroEngine::pending_import_notes
    /// [`notes_needing_snapshot_catchup`]: LoroEngine::notes_needing_snapshot_catchup
    pending_imports: RwLock<HashMap<[u8; 16], PendingImport>>,
    /// Monotonic "inbound apply pass" counter — bumped once per
    /// `apply_relay_updates` batch. One batch = one tick's worth of inbound
    /// relay updates; a pending note whose `first_seen_pass` is strictly below
    /// this has survived at least one whole pass still stuck (the "past one
    /// tick" auto-heal boundary).
    import_pass: AtomicU64,
    /// Count of OUTBOUND STRAND ALARMS (tesela-c7s item 3): incremented by
    /// `produce_relay_updates` every time a note that is dirty since its last
    /// confirmed PUT could NOT ship an incremental delta — its broadcast
    /// cursor is stale-ahead of (covers) the doc's current version, or it
    /// won't decode — so we fall back to a full snapshot instead of shipping a
    /// content-less/empty frame. A rising count is the live signature of the
    /// deposit-strand class; surfaced for the server/FFI tick to log.
    outbound_strand_alarms: AtomicU64,
}

/// Resolve the migrate-on-apply (P1.6) flag from the environment ONCE — mirrors
/// how `TESELA_LORO_RESEED` is read a single time at boot. DEFAULT-OFF: anything
/// but a non-empty value leaves migration disabled.
fn migrate_in_text_from_env() -> bool {
    std::env::var("TESELA_LORO_MIGRATE_IN_TEXT")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
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
                migrate_in_text: migrate_in_text_from_env(),
                apply_locks: RwLock::new(HashMap::new()),
                pending_imports: RwLock::new(HashMap::new()),
                import_pass: AtomicU64::new(0),
                outbound_strand_alarms: AtomicU64::new(0),
            }),
        }
    }

    /// Construct an in-memory engine with the migrate-on-apply (P1.6) flag
    /// FORCED ON — for tests that exercise the lift path without depending on a
    /// process-global env var (`TESELA_LORO_MIGRATE_IN_TEXT`), which would race
    /// across the parallel test runner.
    #[cfg(test)]
    fn new_migrating(device: DeviceId, hlc: Arc<Hlc>) -> Self {
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
                migrate_in_text: true,
                apply_locks: RwLock::new(HashMap::new()),
                pending_imports: RwLock::new(HashMap::new()),
                import_pass: AtomicU64::new(0),
                outbound_strand_alarms: AtomicU64::new(0),
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
        // Restore the causal-gap ledger so a restart doesn't forget a note
        // that was still pending a snapshot catch-up (tesela-c7s item 2).
        let pending_imports = load_pending_imports(&snapshot_dir).await;
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
                migrate_in_text: migrate_in_text_from_env(),
                apply_locks: RwLock::new(HashMap::new()),
                pending_imports: RwLock::new(pending_imports),
                import_pass: AtomicU64::new(0),
                outbound_strand_alarms: AtomicU64::new(0),
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
    /// the note is unknown (never resident and no on-disk snapshot);
    /// otherwise lazy-loads. (Phase 4.)
    pub async fn doc_version(&self, note_id: [u8; 16]) -> Option<Vec<u8>> {
        Some(self.lazy_load_doc(note_id).await?.oplog_vv().encode())
    }

    /// Export a note's Loro updates since the peer's (encoded) version
    /// vector. `since = None` exports full state — a fresh-device
    /// bootstrap. None if the note is unknown or export fails; otherwise
    /// lazy-loads. (Phase 4.)
    pub async fn export_doc_update(
        &self,
        note_id: [u8; 16],
        since: Option<&[u8]>,
    ) -> Option<Vec<u8>> {
        let doc = self.lazy_load_doc(note_id).await?;
        let doc = &doc;
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
            // Decode the cursor and ship the incremental delta. A cursor that
            // FAILS TO DECODE (corrupt, or a version-format/lineage change) or
            // whose incremental export fails must NOT silently strand the note
            // — fall back to a full snapshot so a dirty note always exports.
            // The receiver imports a snapshot idempotently (no convergence
            // risk), and the next confirmed PUT rewrites a fresh, decodable
            // cursor → back to incremental. (2026-06-25: an un-decodable
            // broadcast cursor returned None here, and produce_relay_updates
            // silently skipped the note — iOS edits never reached the desktop.)
            //
            // A cursor that DECODES can still strand the note: if its VV is
            // AT-OR-AHEAD of the doc's CURRENT version (`vv` covers current —
            // a stale-ahead cursor, or one left ahead after an authoritative
            // import rebased current 'backward'), `updates(vv)` is an
            // EMPTY/no-op delta, `.ok()` is `Some(empty)`, and the snapshot
            // `.or_else` never runs — so produce ships a content-less frame
            // forever. Detect that with `includes_vv` and snapshot instead.
            // (2026-06-29: confirmed live on iOS — splice applied=1 but
            // tick_outbound shipped 0 effective ops.) A DISJOINT cursor
            // (concurrent: neither includes the other) is NOT ahead, so it
            // still takes the incremental path — `updates(vv)` there is a real
            // non-empty delta of the ops current holds that vv lacks.
            Some(bytes) => match VersionVector::decode(bytes) {
                Ok(vv) if vv.includes_vv(&doc.oplog_vv()) => {
                    doc.export(ExportMode::Snapshot).ok()
                }
                Ok(vv) => doc
                    .export(ExportMode::updates(&vv))
                    .ok()
                    .or_else(|| doc.export(ExportMode::Snapshot).ok()),
                Err(_) => doc.export(ExportMode::Snapshot).ok(),
            },
            None => doc.export(ExportMode::Snapshot).ok(),
        }
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
    pub async fn read_block_text(&self, note_id: [u8; 16], block_id: [u8; 16]) -> Option<String> {
        let doc = self.lazy_load_doc(note_id).await?;
        let tree = doc.get_tree("blocks");
        let block_hex = hex_id(&block_id);
        let node = find_node_by_block_id(&tree, &block_hex)?;
        read_block_text(&tree, node)
    }

    /// Mint a stable, op-anchored [`loro::cursor::Cursor`] at `utf16_offset`
    /// within a block's `text_seq` LoroText, returned as postcard bytes for
    /// transport. Unlike a raw index, the cursor follows concurrent edits, so a
    /// remote caret stays correct through the other peer's typing. `None` for an
    /// unknown note/block. (Phase 1 presence foundation.)
    pub async fn mint_block_cursor(
        &self,
        note_id: [u8; 16],
        block_id: [u8; 16],
        utf16_offset: u32,
    ) -> Option<Vec<u8>> {
        let doc = self.doc_for_note_mut(note_id).await;
        let tree = doc.get_tree("blocks");
        let block_hex = hex_id(&block_id);
        let node = find_node_by_block_id(&tree, &block_hex)?;
        let meta = tree.get_meta(node).ok()?;
        // The SAME `text_seq` container splice/upsert/read use. Existing-text
        // blocks already have it, so get-or-create returns it (no new child).
        let text: LoroText = meta
            .get_or_create_container("text_seq", LoroText::new())
            .ok()?;
        // Anchor to the LEFT of the offset (the char before it), so the caret
        // rides along when text is inserted before it.
        let cursor = text.get_cursor(utf16_offset as usize, Side::Left)?;
        Some(cursor.encode())
    }

    /// Resolve an encoded cursor (from [`mint_block_cursor`](Self::mint_block_cursor))
    /// to its CURRENT utf16 offset in this engine's doc, accounting for edits
    /// applied since it was minted. `None` if the cursor can't be placed (e.g.
    /// its block was deleted).
    pub async fn resolve_block_cursor(&self, note_id: [u8; 16], cursor_bytes: &[u8]) -> Option<u32> {
        let doc = self.doc_for_note_mut(note_id).await;
        let cursor = Cursor::decode(cursor_bytes).ok()?;
        let pos = doc.get_cursor_pos(&cursor).ok()?;
        Some(pos.current.pos as u32)
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
        // Residency-independent candidate set (tesela-engc.5 audit: this is a
        // FULL-MAP walk, not a single KEYED lookup — `doc_version` /
        // `export_doc_update` below lazy-load on demand, but only for a
        // note_id this loop actually visits). Union memory-resident docs
        // (covers the views registry doc, which the always-resident `index`
        // deliberately excludes) with every note the always-resident index
        // knows about (covers a note whose doc isn't currently in
        // `self.inner.docs` — not-yet-loaded today, or evicted once eviction
        // lands), so an evicted note's un-broadcast local edits are never
        // silently dropped from a relay tick.
        let mut note_ids: std::collections::HashSet<[u8; 16]> =
            self.inner.docs.read().await.keys().copied().collect();
        for entry in self.index_entries().await {
            if let Some(id) = parse_note_id_from_hex(&entry.note_id) {
                note_ids.insert(id);
            }
        }
        let mut out = Vec::new();
        for note_id in note_ids {
            let current = match self.doc_version(note_id).await {
                Some(v) => v,
                None => continue,
            };
            let since = self
                .inner
                .broadcast_cursor
                .read()
                .await
                .get(&note_id)
                .cloned();
            // Nothing new since last broadcast → skip.
            if since.as_deref() == Some(current.as_slice()) {
                continue;
            }
            // OUTBOUND STRAND ALARM (tesela-c7s item 3). We are here because
            // the note is DIRTY (its cursor != its current version). If the
            // cursor is stale-AHEAD of current (it already covers every op the
            // doc holds) or won't decode, an incremental `updates(cursor)` is
            // an empty / no-op frame — the deposit-strand class (2026-06-25
            // undecodable, 2026-06-29 stale-ahead). `export_doc_update`
            // already rescues correctness by falling back to a full snapshot,
            // but a dirty note that could not ship an incremental delta is a
            // BUG worth shouting about, not a silent snapshot every tick: log
            // loudly + bump the alarm so the server/FFI tick surfaces it and
            // the class is observable on the live fleet (where it presented as
            // "ZERO PUT /ops despite fresh edits"). A DISJOINT cursor (neither
            // covers the other) is NOT a strand — it ships a real delta — so
            // it must not raise the alarm.
            if outbound_cursor_stranded(since.as_deref(), &current) {
                self.inner
                    .outbound_strand_alarms
                    .fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    "tesela-sync/loro: OUTBOUND STRAND for {} — dirty note's broadcast \
                     cursor is stale-ahead/undecodable; shipping a full-snapshot fallback \
                     instead of an empty delta (deposit-strand class, tesela-c7s)",
                    hex_id(&note_id)
                );
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

    /// HEAL a stranded outbound cursor after a note's full snapshot was
    /// CONFIRMED deposited to the relay (tesela-c7s item 4). Each pair is
    /// `(note_id, vv_at_snapshot_time)` — the doc's version vector captured at
    /// the moment its snapshot was EXPORTED, NOT re-read at confirm time.
    ///
    /// Why this exists: a stale-ahead / undecodable outbound cursor makes the
    /// broadcast producer ship a snapshot every tick and NEVER re-anchor to an
    /// incremental delta, and the relay snapshot deposit — while it carries the
    /// content — is invisible to peers that only poll `GET ops?since=N`
    /// (they read snapshots only on bootstrap/catch-up). So the fallback that
    /// SUCCEEDED at durability left the strand intact and looped forever. This
    /// re-anchors the cursor to the deposited snapshot's version so the NEXT
    /// local edit ships a real incremental delta over the ops stream again —
    /// the fallback HEALS the strand instead of masking it.
    ///
    /// SAFE by construction (the "snapshot time, not confirm time" rule):
    /// - It only ever moves a STRANDED cursor (undecodable, or one that COVERS
    ///   the snapshot version) DOWN to the snapshot version. A cursor that is
    ///   behind or disjoint from the snapshot is left untouched — the normal
    ///   `produce`/`commit` path owns it and would ship the missing ops as a
    ///   delta; rewinding it would just re-broadcast.
    /// - Because the vv is captured at snapshot-EXPORT time, any local edit
    ///   recorded AFTER the snapshot was cut but before this confirm is NOT
    ///   swallowed: it advances `current` past the snapshot vv, so after the
    ///   repair the cursor (= snapshot vv) is strictly behind `current` and the
    ///   next `produce` ships that edit incrementally. (Re-reading the vv at
    ///   confirm time would instead skip it.)
    pub async fn repair_broadcast_cursors_after_snapshot(
        &self,
        committed: &[([u8; 16], Vec<u8>)],
    ) {
        if committed.is_empty() {
            return;
        }
        let mut repaired = false;
        {
            let mut cur = self.inner.broadcast_cursor.write().await;
            for (note_id, snap_vv) in committed {
                let existing = cur.get(note_id).map(|v| v.as_slice());
                if broadcast_cursor_needs_repair(existing, snap_vv) {
                    cur.insert(*note_id, snap_vv.clone());
                    repaired = true;
                    tracing::warn!(
                        "tesela-sync/loro: REPAIRED outbound cursor for {} to its \
                         snapshot-time version after a confirmed snapshot deposit \
                         (healing the deposit-strand, tesela-c7s)",
                        hex_id(note_id)
                    );
                }
            }
        }
        if repaired {
            self.save_broadcast_cursors().await;
        }
    }

    /// Apply a batch of broadcast per-note Loro updates (the inbound
    /// relay tick). Idempotent + commutative — duplicate / out-of-order
    /// batches are safe. Returns a per-note [`RelayApplyReport`]: which
    /// notes applied cleanly, which were left PENDING by Loro (causal gap
    /// — the caller should snapshot-catch-up those notes), and which
    /// failed (the caller must not silently ack past them). Every failure
    /// is also warn-logged here, so even a caller that drops the report
    /// leaves a trace (audit A4).
    pub async fn apply_relay_updates(&self, updates: &[([u8; 16], Vec<u8>)]) -> RelayApplyReport {
        // One inbound relay BATCH = one apply pass = one "tick" for the
        // causal-gap ledger's "past one tick" auto-heal boundary (tesela-c7s
        // item 2). Bump once per batch, before any per-note record.
        let pass = self.inner.import_pass.fetch_add(1, Ordering::Relaxed) + 1;
        let mut report = RelayApplyReport::default();
        for (note_id, bytes) in updates {
            // Fully-qualified call: `apply_doc_update_status` also exists on
            // the `SyncEngine` trait, so the unqualified call would be
            // ambiguous-by-convention here (and a recursion trap if this body
            // were ever reached through `dyn SyncEngine`). Pin it to the
            // inherent method like every other call site in this file. The
            // status-aware path runs the SAME protected apply as
            // `import_doc_update` but additionally surfaces Loro's pending
            // status instead of discarding it.
            match LoroEngine::apply_doc_update_status(self, *note_id, bytes).await {
                Ok(false) => {
                    report.applied.push(*note_id);
                    // A clean apply HEALED any prior causal gap for this note
                    // (the missing base arrived) — drop it from the ledger.
                    self.clear_pending_import(*note_id).await;
                }
                Ok(true) => {
                    tracing::warn!(
                        "tesela-sync/loro: relay update for {} imported PENDING \
                         (causal gap) — recording in ledger + needs snapshot catch-up",
                        hex_id(note_id)
                    );
                    // DURABLE record (tesela-c7s item 2): not just this warn.
                    self.record_pending_import(*note_id, pass, peers_of_update(bytes))
                        .await;
                    report.pending.push(*note_id);
                }
                Err(e) => {
                    tracing::warn!(
                        "tesela-sync/loro: relay update for {} failed to apply: {e}",
                        hex_id(note_id)
                    );
                    report.failed.push((*note_id, e.to_string()));
                }
            }
        }
        report
    }

    /// Refresh derived state for one note after its doc changed via an
    /// import: re-register its live blocks in block_index and rebuild
    /// its index entry from the doc's root content.
    async fn refresh_note_derived(&self, note_id: [u8; 16], doc: &LoroDoc) {
        // The views registry doc is not a note: no blocks to register, and
        // it must NOT grow a phantom index entry / appear in note lists.
        if Self::is_views_doc(&note_id) {
            return;
        }
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

    /// This engine's Loro PeerID, derived deterministically from its
    /// 16-byte DeviceId (first 8 bytes, top bit cleared to stay in
    /// Loro's valid PeerID range). Stable across restarts so a device's
    /// ops are always attributed to it — the prerequisite for two
    /// engines' per-note docs merging cleanly (Phase 4).
    fn peer_id(&self) -> u64 {
        let b = self.inner.device.as_bytes();
        let raw = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]);
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
    ///
    /// Also turns ON change-timestamp recording (tesela-c7s item 1): every
    /// REAL local authoring op this device commits carries a wall-clock
    /// Unix-seconds stamp, so a stranded/undecodable-cursor investigation can
    /// see WHEN a note last actually changed and any future recency-aware
    /// twin resolution has a signal. This is the "real local authoring only"
    /// scope: `set_doc_peer` is called at doc create/load for per-note docs +
    /// the index + the views registry — never on the deterministic
    /// `builtin_views_seed_update` scratch doc, which builds with a fresh
    /// `LoroDoc` and pins `set_record_timestamp(false)` so its ops stay
    /// byte-identical across devices (the fresh-device-clobber invariant).
    /// Timestamps are runtime-only metadata (`set_record_timestamp`'s own
    /// docs: "not serialized into updates or snapshots"; must be reapplied per
    /// doc) and do NOT feed Loro's map/text LWW (that is `(lamport, peer)`),
    /// so enabling this changes observability, never merge/convergence.
    fn set_doc_peer(&self, doc: &LoroDoc) {
        let _ = doc.set_peer_id(self.peer_id());
        doc.set_record_timestamp(true);
    }

    /// Get-or-create this note's apply-serialization lock (tesela-4ju). See
    /// `Inner::apply_locks` for why `apply_import` must hold this for its
    /// entire plan→import→tombstone sequence.
    async fn apply_lock_for_note(&self, note_id: [u8; 16]) -> Arc<tokio::sync::Mutex<()>> {
        if let Some(lock) = self.inner.apply_locks.read().await.get(&note_id) {
            return lock.clone();
        }
        self.inner
            .apply_locks
            .write()
            .await
            .entry(note_id)
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// Resolve a note's doc, transparently lazy-loading its `.bin` snapshot
    /// from `snapshot_dir` into `self.inner.docs` when the map doesn't
    /// currently hold it — the eviction-ready reload every future `evict()`
    /// depends on (tesela-qql / tesela-engc.5 residency audit): a note
    /// dropped from the map by eviction must be indistinguishable from one
    /// that stayed resident. Returns `None` when the note isn't resident AND
    /// has no on-disk snapshot (a genuinely unknown note, or an in-memory-only
    /// engine with no `snapshot_dir`) — never fabricates content.
    ///
    /// Double-checks under the write lock (`entry().or_insert`) so a race
    /// between two callers loading the same note settles on ONE doc
    /// instance; the loser's freshly-imported doc is simply dropped.
    async fn lazy_load_doc(&self, note_id: [u8; 16]) -> Option<LoroDoc> {
        {
            let docs = self.inner.docs.read().await;
            if let Some(doc) = docs.get(&note_id) {
                return Some(doc.clone());
            }
        }
        let dir = self.inner.snapshot_dir.as_ref()?;
        let path = dir.join(format!("{}.bin", hex_id(&note_id)));
        let bytes = tokio::fs::read(&path).await.ok()?;
        let doc = LoroDoc::new();
        if let Err(e) = doc.import(&bytes) {
            tracing::warn!(
                "tesela-sync/loro: lazy-load snapshot {}: {e}",
                path.display()
            );
            return None;
        }
        self.set_doc_peer(&doc);
        let mut docs = self.inner.docs.write().await;
        Some(docs.entry(note_id).or_insert(doc).clone())
    }

    /// Resolve the note a payload's mutation targets, so `record_local` can
    /// take that note's `apply_locks` guard BEFORE mutating (tesela-4ju
    /// REVIEW REJECT follow-up: without this, a local edit could interleave
    /// with a concurrent `apply_import` for the same note, same as the
    /// apply-vs-apply race the lock was originally added for).
    ///
    /// Ops that carry `note_id` directly return it as-is. `BlockMove` /
    /// `BlockDelete` carry only a `block_id`, so it's resolved through
    /// `block_index` — read-then-release, same as `find_doc_for_block`,
    /// upholding the "`apply_locks` before `docs`/`block_index`" ordering
    /// rule on `Inner::apply_locks` (this lookup fully completes and drops
    /// its lock before the caller acquires the note's apply guard). An
    /// unregistered block_id resolves to `None` — `apply_payload_inner`
    /// treats that as a no-op too, so no lock is needed. Attachment ops
    /// never touch a per-note doc (see the no-op arm in
    /// `apply_payload_inner`) and always resolve to `None`.
    async fn note_id_for_payload(&self, payload: &OpPayload) -> Option<[u8; 16]> {
        match payload {
            OpPayload::NoteUpsert { note_id, .. }
            | OpPayload::NoteDelete { note_id, .. }
            | OpPayload::BlockUpsert { note_id, .. }
            | OpPayload::BlockPropertySet { note_id, .. }
            | OpPayload::PagePropertySet { note_id, .. } => Some(*note_id),
            OpPayload::BlockMove { block_id, .. } | OpPayload::BlockDelete { block_id } => {
                self.inner.block_index.read().await.get(block_id).copied()
            }
            OpPayload::AttachmentUpsert { .. } | OpPayload::AttachmentDelete { .. } => None,
        }
    }

    /// Get-or-create the Loro doc for a given note id, with this engine's
    /// PeerID stamped. Called from `record_local` when a NoteUpsert or
    /// BlockUpsert lands. Tries [`lazy_load_doc`](Self::lazy_load_doc) first
    /// — a note whose doc was dropped from memory (future eviction) but whose
    /// `.bin` survives on disk MUST be reloaded here, not silently recreated
    /// empty (the tesela-qql landmine): every local-edit path funnels
    /// through this one entry point.
    async fn doc_for_note_mut(&self, note_id: [u8; 16]) -> LoroDoc {
        if let Some(doc) = self.lazy_load_doc(note_id).await {
            return doc;
        }
        let mut docs = self.inner.docs.write().await;
        docs.entry(note_id)
            .or_insert_with(|| {
                let doc = LoroDoc::new();
                self.set_doc_peer(&doc);
                doc
            })
            .clone()
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
    async fn find_doc_for_block(&self, block_id: &[u8; 16]) -> Option<([u8; 16], LoroDoc, TreeID)> {
        let note_id = *self.inner.block_index.read().await.get(block_id)?;
        let block_hex = hex_id(block_id);
        // The final `docs` step is the one KEYED lookup here that needs
        // load-on-demand (tesela-engc.5 audit) — `block_index` above is
        // always-resident by design and already resolves the owning note
        // for an evicted-but-on-disk block.
        let doc = self.lazy_load_doc(note_id).await?;
        let tree = doc.get_tree("blocks");
        let node = find_node_by_block_id(&tree, &block_hex)?;
        Some((note_id, doc, node))
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
                    tracing::warn!("tesela-sync/loro: snapshot write {}: {e}", tmp.display());
                    return;
                }
                if let Err(e) = tokio::fs::rename(&tmp, &path).await {
                    tracing::warn!("tesela-sync/loro: snapshot rename {}: {e}", path.display());
                    let _ = tokio::fs::remove_file(&tmp).await;
                }
            }
            None => {
                // Doc gone (NoteDelete). Remove the snapshot if present.
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        tracing::warn!("tesela-sync/loro: snapshot delete {}: {e}", path.display());
                    }
                }
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

    /// Persist the causal-gap ledger to `<snapshot_dir>/_pending_imports.bin`
    /// (postcard of `Vec<PendingImport>`; tesela-c7s item 2). Best-effort —
    /// a lost ledger only costs re-detecting the gap on the next inbound
    /// pending frame, so a failed write is swallowed (mirrors the broadcast
    /// cursor's crash-safety posture).
    async fn save_pending_imports(&self) {
        let Some(dir) = self.inner.snapshot_dir.as_ref() else {
            return;
        };
        let entries: Vec<PendingImport> = self
            .inner
            .pending_imports
            .read()
            .await
            .values()
            .cloned()
            .collect();
        let bytes = match postcard::to_allocvec(&entries) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("tesela-sync/loro: pending-import ledger encode: {e}");
                return;
            }
        };
        let path = dir.join("_pending_imports.bin");
        let tmp = unique_tmp(&path);
        if tokio::fs::write(&tmp, &bytes).await.is_ok() {
            if tokio::fs::rename(&tmp, &path).await.is_err() {
                let _ = tokio::fs::remove_file(&tmp).await;
            }
        } else {
            let _ = tokio::fs::remove_file(&tmp).await;
        }
    }

    /// Record that `note_id`'s inbound update landed PENDING at the current
    /// apply pass (tesela-c7s item 2). Preserves the `first_seen_pass` of an
    /// existing entry (so "past one tick" is measured from the FIRST stall,
    /// not the latest re-observation), refreshes `last_seen_pass`, and unions
    /// in the frame's peers. Persists the ledger. No-op-safe to call twice.
    async fn record_pending_import(&self, note_id: [u8; 16], pass: u64, from_peers: Vec<u64>) {
        {
            let mut ledger = self.inner.pending_imports.write().await;
            let entry = ledger.entry(note_id).or_insert_with(|| PendingImport {
                note_id,
                first_seen_pass: pass,
                last_seen_pass: pass,
                from_peers: Vec::new(),
                ..Default::default()
            });
            entry.last_seen_pass = pass;
            for p in from_peers {
                if !entry.from_peers.contains(&p) {
                    entry.from_peers.push(p);
                }
            }
        }
        self.save_pending_imports().await;
    }

    /// Clear `note_id` from the causal-gap ledger — a later delta OR an
    /// authoritative snapshot fully integrated it, so the gap healed
    /// (tesela-c7s item 2). Persists only when something was actually removed.
    async fn clear_pending_import(&self, note_id: [u8; 16]) {
        let removed = self.inner.pending_imports.write().await.remove(&note_id);
        if removed.is_some() {
            self.save_pending_imports().await;
        }
    }

    /// Snapshot of the causal-gap ledger for observability (tesela-c7s item 2)
    /// — every note whose inbound update is currently stuck behind a missing
    /// base, with when it first stalled and which peers' ops it carried.
    pub async fn pending_import_notes(&self) -> Vec<PendingImport> {
        self.inner.pending_imports.read().await.values().cloned().collect()
    }

    /// Notes that have stayed pending past one full inbound apply pass and so
    /// need an AUTHORITATIVE SNAPSHOT catch-up to heal the causal gap
    /// (tesela-c7s item 2). "Past one tick": a note whose `first_seen_pass` is
    /// strictly below the current pass survived a whole batch still stuck, so
    /// it will not self-heal from buffered deltas — the caller (server / FFI
    /// relay tick) fetches + imports the relay's authoritative snapshot for
    /// exactly these, which clears them via [`clear_pending_import`]. A note
    /// that only JUST went pending this pass is deliberately withheld one pass
    /// so a same-session missing-base delta can still integrate it first.
    ///
    /// BOUNDED (tesela-c7s F3): a note whose snapshot no peer ever deposits can
    /// never heal, so escalating it on EVERY pass forever is pure waste (a relay
    /// `fetch_snapshots` per tick that always comes back empty for it). This is
    /// self-mutating accounting: each returned note's `catchup_attempts` is
    /// incremented and its `last_catchup_pass` stamped, escalations are spaced by
    /// EXPONENTIAL BACKOFF (the Nth is due `min(2^N, 2^CATCHUP_BACKOFF_SHIFT_CAP)`
    /// passes after the previous), and once `catchup_attempts` reaches
    /// [`MAX_CATCHUP_ATTEMPTS`] the note is declared a PERMANENT gap
    /// (`catchup_exhausted = true`, a loud terminal log fires once) and is never
    /// escalated again. It stays in the ledger so [`pending_import_notes`] /
    /// the sync-health surface can show it; only a real heal clears it.
    pub async fn notes_needing_snapshot_catchup(&self) -> Vec<[u8; 16]> {
        let pass = self.inner.import_pass.load(Ordering::Relaxed);
        let mut due = Vec::new();
        let mut changed = false;
        {
            let mut ledger = self.inner.pending_imports.write().await;
            for entry in ledger.values_mut() {
                // One-pass grace: a note that only just went pending this pass
                // may still integrate from a same-session buffered base.
                if entry.first_seen_pass >= pass {
                    continue;
                }
                // Terminal: a permanent gap no longer re-escalates.
                if entry.catchup_exhausted {
                    continue;
                }
                // Exponential backoff between escalations. Before the first
                // escalation the reference is `first_seen_pass`; after, it is the
                // last escalation pass — so a note is due only once `backoff`
                // passes have elapsed since it was last tried.
                let reference = if entry.catchup_attempts == 0 {
                    entry.first_seen_pass
                } else {
                    entry.last_catchup_pass
                };
                let backoff = 1u64 << entry.catchup_attempts.min(CATCHUP_BACKOFF_SHIFT_CAP);
                if pass.saturating_sub(reference) < backoff {
                    continue;
                }
                entry.catchup_attempts += 1;
                entry.last_catchup_pass = pass;
                changed = true;
                if entry.catchup_attempts >= MAX_CATCHUP_ATTEMPTS {
                    entry.catchup_exhausted = true;
                    tracing::error!(
                        "tesela-sync/loro: note {} is a PERMANENT causal gap — {} \
                         authoritative-snapshot catch-up escalations never healed it (no peer \
                         has deposited its snapshot). Marking terminal in the pending-import \
                         ledger for the sync-health surface; it will not re-escalate until a \
                         real base/snapshot arrives.",
                        hex_id(&entry.note_id),
                        entry.catchup_attempts
                    );
                }
                due.push(entry.note_id);
            }
        }
        if changed {
            self.save_pending_imports().await;
        }
        due
    }

    /// Count of outbound strand alarms raised so far (tesela-c7s item 3) — a
    /// monotonic counter the server / FFI relay tick reads to log when a
    /// dirty note could not ship an incremental delta and fell back to a
    /// snapshot. See [`Inner::outbound_strand_alarms`].
    pub fn outbound_strand_alarm_count(&self) -> u64 {
        self.inner.outbound_strand_alarms.load(Ordering::Relaxed)
    }

    /// Reseed every structurally preserving note's Loro doc from the authoritative
    /// `.md` files in `notes_dir` by replaying a `NoteUpsert` per file. For
    /// notes already resident, `apply_payload`'s NoteUpsert tree-reconcile
    /// corrects a drifted/stale doc to match disk (the fix for the stale-shadow
    /// divergences the materialization dry-run found). For new notes it seeds
    /// them. The explicit reseed may canonicalize raw headings, prose, and
    /// fences only when the parsed structural projection is unchanged. This is
    /// deliberately broader than automatic startup stamping, which stays
    /// byte-conservative. This is the canonical-device
    /// bootstrap for the cutover — the source of truth on first authoritative
    /// boot is DISK, not the frozen oplog/snapshots. Returns the number of files
    /// successfully applied; warnings report each skipped note and the skipped
    /// count.
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
        let mut skipped = 0usize;
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
            let parsed = tesela_core::note_tree::parse_note(&content);
            let serialized = tesela_core::note_tree::serialize_note(&parsed);
            if !tesela_core::note_tree::canonicalization_preserves_structure(&content, &serialized)
            {
                tracing::warn!(
                    "tesela-sync/loro: reseed skip {stem} — canonicalization changed its \
                     structural projection; \
                     leaving the file byte-identical and Loro state untouched"
                );
                skipped += 1;
                continue;
            }
            let hash = blake3::hash(stem.as_bytes());
            let mut note_id = [0u8; 16];
            note_id.copy_from_slice(&hash.as_bytes()[..16]);
            let title = frontmatter_title(&content).unwrap_or_else(|| stem.to_string());
            let payload = OpPayload::NoteUpsert {
                note_id,
                display_alias: Some(stem.to_string()),
                title,
                content: serialized,
                created_at_millis: 0,
            };
            if let Err(e) = self.apply_payload(&payload).await {
                tracing::warn!("tesela-sync/loro: reseed apply {stem}: {e}");
                continue;
            }
            count += 1;
        }
        if skipped == 0 {
            tracing::info!("tesela-sync/loro: reseed summary: {count} applied, 0 skipped");
        } else {
            tracing::warn!(
                "tesela-sync/loro: reseed summary: {count} applied, {skipped} skipped because \
                 canonicalization changed their structural projection"
            );
        }
        Ok(count)
    }
}

// ============================================================================
// Views registry (saved-views spec, 2026-06-10)
// ============================================================================
//
// ONE dedicated Loro doc (id = `VIEWS_DOC_ID`) holds every saved view. It
// lives in the engine's `docs` map, so it RIDES the relay exactly like a
// note doc — `produce_relay_updates`, `apply_relay_updates`, snapshot
// deposits (`tracked_note_ids` → `export_doc_update(id, None)`), bootstrap
// imports, and per-doc `.bin` persistence all address `docs` by 16-byte id
// and need no special case. This is the OPPOSITE of the index doc, which is
// a derived projection each peer rebuilds locally and is deliberately NOT
// broadcast; the views registry is authoritative user state and must sync.
//
// What the views doc is EXCLUDED from (the note-shaped machinery):
//   - materialization: no `<slug>.md` is ever written (`materialize_note`
//     guard) and `reseed_from_disk` only ever UPSERTS from `.md` files, so
//     a reseed can't touch (let alone wipe) the views doc;
//   - the index: `refresh_note_derived` / `rebuild_index_from_docs` skip
//     it, so no phantom note appears in note lists;
//   - rendering: `render_note` / `render_note_full` return `None` (which
//     also makes note walkers like the CLI task-backfill skip it);
//   - the disjoint-twin import heal + tombstone pass: the doc has no
//     "blocks" tree — its state is a map of LWW registers, where a raw
//     Loro import IS the correct merge — so `import_doc_update` /
//     `apply_doc_update_status` / `import_authoritative_snapshot` take the
//     plain-import path;
//   - note-shaped ops: an `OpPayload` addressed at `VIEWS_DOC_ID` is
//     refused in `apply_payload_inner` (defense in depth).
//
// CRDT shape: `views` LoroMap keyed by view id → per-view LoroMap of
// scalar fields ({id, name, dsl, order, builtin, display_mode,
// display_group_by?, display_show_done?, display_table_config?}).
// `display_table_config` (tesela-ya4.4) is itself a compound value (hidden
// columns / explicit order / sort), but is stored as ONE JSON-encoded
// string field rather than a nested CRDT map — same flat-scalar,
// whole-field-LWW shape every other `display_*` field already uses, so a
// concurrent edit to (say) `display_group_by` on one device and a table
// column reorder on another still merge cleanly (different keys), even
// though two concurrent EDITS of the table config itself resolve as one
// whole-field LWW rather than merging sub-fields. Field-level LWW: concurrent
// edits of different fields both survive; same-field edits resolve
// deterministically. Ordering is a plain `order` i64 per view (ties break
// by id) — a CRDT list would add tombstone/move complexity for a registry
// of 6–12 entries whose reorders are rare whole-renumber writes.
//
// Known boundary (adversarial review 2026-06-10): two devices that
// INDEPENDENTLY first-create the same view id race on the map key's
// container register — the losing container's fields are dropped
// wholesale. For USER views the UUID ids make that collision impossible.
// For BUILTINS the race is closed structurally: every device authors the
// seed as the SAME deterministic ops (`builtin_views_seed_update`,
// reserved peer 0, no timestamps), and `views_upsert` routes a missing
// builtin entry through that seed too — so concurrent first-writes share
// one canonical container and merge FIELD-wise, never container-wise.
// Belt-and-braces, the call sites also order seed AFTER bootstrap
// (server main.rs; iOS `RelayTicker.shouldSeedBuiltinViews`) so a fresh
// device joining a group usually receives the registry and no-ops the
// seed entirely.
impl LoroEngine {
    /// True when the id addresses the views registry doc (not a note).
    fn is_views_doc(note_id: &[u8; 16]) -> bool {
        *note_id == VIEWS_DOC_ID
    }

    /// Persist the views doc's snapshot (`<dir>/<hex(VIEWS_DOC_ID)>.bin`).
    /// Same per-doc snapshot file every note uses, so boot's
    /// `load_snapshots_from_dir` restores it with no special case.
    async fn persist_views_doc(&self) {
        if let Some(dir) = self.inner.snapshot_dir.as_ref() {
            self.save_snapshot(dir, VIEWS_DOC_ID).await;
        }
    }

    /// All saved views, sorted by `(order, id)` — deterministic across
    /// devices. Empty when the registry doc doesn't exist yet (fresh
    /// device pre-seed / pre-bootstrap).
    pub async fn views_list(&self) -> Vec<crate::engine::ViewRecord> {
        let Some(doc) = self.lazy_load_doc(VIEWS_DOC_ID).await else {
            return Vec::new();
        };
        let value = doc.get_map("views").get_deep_value();
        let mut out = Vec::new();
        if let loro::LoroValue::Map(m) = value {
            for (key, v) in m.iter() {
                let loro::LoroValue::Map(entry) = v else {
                    continue;
                };
                let get_str = |k: &str| -> Option<String> {
                    entry.get(k).and_then(|x| {
                        if let loro::LoroValue::String(s) = x {
                            Some((**s).to_string())
                        } else {
                            None
                        }
                    })
                };
                let get_bool = |k: &str| -> Option<bool> {
                    entry.get(k).and_then(|x| {
                        if let loro::LoroValue::Bool(b) = x {
                            Some(*b)
                        } else {
                            None
                        }
                    })
                };
                let get_i64 = |k: &str| -> Option<i64> {
                    entry.get(k).and_then(|x| {
                        if let loro::LoroValue::I64(n) = x {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                };
                let id = get_str("id").unwrap_or_else(|| key.to_string());
                let mut name = get_str("name").unwrap_or_default();
                if id == INBOX_VIEW_ID && name == "Inbox" {
                    name = "Views".to_string();
                }
                out.push(crate::engine::ViewRecord {
                    id,
                    name,
                    dsl: get_str("dsl").unwrap_or_default(),
                    order: get_i64("order").unwrap_or(0),
                    builtin: get_bool("builtin").unwrap_or(false),
                    display_mode: get_str("display_mode").unwrap_or_else(|| "list".to_string()),
                    display_group_by: get_str("display_group_by").filter(|s| !s.is_empty()),
                    display_show_done: get_bool("display_show_done"),
                    // tesela-ya4.4 — JSON-encoded compound field (see the
                    // views-doc CRDT-shape comment above). A malformed or
                    // absent value degrades to `None` (no override) rather
                    // than failing the whole view's read.
                    display_table_config: get_str("display_table_config")
                        .and_then(|s| serde_json::from_str(&s).ok()),
                });
            }
        }
        out.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        out
    }

    /// Create or update a saved view. Field-level LWW: an EXISTING view's
    /// per-view map is reused and each field written individually, so a
    /// concurrent peer edit of a different field survives the merge (only
    /// a brand-new view id inserts a fresh container). `builtin` is
    /// STICKY — once a view is builtin it stays builtin regardless of the
    /// record's flag, so the delete guard can't be bypassed by first
    /// un-flagging via upsert. The doc's vv changes here, which is what
    /// gets the write picked up by the next `produce_relay_updates` tick.
    pub async fn views_upsert(&self, record: crate::engine::ViewRecord) -> SyncResult<()> {
        if record.id.trim().is_empty() {
            return Err(SyncError::Protocol("view id must be non-empty".into()));
        }
        let doc = self.doc_for_note_mut(VIEWS_DOC_ID).await;
        let views = doc.get_map("views");
        let entry = match views.get(&record.id) {
            Some(loro::ValueOrContainer::Container(loro::Container::Map(m))) => m,
            // First write to a BUILTIN on a device that never seeded or
            // synced (e.g. an iOS hub-mode edit of the fallback Inbox
            // pre-bootstrap): import the deterministic seed first so the
            // fields land in THE canonical seed container — never a fresh
            // same-key container that races the group's and drops the
            // loser's fields (incl. user edits) wholesale on merge.
            _ if record.id == INBOX_VIEW_ID => {
                doc.import(&builtin_views_seed_update()?)
                    .map_err(|e| SyncError::Storage(format!("views seed import: {e}")))?;
                match views.get(&record.id) {
                    Some(loro::ValueOrContainer::Container(loro::Container::Map(m))) => m,
                    _ => {
                        return Err(SyncError::Storage(
                            "views: builtin seed import did not materialize the entry".into(),
                        ))
                    }
                }
            }
            _ => views
                .insert_container(&record.id, loro::LoroMap::new())
                .map_err(|e| SyncError::Storage(format!("views insert_container: {e}")))?,
        };
        let was_builtin = matches!(
            entry.get("builtin").and_then(|v| v.into_value().ok()),
            Some(loro::LoroValue::Bool(true))
        );
        let builtin = record.builtin || was_builtin;
        let ins = |e: loro::LoroError| SyncError::Storage(format!("views insert: {e}"));
        entry.insert("id", record.id.as_str()).map_err(ins)?;
        entry.insert("name", record.name.as_str()).map_err(ins)?;
        entry.insert("dsl", record.dsl.as_str()).map_err(ins)?;
        entry.insert("order", record.order).map_err(ins)?;
        entry.insert("builtin", builtin).map_err(ins)?;
        entry
            .insert("display_mode", record.display_mode.as_str())
            .map_err(ins)?;
        match record.display_group_by.as_deref() {
            Some(v) => entry.insert("display_group_by", v).map_err(ins)?,
            None => {
                let _ = entry.delete("display_group_by");
            }
        }
        match record.display_show_done {
            Some(v) => entry.insert("display_show_done", v).map_err(ins)?,
            None => {
                let _ = entry.delete("display_show_done");
            }
        }
        match record.display_table_config.as_ref() {
            Some(cfg) => {
                let json = serde_json::to_string(cfg).map_err(|e| {
                    SyncError::Storage(format!("display_table_config serialize: {e}"))
                })?;
                entry.insert("display_table_config", json.as_str()).map_err(ins)?;
            }
            None => {
                let _ = entry.delete("display_table_config");
            }
        }
        let _ = doc
            .get_map("meta")
            .insert("schema_version", VIEWS_SCHEMA_VERSION);
        doc.commit();
        self.persist_views_doc().await;
        Ok(())
    }

    /// Delete a saved view by id. `Ok(true)` when removed, `Ok(false)`
    /// when no such view exists, `Err(Protocol)` for a builtin (builtins
    /// are editable, never deletable — the guard lives HERE, at the API;
    /// the CRDT itself would happily delete the key). Concurrent
    /// delete-vs-edit resolves deterministically: the map-key delete wins
    /// over edits INSIDE the removed container, so both peers converge on
    /// the view being gone.
    pub async fn views_delete(&self, view_id: &str) -> SyncResult<bool> {
        let Some(doc) = self.lazy_load_doc(VIEWS_DOC_ID).await else {
            return Ok(false);
        };
        let views = doc.get_map("views");
        let Some(loro::ValueOrContainer::Container(loro::Container::Map(entry))) =
            views.get(view_id)
        else {
            return Ok(false);
        };
        let builtin = matches!(
            entry.get("builtin").and_then(|v| v.into_value().ok()),
            Some(loro::LoroValue::Bool(true))
        );
        if builtin {
            return Err(SyncError::Protocol(format!(
                "view '{view_id}' is builtin and not deletable"
            )));
        }
        views
            .delete(view_id)
            .map_err(|e| SyncError::Storage(format!("views delete: {e}")))?;
        doc.commit();
        self.persist_views_doc().await;
        Ok(true)
    }

    /// Idempotently seed the built-in views (currently: Inbox). No-op when
    /// the Inbox entry already exists — whether seeded locally or received
    /// via sync — so a reseed never clobbers the user's edits to the
    /// builtin's dsl/display. The seed itself is imported as the
    /// DETERMINISTIC update from `builtin_views_seed_update` (reserved
    /// seed peer, identical op IDs on every device), so concurrent
    /// first-seeds are literally the same ops — no same-key container
    /// race exists for a later merge to drop one side's edits
    /// (TDD'd in `offline_first_seed_then_sync_preserves_remote_builtin_edit`
    /// and `concurrent_seed_converges_to_one_inbox`).
    pub async fn ensure_builtin_views(&self) -> SyncResult<()> {
        if let Some(doc) = self.lazy_load_doc(VIEWS_DOC_ID).await {
            if doc.get_map("views").get(INBOX_VIEW_ID).is_some() {
                return Ok(());
            }
        }
        let seed = builtin_views_seed_update()?;
        let doc = self.doc_for_note_mut(VIEWS_DOC_ID).await;
        doc.import(&seed)
            .map_err(|e| SyncError::Storage(format!("views seed import: {e}")))?;
        self.persist_views_doc().await;
        Ok(())
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

/// Load the causal-gap ledger persisted by
/// `LoroEngine::save_pending_imports` (tesela-c7s item 2). Missing/corrupt →
/// empty (the gap re-surfaces on the next inbound pending frame).
async fn load_pending_imports(dir: &Path) -> HashMap<[u8; 16], PendingImport> {
    let path = dir.join("_pending_imports.bin");
    match tokio::fs::read(&path).await {
        Ok(bytes) => match postcard::from_bytes::<Vec<PendingImport>>(&bytes) {
            Ok(entries) => entries.into_iter().map(|p| (p.note_id, p)).collect(),
            Err(e) => {
                tracing::warn!("tesela-sync/loro: pending-import ledger decode: {e}");
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
async fn load_snapshots_from_dir(dir: &Path) -> SyncResult<HashMap<[u8; 16], LoroDoc>> {
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
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| SyncError::Storage(format!("read_dir {}: {e}", dir.display())))?
    {
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
                tracing::warn!("tesela-sync/loro: read snapshot {}: {e}", path.display());
                continue;
            }
        };
        let doc = LoroDoc::new();
        if let Err(e) = doc.import(&bytes) {
            tracing::warn!("tesela-sync/loro: import snapshot {}: {e}", path.display());
            continue;
        }
        docs.insert(note_id, doc);
    }
    Ok(docs)
}

/// Every note_id with a `.bin` snapshot on disk in `dir` — a cheap
/// filename-only scan (never imports bytes). Used by
/// `LoroEngine::rebuild_index_from_docs` to distinguish "not currently
/// memory-resident" (safe — lazy-load reloads it on demand) from
/// "genuinely gone" (missing/corrupt snapshot — the prune-ghost-entries
/// case, review finding [6]).
async fn snapshot_note_ids_on_disk(dir: &Path) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return out,
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("bin") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if parse_note_id_from_hex(stem).is_some() {
            out.insert(stem.to_string());
        }
    }
    out
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
fn flatblock_from_node(tree: &LoroTree, node: TreeID) -> Option<tesela_core::note_tree::FlatBlock> {
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
    // A4 render-time dedup: with migrate-on-write flag-OFF, a legacy in-text
    // `key:: value` line can persist in `text_seq` WHILE a container property
    // for the same key also exists (e.g. set via the structured path) — the
    // serializer would then emit BOTH lines. Drop any solely-`key:: value`
    // prose line whose key matches a container key so the property renders
    // ONCE, with the container value winning. The in-text key is compared
    // case-insensitively (`solely_property_line` lowercases it; container keys
    // are stored verbatim, so we lowercase those for the membership test).
    let text = dedup_intext_props_against_container(text, &properties);
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

/// The UTF-16 splice that turns `current` into `target` while touching
/// only the bytes that actually changed: the longest common prefix and
/// suffix are preserved, and the differing MIDDLE is deleted + re-inserted.
/// Returns `(utf16_offset, utf16_delete_len, insert)`. Offsets are UTF-16
/// to match [`LoroEngine::splice_block_text`] and the cursor model.
///
/// This is the CONVERGENCE-CRITICAL core of [`write_block_text`]: by never
/// deleting+re-inserting the shared prefix/suffix, two replicas that
/// concurrently REWRITE the same block char-merge their divergent middles
/// (the shared base survives ONCE) instead of unioning two whole runs into
/// a concatenation. Computed on Unicode-scalar boundaries (never splitting
/// a surrogate pair), then summed into UTF-16 units.
fn minimal_utf16_diff(current: &str, target: &str) -> (usize, usize, String) {
    let cur: Vec<char> = current.chars().collect();
    let tgt: Vec<char> = target.chars().collect();
    // Longest common prefix (in chars).
    let mut p = 0;
    while p < cur.len() && p < tgt.len() && cur[p] == tgt[p] {
        p += 1;
    }
    // Longest common suffix (in chars), not overlapping the prefix.
    let mut s = 0;
    while s < cur.len() - p && s < tgt.len() - p && cur[cur.len() - 1 - s] == tgt[tgt.len() - 1 - s]
    {
        s += 1;
    }
    let utf16: fn(&[char]) -> usize = |cs| cs.iter().map(|c| c.len_utf16()).sum();
    let off16 = utf16(&cur[..p]);
    let del16 = utf16(&cur[p..cur.len() - s]);
    let ins: String = tgt[p..tgt.len() - s].iter().collect();
    (off16, del16, ins)
}

/// Write a block's whole text into its node's nested `text_seq`
/// [`LoroText`] container — the sequence CRDT that lets concurrent
/// same-block edits INTERLEAVE instead of clobbering (the LWW map
/// register `text` did). `get_or_create_container` is idempotent and
/// returns the existing handler if present, so seed + upsert + heal all
/// converge on ONE container at the stable `text_seq` key. We never write
/// the legacy `text` register again — concurrently minting a different
/// container at the same key can overwrite rather than merge, so a distinct
/// key + get-or-create is the safe path. The legacy `text` register stays
/// readable for old snapshots via [`read_block_text`].
///
/// CONVERGENCE (2026-06-28): this whole-text authoring path used to call
/// `LoroText::update`, whose internal Myers diff is convergent for the
/// common case but is a heuristic (timeout-bounded, alignment-dependent) —
/// fragile for a convergence-critical path. Two guarantees are now explicit:
///
/// 1. **Idempotency guard.** If the container already holds `text`, SKIP the
///    write entirely. The disjoint-twin heal re-issues the same
///    `BlockUpsert{text}` on every inbound frame; the guard stops a re-issue
///    from manufacturing a duplicate run or growing the op history (which
///    would compound under multi-round relay re-broadcast).
/// 2. **Minimal-diff splice.** A genuine rewrite is applied as a minimal
///    UTF-16 offset diff ([`minimal_utf16_diff`]) through the SAME
///    `delete_utf16`/`insert_utf16` primitive `splice_block_text` uses, so
///    concurrent rewrites char-merge on the shared prefix/suffix instead of
///    unioning whole runs into a concatenation.
fn write_block_text(meta: &loro::LoroMap, text: &str) -> SyncResult<()> {
    let text_c: LoroText = meta
        .get_or_create_container("text_seq", LoroText::new())
        .map_err(|e| SyncError::Storage(format!("loro text_seq get_or_create: {e}")))?;
    let current = text_c.to_string();
    // (1) Idempotency guard — identical target ⇒ no write at all.
    if current == text {
        return Ok(());
    }
    // (2) Genuine rewrite ⇒ minimal convergent splice (delete middle, insert
    // the new middle at the same offset).
    let (off16, del16, ins) = minimal_utf16_diff(&current, text);
    if del16 > 0 {
        text_c
            .delete_utf16(off16, del16)
            .map_err(|e| SyncError::Storage(format!("loro text_seq delete_utf16: {e}")))?;
    }
    if !ins.is_empty() {
        text_c
            .insert_utf16(off16, &ins)
            .map_err(|e| SyncError::Storage(format!("loro text_seq insert_utf16: {e}")))?;
    }
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

mod prop_containers;
mod render;
use render::{
    classify_block_prose_and_props, dedup_intext_props_against_container, doc_full_markdown,
    page_properties_materialized, set_page_properties,
};
mod index;
use index::{build_block_index, frontmatter_title, INDEX_SCHEMA_VERSION};
mod apply;
#[cfg(test)]
use apply::probe_import_poison;
mod twins;
use twins::{
    dedup_twins_by_block_id, tombstone_duplicate_twins, twin_winners_for, PeerBlockChange,
    ResolvedValue,
};
#[cfg(test)]
use twins::duplicate_block_ids;

/// True if the tree's live blocks (in render order) match `blocks` by
/// id + STRIPPED prose + indent. Props are deliberately NOT part of the
/// comparison. Used to decide whether a NoteUpsert needs to reconcile the
/// tree (no-op when they already agree, preserving block identity on ordinary
/// re-saves).
///
/// Both sides' prose is compared through the migrate-strip classifier so a
/// migrated prose-only tree (props in a typed container) and an old peer's
/// in-text `key:: value` body resolve to the SAME prose and are NOT seen as
/// drifted — otherwise a property-carrying NoteUpsert would destructively
/// reseed and collapse the typed container back into block text (P1.8).
/// Because prop ops are the SOLE writers of `props`, a props-only difference
/// (id + prose + indent match, props differ) is likewise NEVER drift, so the
/// classifier's lifted props are discarded — a NoteUpsert must never reseed to
/// chase a body's in-text props.
fn tree_matches_blocks(tree: &LoroTree, blocks: &[tesela_core::note_tree::FlatBlock]) -> bool {
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
        // Strip recognized in-text properties from BOTH sides before comparing
        // prose: the live node's `text_seq` is already prose-only after a
        // migrate, while the incoming `FlatBlock.text` may still carry in-text
        // `key:: value` lines (parse_note folds them in). Stripping both yields
        // the same prose so the old-peer body isn't seen as drifted. A
        // props-only difference is NEVER drift — prop ops own `props`; a
        // NoteUpsert must not reseed to chase the body's in-text props.
        let (tree_prose, _) =
            classify_block_prose_and_props(&read_block_text(tree, *node).unwrap_or_default());
        let (body_prose, _) = classify_block_prose_and_props(&block.text);
        let prose_ok = tree_prose == body_prose;
        let indent_ok = read_indent_level(tree, *node).unwrap_or(0) == block.indent;
        if !(id_ok && prose_ok && indent_ok) {
            return false;
        }
    }
    true
}

fn blocks_match_structurally(
    left: &tesela_core::note_tree::FlatBlock,
    right: &tesela_core::note_tree::FlatBlock,
) -> bool {
    let (left_prose, _) = classify_block_prose_and_props(&left.text);
    let (right_prose, _) = classify_block_prose_and_props(&right.text);
    left.indent == right.indent && left_prose == right_prose
}

/// An exact unstamped whole-body reapply mints fresh parser UUIDs even though
/// its ordered structure is unchanged. Rebind only those transient ids to the
/// resident ids; explicit incoming bids are immutable anchors.
///
/// Returns `false` without mutating `blocks` unless the complete live shape is
/// an exact structural replay. A changed unstamped resident body is ambiguous
/// (edit vs insertion vs stale resurrection), so the caller fails closed.
fn adopt_minted_ids_for_exact_reapply(
    tree: &LoroTree,
    blocks: &mut [tesela_core::note_tree::FlatBlock],
    minted_ids: &std::collections::HashSet<uuid::Uuid>,
) -> bool {
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|node| !matches!(tree.is_node_deleted(node), Ok(true)))
        .collect();
    let resident: Vec<tesela_core::note_tree::FlatBlock> = dedup_twins_by_block_id(tree, live)
        .into_iter()
        .filter_map(|node| flatblock_from_node(tree, node))
        .collect();
    if resident.len() != blocks.len() {
        return false;
    }

    let explicit_ids: std::collections::HashSet<uuid::Uuid> = blocks
        .iter()
        .filter(|block| !minted_ids.contains(&block.id))
        .map(|block| block.id)
        .collect();
    let mut remap = HashMap::new();
    for (existing, incoming) in resident.iter().zip(blocks.iter()) {
        if !blocks_match_structurally(existing, incoming) {
            return false;
        }
        if minted_ids.contains(&incoming.id) {
            if explicit_ids.contains(&existing.id) {
                return false;
            }
            remap.insert(incoming.id, existing.id);
        } else if incoming.id != existing.id {
            return false;
        }
    }

    for block in blocks.iter_mut() {
        if let Some(resident_id) = remap.get(&block.id) {
            block.id = *resident_id;
        }
        if let Some(parent) = block.parent.and_then(|parent| remap.get(&parent)) {
            block.parent = Some(*parent);
        }
    }
    true
}

fn tree_has_block_history(tree: &LoroTree) -> bool {
    !tree.nodes().is_empty()
}

/// A legacy root-content migration is trusted to introduce lifted regions
/// that the old bullet-only tree never modeled. Reuse historical identity for
/// every parser-minted block that has an exact structural counterpart, while
/// preserving explicit incoming bids and leaving genuinely new lifted regions
/// with their fresh ids.
fn adopt_historical_ids_for_legacy_migration(
    tree: &LoroTree,
    blocks: &mut [tesela_core::note_tree::FlatBlock],
    minted_ids: &std::collections::HashSet<uuid::Uuid>,
) {
    let live: Vec<TreeID> = tree
        .children(TreeParentId::Root)
        .unwrap_or_default()
        .into_iter()
        .filter(|node| !matches!(tree.is_node_deleted(node), Ok(true)))
        .collect();
    let mut candidates: Vec<tesela_core::note_tree::FlatBlock> =
        dedup_twins_by_block_id(tree, live)
            .into_iter()
            .filter_map(|node| flatblock_from_node(tree, node))
            .filter(|block| !block.id.is_nil())
            .collect();
    let mut seen: std::collections::HashSet<uuid::Uuid> =
        candidates.iter().map(|block| block.id).collect();
    for node in tree
        .nodes()
        .into_iter()
        .filter(|node| matches!(tree.is_node_deleted(node), Ok(true)))
    {
        if let Some(block) = flatblock_from_node(tree, node).filter(|block| !block.id.is_nil()) {
            if seen.insert(block.id) {
                candidates.push(block);
            }
        }
    }

    let explicit_ids: std::collections::HashSet<uuid::Uuid> = blocks
        .iter()
        .filter(|block| !minted_ids.contains(&block.id))
        .map(|block| block.id)
        .collect();
    let mut used = vec![false; candidates.len()];
    let mut remap = HashMap::new();
    for block in blocks.iter() {
        if !minted_ids.contains(&block.id) {
            continue;
        }
        if let Some((idx, existing)) = candidates.iter().enumerate().find(|(idx, existing)| {
            !used[*idx]
                && !explicit_ids.contains(&existing.id)
                && blocks_match_structurally(existing, block)
        }) {
            used[idx] = true;
            remap.insert(block.id, existing.id);
        }
    }
    for block in blocks.iter_mut() {
        if let Some(resident_id) = remap.get(&block.id) {
            block.id = *resident_id;
        }
        if let Some(parent) = block.parent.and_then(|parent| remap.get(&parent)) {
            block.parent = Some(*parent);
        }
    }
}

/// A legacy `root.content` body may be retired only by the same full modeled
/// projection. Explicit ids already persisted in that legacy body must remain
/// explicit identity anchors; bidless regions may receive canonical ids.
fn legacy_content_matches_incoming(legacy: &str, incoming: &str) -> bool {
    if !tesela_core::note_tree::canonicalization_preserves_structure(legacy, incoming) {
        return false;
    }
    let (legacy_tree, legacy_minted) = tesela_core::note_tree::parse_note_with_minted_ids(legacy);
    let incoming_tree = tesela_core::note_tree::parse_note(incoming);
    let legacy_minted: std::collections::HashSet<uuid::Uuid> = legacy_minted.into_iter().collect();
    legacy_tree
        .blocks
        .iter()
        .zip(incoming_tree.blocks.iter())
        .all(|(left, right)| legacy_minted.contains(&left.id) || left.id == right.id)
}

/// Non-destructive NoteUpsert body reconcile (2026-06-10). Brings the live
/// tree toward the parsed body WITHOUT clearing it:
///
/// - bid has a LIVE node → update prose / indent / parent in place, only
///   when drifted (prose compared property-stripped, mirroring
///   [`tree_matches_blocks`]). The node — and its `text_seq` / `props`
///   containers — keeps its identity, so peers keep merging into it.
/// - bid has NO node at all → create it (positioned after its predecessor
///   in body order, mirroring the BlockUpsert create path).
/// - bid has ONLY TOMBSTONED node(s) → SKIP. A whole-content upsert is not
///   author intent about existence; re-creating a deleted bid is exactly
///   the resurrection that reverted iOS deletes (authors never reuse bids,
///   so a genuine re-add always arrives under a fresh id).
/// - live nodes whose bid is ABSENT from the body → LEFT ALONE (the
///   stale-PUT anti-clobber rule, data-loss vector #2: deletes flow only
///   as explicit `BlockDelete` ops).
///
/// Ordering drift alone (same ids/text/indent, different order) is NOT
/// healed here — cross-device order convergence belongs to `BlockMove` /
/// Loro's movable-tree merge, and re-minting nodes to fix order is how the
/// twin bomb started.
fn reconcile_tree_to_blocks(
    tree: &LoroTree,
    blocks: &[tesela_core::note_tree::FlatBlock],
) -> SyncResult<()> {
    // Bids that ever had a node deleted (deleted-wins set). Collected
    // before any mutation below.
    let tombstoned: std::collections::HashSet<String> = tree
        .nodes()
        .into_iter()
        .filter(|n| matches!(tree.is_node_deleted(n), Ok(true)))
        .filter_map(|n| read_meta_str(tree, n, "block_id"))
        .collect();
    let mut prev_live: Option<[u8; 16]> = None;
    for block in blocks {
        let block_hex = hex::encode(block.id.as_bytes());
        let bid_bytes = *block.id.as_bytes();
        match find_node_by_block_id(tree, &block_hex) {
            Some(node) => {
                let meta = tree
                    .get_meta(node)
                    .map_err(|e| SyncError::Storage(format!("reconcile get_meta: {e}")))?;
                let (tree_prose, _) = classify_block_prose_and_props(
                    &read_block_text(tree, node).unwrap_or_default(),
                );
                let (body_prose, _) = classify_block_prose_and_props(&block.text);
                if tree_prose != body_prose {
                    write_block_text(&meta, block.text.as_str())?;
                }
                if read_indent_level(tree, node).unwrap_or(0) != block.indent {
                    meta.insert("indent_level", block.indent as i64)
                        .map_err(|e| SyncError::Storage(format!("reconcile meta insert: {e}")))?;
                }
                let parent_hex = block
                    .parent
                    .map(|p| hex::encode(p.as_bytes()))
                    .unwrap_or_default();
                if read_meta_str(tree, node, "parent").unwrap_or_default() != parent_hex {
                    meta.insert("parent", parent_hex.as_str())
                        .map_err(|e| SyncError::Storage(format!("reconcile meta insert: {e}")))?;
                }
                prev_live = Some(bid_bytes);
            }
            None => {
                if tombstoned.contains(&block_hex) {
                    // Deleted-wins: never resurrect via a whole-content upsert.
                    continue;
                }
                let node = if let Some(previous) = prev_live.as_ref() {
                    create_block_node_positioned(tree, Some(previous))?
                } else {
                    tree.create_at(TreeParentId::Root, 0)
                        .map_err(|e| SyncError::Storage(format!("loro tree create: {e}")))?
                };
                let meta = tree
                    .get_meta(node)
                    .map_err(|e| SyncError::Storage(format!("reconcile get_meta: {e}")))?;
                meta.insert("block_id", block_hex.as_str())
                    .map_err(|e| SyncError::Storage(format!("reconcile meta insert: {e}")))?;
                write_block_text(&meta, block.text.as_str())?;
                // Eager-seed the props containers (P1.9b) — same discipline
                // as `seed_tree_from_flatblocks` / the BlockUpsert arm.
                let _ = prop_containers::node_prop_containers(&meta)?;
                meta.insert("indent_level", block.indent as i64)
                    .map_err(|e| SyncError::Storage(format!("reconcile meta insert: {e}")))?;
                meta.insert(
                    "parent",
                    block
                        .parent
                        .map(|p| hex::encode(p.as_bytes()))
                        .unwrap_or_default(),
                )
                .map_err(|e| SyncError::Storage(format!("reconcile meta insert: {e}")))?;
                prev_live = Some(bid_bytes);
            }
        }
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

/// True when block `block_id`'s typed-props container currently stores `key` as
/// a nested `LoroText` child. Used by the engine lifecycle hook to author each
/// rolled key in the representation the key ALREADY has (a text-typed key stays
/// text; a scalar/absent key stays scalar) — so the engine writer never FLIPS a
/// key's representation, which would orphan the old child container and churn
/// the doc. Pure read (mirrors `read_block_text`'s non-minting `text_seq`
/// inspection); `false` for an unknown note/block/key or a scalar occupant.
fn block_prop_is_text(doc: &LoroDoc, block_id: [u8; 16], key: &str) -> bool {
    let tree = doc.get_tree("blocks");
    let Some(node) = find_node_by_block_id(&tree, &hex_id(&block_id)) else {
        return false;
    };
    let Ok(meta) = tree.get_meta(node) else {
        return false;
    };
    let Some((props, _prop_keys)) = prop_containers::read_node_prop_containers(&meta) else {
        return false;
    };
    matches!(
        props.get(key),
        Some(loro::ValueOrContainer::Container(c)) if c.get_type() == loro::ContainerType::Text
    )
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

#[async_trait]
impl SyncEngine for LoroEngine {
    fn device(&self) -> DeviceId {
        self.inner.device
    }

    /// Local-side mutation. Stamps a fresh HLC + content hash, then
    /// runs the payload through the same per-op logic that
    /// `apply_changes` uses for peer-originated ops.
    ///
    /// Serialized against a concurrent `apply_import` (or another
    /// `record_local`) for the SAME note (tesela-4ju REVIEW REJECT,
    /// 2026-07-02): without this, a local edit could land between
    /// `apply_import`'s props-plan fork and its twin tombstone, and get
    /// silently dropped by a tombstone pass sized to a plan that predates
    /// this edit. Takes the note's `apply_locks` guard (see
    /// `Inner::apply_locks` for the ordering rule) via
    /// `note_id_for_payload`, then delegates to the lock-free
    /// `record_local_locked`. Ops with no resolvable note (an unknown
    /// block, an attachment-only op — see `note_id_for_payload`) skip
    /// locking; there's no per-note doc mutation to serialize.
    async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash> {
        match self.note_id_for_payload(&payload).await {
            Some(note_id) => {
                let apply_lock = self.apply_lock_for_note(note_id).await;
                let _apply_guard = apply_lock.lock().await;
                self.record_local_locked(payload).await
            }
            None => self.record_local_locked(payload).await,
        }
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

    async fn repair_broadcast_cursors_after_snapshot(&self, committed: &[([u8; 16], Vec<u8>)]) {
        LoroEngine::repair_broadcast_cursors_after_snapshot(self, committed).await
    }

    async fn notes_needing_snapshot_catchup(&self) -> Vec<[u8; 16]> {
        LoroEngine::notes_needing_snapshot_catchup(self).await
    }

    async fn outbound_strand_alarm_count(&self) -> u64 {
        LoroEngine::outbound_strand_alarm_count(self)
    }

    async fn apply_relay_updates(&self, updates: &[([u8; 16], Vec<u8>)]) -> RelayApplyReport {
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
    async fn export_doc_update(&self, note_id: [u8; 16], since: Option<&[u8]>) -> Option<Vec<u8>> {
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
    async fn apply_doc_update_status(&self, note_id: [u8; 16], bytes: &[u8]) -> SyncResult<bool> {
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

    async fn mint_block_cursor(
        &self,
        note_id: [u8; 16],
        block_id: [u8; 16],
        utf16_offset: u32,
    ) -> Option<Vec<u8>> {
        LoroEngine::mint_block_cursor(self, note_id, block_id, utf16_offset).await
    }

    async fn resolve_block_cursor(&self, note_id: [u8; 16], cursor_bytes: &[u8]) -> Option<u32> {
        LoroEngine::resolve_block_cursor(self, note_id, cursor_bytes).await
    }

    async fn tracked_note_ids(&self) -> Vec<[u8; 16]> {
        self.note_ids().await
    }

    async fn index_entries(&self) -> Vec<crate::engine::IndexEntry> {
        LoroEngine::index_entries(self).await
    }

    async fn views_list(&self) -> Vec<crate::engine::ViewRecord> {
        LoroEngine::views_list(self).await
    }

    async fn views_upsert(&self, record: crate::engine::ViewRecord) -> SyncResult<()> {
        LoroEngine::views_upsert(self, record).await
    }

    async fn views_delete(&self, view_id: &str) -> SyncResult<bool> {
        LoroEngine::views_delete(self, view_id).await
    }

    async fn ensure_builtin_views(&self) -> SyncResult<()> {
        LoroEngine::ensure_builtin_views(self).await
    }
}

impl LoroEngine {
    /// The un-locked body of `record_local` (tesela-4ju REVIEW REJECT
    /// follow-up). `SyncEngine::record_local` takes the target note's
    /// `apply_locks` guard, then calls this. Internal callers that ALREADY
    /// hold that guard — currently only `reassert_prop_heals`, invoked from
    /// inside `apply_import` and `heal_disjoint_twins`, both of which hold
    /// the note's guard for their whole body — must call this directly
    /// instead of the public `record_local`: re-entering the same note's
    /// `tokio::sync::Mutex` from within its own critical section deadlocks
    /// (it is not reentrant).
    async fn record_local_locked(&self, payload: OpPayload) -> SyncResult<ContentHash> {
        let hlc = self.inner.hlc.now();
        let op = EncodedOp::new(hlc, crate::SYNC_SCHEMA_VERSION, payload.clone(), None)?;
        let hash = op.content_hash;
        self.apply_payload(&payload).await?;
        Ok(hash)
    }

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
                } => display_alias.clone().or(self.slug_for_note(*note_id).await),
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

    /// Inner per-payload apply that returns the affected note_id (so
    /// the public wrapper knows which snapshot to refresh). Returns
    /// `None` for ops that don't touch a single note (AttachmentUpsert,
    /// no-op cases) — those don't trigger a snapshot write.
    async fn apply_payload_inner(&self, payload: &OpPayload) -> SyncResult<Option<[u8; 16]>> {
        // Defense in depth: refuse note-shaped ops addressed at the views
        // registry doc. The views doc is mutated ONLY via the views_* API;
        // letting a NoteUpsert/BlockUpsert land there would graft a
        // "blocks" tree / root meta onto it and drag it into note-shaped
        // machinery (a NoteDelete would silently drop the whole registry).
        let op_target = match payload {
            OpPayload::NoteUpsert { note_id, .. }
            | OpPayload::NoteDelete { note_id, .. }
            | OpPayload::BlockUpsert { note_id, .. }
            | OpPayload::AttachmentUpsert { note_id, .. }
            | OpPayload::BlockPropertySet { note_id, .. }
            | OpPayload::PagePropertySet { note_id, .. } => Some(*note_id),
            OpPayload::BlockMove { .. }
            | OpPayload::BlockDelete { .. }
            | OpPayload::AttachmentDelete { .. } => None,
        };
        if op_target.is_some_and(|id| Self::is_views_doc(&id)) {
            tracing::warn!(
                "tesela-sync/loro: refusing note-shaped op {:?} addressed at the \
                 views registry doc — use the views_* API",
                payload.kind()
            );
            return Ok(None);
        }
        let touched = match payload {
            OpPayload::NoteUpsert {
                note_id,
                content,
                title,
                display_alias,
                ..
            } => {
                let (mut parsed, minted_ids) =
                    tesela_core::note_tree::parse_note_with_minted_ids(content);
                let minted_ids: std::collections::HashSet<uuid::Uuid> =
                    minted_ids.into_iter().collect();
                let canonical = tesela_core::note_tree::serialize_note(&parsed);
                if !tesela_core::note_tree::canonicalization_preserves_structure(
                    content, &canonical,
                ) {
                    return Err(SyncError::Protocol(format!(
                        "NoteUpsert {} changed structure during canonicalization",
                        hex_id(note_id)
                    )));
                }
                let doc = self.doc_for_note_mut(*note_id).await;
                let root_meta = doc.get_map("root");
                let legacy_content = root_meta
                    .get("content")
                    .and_then(|value| value.into_value().ok())
                    .and_then(|value| value.into_string().ok())
                    .map(|value| (*value).clone())
                    .unwrap_or_default();
                if !legacy_content.is_empty()
                    && !legacy_content_matches_incoming(&legacy_content, content)
                {
                    return Err(SyncError::Protocol(format!(
                        "NoteUpsert {} does not match resident legacy content",
                        hex_id(note_id)
                    )));
                }

                let tree = doc.get_tree("blocks");
                let exact_unstamped_reapply = if !legacy_content.is_empty() {
                    adopt_historical_ids_for_legacy_migration(
                        &tree,
                        &mut parsed.blocks,
                        &minted_ids,
                    );
                    false
                } else if !minted_ids.is_empty() && tree_has_block_history(&tree) {
                    if !adopt_minted_ids_for_exact_reapply(&tree, &mut parsed.blocks, &minted_ids) {
                        return Err(SyncError::Protocol(format!(
                            "NoteUpsert {} has changed unstamped blocks against resident history",
                            hex_id(note_id)
                        )));
                    }
                    true
                } else {
                    false
                };

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
                let frontmatter = parsed.frontmatter.clone().unwrap_or_default();
                root_meta
                    .insert("frontmatter", frontmatter.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro insert: {e}")))?;
                root_meta
                    .insert("slug", display_alias.as_deref().unwrap_or(""))
                    .map_err(|e| SyncError::Storage(format!("loro insert: {e}")))?;
                root_meta
                    .insert("title", title.as_str())
                    .map_err(|e| SyncError::Storage(format!("loro insert: {e}")))?;

                // Page properties are authoritative from the full
                // content and overwritten wholesale on every NoteUpsert
                // (they only arrive via full-content ops, never block
                // ops). Stored as an ordered list so render preserves
                // their on-disk order deterministically.
                set_page_properties(&doc, &parsed.page_properties)?;
                // Reconcile the block tree to the parsed body —
                // NON-DESTRUCTIVELY (2026-06-10, the iOS delete-revert
                // product bug). The historical path here was a destructive
                // reseed (`clear_block_tree` + `seed_tree_from_flatblocks`)
                // gated "server-only" on `materialize_dir` — but post-Loro-
                // cutover EVERY engine is an authoritative writer with
                // `materialize_dir` set (iOS `open_loro`, the desktop, the
                // server), so the gate was vacuously true everywhere and a
                // stale full-content NoteUpsert (legacy base-less PUT, the
                // frontmatter fallback, `reseed_from_disk`) could:
                //   1. RESURRECT blocks the user explicitly deleted (the
                //      stale body still carries them — data-loss vector #2's
                //      mirror image), and
                //   2. DELETE blocks absent from the stale body (vector #2
                //      itself), and
                //   3. re-mint every block node (fresh TreeIDs/container
                //      ids) — the disjoint-twin factory.
                // `reconcile_tree_to_blocks` instead updates matching bids
                // in place (lineage preserved), creates only never-seen
                // bids, SKIPS bids with a tombstoned node (deleted-wins —
                // only an explicit BlockUpsert/BlockDelete is author intent
                // for existence), and leaves live blocks absent from the
                // content untouched (anti-clobber). The common no-op
                // re-save still short-circuits via `tree_matches_blocks`.
                if !exact_unstamped_reapply && !tree_matches_blocks(&tree, &parsed.blocks) {
                    reconcile_tree_to_blocks(&tree, &parsed.blocks)?;
                }
                if root_meta.get("content").is_some() {
                    root_meta
                        .delete("content")
                        .map_err(|e| SyncError::Storage(format!("loro root delete: {e}")))?;
                }
                // Index the materialized post-reconcile projection, not the
                // possibly stale incoming subset. This keeps backlinks/tags
                // aligned with absence-is-not-delete and legacy lift.
                let indexed_content = doc_full_markdown(&doc);
                let indexed_page_properties = page_properties_materialized(&doc);
                self.index_upsert(
                    *note_id,
                    display_alias.as_deref(),
                    title,
                    &indexed_content,
                    &indexed_page_properties,
                );
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
                // Eager-seed `props`/`prop_keys` (P1.9b) at the SECOND
                // block-node creation site — mirrors the seed-loop above so a
                // BlockUpsert-created block also carries the empty containers
                // into shared history, making concurrent first-property sets
                // converge on ONE child container instead of rival ones. Done
                // BEFORE the text write so the migrate-on-apply path (P1.6)
                // folds into the SAME shared map.
                let (props, prop_keys) = prop_containers::node_prop_containers(&meta)?;
                // Migrate-on-apply (P1.6, flag DEFAULT-OFF): lift recognized
                // SOLELY-`key:: value` continuation lines OUT of the incoming
                // prose into the typed container, so a mixed-fleet old peer's
                // in-text property doesn't stay text-only (and doesn't
                // double-emit). Deterministic-shape: same incoming text + same
                // classification → same prose-strip + same prop ops on every
                // device, so concurrent migrators converge. Conservative strip
                // (a false-positive mid-prose strip is irreversible). Idempotent:
                // already-clean prose classifies to zero props → no-op. The
                // render keeps emitting `key:: value` lines (dual-read).
                if self.inner.migrate_in_text {
                    let (prose_only, lifted) = classify_block_prose_and_props(text.as_str());
                    write_block_text(&meta, prose_only.as_str())?;
                    for (key, value) in &lifted {
                        // `tags::` is a multi-value key → AddToList (union),
                        // never a scalar register (which would LWW-clobber a
                        // concurrent tag). Everything else lands as a text
                        // scalar — the value round-trips its canonical string
                        // form (matches what `materialize_props` re-emits).
                        let op = if key == "tags" {
                            PropOp::AddToList(PropScalar::Text(value.clone()))
                        } else {
                            PropOp::SetScalar(PropScalar::Text(value.clone()))
                        };
                        apply_prop_op(&props, &prop_keys, key, &op)?;
                    }
                } else {
                    write_block_text(&meta, text.as_str())?;
                }
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
                let Some((note_id, doc, node)) = self.find_doc_for_block(block_id).await else {
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
                meta.insert("parent", new_parent.map(|p| hex_id(&p)).unwrap_or_default())
                    .map_err(|e| SyncError::Storage(format!("loro meta insert: {e}")))?;
                doc.commit();
                Some(note_id)
            }
            OpPayload::BlockDelete { block_id } => {
                let Some((note_id, doc, node)) = self.find_doc_for_block(block_id).await else {
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
                // Tombstone EVERY remaining live node carrying this bid, not
                // just the first match (2026-06-10). Docs in the wild can hold
                // same-bid twins (disjoint-lineage residue the renderer dedups
                // via `dedup_twins_by_block_id`): deleting only one node
                // leaves the survivor rendering, so the user's delete silently
                // reverts on the next materialize. Author intent is bid-level
                // — kill them all.
                while let Some(twin) = find_node_by_block_id(&tree, &deleted_hex) {
                    tree.delete(twin)
                        .map_err(|e| SyncError::Storage(format!("loro tree delete: {e}")))?;
                }
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
mod tests;
