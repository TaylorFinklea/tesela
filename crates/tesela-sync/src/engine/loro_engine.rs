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
    entry.insert("name", "Inbox").map_err(ins)?;
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
// display_group_by?, display_show_done?}). Field-level LWW: concurrent
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
                out.push(crate::engine::ViewRecord {
                    id: get_str("id").unwrap_or_else(|| key.to_string()),
                    name: get_str("name").unwrap_or_default(),
                    dsl: get_str("dsl").unwrap_or_default(),
                    order: get_i64("order").unwrap_or(0),
                    builtin: get_bool("builtin").unwrap_or(false),
                    display_mode: get_str("display_mode").unwrap_or_else(|| "list".to_string()),
                    display_group_by: get_str("display_group_by").filter(|s| !s.is_empty()),
                    display_show_done: get_bool("display_show_done"),
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
    set_page_properties,
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
                let node = create_block_node_positioned(tree, prev_live.as_ref())?;
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
                let tree = doc.get_tree("blocks");
                if !tree_matches_blocks(&tree, &parsed.blocks) {
                    reconcile_tree_to_blocks(&tree, &parsed.blocks)?;
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

    #[tokio::test]
    async fn snapshot_export_import_preserves_props_only_empty_block() {
        let dev1 = DeviceId::from_bytes([0xe1; 16]);
        let e1 = LoroEngine::new(dev1, Arc::new(Hlc::new(dev1)));
        let dev2 = DeviceId::from_bytes([0xe2; 16]);
        let e2 = LoroEngine::new(dev2, Arc::new(Hlc::new(dev2)));
        let note_id = [0x42; 16];
        let block_id = [0x24; 16];

        e1.record_local(OpPayload::BlockUpsert {
            block_id,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
        e1.record_local(OpPayload::BlockPropertySet {
            note_id,
            block_id,
            key: "priority".into(),
            value: PropOp::SetScalar(PropScalar::Text("p2".into())),
        })
        .await
        .unwrap();

        let rendered = e1.render_note(note_id).await.unwrap();
        assert_eq!(
            rendered,
            "- <!-- bid:24242424-2424-2424-2424-242424242424 -->\n  priority:: p2\n"
        );

        let snapshot = e1.export_doc_update(note_id, None).await.unwrap();
        e2.import_doc_update(note_id, &snapshot).await.unwrap();

        assert_eq!(e2.render_note(note_id).await.unwrap(), rendered);
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

    /// tesela-ows.1 step 2 — ACCEPTANCE: a `status:: done` flip arriving over
    /// the wire into the ENGINE (relay/WS/FFI `.relay` import path, NOT an HTTP
    /// PUT) triggers the recurrence bump exactly ONCE on the receiving device,
    /// and the rolled state converges to every peer without a second bump.
    ///
    /// Exercises the hardest realistic shape: the flip is authored as a
    /// container `BlockPropertySet` (the FFI path), and the roll is authored
    /// back as CONTAINER prop sets (Lead constraint (a)) — no container clear,
    /// no in-text eviction. The container value wins render-time dedup, so the
    /// rolled deadline/status render with no render-side change.
    ///
    /// Revert-discriminating: on pre-fix code (no engine hook) the import merges
    /// `status:: done` with no roll, so the deadline / `recurrence_done` /
    /// `status:: todo` assertions below FAIL.
    #[tokio::test]
    async fn relay_done_flip_triggers_recurrence_bump_once_and_converges() {
        let dev1 = DeviceId::from_bytes([0xe1; 16]);
        let e1 = LoroEngine::new(dev1, Arc::new(Hlc::new(dev1)));
        let dev2 = DeviceId::from_bytes([0xe2; 16]);
        let e2 = LoroEngine::new(dev2, Arc::new(Hlc::new(dev2)));

        let note = [0x77; 16];
        let block: [u8; 16] = [0x07; 16];
        let bid_hex = "07070707-0707-0707-0707-070707070707";

        // e1 seeds a note with one recurring todo block on a shared lineage.
        e1.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("chores".into()),
            title: "Chores".into(),
            content: format!("- water plants <!-- bid:{bid_hex} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();
        for (k, v) in [
            ("recurring", "daily count 3"),
            ("deadline", "[[2026-05-07]]"),
            ("status", "todo"),
        ] {
            e1.record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: block,
                key: k.into(),
                value: PropOp::SetScalar(PropScalar::Text(v.into())),
            })
            .await
            .unwrap();
        }

        // e2 bootstraps from e1's full state (both share the block's lineage).
        let base = e1.export_doc_update(note, None).await.unwrap();
        e2.import_doc_update(note, &base).await.unwrap();
        assert_eq!(
            e1.render_note(note).await,
            e2.render_note(note).await,
            "bootstrapped equal"
        );

        // e2 (a non-lifecycle writer, e.g. iOS FFI) flips status → done. No bump
        // is authored on e2 — `record_local` (the local author path) is NOT
        // hooked; the roll happens when a peer IMPORTS this flip.
        let e2_before_flip = e2.doc_version(note).await;
        e2.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("done".into())),
        })
        .await
        .unwrap();
        let flip = e2
            .export_doc_update(note, e2_before_flip.as_deref())
            .await
            .unwrap();
        let e2_after_flip = e2.doc_version(note).await;
        assert!(
            e2.render_note(note).await.unwrap().contains("status:: done"),
            "e2's own flip stays `done` until the bump comes back"
        );

        // Relay delivers e2's flip to e1 → e1's apply_import runs the lifecycle.
        e1.import_doc_update(note, &flip).await.unwrap();
        let r1 = e1.render_note(note).await.unwrap();
        assert!(
            r1.contains("deadline:: [[2026-05-08]]"),
            "deadline advanced one day on e1: {r1:?}"
        );
        assert!(
            r1.contains("status:: todo") && !r1.contains("status:: done"),
            "status rolled back to todo on e1 (no residual done): {r1:?}"
        );
        assert!(
            r1.contains("recurrence_done:: 1"),
            "recurrence_done stamped on e1: {r1:?}"
        );
        assert!(
            r1.contains("last_completed:: [[2026-05-07]]"),
            "completion memory stamped in the CONTAINER on e1: {r1:?}"
        );
        assert_eq!(
            r1.matches("recurrence_done::").count(),
            1,
            "exactly one recurrence_done line — no double bump: {r1:?}"
        );

        // The bump broadcasts back to e2. It converges AND does not re-bump
        // (the frame carries a done→todo transition, never a fresh flip TO
        // done — the recursive-reimport guard).
        let bump = e1
            .export_doc_update(note, e2_after_flip.as_deref())
            .await
            .unwrap();
        e2.import_doc_update(note, &bump).await.unwrap();
        let r2 = e2.render_note(note).await.unwrap();
        assert_eq!(r1, r2, "e1 and e2 converge after the bump broadcast");
        assert_eq!(
            r2.matches("recurrence_done::").count(),
            1,
            "no double bump on e2 after importing the roll: {r2:?}"
        );

        // Re-delivering the ORIGINAL flip to e1 is idempotent — the frame adds
        // nothing causally new (no flip gate) and the guard is the backstop.
        e1.import_doc_update(note, &flip).await.unwrap();
        let r1b = e1.render_note(note).await.unwrap();
        assert_eq!(
            r1b.matches("recurrence_done::").count(),
            1,
            "re-delivered flip must not advance the series again: {r1b:?}"
        );
        assert!(r1b.contains("deadline:: [[2026-05-08]]"), "{r1b:?}");
    }

    /// tesela-ows.1 step 2 — the data-loss-class REGRESSION (Lead constraint
    /// (a), why attempt 2 died): two independent completions of the SAME
    /// recurring occurrence authored on DISJOINT lineages and delivered crossed
    /// must converge to EXACTLY ONE advance — AND the rolled peer's completion
    /// memory (`recurrence_done` / `last_completed`) must SURVIVE the
    /// disjoint-twin heal. It survives here precisely because the roll is
    /// authored into the typed props CONTAINER, which twin-heal's per-key union
    /// preserves; attempt 2 evicted it to in-text and a max-`TreeID` pick on the
    /// non-rolling twin wiped it.
    ///
    /// Assertions are robust to the twin-heal union order (which twin's scalar
    /// wins a same-key collision): the invariants are "exactly one bump", "no
    /// double advance to 05-09", and "completion memory preserved" — never a
    /// dependence on which node the max-`TreeID` rule kept.
    #[tokio::test]
    async fn crossed_duplicate_completions_converge_single_bump_no_dataloss() {
        let mk = |b: u8| {
            let d = DeviceId::from_bytes([b; 16]);
            LoroEngine::new(d, Arc::new(Hlc::new(d)))
        };
        let e_author = mk(0xe0);
        let e1 = mk(0xe1); // roller/target, shares lineage L1 with e_author
        let e2 = mk(0xe2); // disjoint duplicate author (lineage L2)

        let note = [0x77; 16];
        let block: [u8; 16] = [0x07; 16];
        let bid_hex = "07070707-0707-0707-0707-070707070707";

        async fn seed(e: &LoroEngine, note: [u8; 16], block: [u8; 16], bid_hex: &str, status: &str) {
            e.record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("chores".into()),
                title: "Chores".into(),
                content: format!("- water plants <!-- bid:{bid_hex} -->\n"),
                created_at_millis: 1,
            })
            .await
            .unwrap();
            for (k, v) in [
                ("recurring", "daily count 5"),
                ("deadline", "[[2026-05-07]]"),
                ("status", status),
            ] {
                e.record_local(OpPayload::BlockPropertySet {
                    note_id: note,
                    block_id: block,
                    key: k.into(),
                    value: PropOp::SetScalar(PropScalar::Text(v.into())),
                })
                .await
                .unwrap();
            }
        }

        // Lineage L1: e_author seeds O1 todo; e1 bootstraps from it (SHARED).
        seed(&e_author, note, block, bid_hex, "todo").await;
        let base = e_author.export_doc_update(note, None).await.unwrap();
        e1.import_doc_update(note, &base).await.unwrap();

        // e_author completes O1 (FFI flip); e1 imports → e1 ROLLS O1→O2.
        let before = e_author.doc_version(note).await;
        e_author
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: block,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("done".into())),
            })
            .await
            .unwrap();
        let flip = e_author
            .export_doc_update(note, before.as_deref())
            .await
            .unwrap();
        e1.import_doc_update(note, &flip).await.unwrap();
        let r1_roll = e1.render_note(note).await.unwrap();
        assert!(
            r1_roll.contains("deadline:: [[2026-05-08]]")
                && r1_roll.contains("recurrence_done:: 1")
                && r1_roll.contains("last_completed:: [[2026-05-07]]"),
            "e1 rolled O1→O2 exactly once with completion memory: {r1_roll:?}"
        );

        // Lineage L2 (DISJOINT): e2 independently authored the SAME bid at O1 and
        // completed it — a duplicate completion of the SAME occurrence O1.
        seed(&e2, note, block, bid_hex, "done").await;
        let dup = e2.export_doc_update(note, None).await.unwrap();

        // e1 (at O2) imports the disjoint done-twin. It must NOT advance again,
        // and the twin heal must NOT wipe e1's completion memory.
        e1.import_doc_update(note, &dup).await.unwrap();
        let r1 = e1.render_note(note).await.unwrap();
        assert_eq!(
            r1.matches("recurrence_done::").count(),
            1,
            "duplicate completion of O1 must not add a second bump: {r1:?}"
        );
        assert!(
            r1.contains("recurrence_done:: 1") && r1.contains("last_completed:: [[2026-05-07]]"),
            "completion memory PRESERVED through the disjoint-twin heal: {r1:?}"
        );
        assert!(
            !r1.contains("[[2026-05-09]]"),
            "no double advance to O3 (05-09): {r1:?}"
        );

        // All peers converge to the SAME single-bump state.
        let e1_state = e1.export_doc_update(note, None).await.unwrap();
        e_author.import_doc_update(note, &e1_state).await.unwrap();
        e2.import_doc_update(note, &e1_state).await.unwrap();
        let ra = e_author.render_note(note).await.unwrap();
        let rb = e2.render_note(note).await.unwrap();
        assert_eq!(
            ra.matches("recurrence_done::").count(),
            1,
            "e_author converges to one bump: {ra:?}"
        );
        assert_eq!(
            rb.matches("recurrence_done::").count(),
            1,
            "e2 converges to one bump: {rb:?}"
        );
        assert!(
            !ra.contains("[[2026-05-09]]") && !rb.contains("[[2026-05-09]]"),
            "no peer double-advanced: e_author={ra:?} e2={rb:?}"
        );
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
        assert_eq!(
            t1, t2,
            "engines diverged after concurrent positional insert"
        );
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
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
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
        let reloaded = LoroEngine::with_snapshot_dir(test_device(), hlc2, dir.clone())
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
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
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
        let reloaded = LoroEngine::with_snapshot_dir(test_device(), hlc2, dir)
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
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
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
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
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
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir.clone())
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
        let content =
            "---\ntitle: Full\n---\n\n- hello <!-- bid:00000000-0000-0000-0000-000000000001 -->\n";

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
        assert!(
            entry.tags.contains(&"alpha".to_string()),
            "frontmatter tag: {:?}",
            entry.tags
        );
        assert!(
            entry.tags.contains(&"beta".to_string()),
            "inline body tag: {:?}",
            entry.tags
        );
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
        // Review finding [2], revised 2026-06-10: a full-content NoteUpsert
        // re-syncs an already-populated tree NON-destructively — it heals
        // the blocks its body carries (text/indent, in place, lineage
        // preserved) but must NOT remove live blocks absent from the body
        // (the stale-PUT anti-clobber rule; on the real fleet "absent from
        // a whole-content save" is routinely a peer's concurrent block, and
        // the old clear+reseed deleting it was data-loss vector #2 — and
        // the same reseed RESURRECTED explicitly-deleted blocks, the
        // 2026-06-10 iOS delete-revert bug). Removal flows ONLY through an
        // explicit BlockDelete.
        let tmp = tempfile::tempdir().unwrap();
        let engine = LoroEngine::with_dirs(
            test_device(),
            Arc::new(Hlc::new(test_device())),
            tmp.path().join("loro"),
            Some(tmp.path().join("notes")),
        )
        .await
        .unwrap();
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

        // Drift the tree out of band: a stale extra block AND a text drift
        // on a body block.
        let stale_bid: [u8; 16] = [0x33; 16];
        {
            let doc = engine.doc_for_note_mut(note_id).await;
            let tree = doc.get_tree("blocks");
            let n = tree.create(TreeParentId::Root).unwrap();
            let m = tree.get_meta(n).unwrap();
            m.insert("block_id", hex_id(&stale_bid).as_str()).unwrap();
            write_block_text(&m, "STALE").unwrap();
            m.insert("indent_level", 0i64).unwrap();
            let drifted = find_node_by_block_id(&tree, "11111111111111111111111111111111").unwrap();
            let dm = tree.get_meta(drifted).unwrap();
            write_block_text(&dm, "one DRIFTED").unwrap();
            doc.commit();
            // On a real flow a peer block arrives via import, whose
            // `refresh_note_derived` registers it in the block_index — do
            // the same so the explicit delete below can resolve its doc.
            engine.refresh_note_derived(note_id, &doc).await;
        }
        assert!(engine.render_note(note_id).await.unwrap().contains("STALE"));
        assert!(engine
            .render_note(note_id)
            .await
            .unwrap()
            .contains("one DRIFTED"));

        // Re-save the canonical body: body blocks heal in place; the
        // unknown live block SURVIVES (no destructive reseed).
        engine.record_local(up(body.to_string())).await.unwrap();
        let rendered = engine.render_note(note_id).await.unwrap();
        assert!(
            !rendered.contains("one DRIFTED") && rendered.contains("- one"),
            "body-block text drift heals in place: {rendered:?}"
        );
        assert!(
            rendered.contains("STALE"),
            "a live block absent from the body must survive a whole-content save: {rendered:?}"
        );

        // Removal is explicit-only.
        engine
            .record_local(OpPayload::BlockDelete {
                block_id: stale_bid,
            })
            .await
            .unwrap();
        let rendered = engine.render_note(note_id).await.unwrap();
        assert!(!rendered.contains("STALE"), "{rendered:?}");
        assert_eq!(
            rendered, body,
            "explicit delete restores the canonical body"
        );
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
            block_id: [0xaa; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "a".into(),
            indent_level: 0,
            text: "A edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
        b.record_local(OpPayload::BlockUpsert {
            block_id: [0xbb; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "b".into(),
            indent_level: 0,
            text: "B edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
        c.record_local(OpPayload::BlockUpsert {
            block_id: [0xcc; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "c".into(),
            indent_level: 0,
            text: "C edit".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

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
        assert_eq!(
            nothing, 0,
            "no new broadcasts at steady state (bounded re-broadcast)"
        );
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
        let a_concrete = LoroEngine::new(
            DeviceId::from_bytes([0xc1; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xc1; 16]))),
        );
        let b_concrete = LoroEngine::new(
            DeviceId::from_bytes([0xd2; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xd2; 16]))),
        );
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
                block_id: [0xca; 16],
                note_id: note,
                parent_block_id: None,
                order_key: "a".into(),
                indent_level: 0,
                text: "from A".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        b_concrete
            .record_local(OpPayload::BlockUpsert {
                block_id: [0xcb; 16],
                note_id: note,
                parent_block_id: None,
                order_key: "b".into(),
                indent_level: 0,
                text: "from B".into(),
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
        let a = LoroEngine::new(
            DeviceId::from_bytes([0xa1; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xa1; 16]))),
        );
        let b = LoroEngine::new(
            DeviceId::from_bytes([0xb2; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xb2; 16]))),
        );
        assert_ne!(
            a.peer_id(),
            b.peer_id(),
            "devices must have distinct peer ids"
        );
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
        assert_eq!(
            a.render_note(note).await,
            b.render_note(note).await,
            "bootstrapped equal"
        );

        // Concurrent edits: A appends a block, B appends a different one.
        a.record_local(OpPayload::BlockUpsert {
            block_id: [0xaa; 16],
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
            block_id: [0xbb; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "b".into(),
            indent_level: 0,
            text: "from B".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

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
            if !u.is_empty() {
                b.import_doc_update(note, &u).await.unwrap();
            }
        }
        assert_eq!(
            a.render_note(note).await.unwrap(),
            ra,
            "stable after re-exchange"
        );
        assert_eq!(
            b.render_note(note).await.unwrap(),
            rb,
            "stable after re-exchange"
        );
    }

    #[tokio::test]
    async fn concurrent_first_property_set_on_shared_block_both_survive() {
        // P1.9b FOUNDATION: two devices share a base note carrying ONE
        // propsless block (seeded via NoteUpsert, so the block node reaches
        // SHARED history before the peers diverge). A first-sets scalar
        // `status`, B first-sets scalar `priority` — DISTINCT keys —
        // concurrently. After a bidirectional exchange BOTH keys must be
        // present on BOTH replicas.
        //
        // Without eager-seeding the per-block `props`/`prop_keys` containers
        // at the shared-base creation site, each device's FIRST property set
        // MINTS a rival `props` map (Loro derives the child container id from
        // the creating op). On merge the two rival maps collide at the same
        // node-meta register and one OVERWRITES the other (LWW, not union),
        // so one device's property vanishes. Seeding the empty containers
        // into shared history first makes both get-or-create resolve to the
        // SAME child id → union.
        let a = LoroEngine::new(
            DeviceId::from_bytes([0xa1; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xa1; 16]))),
        );
        let b = LoroEngine::new(
            DeviceId::from_bytes([0xb2; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xb2; 16]))),
        );
        let note = [0x88; 16];
        // The seeded block's id is the bid in the comment: bytes [0x07; 16].
        let block: [u8; 16] = [0x07; 16];

        a.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("shared".into()),
            title: "Shared".into(),
            content: "- base <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

        // B bootstraps from A's full state — the shared base now lives in
        // both peers' history (the propsless block node included).
        let bootstrap = a.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &bootstrap).await.unwrap();
        assert_eq!(
            a.render_note(note).await,
            b.render_note(note).await,
            "bootstrapped equal"
        );

        // Concurrent FIRST property sets on the SAME shared block, DISTINCT
        // keys. Neither device has seen the other's set yet.
        a.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();
        b.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "priority".into(),
            value: PropOp::SetScalar(crate::PropScalar::Int(3)),
        })
        .await
        .unwrap();

        // Exchange both ways, using each peer's version vector as the cursor.
        let b_vv = b.doc_version(note).await;
        let a_upd = a.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_upd).await.unwrap();
        let a_vv = a.doc_version(note).await;
        let b_upd = b.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        a.import_doc_update(note, &b_upd).await.unwrap();

        let ra = a.render_note(note).await.unwrap();
        let rb = b.render_note(note).await.unwrap();
        assert_eq!(ra, rb, "replicas converge to identical state");
        assert!(
            ra.contains("status:: doing"),
            "A's property must survive on the merged replica, got: {ra:?}"
        );
        assert!(
            ra.contains("priority:: 3"),
            "B's property must survive on the merged replica, got: {ra:?}"
        );
    }

    #[tokio::test]
    async fn concurrent_same_key_scalar_set_is_deterministic_lww() {
        // P1.9b: a same-key concurrent scalar set is LWW-by-HLC (the v1
        // product decision — a scalar has no union semantics). The invariant
        // we DO require is that both replicas pick the IDENTICAL winner after
        // a bidirectional exchange (deterministic, no oscillation), and the
        // losing key isn't dropped wholesale.
        let a = LoroEngine::new(
            DeviceId::from_bytes([0xc1; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xc1; 16]))),
        );
        let b = LoroEngine::new(
            DeviceId::from_bytes([0xd2; 16]),
            Arc::new(Hlc::new(DeviceId::from_bytes([0xd2; 16]))),
        );
        let note = [0x99; 16];
        let block: [u8; 16] = [0x07; 16];

        a.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("shared".into()),
            title: "Shared".into(),
            content: "- base <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
        let bootstrap = a.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &bootstrap).await.unwrap();

        // Same key `status`, conflicting concurrent values.
        a.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();
        b.record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(crate::PropScalar::Text("done".into())),
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
        assert_eq!(
            ra, rb,
            "same-key scalar LWW converges to one winner on both"
        );
        assert!(
            ra.contains("status:: doing") || ra.contains("status:: done"),
            "exactly one of the conflicting values must win, got: {ra:?}"
        );

        // Re-exchange must be a stable no-op (no oscillation between winners).
        let b_vv2 = b.doc_version(note).await;
        if let Some(u) = a.export_doc_update(note, b_vv2.as_deref()).await {
            if !u.is_empty() {
                b.import_doc_update(note, &u).await.unwrap();
            }
        }
        assert_eq!(
            a.render_note(note).await.unwrap(),
            ra,
            "stable after re-exchange"
        );
        assert_eq!(
            b.render_note(note).await.unwrap(),
            rb,
            "stable after re-exchange"
        );
    }

    #[tokio::test]
    async fn note_upsert_does_not_clobber_concurrent_block_property() {
        // P1.8: prop ops are the SOLE writers of `props`. After a property
        // migrates into a typed `props` container (prose-only `text_seq`), an
        // OLD-PEER full-content NoteUpsert re-injects the property as an
        // in-text `key:: value` continuation line. `parse_note` folds that
        // line back into `FlatBlock.text` (and leaves `FlatBlock.properties`
        // empty), so the incoming block's text is `"buy milk\nstatus:: doing"`
        // while the live tree's block text is the prose-only `"buy milk"`. The
        // OLD `tree_matches_blocks` compares raw text → MISMATCH → reseed →
        // the typed `status` container is destroyed and the property collapses
        // back into prose text (re-embedded, no longer a mergeable container).
        //
        // The fix: strip recognized `key:: value` lines from the incoming body
        // before comparing prose, AND compare materialized props per block.
        // Stripped prose (`buy milk`) matches the tree's prose AND the body's
        // lifted props (`status:: doing`) match the container's materialized
        // props → NOT drifted → no reseed → the container survives.
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        let note = [0x71; 16];
        let block: [u8; 16] = [0x07; 16];

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: "- buy milk <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // A property set lands on the block as a typed container (the SOLE
        // writer of `props`); the block's prose stays prose-only.
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: block,
                key: "status".into(),
                value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();
        assert!(
            engine
                .render_note(note)
                .await
                .unwrap()
                .contains("status:: doing"),
            "property is set before the re-save"
        );

        // An OLD-PEER full-content NoteUpsert that carries the property as an
        // IN-TEXT continuation line (the un-migrated shape). It must be
        // recognized as the same block (stripped prose + props both match) and
        // must NOT reseed — leaving the typed container intact.
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: "- buy milk <!-- bid:07070707-0707-0707-0707-070707070707 -->\n  status:: doing\n".into(),
                created_at_millis: 2,
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note).await.unwrap();
        assert!(
            rendered.contains("status:: doing"),
            "property must survive an old-peer in-text NoteUpsert, got: {rendered:?}"
        );
        assert!(
            rendered.contains("buy milk"),
            "prose must survive too, got: {rendered:?}"
        );
        // The property must remain a TYPED container — the block's prose-only
        // `text_seq` must NOT have the property re-embedded into it (a reseed
        // would fold `status:: doing` back into block text).
        {
            let doc = engine.doc_for_note_mut(note).await;
            let tree = doc.get_tree("blocks");
            let node = find_node_by_block_id(&tree, &hex::encode(block)).unwrap();
            assert_eq!(
                read_block_text(&tree, node).as_deref(),
                Some("buy milk"),
                "block text stays prose-only — the property is NOT folded back into text"
            );
            let meta = tree.get_meta(node).unwrap();
            let (props, _) = prop_containers::read_node_prop_containers(&meta).unwrap();
            assert_eq!(
                prop_containers::prop_get_scalar(&props, "status"),
                Some(crate::PropScalar::Text("doing".into())),
                "the typed container survives"
            );
        }
    }

    #[tokio::test]
    async fn note_upsert_does_not_clobber_concurrent_block_property_on_server() {
        // P1.8 — the SERVER variant of the clobber test. This is the actual
        // TDD proof that the `tree_matches_blocks` prose-strip is load-bearing.
        //
        // On a DEVICE engine the reseed gate (`tree_is_empty ||
        // materialize_dir.is_some()`) skips the reseed entirely, so the typed
        // container survives an old-peer in-text NoteUpsert no matter what
        // `tree_matches_blocks` returns — the device variant proves nothing
        // about the prose-strip. The reseed only fires on the AUTHORITATIVE
        // writer (materialize_dir set). Here, if the prose-strip is reverted to
        // the old raw `read_block_text(...) == block.text` compare, the live
        // tree's prose-only `"buy milk"` won't equal the body's
        // `"buy milk\nstatus:: doing"` → drift → reseed → the new block is
        // seeded from the body with the property folded BACK into block text,
        // so `read_block_text` would return `Some("buy milk\nstatus:: doing")`
        // and the `block text stays prose-only` assertion below FAILS. The
        // prose-strip makes the old-peer body NOT count as drift, so the
        // server never reseeds and the prose stays prose-only.
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
        let note = blake3_note_id("server-clobber");
        let block: [u8; 16] = [0x07; 16];

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: "- buy milk <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // A property set lands on the block as a typed container; prose stays
        // prose-only.
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: block,
                key: "status".into(),
                value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();
        assert!(
            engine
                .render_note(note)
                .await
                .unwrap()
                .contains("status:: doing"),
            "property is set before the re-save"
        );

        // An OLD-PEER full-content NoteUpsert carrying the property as an
        // IN-TEXT continuation line. On the SERVER the reseed gate is open, so
        // the ONLY thing keeping this from reseeding (and folding the property
        // back into block text) is the prose-strip in `tree_matches_blocks`.
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: "- buy milk <!-- bid:07070707-0707-0707-0707-070707070707 -->\n  status:: doing\n".into(),
                created_at_millis: 2,
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note).await.unwrap();
        assert!(
            rendered.contains("status:: doing"),
            "property must survive an old-peer in-text NoteUpsert on the server, got: {rendered:?}"
        );
        assert!(
            rendered.contains("buy milk"),
            "prose must survive too, got: {rendered:?}"
        );
        // The load-bearing assertion: the block's prose-only `text_seq` must
        // NOT have the property re-embedded. A reseed (which the prose-strip
        // prevents) would seed the block from the body's
        // `"buy milk\nstatus:: doing"` and this would be `Some(...status...)`.
        let doc = engine.doc_for_note_mut(note).await;
        let tree = doc.get_tree("blocks");
        let node = find_node_by_block_id(&tree, &hex::encode(block)).unwrap();
        assert_eq!(
            read_block_text(&tree, node).as_deref(),
            Some("buy milk"),
            "block text stays prose-only — the server did NOT reseed the old-peer in-text body"
        );
        let meta = tree.get_meta(node).unwrap();
        let (props, _) = prop_containers::read_node_prop_containers(&meta).unwrap();
        assert_eq!(
            prop_containers::prop_get_scalar(&props, "status"),
            Some(crate::PropScalar::Text("doing".into())),
            "the typed container survives on the server path"
        );
    }

    #[tokio::test]
    async fn note_upsert_drift_reseed_preserves_props() {
        // P1.8: when a reseed is GENUINELY unavoidable (structural drift the
        // block-granular diff didn't capture — here a brand-new block in the
        // body), the surviving block_id's materialized props must be
        // snapshotted before `clear_block_tree` and replayed after the
        // reseed. Reseed stays SERVER-ONLY (gate on materialize_dir), so this
        // runs on an authoritative engine. Without the snapshot/replay, the
        // reseed drops the property (clear_block_tree tombstones the node and
        // the body never carried the prop).
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
        let note = blake3_note_id("drift");
        let block_a: [u8; 16] = [0x07; 16];
        let body_one = "- one <!-- bid:07070707-0707-0707-0707-070707070707 -->\n";

        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("drift".into()),
                title: "Drift".into(),
                content: body_one.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: block_a,
                key: "status".into(),
                value: PropOp::SetScalar(crate::PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();
        assert!(engine
            .render_note(note)
            .await
            .unwrap()
            .contains("status:: doing"));

        // A NoteUpsert whose body has the SAME first block PLUS a brand-new
        // second block — genuine drift forcing a reseed.
        let body_two = "- one <!-- bid:07070707-0707-0707-0707-070707070707 -->\n\
                        - two <!-- bid:08080808-0808-0808-0808-080808080808 -->\n";
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("drift".into()),
                title: "Drift".into(),
                content: body_two.into(),
                created_at_millis: 2,
            })
            .await
            .unwrap();

        let rendered = engine.render_note(note).await.unwrap();
        assert!(
            rendered.contains("two"),
            "the drift body's new block must land, got: {rendered:?}"
        );
        assert!(
            rendered.contains("status:: doing"),
            "the surviving block's property must be replayed across the reseed, got: {rendered:?}"
        );
    }

    #[tokio::test]
    async fn note_upsert_never_destructively_reseeds() {
        // P1.8 regression, generalized 2026-06-10: a destructive reseed
        // re-mints the block tree (fresh node ids), minting rival container
        // ids that overwrite instead of merge across peers. Post-cutover
        // EVERY engine is an authoritative writer, so the old "server-only"
        // materialize_dir gate was vacuous — NoteUpsert now reconciles
        // non-destructively on every engine: a drifting full-content
        // NoteUpsert must NOT remove blocks absent from its body; they
        // converge via explicit block ops / the twin heal instead.
        let dev = test_device();
        let device_engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        let note = [0x73; 16];
        let body_one = "- one <!-- bid:07070707-0707-0707-0707-070707070707 -->\n";
        device_engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: body_one.into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // Drift the shadow tree out of band: append a stale block.
        {
            let doc = device_engine.doc_for_note_mut(note).await;
            let tree = doc.get_tree("blocks");
            let n = tree.create(TreeParentId::Root).unwrap();
            let m = tree.get_meta(n).unwrap();
            m.insert("block_id", "33333333-3333-3333-3333-333333333333")
                .unwrap();
            write_block_text(&m, "STALE").unwrap();
            m.insert("indent_level", 0i64).unwrap();
            doc.commit();
        }
        assert!(device_engine
            .render_note(note)
            .await
            .unwrap()
            .contains("STALE"));

        // A drifting full-content NoteUpsert on the NON-authoritative engine
        // must NOT reseed (which would re-mint rival container ids); the stale
        // block stays until block ops / the twin heal converge it.
        device_engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: body_one.into(),
                created_at_millis: 2,
            })
            .await
            .unwrap();
        assert!(
            device_engine
                .render_note(note)
                .await
                .unwrap()
                .contains("STALE"),
            "a device (non-authoritative) engine must NOT reseed on drift"
        );
    }

    #[tokio::test]
    async fn note_upsert_does_not_delete_absent_blocks_on_authoritative_engine() {
        // Data-loss vector #2 at the ENGINE level (2026-06-10): on an
        // AUTHORITATIVE engine (materialize_dir set — post-cutover that is
        // every engine: iOS, desktop, server), a stale full-content
        // NoteUpsert whose body LACKS a block a peer added must NOT delete
        // it. The old clear+reseed did exactly that.
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
        let note = blake3_note_id("anticlobber");
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("anticlobber".into()),
                title: "A".into(),
                content: "- mine <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        // A peer's block lands via an explicit BlockUpsert.
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: [0x08; 16],
                note_id: note,
                parent_block_id: None,
                order_key: "00000001".into(),
                indent_level: 0,
                text: "peer block".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        // A STALE whole-content upsert (authored before the peer's block).
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("anticlobber".into()),
                title: "A".into(),
                content: "- mine <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
                created_at_millis: 2,
            })
            .await
            .unwrap();
        let rendered = engine.render_note(note).await.unwrap();
        assert!(
            rendered.contains("peer block"),
            "a stale NoteUpsert must not delete blocks absent from its body: {rendered:?}"
        );
        assert!(rendered.contains("mine"), "{rendered:?}");
    }

    #[tokio::test]
    async fn block_delete_tombstones_every_same_bid_twin() {
        // 2026-06-10 (the iOS delete-revert product bug, twin half): docs in
        // the wild can carry same-bid TWINS (disjoint-lineage residue) that
        // the renderer dedups via `dedup_twins_by_block_id` — the user sees
        // ONE block. A BlockDelete that tombstones only the first matching
        // node leaves the survivor rendering, so the delete silently
        // reverts on the next materialize. Author intent is bid-level.
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        let note = [0x74; 16];
        let bid: [u8; 16] = [0x07; 16];
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("twins".into()),
                title: "T".into(),
                content: "- keep <!-- bid:09090909-0909-0909-0909-090909090909 -->\n- doomed <!-- bid:07070707-0707-0707-0707-070707070707 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        // Inject a rival live node with the SAME bid (what a disjoint
        // lineage union leaves behind when the dedup can't run).
        {
            let doc = engine.doc_for_note_mut(note).await;
            let tree = doc.get_tree("blocks");
            let n = tree.create(TreeParentId::Root).unwrap();
            let m = tree.get_meta(n).unwrap();
            m.insert("block_id", hex_id(&bid).as_str()).unwrap();
            write_block_text(&m, "doomed twin").unwrap();
            m.insert("indent_level", 0i64).unwrap();
            doc.commit();
        }
        engine
            .record_local(OpPayload::BlockDelete { block_id: bid })
            .await
            .unwrap();
        let rendered = engine.render_note(note).await.unwrap();
        assert!(
            !rendered.contains("doomed"),
            "BlockDelete must tombstone EVERY live node carrying the bid: {rendered:?}"
        );
        assert!(rendered.contains("keep"), "{rendered:?}");
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
            rendered, "- keep <!-- bid:10101010-1010-1010-1010-101010101010 -->\n",
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
                block_id: a,
                note_id,
                parent_block_id: None,
                order_key: "a".into(),
                indent_level: 0,
                text: "A".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: b,
                note_id,
                parent_block_id: Some(a),
                order_key: "b".into(),
                indent_level: 1,
                text: "B".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: c,
                note_id,
                parent_block_id: Some(b),
                order_key: "c".into(),
                indent_level: 2,
                text: "C".into(),
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
        assert_eq!(
            engine.index_entries().await.len(),
            2,
            "ghost present pre-rebuild"
        );

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
        idx.import(&tokio::fs::read(&idx_path).await.unwrap())
            .unwrap();
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
        assert!(
            entries[0].tags.contains(&"alpha".to_string()),
            "tags: {:?}",
            entries[0].tags
        );
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
            let entry = notes
                .insert_container(&hex_id(&note_id), loro::LoroMap::new())
                .unwrap();
            entry.insert("title", "Kept").unwrap();
            entry.insert("slug", "kept-slug").unwrap();
            engine.inner.index.commit();
        }

        engine.rebuild_index_from_docs().await;
        let entries = engine.index_entries().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].slug, "kept-slug",
            "slug preserved from prior index"
        );
        assert_eq!(entries[0].title, "Kept");
        assert!(
            entries[0].tags.contains(&"z".to_string()),
            "tags derived: {:?}",
            entries[0].tags
        );
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
        assert!(
            e.tags.contains(&"daily".to_string()),
            "frontmatter tag: {:?}",
            e.tags
        );
        assert!(
            e.tags.contains(&"project".to_string()),
            "page-prop tag: {:?}",
            e.tags
        );
        assert!(
            e.tags.contains(&"urgent".to_string()),
            "inline tag: {:?}",
            e.tags
        );
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
        let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes.clone()))
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
        assert!(
            on_disk.contains("- one ") && on_disk.contains("- two "),
            "block append materialized: {on_disk:?}"
        );
        assert!(
            on_disk.starts_with("---\ntitle: Daily\n---\n"),
            "frontmatter preserved"
        );

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
        assert!(
            !tmp.path().join("notes").exists(),
            "no .md materialization when non-authoritative"
        );
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
        let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes.clone()))
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
        assert!(
            rb.contains("- just text") && rb.contains("<!-- bid:"),
            "beta canonicalized: {rb:?}"
        );
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
            let decoded = decode_loro_relay_payload(&wire)
                .unwrap()
                .expect("v2 payload");
            let pairs: Vec<([u8; 16], Vec<u8>)> = decoded
                .into_iter()
                .map(|u| (u.doc, u.update_bytes))
                .collect();
            let n = to.apply_relay_updates(&pairs).await.applied_count();
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
        assert!(
            again.is_empty(),
            "persisted cursor suppresses re-broadcast after restart"
        );
    }

    #[tokio::test]
    async fn produce_re_emits_when_broadcast_cursor_is_undecodable() {
        // Regression (2026-06-25): a corrupt / incompatible persisted
        // broadcast cursor must NOT permanently strand a note's outbound.
        // Before the fix, export_doc_update returned None on a VersionVector
        // decode failure and produce_relay_updates silently SKIPPED the dirty
        // note (no `else` at the `if let Some(bytes)` push) — so the device
        // never re-broadcast it. On iOS this presented as: a today edit
        // records (splice applied=1) but tick_outbound sends 0 ops, no error,
        // forever → iOS edits never reach the desktop.
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("loro");
        let notes = tmp.path().join("notes");
        let dev = test_device();
        let note = blake3_note_id("stuck");
        let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes))
            .await
            .unwrap();
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("stuck".into()),
                title: "S".into(),
                content: "- hi <!-- bid:70707070-7070-7070-7070-707070707070 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        // Corrupt / incompatible persisted cursor for this note (e.g. a
        // version-format change or a stale lineage), so the incremental
        // export from it cannot be produced.
        engine
            .inner
            .broadcast_cursor
            .write()
            .await
            .insert(note, vec![0xff, 0xff, 0xff, 0xff]);
        let out = engine.produce_relay_updates().await;
        assert_eq!(
            out.len(),
            1,
            "a dirty note whose broadcast cursor won't decode must still export \
             (full-snapshot fallback), never be silently skipped"
        );
    }

    #[tokio::test]
    async fn produce_re_emits_snapshot_when_broadcast_cursor_is_ahead_of_current() {
        // Regression (2026-06-29): an authoritative import can rebase a note's
        // doc 'backward' (the convergence/bootstrap heal imports do this),
        // leaving the persisted broadcast cursor AT-OR-AHEAD of the doc's
        // current version. The cursor still DECODES (so the undecodable
        // snapshot fallback never fires) and is != current bytes (so produce's
        // dirty-skip never fires) — but `updates(since_vv)` is then an
        // EMPTY/no-op delta because since_vv already covers current. Before the
        // fix, export_doc_update shipped that empty delta (`.ok()` was
        // `Some(empty)`, so the snapshot `.or_else` never ran) and the note's
        // REAL current content never reached the relay. On iOS this presented
        // as: a today edit records (splice applied=1) but tick_outbound ships a
        // content-less frame, no error, forever → iOS edits never reach the
        // desktop.
        use loro::VersionVector;

        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("loro");
        let notes = tmp.path().join("notes");
        let dev = test_device();
        let note = blake3_note_id("ahead");
        let engine = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, Some(notes))
            .await
            .unwrap();
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("ahead".into()),
                title: "A".into(),
                content: "- convergence-canary <!-- bid:80808080-8080-8080-8080-808080808080 -->\n"
                    .into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // Realistic path: produce + commit so a decodable cursor is persisted at
        // the doc's current version.
        let first = engine.produce_relay_updates().await;
        assert_eq!(first.len(), 1, "first produce emits the note");
        let committed: Vec<([u8; 16], Vec<u8>)> =
            first.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
        engine.commit_broadcast_cursors(&committed).await;
        assert!(
            engine.produce_relay_updates().await.is_empty(),
            "committed cursor at current → nothing dirty"
        );

        // Now simulate a backward rebase: bump the persisted cursor PAST the
        // doc's current version (the same net state as an authoritative import
        // that rebased current backward). The cursor decodes fine and differs
        // from current bytes, so neither the undecodable fallback nor produce's
        // dirty-skip applies — yet `updates(cursor)` would be empty.
        let current_enc = engine.doc_version(note).await.unwrap();
        let mut ahead = VersionVector::decode(&current_enc).unwrap();
        let bumps: Vec<(u64, i32)> = ahead.iter().map(|(p, c)| (*p, *c)).collect();
        assert!(!bumps.is_empty(), "doc must have ops to bump past");
        for (peer, counter) in bumps {
            ahead.set_end(loro::ID::new(peer, counter + 8));
        }
        let ahead_enc = ahead.encode();
        assert_ne!(
            ahead_enc, current_enc,
            "crafted cursor must differ from current bytes (stay dirty)"
        );
        engine
            .inner
            .broadcast_cursor
            .write()
            .await
            .insert(note, ahead_enc);

        // The dirty note must export a delta that brings a receiver to CURRENT.
        let out = engine.produce_relay_updates().await;
        assert_eq!(
            out.len(),
            1,
            "a dirty note whose cursor is ahead-of-current must still export"
        );
        let (got_id, bytes, _vv) = &out[0];
        assert_eq!(*got_id, note);

        // The exported bytes must import cleanly into a FRESH engine and
        // reproduce the note's CURRENT content — i.e. a real snapshot, not an
        // empty no-op delta.
        let tmp2 = tempfile::tempdir().unwrap();
        let dev2 = DeviceId::from_bytes([2u8; 16]);
        let fresh = LoroEngine::with_dirs(
            dev2,
            Arc::new(Hlc::new(dev2)),
            tmp2.path().join("loro"),
            Some(tmp2.path().join("notes")),
        )
        .await
        .unwrap();
        fresh.import_doc_update(note, bytes).await.unwrap();
        let rendered = fresh
            .render_note(note)
            .await
            .expect("fresh engine should hold the note after import");
        assert!(
            rendered.contains("convergence-canary"),
            "ahead-cursor produce must ship a full snapshot reproducing CURRENT \
             content, not an empty no-op delta; got: {rendered:?}"
        );
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
        assert_eq!(
            retry.len(),
            1,
            "failed send re-emits the delta — not dropped"
        );
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
    async fn local_edits_carry_timestamps_but_the_builtin_views_seed_stays_ts0() {
        // tesela-c7s item 1, precisely scoped. Two invariants that MUST hold
        // together:
        //  (a) a REAL local authoring op carries a wall-clock timestamp (> 0),
        //      so a strand investigation can see when a note last changed;
        //  (b) the DETERMINISTIC `builtin_views_seed_update` stays ts == 0, so
        //      its bytes are byte-identical on every device (the fresh-device-
        //      clobber invariant — two independent seeds must author the SAME
        //      op ids, which a per-device wall-clock stamp would break).
        //
        // REVERT-DISCRIMINATING both ways: removing `set_record_timestamp(true)`
        // from `set_doc_peer` drops (a) to ts == 0; flipping the seed builder to
        // `set_record_timestamp(true)` raises (b) above 0 (and would also break
        // `views_seed_update_is_deterministic`).
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        let note = blake3_note_id("stamped");
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("stamped".into()),
                title: "S".into(),
                content: "- hi <!-- bid:e5e5e5e5-e5e5-e5e5-e5e5-e5e5e5e5e5e5 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        let snapshot = engine.export_doc_update(note, None).await.unwrap();
        let meta = LoroDoc::decode_import_blob_meta(&snapshot, false).unwrap();
        assert!(
            meta.end_timestamp > 0,
            "a real local edit must record a wall-clock change timestamp; got {}",
            meta.end_timestamp
        );

        let seed = builtin_views_seed_update().unwrap();
        let seed_meta = LoroDoc::decode_import_blob_meta(&seed, false).unwrap();
        assert_eq!(
            seed_meta.end_timestamp, 0,
            "the deterministic builtin-views seed MUST stay ts=0 (byte-identical \
             across devices); got {}",
            seed_meta.end_timestamp
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
        let full_snapshot = author
            .export_doc_update(note, None)
            .await
            .expect("snapshot");
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
    /// A-twin unions with the server's; under the PURE max-`TreeID` rule
    /// (tesela-fte) the survivor per bid is ONLY the higher-`TreeID`
    /// (higher-peer) twin — the device (0x7f) outranks the server (0x5e), so
    /// A resolves to the device's re-shipped "Awesome" and B to "B device".
    /// The stale-guard that formerly preserved "Awesome sweet" is dropped
    /// (product-approved 2026-07-01: higher-TreeID text wins).
    const A_BID: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
    const B_BID: &str = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";
    const A_BID_BYTES: [u8; 16] = [0x0a; 16];
    const B_BID_BYTES: [u8; 16] = [0x0b; 16];

    async fn seed_disjoint(server: &LoroEngine, device: &LoroEngine, note: [u8; 16]) {
        // BOTH author the same note body independently — disjoint Loro
        // lineages (distinct TreeIDs for the same block_ids).
        let content = format!("- Awesome <!-- bid:{A_BID} -->\n- B base <!-- bid:{B_BID} -->\n");
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

    // FLIPPED by tesela-fte (pure max-`TreeID`): formerly
    // `ws_apply_stale_snapshot_does_not_revert_peer_edit`, which asserted the
    // stale-guard preserved the server's newer "Awesome sweet". Under pure
    // max-`TreeID` the survivor per bid is ONLY the higher-`TreeID`
    // (higher-peer) twin: the device (0x7f) outranks the server (0x5e), so A
    // resolves to the device's re-shipped stale "Awesome" and the server's
    // "Awesome sweet" is dropped. B still resolves to the device's genuine "B
    // device". Product-approved 2026-07-01: higher-TreeID text wins over the
    // genuine-edit/stale-guard preference.
    #[tokio::test]
    async fn ws_apply_disjoint_conflict_resolves_to_max_treeid_twin() {
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let ddev = DeviceId::from_bytes([0x7f; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let note = blake3_note_id("daily");

        seed_disjoint(&server, &device, note).await;

        // Server edits A via HTTP-style block op (the newer value).
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

        // Server applies the device's snapshot via the WS-apply path.
        server.import_doc_update(note, &snapshot).await.unwrap();

        let a = block_text(&server, note, A_BID_BYTES)
            .await
            .unwrap_or_default();
        let b = block_text(&server, note, B_BID_BYTES)
            .await
            .unwrap_or_default();
        assert_eq!(
            a, "Awesome",
            "pure max-`TreeID`: the higher-peer (device 0x7f) twin wins, even a \
             stale re-ship — stale-guard dropped (got {a:?})"
        );
        assert_eq!(
            b, "B device",
            "B: the higher-peer (device) twin's genuine edit (got {b:?})"
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
        // Applying the same disjoint snapshot twice must not corrupt state; the
        // second apply is a no-op (both blocks stable). FLIPPED by tesela-fte:
        // under pure max-`TreeID` A resolves to the higher-peer device twin's
        // "Awesome" (not the server's "Awesome sweet" — stale-guard dropped).
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
        let a1 = block_text(&server, note, A_BID_BYTES)
            .await
            .unwrap_or_default();
        let b1 = block_text(&server, note, B_BID_BYTES)
            .await
            .unwrap_or_default();
        // Second apply of the identical frame.
        server.import_doc_update(note, &snapshot).await.unwrap();
        let a2 = block_text(&server, note, A_BID_BYTES)
            .await
            .unwrap_or_default();
        let b2 = block_text(&server, note, B_BID_BYTES)
            .await
            .unwrap_or_default();

        assert_eq!(a1, "Awesome", "pure max-`TreeID`: higher-peer device twin wins");
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
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "server edit".into(),
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
                text: "device edit".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let delta = device
            .export_doc_update(note, dev_vv.as_deref())
            .await
            .unwrap();

        // Server applies the device's delta. Loro's LoroText merge picks the
        // SAME converged value on both sides — and re-applying must be stable.
        server.import_doc_update(note, &delta).await.unwrap();
        // Round-trip the server's state back to the device to converge.
        let dev_vv2 = device.doc_version(note).await;
        let srv_delta = server
            .export_doc_update(note, dev_vv2.as_deref())
            .await
            .unwrap();
        device.import_doc_update(note, &srv_delta).await.unwrap();

        let sa = block_text(&server, note, A_BID_BYTES)
            .await
            .unwrap_or_default();
        let da = block_text(&device, note, A_BID_BYTES)
            .await
            .unwrap_or_default();
        assert_eq!(
            sa, da,
            "shared-register concurrent edit converges on both sides"
        );
        // The LoroText merge INTERLEAVES both whole-text edits rather than
        // LWW-picking one: the result is NEITHER whole string (no clobber) and
        // is longer than either input — both sides contributed characters.
        assert_ne!(
            sa, "server edit",
            "not an LWW pick of the server's whole edit"
        );
        assert_ne!(
            sa, "device edit",
            "not an LWW pick of the device's whole edit"
        );
        assert!(
            sa.len() > "server edit".len() && sa.contains("device"),
            "both concurrent edits' contributions survive the merge: {sa:?}"
        );

        // Stable: re-applying the same delta does not flip the value.
        server.import_doc_update(note, &delta).await.unwrap();
        let sa2 = block_text(&server, note, A_BID_BYTES)
            .await
            .unwrap_or_default();
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
    async fn splice_shared_base(note: [u8; 16], text: &str) -> (LoroEngine, LoroEngine) {
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

    // ── Convergent whole-text writes (the same-bid lineage-union fix) ──────
    //
    // `write_block_text` is the WHOLE-TEXT authoring path (BlockUpsert / the
    // disjoint-twin heal / reconcile). Two replicas on a SHARED text_seq
    // lineage that each REWRITE the block to a DIFFERENT whole string must
    // converge to ONE coherent value — the shared base preserved ONCE, only
    // the divergent tails char-merging — NOT the two full strings
    // concatenated (the "Bothnice onenice one" signature). And re-applying
    // the SAME whole text (the heal's idempotent re-issue) must never grow or
    // duplicate runs.

    #[tokio::test]
    async fn write_block_text_concurrent_rewrites_one_coherent_value() {
        // Shared base "hello"; A rewrites whole-text → "hello world", B
        // rewrites whole-text → "hello there", concurrently. After merge both
        // replicas converge AND the shared base "hello" survives exactly ONCE
        // (a minimal-diff char-merge), not "hello worldhello there" (the
        // whole-replace union that duplicates the base).
        let note = blake3_note_id("write-converge");
        let (a, b) = splice_shared_base(note, "hello").await;

        let a_vv = a.doc_version(note).await;
        let b_vv = b.doc_version(note).await;

        // Concurrent WHOLE-TEXT rewrites (the BlockUpsert authoring path).
        upsert_block(&a, note, A_BID_BYTES, "hello world", None).await;
        upsert_block(&b, note, A_BID_BYTES, "hello there", None).await;

        let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_delta).await.unwrap();
        a.import_doc_update(note, &b_delta).await.unwrap();

        let ta = block_text(&a, note, A_BID_BYTES).await.unwrap_or_default();
        let tb = block_text(&b, note, A_BID_BYTES).await.unwrap_or_default();

        assert_eq!(ta, tb, "replicas converge to the same merged text: {ta:?}");
        // The shared base appears exactly once — the divergent tails merge,
        // the common prefix is NOT re-inserted by both writers.
        assert_eq!(
            ta.matches("hello").count(),
            1,
            "shared base 'hello' preserved once, not concatenated: {ta:?}"
        );
        // Both divergent edits survive the char-merge.
        assert!(ta.contains("world"), "A's edit survives: {ta:?}");
        assert!(ta.contains("there"), "B's edit survives: {ta:?}");
    }

    #[tokio::test]
    async fn write_block_text_empty_base_concurrent_char_merges() {
        // Shared EMPTY placeholder (the daily empty block); two devices type
        // DIFFERENT whole text into it concurrently, then merge. With no
        // common ancestor content to anchor a diff, this is the irreducible
        // char-merge case (same semantics as the splice interleave): both
        // replicas converge to ONE shared value and BOTH fragments survive
        // (neither replica clobbers the other). The fix's job is convergence +
        // no compounding, NOT to magically pick a single fork's text here.
        let note = blake3_note_id("write-empty-merge");
        let (a, b) = splice_shared_base(note, "").await;
        let a_vv = a.doc_version(note).await;
        let b_vv = b.doc_version(note).await;
        upsert_block(&a, note, A_BID_BYTES, "Both", None).await;
        upsert_block(&b, note, A_BID_BYTES, "nice one", None).await;
        let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_delta).await.unwrap();
        a.import_doc_update(note, &b_delta).await.unwrap();
        let ta = block_text(&a, note, A_BID_BYTES).await.unwrap_or_default();
        let tb = block_text(&b, note, A_BID_BYTES).await.unwrap_or_default();
        assert_eq!(ta, tb, "replicas converge: A={ta:?} B={tb:?}");
        assert!(ta.contains("Both"), "A's authoring survives: {ta:?}");
        assert!(ta.contains("nice one"), "B's authoring survives: {ta:?}");
        // No compounding: each fragment appears exactly once (no third run).
        assert_eq!(ta.matches("Both").count(), 1, "no duplicate run: {ta:?}");
        assert_eq!(ta.matches("nice one").count(), 1, "no duplicate run: {ta:?}");
    }

    #[tokio::test]
    async fn write_block_text_reapply_is_idempotent() {
        // Re-authoring a block with the SAME whole text (the disjoint-twin
        // heal re-issues record_local(BlockUpsert{text}) on every import) must
        // be a true no-op on the text_seq — never appending a second run that
        // grows/duplicates the value, and never growing the doc's op history
        // (the lever that compounds under multi-round relay re-broadcast).
        let note = blake3_note_id("write-idempotent");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        upsert_block(&engine, note, A_BID_BYTES, "nice one", None).await;
        let v0 = engine.doc_version(note).await;
        // Re-apply the identical whole text several times.
        for _ in 0..3 {
            upsert_block(&engine, note, A_BID_BYTES, "nice one", None).await;
        }
        let v1 = engine.doc_version(note).await;
        assert_eq!(
            block_text(&engine, note, A_BID_BYTES).await.as_deref(),
            Some("nice one"),
            "re-applying identical whole text never grows/duplicates the value"
        );
        assert_eq!(
            v0, v1,
            "re-applying identical whole text never grows the op history"
        );
    }

    #[tokio::test]
    async fn write_block_text_distinct_new_blocks_stay_separate() {
        // Two replicas on a shared base each ADD a NEW block with a DISTINCT
        // bid (the iOS fresh-v4 case). After merge the note holds BOTH new
        // blocks as separate values — distinct bids never share a text_seq, so
        // there is no concatenation.
        let note = blake3_note_id("write-distinct");
        let (a, b) = splice_shared_base(note, "base").await;

        let a_vv = a.doc_version(note).await;
        let b_vv = b.doc_version(note).await;

        // Distinct fresh bids, one per replica.
        let new_a: [u8; 16] = [0xc1; 16];
        let new_b: [u8; 16] = [0xc2; 16];
        upsert_block(&a, note, new_a, "alpha block", None).await;
        upsert_block(&b, note, new_b, "beta block", None).await;

        let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_delta).await.unwrap();
        a.import_doc_update(note, &b_delta).await.unwrap();

        for (label, eng) in [("a", &a), ("b", &b)] {
            assert_eq!(
                block_text(eng, note, new_a).await.as_deref(),
                Some("alpha block"),
                "{label}: A's new block is its own coherent value"
            );
            assert_eq!(
                block_text(eng, note, new_b).await.as_deref(),
                Some("beta block"),
                "{label}: B's new block is its own coherent value"
            );
        }
    }

    // ── BOOTSTRAP-BEFORE-AUTHOR: multi-device daily convergence ──────────
    //
    // The production garble (Taylor's daily, bid c35861c0):
    // `Bothnice onenice one` = SEPARATE intended blocks' text concatenated
    // into ONE block, plus persistent divergence. Root cause: a device
    // authored today's daily on a FRESH DISJOINT LoroDoc; when it later
    // received the relay's authoritative version of the same bid (created on
    // another device + synced), `apply_relay_updates` merged the disjoint
    // lineages → per-block `text_seq` UNION / same-bid twins. The fix is
    // CLIENT-SIDE bootstrap-before-author: import the relay's authoritative
    // doc (shared base) BEFORE the first local edit, so the edit lands on the
    // existing lineage → clean char-merge.

    /// A realistic content bid: a UUID-shaped 16-byte id a client actually
    /// authors + syncs (`parse_body_blocks` / iOS stamp these), NOT the parked
    /// deterministic-seed placeholder. The production garble used such a bid
    /// (c35861c0) created on iOS, synced, then re-edited on the desktop's
    /// disjoint lineage.
    fn content_bid(seed: &str) -> [u8; 16] {
        let h = blake3::hash(seed.as_bytes());
        let mut id = [0u8; 16];
        id.copy_from_slice(&h.as_bytes()[..16]);
        id
    }

    /// Count LIVE tree nodes carrying `block` on a note's doc — disjoint
    /// same-bid twins show as > 1.
    async fn block_twin_count(engine: &LoroEngine, note_id: [u8; 16], block: [u8; 16]) -> usize {
        let docs = engine.inner.docs.read().await;
        let Some(doc) = docs.get(&note_id) else {
            return 0;
        };
        let tree = doc.get_tree("blocks");
        let hex = hex_id(&block);
        let mut n = 0;
        for node in tree.children(TreeParentId::Root).unwrap_or_default() {
            if matches!(tree.is_node_deleted(&node), Ok(true)) {
                continue;
            }
            if read_meta_str(&tree, node, "block_id").as_deref() == Some(hex.as_str()) {
                n += 1;
            }
        }
        n
    }

    // CASE-GARBLE (reproduces the bug). REALISTIC — no hardcoded shared
    // placeholder bid. Engine A creates today's daily + a block with a content
    // bid + text, exports the authoritative snapshot (what the relay holds).
    // Engine B authored its OWN fresh DISJOINT daily doc and edited the SAME
    // bid (the bid reached B via materialized markdown). When B then imports
    // A's authoritative ops through the relay apply path (no shared base), the
    // two disjoint `text_seq` lineages UNION into one garbled block and/or
    // leave disjoint same-bid twins (divergence). This asserts the failure
    // reproduces, so the fix below is proven against a real garble.
    // FORMERLY `daily_disjoint_author_then_relay_import_garbles`, which asserted
    // the BUG reproduced (union / twin divergence / data loss). tesela-y11's
    // deterministic disjoint-twin resolution now converges this case CLEANLY on
    // the relay apply path WITHOUT needing bootstrap-before-author: the twins are
    // deduped to ONE node whose text is the deterministic winner (pure max-`TreeID`,
    // tesela-fte), never a `Both`+`nice one` concatenation and never a
    // persistent twin. (One genuine value wins a same-block conflict — inherent
    // to single-line text; the char-MERGE where both survive is the shared-base
    // path in the next test.)
    #[tokio::test]
    async fn daily_disjoint_author_then_relay_import_converges_no_garble() {
        let note = blake3_note_id("2026-06-29");
        let bid = content_bid("c35861c0-daily-block");

        // Engine A: fresh daily, authors the block, exports the authoritative
        // snapshot (the relay's version of this note).
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        upsert_block(&a, note, bid, "Both", None).await;
        let auth = a.export_doc_update(note, None).await.unwrap();

        // Engine B: its OWN fresh DISJOINT daily, edits the SAME bid (got from
        // materialized markdown) — a disjoint twin of the same block_id.
        let dev_b = DeviceId::from_bytes([0xb1; 16]);
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        upsert_block(&b, note, bid, "nice one", None).await;

        // B imports A's authoritative ops via the relay apply path. No shared
        // base → disjoint-lineage merge, now resolved deterministically.
        let _ = b.apply_relay_updates(&[(note, auth)]).await;

        let tb = block_text(&b, note, bid).await.unwrap_or_default();
        let twins = block_twin_count(&b, note, bid).await;

        assert!(
            !(tb.contains("Both") && tb.contains("nice one")),
            "disjoint merge must NOT union/garble the two runs: {tb:?}"
        );
        assert_eq!(twins, 1, "disjoint merge must dedup to ONE live node: twins={twins}");
        assert!(
            tb == "Both" || tb == "nice one",
            "must keep ONE coherent genuine value (no garble, no empty): {tb:?}"
        );
    }

    // CASE-FIXED. Engine B imports A's authoritative snapshot
    // (`import_authoritative_snapshot`) BEFORE its first local edit of the
    // shared block, so it authors into A's EXISTING lineage. A and B then edit
    // the SAME block concurrently → clean char-merge: ONE coherent block, no
    // concatenation, no twins. ALSO: a local un-broadcast edit on B (its own
    // new block, never synced) must SURVIVE the bootstrap import (the import is
    // a non-destructive merge, never a wholesale replace).
    #[tokio::test]
    async fn daily_bootstrap_before_author_converges_clean() {
        let note = blake3_note_id("2026-06-29");
        let bid = content_bid("c35861c0-daily-block");

        // Engine A: authoritative — fresh daily + block, exported snapshot.
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        upsert_block(&a, note, bid, "Both", None).await;
        let auth = a.export_doc_update(note, None).await.unwrap();

        // Engine B: has a LOCAL UN-BROADCAST edit (its own new block) BEFORE
        // bootstrap — must survive the authoritative import.
        let dev_b = DeviceId::from_bytes([0xb1; 16]);
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        let local_bid = content_bid("b-local-unbroadcast");
        upsert_block(&b, note, local_bid, "local draft", None).await;

        // BOOTSTRAP-BEFORE-AUTHOR: import A's authoritative doc (shared base)
        // BEFORE B's first edit of the shared block.
        b.import_authoritative_snapshot(note, &auth).await.unwrap();

        // Clobber guard: B's local un-broadcast edit survived the bootstrap.
        assert_eq!(
            block_text(&b, note, local_bid).await.as_deref(),
            Some("local draft"),
            "local un-broadcast edit must survive bootstrap import"
        );
        // B now shares A's lineage for `bid` — exactly one node, A's value.
        assert_eq!(
            block_text(&b, note, bid).await.as_deref(),
            Some("Both"),
            "bootstrap establishes A's value as the shared base"
        );
        assert_eq!(
            block_twin_count(&b, note, bid).await,
            1,
            "bootstrap leaves a single shared node, no twin"
        );

        // CONCURRENT edits on the SHARED lineage: A appends "!" (offset 4),
        // B appends " yeah" (offset 4).
        a.splice_block_text(note, bid, 4, 0, "!").await.unwrap();
        b.splice_block_text(note, bid, 4, 0, " yeah").await.unwrap();

        // Converge by cross-importing each replica's FULL doc. (A full export
        // is used rather than a since-vv delta because B's pre-bootstrap local
        // block is an op A has never seen — a since-vv delta of just B's splice
        // would import PENDING against that causal gap. The full snapshot
        // carries the local block too, so it lands and the splices char-merge.)
        let a_full = a.export_doc_update(note, None).await.unwrap();
        let b_full = b.export_doc_update(note, None).await.unwrap();
        b.import_doc_update(note, &a_full).await.unwrap();
        a.import_doc_update(note, &b_full).await.unwrap();

        let ta = block_text(&a, note, bid).await.unwrap_or_default();
        let tb = block_text(&b, note, bid).await.unwrap_or_default();

        // Clean convergence: byte-identical, ONE node, no concatenation.
        assert_eq!(ta, tb, "replicas converge: A={ta:?} B={tb:?}");
        assert_eq!(
            block_twin_count(&a, note, bid).await,
            1,
            "no twins on A after concurrent edit: {ta:?}"
        );
        assert_eq!(
            block_twin_count(&b, note, bid).await,
            1,
            "no twins on B after concurrent edit: {tb:?}"
        );
        // The shared base "Both" survives exactly once (char-merge, not the
        // disjoint union that duplicated runs).
        assert_eq!(
            ta.matches("Both").count(),
            1,
            "shared base preserved once, not concatenated: {ta:?}"
        );
        // Both concurrent contributions survive the merge.
        assert!(ta.contains('!'), "A's concurrent edit survives: {ta:?}");
        assert!(ta.contains("yeah"), "B's concurrent edit survives: {ta:?}");
        // B's local un-broadcast block is still intact and coherent.
        assert_eq!(
            block_text(&b, note, local_bid).await.as_deref(),
            Some("local draft"),
            "local block still coherent after converge"
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
        a.splice_block_text(note, block, 5, 0, " world")
            .await
            .unwrap();
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

    // ---- P1.9 disjoint-twin heal carries props ----

    // ⭐ Two devices each author the SAME block_id INDEPENDENTLY (disjoint Loro
    // lineages via `seed_disjoint`), and each first-sets a DISTINCT scalar
    // property on its own twin. When one device's snapshot is applied via the
    // WS-apply path, `tombstone_duplicate_twins` keeps ONE twin (max-`TreeID`)
    // and tombstones the loser — so the loser-twin's property would VANISH
    // without the heal carrying it forward. The heal must read every twin's
    // props in the fork BEFORE the tombstone, merge per key, and re-assert each
    // onto the survivor → BOTH distinct properties survive.
    #[tokio::test]
    async fn disjoint_twins_each_with_distinct_property_both_survive() {
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let ddev = DeviceId::from_bytes([0x7f; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let note = blake3_note_id("daily");

        // Disjoint lineages: server + device each author blocks A and B
        // independently (distinct TreeIDs for the same block_ids).
        seed_disjoint(&server, &device, note).await;

        // Each device first-sets a DISTINCT scalar property on block A — on its
        // OWN twin (each set mints a `props` container on a rival node).
        server
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();
        device
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "priority".into(),
                value: PropOp::SetScalar(PropScalar::Int(3)),
            })
            .await
            .unwrap();

        // Device exports a FULL SNAPSHOT; server applies it via the WS path.
        let snapshot = device.export_doc_update(note, None).await.unwrap();
        server.import_doc_update(note, &snapshot).await.unwrap();

        // BOTH the server's own property AND the device-twin's property must
        // survive on the surviving node after the tombstone.
        assert_eq!(
            block_prop_scalar(&server, note, A_BID_BYTES, "status").await,
            Some(PropScalar::Text("doing".into())),
            "the server's own property must survive the twin dedup"
        );
        assert_eq!(
            block_prop_scalar(&server, note, A_BID_BYTES, "priority").await,
            Some(PropScalar::Int(3)),
            "the tombstoned twin's property must be carried onto the survivor"
        );
    }

    // ⭐ Two disjoint twins each AddToList a DISTINCT value to the SAME list key.
    // The heal must UNION the loser-twin's missing members onto the survivor's
    // list (via per-key AddToList re-assert), never replace the winner's list
    // wholesale → survivor list = [x, y] deduped.
    #[tokio::test]
    async fn disjoint_twins_each_add_to_same_list_key_union() {
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let ddev = DeviceId::from_bytes([0x7f; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let note = blake3_note_id("daily");

        seed_disjoint(&server, &device, note).await;

        // Each twin adds a DISTINCT value to the SAME list key `tags` on block A
        // — on rival list containers (disjoint lineage, no shared base list).
        server
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "tags".into(),
                value: PropOp::AddToList(PropScalar::Text("x".into())),
            })
            .await
            .unwrap();
        device
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "tags".into(),
                value: PropOp::AddToList(PropScalar::Text("y".into())),
            })
            .await
            .unwrap();

        let snapshot = device.export_doc_update(note, None).await.unwrap();
        server.import_doc_update(note, &snapshot).await.unwrap();

        let mut tags: Vec<String> = block_prop_list(&server, note, A_BID_BYTES, "tags")
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
            vec!["x".to_string(), "y".to_string()],
            "the heal must UNION both twins' list members onto the survivor \
             (deduped), not replace the winner's list wholesale"
        );
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
            (
                "status",
                PropOp::SetScalar(PropScalar::Text("doing".into())),
            ),
            ("priority", PropOp::SetScalar(PropScalar::Int(3))),
            ("tags", PropOp::AddToList(PropScalar::Text("Task".into()))),
            ("tags", PropOp::AddToList(PropScalar::Text("Urgent".into()))),
            ("note", PropOp::SetText("freeform".into())),
        ];
        let ops_order_2 = vec![
            ("tags", PropOp::AddToList(PropScalar::Text("Urgent".into()))),
            ("note", PropOp::SetText("freeform".into())),
            ("priority", PropOp::SetScalar(PropScalar::Int(3))),
            (
                "status",
                PropOp::SetScalar(PropScalar::Text("doing".into())),
            ),
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
            assert!(
                ra.contains(needle),
                "converged render missing {needle}: {ra}"
            );
        }

        // Migrated-vs-unmigrated byte equality (P1.6 determinism gate): a block
        // whose `status` arrives as a TYPED scalar prop op (the unmigrated /
        // already-clean path) and a block whose `status` arrives as an in-text
        // `status:: doing` line lifted by migrate-on-apply must render
        // byte-identical markdown. Same CRDT state → same bytes, no matter how
        // the property got there.
        let note2 = blake3_note_id("mat-migrate-determinism");

        let dev_clean = DeviceId::from_bytes([0xe1; 16]);
        let clean = LoroEngine::new(dev_clean, Arc::new(Hlc::new(dev_clean)));
        upsert_block(&clean, note2, A_BID_BYTES, "buy milk", None).await;
        clean
            .record_local(OpPayload::BlockPropertySet {
                note_id: note2,
                block_id: A_BID_BYTES,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();

        let dev_mig = DeviceId::from_bytes([0xe2; 16]);
        let migrating = LoroEngine::new_migrating(dev_mig, Arc::new(Hlc::new(dev_mig)));
        migrating
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note2,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "buy milk\nstatus:: doing".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        assert_eq!(
            clean.render_note_full(note2).await.unwrap(),
            migrating.render_note_full(note2).await.unwrap(),
            "a typed-prop-op block and a migrate-lifted in-text block render \
             byte-identical markdown"
        );
        // The migrating engine must have ACTUALLY lifted the property into a
        // typed container (prose-only text_seq), not merely left it in-text and
        // coincidentally rendered the same bytes.
        assert_eq!(
            block_text(&migrating, note2, A_BID_BYTES).await.as_deref(),
            Some("buy milk"),
            "migrate lifted the property — block text is prose-only"
        );
        assert_eq!(
            block_prop_scalar(&migrating, note2, A_BID_BYTES, "status").await,
            Some(PropScalar::Text("doing".into())),
            "migrate produced a typed container value"
        );
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

    // A4 — render-time dedup: when a block carries BOTH a legacy in-text
    // `status:: a` line (flag OFF, never lifted) AND a container `status`
    // property, the materializer must emit the property ONCE, with the
    // CONTAINER value winning. Guards the un-migrated legacy/dual-write dup.
    #[tokio::test]
    async fn render_dedups_intext_property_when_container_prop_exists() {
        let note = blake3_note_id("mat-dedup");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        let bid = uuid::Uuid::from_bytes(A_BID_BYTES);
        // Legacy in-text `status:: a` lands in text_seq and is NOT lifted
        // (non-migrating engine).
        let content = format!("- Task <!-- bid:{} -->\n  status:: a\n", bid);
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("dedup".into()),
                title: "dedup".into(),
                content,
                created_at_millis: 1,
            })
            .await
            .unwrap();
        // A container `status` property for the SAME key, different value.
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("b".into())),
            })
            .await
            .unwrap();

        let full = engine.render_note_full(note).await.unwrap();
        assert_eq!(
            full,
            format!("- Task <!-- bid:{} -->\n  status:: b\n", bid),
            "container prop wins; the duplicate in-text status line is dropped at render"
        );
        assert_eq!(
            full.matches("status::").count(),
            1,
            "exactly one status line"
        );
    }

    // A4 case-fold: the in-text key is compared case-insensitively to the
    // container keys, so an in-text `status:: a` is still deduped when the
    // container key was set with different case (`Status`). Container wins.
    #[tokio::test]
    async fn render_dedups_intext_property_case_insensitively() {
        let note = blake3_note_id("mat-dedup-case");
        let dev = test_device();
        let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
        let bid = uuid::Uuid::from_bytes(A_BID_BYTES);
        let content = format!("- Task <!-- bid:{} -->\n  status:: a\n", bid);
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("dedup-case".into()),
                title: "dedup-case".into(),
                content,
                created_at_millis: 1,
            })
            .await
            .unwrap();
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "Status".into(),
                value: PropOp::SetScalar(PropScalar::Text("b".into())),
            })
            .await
            .unwrap();

        let full = engine.render_note_full(note).await.unwrap();
        assert!(
            !full.contains("status:: a"),
            "the lowercase in-text dup is dropped despite the container key's case: {full:?}"
        );
        assert!(
            full.contains("Status:: b"),
            "the container value (verbatim key) is kept: {full:?}"
        );
    }

    // P1.6 — migrate-on-apply. With the flag ON, a `BlockUpsert` whose incoming
    // text carries a SOLELY `key:: value` continuation line lifts it OUT of the
    // prose into the typed `props`/`prop_keys` container: the block's
    // `text_seq` becomes prose-only and the property reads back as a typed
    // scalar. Re-applying the SAME (already-clean) BlockUpsert is a no-op (the
    // prose is already stripped → nothing to lift → no double-set).
    #[tokio::test]
    async fn migrate_on_apply_lifts_intext_prop_and_is_idempotent() {
        let note = blake3_note_id("migrate-lift");
        let dev = test_device();
        let engine = LoroEngine::new_migrating(dev, Arc::new(Hlc::new(dev)));

        // A BlockUpsert carrying an in-text property (the un-migrated shape a
        // mixed-fleet old peer authors): prose line + a solely-`key:: value`
        // continuation line, joined by '\n' the way `parse_note` folds it.
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "buy milk\nstatus:: doing".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        // The property was LIFTED: prose-only text_seq + a typed container.
        assert_eq!(
            block_text(&engine, note, A_BID_BYTES).await.as_deref(),
            Some("buy milk"),
            "migrate strips the property line from prose"
        );
        assert_eq!(
            block_prop_scalar(&engine, note, A_BID_BYTES, "status").await,
            Some(PropScalar::Text("doing".into())),
            "migrate folds the stripped line into the typed props container"
        );
        // The rendered VIEW still emits the property as a `key:: value` line
        // (dual-read: an old reader still SEES it).
        let rendered = engine.render_note(note).await.unwrap();
        assert!(
            rendered.contains("status:: doing"),
            "rendered view re-emits the lifted property, got: {rendered:?}"
        );

        // Idempotent: re-applying the SAME logical block (now already clean
        // prose, no in-text property) finds nothing to lift and leaves the
        // container untouched (one value, not a re-set duplicate).
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "buy milk".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        assert_eq!(
            block_text(&engine, note, A_BID_BYTES).await.as_deref(),
            Some("buy milk"),
            "re-apply of clean prose leaves text_seq prose-only"
        );
        assert_eq!(
            block_prop_scalar(&engine, note, A_BID_BYTES, "status").await,
            Some(PropScalar::Text("doing".into())),
            "re-apply does not disturb the already-lifted property"
        );
    }

    // P1.6 — `tags::` routes to AddToList (a list container), NOT a scalar, so a
    // migrated tags line union-merges across replicas instead of LWW-clobbering.
    #[tokio::test]
    async fn migrate_on_apply_routes_tags_to_list() {
        let note = blake3_note_id("migrate-tags");
        let dev = test_device();
        let engine = LoroEngine::new_migrating(dev, Arc::new(Hlc::new(dev)));

        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "a task\ntags:: urgent".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        assert_eq!(
            block_text(&engine, note, A_BID_BYTES).await.as_deref(),
            Some("a task"),
            "tags line stripped from prose"
        );
        assert_eq!(
            block_prop_list(&engine, note, A_BID_BYTES, "tags").await,
            vec![PropScalar::Text("urgent".into())],
            "tags:: routes to a list container (AddToList), not a scalar"
        );
        // It is NOT a scalar.
        assert_eq!(
            block_prop_scalar(&engine, note, A_BID_BYTES, "tags").await,
            None,
            "tags must be a list, never a scalar register"
        );
    }

    // P1.6 mixed-fleet — an OLD peer that can't read containers re-injects the
    // property as an in-text `key:: value` line on a NoteUpsert / BlockUpsert.
    // With migrate ON the line is lifted back into the container; the rendered
    // view emits the property exactly ONCE (no double-emit from a container
    // value PLUS a re-injected in-text line). Mirrors
    // `legacy_in_text_property_round_trips_without_double_emit`.
    #[tokio::test]
    async fn mixed_fleet_old_peer_reinjects_no_double_emit() {
        let note = blake3_note_id("migrate-mixed-fleet");
        let dev = test_device();
        let engine = LoroEngine::new_migrating(dev, Arc::new(Hlc::new(dev)));

        // First apply lifts the property into the container.
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "buy milk\nstatus:: doing".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        // An OLD peer re-broadcasts the block with the property STILL in-text
        // (it never learned to read the container). Migrate lifts it again →
        // prose-only + one container value, never two.
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id: note,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "buy milk\nstatus:: doing".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        assert_eq!(
            block_text(&engine, note, A_BID_BYTES).await.as_deref(),
            Some("buy milk"),
            "the re-injected in-text property is lifted again, not re-embedded"
        );
        let rendered = engine.render_note(note).await.unwrap();
        // Exactly one `status:: doing` line — the container value emitted once,
        // NOT a container value plus a lingering in-text line.
        assert_eq!(
            rendered.matches("status:: doing").count(),
            1,
            "no double-emit: property renders exactly once, got: {rendered:?}"
        );
    }

    // P1.6 — two devices on a SHARED lineage each migrate the SAME block's
    // in-text `status::` property concurrently. Because migrate is
    // deterministic-shape (same incoming text + same classification → same
    // prose-strip + same scalar set), both replicas converge to identical props
    // after exchange (same-key scalar collision = LWW, identical winner).
    #[tokio::test]
    async fn concurrent_migrate_same_block_converges() {
        let note = blake3_note_id("migrate-concurrent");

        // Shared base: a block with prose only, on a shared Loro lineage so the
        // `props` map container is in shared history before peers diverge (the
        // eager-seed precondition from P1.9b).
        let dev_seed = DeviceId::from_bytes([0xc0; 16]);
        let seed = LoroEngine::new(dev_seed, Arc::new(Hlc::new(dev_seed)));
        upsert_block(&seed, note, A_BID_BYTES, "buy milk", None).await;
        let base = seed.export_doc_update(note, None).await.unwrap();

        // Two migrating replicas import the shared base.
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let a = LoroEngine::new_migrating(dev_a, Arc::new(Hlc::new(dev_a)));
        a.import_doc_update(note, &base).await.unwrap();
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let b = LoroEngine::new_migrating(dev_b, Arc::new(Hlc::new(dev_b)));
        b.import_doc_update(note, &base).await.unwrap();

        // Each concurrently applies a BlockUpsert that carries the SAME in-text
        // property — both migrate it identically.
        let a_vv = a.doc_version(note).await;
        let b_vv = b.doc_version(note).await;
        for engine in [&a, &b] {
            engine
                .record_local(OpPayload::BlockUpsert {
                    block_id: A_BID_BYTES,
                    note_id: note,
                    parent_block_id: None,
                    order_key: "00000000".into(),
                    indent_level: 0,
                    text: "buy milk\nstatus:: doing".into(),
                    after_block_id: None,
                })
                .await
                .unwrap();
        }

        // Exchange concurrent deltas both ways.
        let a_delta = a.export_doc_update(note, a_vv.as_deref()).await.unwrap();
        let b_delta = b.export_doc_update(note, b_vv.as_deref()).await.unwrap();
        b.import_doc_update(note, &a_delta).await.unwrap();
        a.import_doc_update(note, &b_delta).await.unwrap();

        // Both replicas converge: identical typed props AND identical rendered
        // markdown.
        assert_eq!(
            block_prop_scalar(&a, note, A_BID_BYTES, "status").await,
            block_prop_scalar(&b, note, A_BID_BYTES, "status").await,
            "concurrent migrators converge on the same scalar (LWW winner)"
        );
        assert_eq!(
            block_prop_scalar(&a, note, A_BID_BYTES, "status").await,
            Some(PropScalar::Text("doing".into())),
            "the converged scalar is the migrated value"
        );
        let ra = a.render_note_full(note).await.unwrap();
        let rb = b.render_note_full(note).await.unwrap();
        assert_eq!(
            ra, rb,
            "concurrent migrators render byte-identical markdown"
        );
    }

    // ─── Views registry (saved-views spec, 2026-06-10) ───────────────────

    fn user_view(id: &str, name: &str, dsl: &str, order: i64) -> crate::engine::ViewRecord {
        crate::engine::ViewRecord {
            id: id.to_string(),
            name: name.to_string(),
            dsl: dsl.to_string(),
            order,
            builtin: false,
            display_mode: "list".to_string(),
            display_group_by: None,
            display_show_done: None,
        }
    }

    /// Ship `from`'s produced relay updates to `to` through the real wire
    /// codec, then commit `from`'s broadcast cursor (a confirmed send).
    /// Same shape as `two_authoritative_engines_converge_through_wire_codec`'s
    /// inline helper.
    async fn ship_relay(from: &LoroEngine, to: &LoroEngine) -> usize {
        use crate::wire::{decode_loro_relay_payload, encode_loro_relay_payload, LoroDocUpdate};
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
            updates.into_iter().map(|(d, _b, vv)| (d, vv)).collect();
        let wire = encode_loro_relay_payload(&payload).unwrap();
        let decoded = decode_loro_relay_payload(&wire)
            .unwrap()
            .expect("v2 payload");
        let pairs: Vec<([u8; 16], Vec<u8>)> = decoded
            .into_iter()
            .map(|u| (u.doc, u.update_bytes))
            .collect();
        let n = to.apply_relay_updates(&pairs).await.applied_count();
        from.commit_broadcast_cursors(&committed).await;
        n
    }

    #[tokio::test]
    async fn views_upsert_list_round_trip_sorted_by_order() {
        let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
        let mut kanban = user_view("v-kanban", "Board", "tag:project", 20);
        kanban.display_mode = "kanban".to_string();
        kanban.display_group_by = Some("status".to_string());
        kanban.display_show_done = Some(true);
        e.views_upsert(kanban.clone()).await.unwrap();
        e.views_upsert(user_view("v-week", "This week", "has:scheduled", 10))
            .await
            .unwrap();

        let views = e.views_list().await;
        assert_eq!(views.len(), 2);
        assert_eq!(
            views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            vec!["v-week", "v-kanban"],
            "sorted by (order, id)"
        );
        assert_eq!(views[1], kanban, "all fields round-trip");

        // Update one field; the others persist (field-level write).
        let mut renamed = kanban.clone();
        renamed.name = "Project board".to_string();
        e.views_upsert(renamed.clone()).await.unwrap();
        let views = e.views_list().await;
        assert_eq!(views.len(), 2, "upsert of existing id is an update");
        assert_eq!(views[1], renamed);
    }

    #[tokio::test]
    async fn views_delete_guards_builtin_and_removes_user_view() {
        let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
        e.ensure_builtin_views().await.unwrap();
        e.views_upsert(user_view("v-user", "Mine", "tag:x", 10))
            .await
            .unwrap();

        // Builtin: not deletable — enforced at the API.
        let err = e.views_delete(INBOX_VIEW_ID).await;
        assert!(err.is_err(), "builtin delete must error: {err:?}");
        assert!(
            e.views_list().await.iter().any(|v| v.id == INBOX_VIEW_ID),
            "inbox survives the delete attempt"
        );

        // User view: deletable; second delete reports false.
        assert!(e.views_delete("v-user").await.unwrap());
        assert!(!e.views_delete("v-user").await.unwrap());
        assert!(
            !e.views_list().await.iter().any(|v| v.id == "v-user"),
            "user view removed"
        );

        // Unknown id: Ok(false), no error.
        assert!(!e.views_delete("nope").await.unwrap());
    }

    #[tokio::test]
    async fn views_upsert_cannot_unflag_builtin() {
        // The delete guard would be bypassable by first upserting
        // builtin=false — `builtin` is sticky to close that hole.
        let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
        e.ensure_builtin_views().await.unwrap();
        let mut edited = e.views_list().await[0].clone();
        assert_eq!(edited.id, INBOX_VIEW_ID);
        edited.builtin = false;
        edited.dsl = "status:todo".to_string();
        e.views_upsert(edited).await.unwrap();

        let inbox = e.views_list().await[0].clone();
        assert!(inbox.builtin, "builtin flag is sticky across upserts");
        assert_eq!(inbox.dsl, "status:todo", "the edit itself landed");
        assert!(e.views_delete(INBOX_VIEW_ID).await.is_err());
    }

    #[tokio::test]
    async fn ensure_builtin_views_is_idempotent_and_preserves_user_edits() {
        let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
        e.ensure_builtin_views().await.unwrap();
        e.ensure_builtin_views().await.unwrap();
        let views = e.views_list().await;
        assert_eq!(views.len(), 1, "double seed yields ONE inbox");
        assert_eq!(views[0].id, INBOX_VIEW_ID);
        assert_eq!(views[0].dsl, INBOX_DEFAULT_DSL);
        assert!(views[0].builtin);

        // The builtin is editable; a later reseed must NOT clobber the edit.
        let mut edited = views[0].clone();
        edited.dsl = "status:todo -has:deadline".to_string();
        e.views_upsert(edited.clone()).await.unwrap();
        e.ensure_builtin_views().await.unwrap();
        assert_eq!(
            e.views_list().await[0].dsl,
            edited.dsl,
            "reseed preserves the user's dsl edit"
        );
    }

    #[tokio::test]
    async fn concurrent_seed_converges_to_one_inbox() {
        // Two devices both seed BEFORE ever syncing — the deterministic
        // seed means both author the SAME ops, and the group converges to
        // ONE Inbox with the default fields (no container race at all).
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        a.ensure_builtin_views().await.unwrap();
        b.ensure_builtin_views().await.unwrap();

        ship_relay(&a, &b).await;
        ship_relay(&b, &a).await;
        ship_relay(&a, &b).await;

        let va = a.views_list().await;
        let vb = b.views_list().await;
        assert_eq!(va, vb, "engines converge");
        assert_eq!(va.len(), 1, "exactly ONE inbox group-wide");
        assert_eq!(va[0].id, INBOX_VIEW_ID);
        assert_eq!(va[0].dsl, INBOX_DEFAULT_DSL);
        assert!(va[0].builtin);
    }

    #[tokio::test]
    async fn fresh_device_that_syncs_before_seeding_noops_and_preserves_edit() {
        // The bring-up ordering contract (main.rs / RelayTicker.viewsList):
        // a relay-configured fresh device bootstraps BEFORE seeding, so the
        // seed sees the group's registry — including a user-edited builtin
        // — and no-ops instead of authoring anything.
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let dev_c = DeviceId::from_bytes([0xc3; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        let c = LoroEngine::new(dev_c, Arc::new(Hlc::new(dev_c)));
        a.ensure_builtin_views().await.unwrap();
        let mut edited = a.views_list().await[0].clone();
        edited.dsl = "status:todo -has:deadline".to_string();
        a.views_upsert(edited.clone()).await.unwrap();

        // C receives the group state FIRST (bootstrap-before-seed)…
        ship_relay(&a, &c).await;
        // …so its seed no-ops and A's edit survives on both.
        c.ensure_builtin_views().await.unwrap();
        ship_relay(&c, &a).await;
        let va = a.views_list().await;
        let vc = c.views_list().await;
        assert_eq!(va, vc, "engines converge");
        assert_eq!(va.len(), 1, "exactly ONE inbox");
        assert_eq!(va[0].dsl, edited.dsl, "A's edit survives the join");
    }

    #[tokio::test]
    async fn offline_first_seed_then_sync_preserves_remote_builtin_edit() {
        // The INVERTED order: C seeds while truly offline-never-synced,
        // then joins. Every device authors the seed as the SAME
        // deterministic ops (fixed seed peer, no timestamps), so there is
        // no same-key container race to lose — A's edit must survive for
        // BOTH peer orderings, not just the one where A's container wins
        // the map-key LWW coin flip.
        for (bytes_a, bytes_c) in [([0xa1u8; 16], [0xc3u8; 16]), ([0xc3u8; 16], [0xa1u8; 16])] {
            let dev_a = DeviceId::from_bytes(bytes_a);
            let dev_c = DeviceId::from_bytes(bytes_c);
            let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
            let c = LoroEngine::new(dev_c, Arc::new(Hlc::new(dev_c)));
            a.ensure_builtin_views().await.unwrap();
            let mut edited = a.views_list().await[0].clone();
            edited.dsl = "status:todo -has:deadline".to_string();
            a.views_upsert(edited.clone()).await.unwrap();

            // C seeds with no shared history at all, then syncs.
            c.ensure_builtin_views().await.unwrap();
            ship_relay(&a, &c).await;
            ship_relay(&c, &a).await;
            ship_relay(&a, &c).await;

            let va = a.views_list().await;
            let vc = c.views_list().await;
            assert_eq!(va, vc, "engines converge (A={bytes_a:02x?})");
            assert_eq!(va.len(), 1, "exactly ONE inbox (A={bytes_a:02x?})");
            assert_eq!(
                va[0].dsl, edited.dsl,
                "A's edit survives an offline-first seed (A={bytes_a:02x?})"
            );
            assert!(va[0].builtin);
        }
    }

    #[tokio::test]
    async fn builtin_seed_ops_are_identical_across_devices() {
        // Determinism pin: two devices seeding independently author
        // byte-identical seed updates (reserved peer, no timestamps), so a
        // one-way ship leaves the receiver unchanged — its version vector
        // already covers the seed ops.
        assert_eq!(
            builtin_views_seed_update().unwrap(),
            builtin_views_seed_update().unwrap(),
            "seed update bytes are deterministic"
        );
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        a.ensure_builtin_views().await.unwrap();
        b.ensure_builtin_views().await.unwrap();
        let before = b.views_list().await;
        ship_relay(&a, &b).await;
        assert_eq!(b.views_list().await, before, "A's seed is already known");
        assert_eq!(before.len(), 1);
    }

    #[tokio::test]
    async fn builtin_upsert_on_unseeded_device_routes_through_seed_container() {
        // iOS hub-mode shape: a never-synced device EDITS the builtin
        // directly (views_upsert, no prior seed — the UI edits the
        // fallback Inbox). The upsert must land its fields in THE
        // deterministic seed container so a later join field-merges with
        // the group instead of racing whole containers — for BOTH peer
        // orderings, not just the one where C's container would win.
        for (bytes_a, bytes_c) in [([0xa1u8; 16], [0xc3u8; 16]), ([0xc3u8; 16], [0xa1u8; 16])] {
            let dev_a = DeviceId::from_bytes(bytes_a);
            let dev_c = DeviceId::from_bytes(bytes_c);
            let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
            let c = LoroEngine::new(dev_c, Arc::new(Hlc::new(dev_c)));
            a.ensure_builtin_views().await.unwrap();
            let mut a_edit = a.views_list().await[0].clone();
            a_edit.dsl = "status:todo".to_string();
            a.views_upsert(a_edit).await.unwrap();

            // C renames the builtin with no seed and no shared history.
            let mut c_record = user_view(INBOX_VIEW_ID, "Triage", INBOX_DEFAULT_DSL, 0);
            c_record.builtin = true;
            c.views_upsert(c_record).await.unwrap();

            ship_relay(&a, &c).await;
            ship_relay(&c, &a).await;
            ship_relay(&a, &c).await;

            let va = a.views_list().await;
            assert_eq!(va, c.views_list().await, "engines converge");
            assert_eq!(va.len(), 1, "exactly ONE inbox");
            // Field-level merge, not wholesale container loss: C's rename
            // survives; dsl (written concurrently by BOTH upserts) resolves
            // to one deterministic LWW winner — never a third value.
            assert_eq!(
                va[0].name, "Triage",
                "C's rename survives (A={bytes_a:02x?})"
            );
            assert!(
                va[0].dsl == "status:todo" || va[0].dsl == INBOX_DEFAULT_DSL,
                "dsl is one LWW winner: {}",
                va[0].dsl
            );
            assert!(va[0].builtin);
        }
    }

    #[tokio::test]
    async fn concurrent_upsert_of_different_views_both_survive() {
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        // Shared base: A seeds, B receives it.
        a.ensure_builtin_views().await.unwrap();
        ship_relay(&a, &b).await;

        // Concurrent: each device creates a DIFFERENT view.
        a.views_upsert(user_view("v-from-a", "A's", "tag:a", 10))
            .await
            .unwrap();
        b.views_upsert(user_view("v-from-b", "B's", "tag:b", 20))
            .await
            .unwrap();
        ship_relay(&a, &b).await;
        ship_relay(&b, &a).await;
        ship_relay(&a, &b).await;

        let va = a.views_list().await;
        assert_eq!(va, b.views_list().await, "engines converge");
        assert_eq!(
            va.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
            vec![INBOX_VIEW_ID, "v-from-a", "v-from-b"],
            "both concurrent creations survive (+ the seeded inbox)"
        );
    }

    #[tokio::test]
    async fn concurrent_edit_of_same_view_dsl_is_lww_and_other_fields_survive() {
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        a.views_upsert(user_view("v-shared", "Shared", "tag:base", 10))
            .await
            .unwrap();
        ship_relay(&a, &b).await;

        // Concurrent: A edits the dsl, B edits the name (different fields
        // of the SAME view — field-level LWW keeps both).
        let mut on_a = a.views_list().await[0].clone();
        on_a.dsl = "tag:edited-by-a".to_string();
        a.views_upsert(on_a).await.unwrap();
        let mut on_b = b.views_list().await[0].clone();
        on_b.name = "Renamed by B".to_string();
        b.views_upsert(on_b).await.unwrap();
        ship_relay(&a, &b).await;
        ship_relay(&b, &a).await;
        ship_relay(&a, &b).await;

        let va = a.views_list().await;
        assert_eq!(va, b.views_list().await, "engines converge");
        // B's upsert re-wrote dsl with its stale base value — same-field
        // LWW resolves deterministically to ONE of the two; the rename
        // (the field only B touched with a NEW value) must survive.
        assert_eq!(va[0].name, "Renamed by B");
        assert!(
            va[0].dsl == "tag:edited-by-a" || va[0].dsl == "tag:base",
            "dsl is one LWW winner, not a mash: {}",
            va[0].dsl
        );
    }

    #[tokio::test]
    async fn views_delete_vs_concurrent_edit_converges_deterministically() {
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        a.views_upsert(user_view("v-doomed", "Doomed", "tag:x", 10))
            .await
            .unwrap();
        ship_relay(&a, &b).await;

        // Concurrent: A deletes the view, B edits its dsl.
        assert!(a.views_delete("v-doomed").await.unwrap());
        let mut on_b = b.views_list().await[0].clone();
        on_b.dsl = "tag:edited".to_string();
        b.views_upsert(on_b).await.unwrap();
        ship_relay(&a, &b).await;
        ship_relay(&b, &a).await;
        ship_relay(&a, &b).await;

        let va = a.views_list().await;
        let vb = b.views_list().await;
        assert_eq!(va, vb, "delete vs edit converges to the same state");
        // The map-key delete outranks edits INSIDE the (removed)
        // container: deterministic delete-wins on both replicas.
        assert!(va.is_empty(), "deleted view stays deleted: {va:?}");
    }

    #[tokio::test]
    async fn views_doc_rides_relay_update_path_and_deposit_streams() {
        // Spec item 5: A creates a view → B receives it via the relay
        // update path → B edits the dsl → A converges. Plus: the views doc
        // id is in `tracked_note_ids` (what `deposit_snapshots` iterates).
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

        a.ensure_builtin_views().await.unwrap();
        a.views_upsert(user_view("v-travel", "Travel", "tag:trip", 10))
            .await
            .unwrap();
        assert!(
            SyncEngine::tracked_note_ids(&a)
                .await
                .contains(&VIEWS_DOC_ID),
            "views doc is in the deposit walk (tracked_note_ids)"
        );

        assert!(ship_relay(&a, &b).await >= 1, "B received the views doc");
        assert_eq!(a.views_list().await, b.views_list().await, "bootstrapped");

        // B edits the dsl; A converges through the same path.
        let mut travel = b
            .views_list()
            .await
            .into_iter()
            .find(|v| v.id == "v-travel")
            .unwrap();
        travel.dsl = "tag:trip status:todo".to_string();
        b.views_upsert(travel.clone()).await.unwrap();
        ship_relay(&b, &a).await;
        let on_a = a
            .views_list()
            .await
            .into_iter()
            .find(|v| v.id == "v-travel")
            .unwrap();
        assert_eq!(on_a, travel, "A converged on B's edit");

        // One bounded transitive re-broadcast (A re-emits the delta it just
        // imported — idempotent on B), then steady state: nothing to send.
        ship_relay(&a, &b).await;
        assert_eq!(ship_relay(&b, &a).await, 0);
        assert_eq!(ship_relay(&a, &b).await, 0);
    }

    #[tokio::test]
    async fn views_doc_survives_snapshot_deposit_bootstrap_round() {
        // The relay compaction path: `deposit_snapshots` exports
        // `export_doc_update(id, None)` per tracked doc; a fresh device's
        // `bootstrap_from_snapshots` imports each via `import_doc_update`.
        // Mirror that engine-level seam for the views doc.
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        a.ensure_builtin_views().await.unwrap();
        a.views_upsert(user_view("v-x", "X", "tag:x", 10))
            .await
            .unwrap();

        let snapshot = a
            .export_doc_update(VIEWS_DOC_ID, None)
            .await
            .expect("views doc exports a full snapshot for deposit");

        // Fresh device bootstraps from the deposited snapshot.
        let dev_c = DeviceId::from_bytes([0xc3; 16]);
        let c = LoroEngine::new(dev_c, Arc::new(Hlc::new(dev_c)));
        c.import_doc_update(VIEWS_DOC_ID, &snapshot).await.unwrap();
        assert_eq!(c.views_list().await, a.views_list().await, "bootstrap");

        // The targeted catch-up path (`import_authoritative_snapshot`) is
        // idempotent on the same bytes.
        c.import_authoritative_snapshot(VIEWS_DOC_ID, &snapshot)
            .await
            .unwrap();
        assert_eq!(c.views_list().await, a.views_list().await, "idempotent");
    }

    #[tokio::test]
    async fn views_doc_is_excluded_from_note_machinery() {
        let tmp = tempfile::tempdir().unwrap();
        let dev = test_device();
        let e = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            tmp.path().join("loro"),
            Some(tmp.path().join("notes")),
        )
        .await
        .unwrap();
        e.ensure_builtin_views().await.unwrap();

        // A real note for contrast.
        let note = blake3_note_id("real-note");
        e.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("real-note".into()),
            title: "Real".into(),
            content: "- hi <!-- bid:70707070-7070-7070-7070-707070707070 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

        // Not indexed.
        let views_hex = hex_id(&VIEWS_DOC_ID);
        assert!(
            !e.index_entries()
                .await
                .iter()
                .any(|x| x.note_id == views_hex),
            "no phantom index entry for the views doc"
        );
        // Not renderable / not a note for walkers.
        assert!(LoroEngine::render_note(&e, VIEWS_DOC_ID).await.is_none());
        assert!(LoroEngine::render_note_full(&e, VIEWS_DOC_ID)
            .await
            .is_none());
        // Not materialized: notes/ holds exactly the real note.
        let mut files = Vec::new();
        let mut rd = tokio::fs::read_dir(tmp.path().join("notes")).await.unwrap();
        while let Some(entry) = rd.next_entry().await.unwrap() {
            files.push(entry.file_name().to_string_lossy().to_string());
        }
        assert_eq!(files, vec!["real-note.md"], "views doc never hits notes/");
        // But its snapshot IS persisted like any doc's.
        assert!(
            tmp.path()
                .join("loro")
                .join(format!("{views_hex}.bin"))
                .exists(),
            "views doc snapshot persisted"
        );

        // Note-shaped ops addressed at the views doc are refused no-ops.
        let before = e.views_list().await;
        e.apply_payload(&OpPayload::NoteUpsert {
            note_id: VIEWS_DOC_ID,
            display_alias: Some("evil".into()),
            title: "Evil".into(),
            content: "- nope\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
        e.apply_payload(&OpPayload::NoteDelete {
            note_id: VIEWS_DOC_ID,
            display_alias: None,
        })
        .await
        .unwrap();
        assert_eq!(
            e.views_list().await,
            before,
            "NoteUpsert/NoteDelete at the views doc are no-ops"
        );
        assert!(
            !e.index_entries()
                .await
                .iter()
                .any(|x| x.note_id == views_hex),
            "still not indexed after the refused ops"
        );
    }

    #[tokio::test]
    async fn views_doc_survives_reseed_from_disk() {
        // `reseed_from_disk` replays NoteUpserts from `.md` files — it must
        // leave the views registry untouched (it only ever UPSERTS notes).
        let tmp = tempfile::tempdir().unwrap();
        let dev = test_device();
        let e = LoroEngine::with_dirs(
            dev,
            Arc::new(Hlc::new(dev)),
            tmp.path().join("loro"),
            Some(tmp.path().join("notes")),
        )
        .await
        .unwrap();
        e.ensure_builtin_views().await.unwrap();
        e.views_upsert(user_view("v-keep", "Keep", "tag:keep", 10))
            .await
            .unwrap();
        let before = e.views_list().await;

        tokio::fs::write(tmp.path().join("notes").join("seeded.md"), "- from disk\n")
            .await
            .unwrap();
        let count = e.reseed_from_disk(&tmp.path().join("notes")).await.unwrap();
        assert_eq!(count, 1, "reseed processed the md file");
        assert_eq!(e.views_list().await, before, "views registry untouched");
    }

    #[tokio::test]
    async fn views_persist_across_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let snap = tmp.path().join("loro");
        let dev = test_device();
        let expected;
        {
            let e = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap.clone(), None)
                .await
                .unwrap();
            e.ensure_builtin_views().await.unwrap();
            e.views_upsert(user_view("v-persist", "P", "tag:p", 10))
                .await
                .unwrap();
            expected = e.views_list().await;
            assert_eq!(expected.len(), 2);
        }
        // Reopen from the same snapshot dir: the views doc loads like any
        // per-doc snapshot, and the seed stays a no-op.
        let e = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, None)
            .await
            .unwrap();
        e.ensure_builtin_views().await.unwrap();
        assert_eq!(e.views_list().await, expected, "registry survives restart");
    }

    // -----------------------------------------------------------------
    // Residency audit (tesela-engc.5): lazy-load regression tests
    // (tesela-qql). The full classification table of every walk over
    // `self.inner.docs` lives in the bead's close note; these three
    // encode the highest-severity assumptions a future evict() must not
    // violate — that a note's `LoroDoc` can be dropped from
    // `self.inner.docs` while its `.bin` snapshot survives on disk, and
    // every one of these three call sites must keep working transparently.
    // Un-ignored now that `doc_for_note_mut` / the apply_import heal gate /
    // `produce_relay_updates` all lazy-load or consult a
    // residency-independent signal.
    // -----------------------------------------------------------------

    #[tokio::test]
    async fn doc_for_note_mut_must_not_recreate_evicted_note() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir)
            .await
            .unwrap();
        let note_id = [0x33; 16];
        let existing_block = [0x44; 16];

        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: existing_block,
                note_id,
                parent_block_id: None,
                order_key: "a0".into(),
                indent_level: 0,
                text: "pre-eviction content".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let before = engine.render_note(note_id).await.unwrap();
        assert!(before.contains("pre-eviction content"));

        // Simulate eviction: the note's snapshot is safely on disk (the
        // BlockUpsert above just wrote it via `save_snapshot`), but the
        // in-memory doc is dropped — exactly what a future evict() would
        // leave behind. `doc_for_note_mut` (loro_engine.rs:1587) is the
        // ONLY entry point that resolves a note's doc for a local edit.
        engine.inner.docs.write().await.remove(&note_id);

        let new_block = [0x55; 16];
        engine
            .record_local(OpPayload::BlockUpsert {
                block_id: new_block,
                note_id,
                parent_block_id: None,
                order_key: "a1".into(),
                indent_level: 0,
                text: "post-eviction content".into(),
                after_block_id: Some(existing_block),
            })
            .await
            .unwrap();

        let after = engine.render_note(note_id).await.unwrap();
        assert!(
            after.contains("pre-eviction content"),
            "doc_for_note_mut unconditionally `or_insert_with`s a FRESH empty \
             LoroDoc on a docs-map miss — an evicted note's entire prior \
             history is silently discarded on the next local edit (got {after:?})"
        );
        assert!(after.contains("post-eviction content"));
    }

    // NOTE: this test's ORIGINAL (tesela-engc.5) assertion expected the
    // server's evicted-then-reimported block A text to survive collapse
    // ("Awesome sweet") over the device's disjoint twin. That predates
    // tesela-fte (`e4a61454`, landed AFTER the residency audit), which
    // deleted the genuine-edit/stale-guard discriminator and made the twin
    // TEXT survivor a PURE function of max-`TreeID` (peer, then counter) —
    // see `ws_apply_disjoint_conflict_resolves_to_max_treeid_twin`. Since
    // each engine's peer id is constant across all its own history, the
    // higher-peer engine (device, 0x7f) wins EVERY disjoint-twin block's
    // TEXT uniformly, so no two-engine scenario can make "server keeps A,
    // device keeps B" true anymore — that combination is no longer
    // reachable regardless of residency/eviction.
    //
    // What the heal GATE (`has_local_state`/`plan_gate`) actually protects
    // is orthogonal to the text-survivor rule: it's whether the tombstoned
    // LOSER's `props` are unioned onto the survivor (`reassert_prop_heals`).
    // That's the meaningful, still-discriminating regression surface for
    // the tesela-qql landmine: the server's own PROPERTY on its (about to
    // lose) A-twin must not be silently dropped just because the note
    // wasn't memory-resident when the inbound frame arrived — mirrors
    // `disjoint_twins_each_with_distinct_property_both_survive`, plus the
    // evict-between-edit-and-import step.
    #[tokio::test]
    async fn apply_import_heal_gate_must_protect_evicted_note_local_edits() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::with_snapshot_dir(sdev, Arc::new(Hlc::new(sdev)), dir)
            .await
            .unwrap();
        let ddev = DeviceId::from_bytes([0x7f; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let note = blake3_note_id("daily-evicted");

        seed_disjoint(&server, &device, note).await;

        // Server's own genuine property on its (disjoint) twin of A — the
        // value that must survive the twin-heal's props-union reassert even
        // though pure max-`TreeID` always keeps device's TEXT as the
        // surviving node for every block in this note.
        server
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();

        // Device genuinely edits B, then exports a full snapshot (the
        // cold-launch first-push frame that triggered the incident).
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

        // Evict the SERVER's note between its own edit and the inbound
        // frame — exactly the window the heal gate samples. The note's
        // snapshot is safely on disk; only the in-memory entry is gone.
        server.inner.docs.write().await.remove(&note);

        server.import_doc_update(note, &snapshot).await.unwrap();

        assert_eq!(
            block_prop_scalar(&server, note, A_BID_BYTES, "status").await,
            Some(PropScalar::Text("doing".into())),
            "an evicted-but-locally-edited note must still get twin-heal \
             props protection on the next Delta import — the server's own \
             property must NOT be silently dropped just because the note \
             wasn't memory-resident when the frame arrived"
        );
        let b = block_text(&server, note, B_BID_BYTES)
            .await
            .unwrap_or_default();
        assert_eq!(
            b, "B device",
            "the device's genuine edit must still apply (got {b:?})"
        );
    }

    #[tokio::test]
    async fn produce_relay_updates_must_include_evicted_dirty_note() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir)
            .await
            .unwrap();
        let note_id = [0x77; 16];
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("evicted".into()),
                title: "Evicted".into(),
                content: "---\ntitle: Evicted\n---\n- hello\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // No broadcast cursor has been committed yet, so this note has
        // ops pending relay. Simulate eviction (the resident doc is
        // dropped; its snapshot is on disk).
        engine.inner.docs.write().await.remove(&note_id);

        let updates = engine.produce_relay_updates().await;
        assert!(
            updates.iter().any(|(id, _, _)| *id == note_id),
            "produce_relay_updates (loro_engine.rs:1197) walks \
             self.inner.docs.keys() directly — an evicted note's \
             un-broadcast local edits silently never reach the relay"
        );
    }

    /// tesela-engc.5 audit, highest-severity UNSTUBBED item:
    /// `rebuild_index_from_docs` used to prune any index entry whose note
    /// wasn't in `self.inner.docs` — safe only because it's called
    /// exclusively at boot, right after eager `load_snapshots_from_dir`,
    /// where the two sets are identical. Simulate the residency gap a
    /// future evict() (or a partial/lazy boot) would leave: the note's
    /// snapshot is safely on disk, but the in-memory doc is gone.
    #[tokio::test]
    async fn rebuild_index_from_docs_must_not_prune_evicted_note_with_disk_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("loro");
        let hlc = Arc::new(Hlc::new(test_device()));
        let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir)
            .await
            .unwrap();
        let note_id = [0x66; 16];
        engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("evicted".into()),
                title: "Evicted".into(),
                content: "- hello\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        assert_eq!(
            engine.index_entries().await.len(),
            1,
            "indexed pre-eviction"
        );

        // Simulate eviction: the snapshot is safely on disk (the NoteUpsert
        // above wrote it via `save_snapshot`), but the in-memory doc is
        // dropped — exactly what a future evict() would leave behind.
        engine.inner.docs.write().await.remove(&note_id);

        engine.rebuild_index_from_docs().await;

        let entries = engine.index_entries().await;
        assert_eq!(
            entries.len(),
            1,
            "rebuild_index_from_docs must not prune a note's index entry \
             just because it isn't memory-resident — only a note with no \
             on-disk snapshot at all is a genuine ghost (got {entries:?})"
        );
        assert_eq!(entries[0].title, "Evicted");
    }

    /// STEP 1 of tesela-engc.4: measure `probe_import_poison`'s real cost
    /// shape on mosaic-realistic docs before deciding whether a skip is
    /// worth adding. Doc sizes are derived from the live
    /// `~/Library/Application Support/tesela/logseq/.tesela/loro` mosaic
    /// (305 note snapshots: mean 7.1KB, median 3KB, p90 16.8KB, max 83KB).
    /// Simulates a genuine two-device inbound DELTA (device 2 imports
    /// device 1's snapshot, adds one block, exports `updates(&vv1)` — the
    /// same shape a relay tick actually ships) alongside a full-snapshot
    /// catch-up frame, and times the probe's three sub-steps against a
    /// plain `doc.import` of the same bytes.
    ///
    /// `#[ignore]`d — a manual perf probe (numbers land in the bead close
    /// note / decisions.md), not a CI-gated timing assertion. Run with:
    /// `cargo test -p tesela-sync --lib poison_probe_cost_measurement -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "manual perf measurement (tesela-engc.4), not a CI timing gate"]
    async fn poison_probe_cost_measurement() {
        use std::time::{Duration, Instant};

        async fn build_note(
            device: [u8; 16],
            note_id: [u8; 16],
            block_count: usize,
            text_len: usize,
        ) -> LoroEngine {
            let hlc = Arc::new(Hlc::new(DeviceId::from_bytes(device)));
            let engine = LoroEngine::new(DeviceId::from_bytes(device), hlc);
            let filler: String = "lorem ipsum dolor sit amet consectetur "
                .repeat(text_len / 40 + 1)
                .chars()
                .take(text_len)
                .collect();
            for i in 0..block_count {
                let mut bid = [0u8; 16];
                bid[..8].copy_from_slice(&(i as u64).to_be_bytes());
                bid[15] = 1;
                upsert_block(&engine, note_id, bid, &filler, None).await;
            }
            engine
        }

        fn time_it<T>(f: impl FnOnce() -> T) -> (T, Duration) {
            let start = Instant::now();
            let out = f();
            (out, start.elapsed())
        }

        // (label, block_count, text_len) shaped to hit the mosaic's mean /
        // median / p90 / max snapshot sizes.
        let shapes: [(&str, usize, usize); 4] = [
            ("median (~3KB)", 8, 250),
            ("mean (~7KB)", 20, 250),
            ("p90 (~17KB)", 45, 250),
            ("max (~83KB)", 220, 250),
        ];

        eprintln!(
            "\nlabel            snapshot_B  delta_B   probe(delta)_us  raw_import(delta)_us  probe(snapshot)_us  raw_import(snapshot)_us"
        );
        for (label, block_count, text_len) in shapes {
            let note_id = [5u8; 16];
            let engine1 = build_note([1u8; 16], note_id, block_count, text_len).await;
            let doc1 = engine1.doc_for_note_mut(note_id).await;
            let vv1 = doc1.oplog_vv();
            let snapshot_bytes = doc1.export(ExportMode::Snapshot).unwrap();

            // A genuine inbound DELTA: device 2 imports device 1's snapshot,
            // adds ONE block (a peer edit), exports only what device 1 lacks.
            let hlc2 = Arc::new(Hlc::new(DeviceId::from_bytes([2u8; 16])));
            let engine2 = LoroEngine::new(DeviceId::from_bytes([2u8; 16]), hlc2);
            engine2
                .import_authoritative_snapshot(note_id, &snapshot_bytes)
                .await
                .unwrap();
            let mut peer_bid = [0u8; 16];
            peer_bid[15] = 2;
            upsert_block(&engine2, note_id, peer_bid, "peer edit block text", None).await;
            let doc2 = engine2.doc_for_note_mut(note_id).await;
            let delta_bytes = doc2.export(ExportMode::updates(&vv1)).unwrap();

            const N: u32 = 50;
            let mut probe_delta_total = Duration::ZERO;
            let mut raw_delta_total = Duration::ZERO;
            let mut probe_snap_total = Duration::ZERO;
            let mut raw_snap_total = Duration::ZERO;
            for _ in 0..N {
                let (_, d) = time_it(|| probe_import_poison(&doc1, &delta_bytes));
                probe_delta_total += d;
                let fork = LoroDoc::new();
                fork.import(&snapshot_bytes).unwrap();
                let (_, d) = time_it(|| fork.import(&delta_bytes));
                raw_delta_total += d;

                let (_, d) = time_it(|| probe_import_poison(&doc1, &snapshot_bytes));
                probe_snap_total += d;
                let fork2 = LoroDoc::new();
                let (_, d) = time_it(|| fork2.import(&snapshot_bytes));
                raw_snap_total += d;
            }
            eprintln!(
                "{label:<16} {:>9}B {:>7}B {:>15}us {:>19}us {:>17}us {:>21}us",
                snapshot_bytes.len(),
                delta_bytes.len(),
                (probe_delta_total / N).as_micros(),
                (raw_delta_total / N).as_micros(),
                (probe_snap_total / N).as_micros(),
                (raw_snap_total / N).as_micros(),
            );
        }
    }

    // Verification gap closed (audit L6, tesela-9t0): deleted-wins
    // (`reconcile_tree_to_blocks`'s tombstoned-skip, ~line 3272) depends on
    // tombstones surviving a GC-compacted `ExportMode::Snapshot` round-trip
    // through a FRESH engine that never saw the delete op directly — only
    // via the snapshot's current-state bytes. Prove it: delete a block,
    // export a snapshot, import fresh on a 2nd engine, then apply a stale
    // NoteUpsert on that 2nd engine whose body still carries the deleted
    // bid — it must stay deleted, not resurrect.
    #[tokio::test]
    async fn deleted_wins_survives_snapshot_gc_round_trip() {
        let note = blake3_note_id("gc-tombstone");
        let bid = content_bid("gc-tombstone-block");

        // Engine A: create + delete a block, then export a GC-compacted
        // snapshot (ExportMode::Snapshot — the same bytes save_snapshot
        // writes; see export_doc_update's doc comment).
        let dev_a = DeviceId::from_bytes([0xa9; 16]);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        upsert_block(&a, note, bid, "doomed", None).await;
        a.record_local(OpPayload::BlockDelete { block_id: bid })
            .await
            .unwrap();
        let snapshot = a.export_doc_update(note, None).await.unwrap();

        // Engine B: FRESH — never saw the delete op directly, only the
        // GC-compacted snapshot's current state.
        let dev_b = DeviceId::from_bytes([0xb9; 16]);
        let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
        b.import_authoritative_snapshot(note, &snapshot)
            .await
            .unwrap();
        assert_eq!(
            block_text(&b, note, bid).await,
            None,
            "fresh import of the GC snapshot must land the block deleted"
        );

        // A STALE whole-content NoteUpsert on B still carries the deleted
        // bid in its body (as if authored before the delete propagated).
        let bid_uuid = uuid::Uuid::from_bytes(bid);
        b.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("gc-tombstone".into()),
            title: "GC".into(),
            content: format!("- doomed <!-- bid:{bid_uuid} -->\n"),
            created_at_millis: 2,
        })
        .await
        .unwrap();

        assert_eq!(
            block_text(&b, note, bid).await,
            None,
            "deleted-wins must survive a GC-compacted snapshot round-trip: \
             a stale NoteUpsert on a fresh engine must not resurrect a bid \
             tombstoned only in the imported snapshot's current state"
        );
    }

    // -----------------------------------------------------------------
    // Per-note apply serialization (tesela-4ju): adversarial-review
    // finding #4 on tesela-y11 asked whether `apply_import`'s
    // plan→import→tombstone sequence can interleave across CONCURRENT
    // applies for the SAME note (the docs-map write lock, taken only
    // inside `doc_for_note_mut`, is released before the sequence starts —
    // so without an additional per-note lock, two racing applies could
    // interleave and the second's twins could be tombstoned by the
    // first's stale plan, or vice versa). `apply_lock_for_note` +
    // `apply_import` holding it for the whole body (loro_engine.rs) close
    // that window. These two tests prove it at both levels: the lock
    // primitive itself, and an end-to-end hammer of the public apply API.
    // -----------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn apply_lock_serializes_same_note_not_different_notes() {
        let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
        let note_a = [0xaa; 16];
        let note_b = [0xbb; 16];

        let lock_a1 = engine.apply_lock_for_note(note_a).await;
        let lock_a2 = engine.apply_lock_for_note(note_a).await;
        assert!(
            Arc::ptr_eq(&lock_a1, &lock_a2),
            "the same note_id must resolve to the SAME lock across calls, \
             or two concurrent applies for that note would each grab an \
             independent (non-serializing) mutex"
        );

        let lock_b = engine.apply_lock_for_note(note_b).await;
        assert!(
            !Arc::ptr_eq(&lock_a1, &lock_b),
            "different notes must NOT share a lock — that would serialize \
             unrelated notes' applies against each other for no reason"
        );

        // Prove actual mutual exclusion: hold note_a's lock, spawn a task
        // that also wants note_a's lock and records when it acquires it;
        // it must NOT acquire until the holder releases.
        let order = Arc::new(tokio::sync::Mutex::new(Vec::<&'static str>::new()));
        let guard = lock_a1.lock().await;
        let order2 = order.clone();
        let lock_a3 = engine.apply_lock_for_note(note_a).await;
        let waiter = tokio::spawn(async move {
            let _g = lock_a3.lock().await;
            order2.lock().await.push("waiter-acquired");
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        order.lock().await.push("holder-still-held");
        drop(guard);
        waiter.await.unwrap();
        let seq = order.lock().await.clone();
        assert_eq!(
            seq,
            vec!["holder-still-held", "waiter-acquired"],
            "the waiter must not acquire note_a's apply lock while the \
             first holder is still active — mutual exclusion is broken"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn apply_import_hammer_one_note_converges_without_leftover_twins() {
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let note = blake3_note_id("apply-race-note");

        // 8 devices each independently author the SAME note body (disjoint
        // Loro lineages — same block_ids A_BID/B_BID, distinct TreeIDs per
        // device), producing 8 full-snapshot frames the server will import
        // CONCURRENTLY. Each frame carries a genuinely different text for A
        // so a corrupted/interleaved heal would be visible as garbled or
        // vanished text, not just a silently-wrong-but-plausible value.
        let mut frames = Vec::new();
        for i in 0u8..8 {
            let ddev = DeviceId::from_bytes([i + 1; 16]);
            let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
            device
                .record_local(OpPayload::NoteUpsert {
                    note_id: note,
                    display_alias: Some("race".into()),
                    title: "Race".into(),
                    content: format!(
                        "- Awesome from {i} <!-- bid:{A_BID} -->\n\
                         - B from {i} <!-- bid:{B_BID} -->\n"
                    ),
                    created_at_millis: 1,
                })
                .await
                .unwrap();
            frames.push(device.export_doc_update(note, None).await.unwrap());
        }

        // Hammer: import all 8 frames into the SAME server note
        // CONCURRENTLY from separate tokio tasks on a multi-thread runtime.
        // Pre-fix (no per-note apply lock) this races each frame's
        // props-plan fork against every other frame's raw import +
        // tombstone; post-fix (tesela-4ju) the per-note mutex in
        // `apply_import` forces them one at a time, so the result must be
        // identical to a sequential run: exactly one surviving node per
        // block_id, nothing vanished.
        let mut set = tokio::task::JoinSet::new();
        for bytes in frames {
            let server = server.clone();
            set.spawn(async move { server.import_doc_update(note, &bytes).await });
        }
        while let Some(res) = set.join_next().await {
            res.expect("apply task must not panic")
                .expect("concurrent import_doc_update must not error");
        }

        {
            let docs = server.inner.docs.read().await;
            let doc = docs
                .get(&note)
                .expect("note resident after concurrent imports");
            assert!(
                duplicate_block_ids(doc).is_empty(),
                "concurrent apply_import calls for one note must still \
                 converge to a single surviving node per block_id — leftover \
                 twins mean the plan→import→tombstone sequence interleaved \
                 across callers"
            );
        }

        let a = block_text(&server, note, A_BID_BYTES).await;
        let b = block_text(&server, note, B_BID_BYTES).await;
        assert!(
            a.is_some(),
            "block A must survive concurrent hammering, not vanish entirely"
        );
        assert!(
            b.is_some(),
            "block B must survive concurrent hammering, not vanish entirely"
        );
    }

    // -----------------------------------------------------------------
    // tesela-4ju REVIEW REJECT (2026-07-02): the per-note `apply_locks`
    // guard closed apply-vs-apply, but `record_local` and
    // `heal_disjoint_twins` ran their own plan/import/tombstone-shaped
    // sequences WITHOUT taking it — the same interleave class stayed open
    // through those two paths. Both now take the SAME per-note guard
    // (`note_id_for_payload` + `apply_lock_for_note`). These two tests race
    // each path against a concurrent `apply_import` for the SAME note.
    // -----------------------------------------------------------------

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn record_local_races_apply_import_for_same_note_preserves_local_edits() {
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let note = blake3_note_id("record-vs-import-race");

        // Server already resident (so the Delta import's twin-heal plan gate
        // — `already_resident && !is_views` — is active): exactly the
        // residency this bead's finding is about.
        server
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("race".into()),
                title: "Race".into(),
                content: format!(
                    "- Server original <!-- bid:{A_BID} -->\n\
                     - B server <!-- bid:{B_BID} -->\n"
                ),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // A peer authors the SAME note on a disjoint Loro lineage (fresh
        // TreeIDs for the same block_ids) with genuinely different text —
        // importing it mints server-side twins that `apply_import` must
        // resolve concurrently with the local edits below.
        let pdev = DeviceId::from_bytes([0x7f; 16]);
        let peer = LoroEngine::new(pdev, Arc::new(Hlc::new(pdev)));
        peer.record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("race".into()),
            title: "Race".into(),
            content: format!(
                "- Peer incoming <!-- bid:{A_BID} -->\n\
                 - B peer <!-- bid:{B_BID} -->\n"
            ),
            created_at_millis: 1,
        })
        .await
        .unwrap();
        let peer_frame = peer.export_doc_update(note, None).await.unwrap();

        // Race: N concurrent LOCAL edits (`record_local`) against block A's
        // "race_tag" list, plus the ONE inbound import, all launched
        // together for the SAME note. The per-note `apply_locks` guard
        // serializes them into SOME total order (never interleaved
        // mid-sequence) — regardless of that order, every local AddToList
        // must survive: applied directly to the still-live node if it runs
        // AFTER the import's tombstone, or captured by the props-plan union
        // fork (`peer_genuine_block_changes`/`twin_winners_for`) and
        // re-asserted onto the survivor if it runs BEFORE the import.
        const N: u8 = 6;
        let mut set = tokio::task::JoinSet::new();
        for i in 0..N {
            let server = server.clone();
            set.spawn(async move {
                server
                    .record_local(OpPayload::BlockPropertySet {
                        note_id: note,
                        block_id: A_BID_BYTES,
                        key: "race_tag".into(),
                        value: PropOp::AddToList(PropScalar::Text(format!("local-{i}"))),
                    })
                    .await
                    .map(|_| ())
            });
        }
        {
            let server = server.clone();
            let peer_frame = peer_frame.clone();
            set.spawn(async move { server.import_doc_update(note, &peer_frame).await });
        }
        while let Some(res) = set.join_next().await {
            res.expect("race task must not panic")
                .expect("record_local/import_doc_update must not error");
        }

        {
            let docs = server.inner.docs.read().await;
            let doc = docs.get(&note).expect("note resident after the race");
            assert!(
                duplicate_block_ids(doc).is_empty(),
                "record_local racing apply_import for the same note must still \
                 converge to a single surviving node per block_id"
            );
        }

        let mut tags: Vec<String> = block_prop_list(&server, note, A_BID_BYTES, "race_tag")
            .await
            .into_iter()
            .map(|s| match s {
                PropScalar::Text(t) => t,
                other => format!("{other:?}"),
            })
            .collect();
        tags.sort();
        let expected: Vec<String> = (0..N).map(|i| format!("local-{i}")).collect();
        assert_eq!(
            tags, expected,
            "every concurrent record_local AddToList must survive a racing \
             apply_import for the SAME note — a dropped entry here means the \
             local edit landed between apply_import's props-plan fork and its \
             twin tombstone and got silently discarded (the tesela-4ju REVIEW \
             REJECT finding this test guards)"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn heal_disjoint_twins_races_apply_import_for_same_note_without_corruption() {
        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
        let note = blake3_note_id("heal-vs-import-race");
        const C_BID_BYTES: [u8; 16] = [0x0c; 16];

        // Seed block A via the normal self-healing path (single live node —
        // no apply path ever leaves a standing twin post-tesela-fte, per
        // relay_inbound_rebase.rs's c14 comment).
        server
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("race".into()),
                title: "Race".into(),
                content: format!("- Base A <!-- bid:{A_BID} -->\n"),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        server
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: A_BID_BYTES,
                key: "status".into(),
                value: PropOp::SetScalar(PropScalar::Text("doing".into())),
            })
            .await
            .unwrap();

        // Fabricate a GENUINE standing twin for A directly on the doc,
        // bypassing every self-healing apply path — the only realistic
        // source of a persistent twin (legacy pre-fix `.bin` residue loaded
        // from disk). Gives `heal_disjoint_twins` real work, so this test
        // exercises its ACTUAL plan→tombstone→reassert body racing a
        // concurrent `apply_import`, not a no-op scan.
        {
            let doc = server.doc_for_note_mut(note).await;
            let tree = doc.get_tree("blocks");
            let raw_twin = tree.create(TreeParentId::Root).unwrap();
            let meta = tree.get_meta(raw_twin).unwrap();
            meta.insert("block_id", hex_id(&A_BID_BYTES).as_str())
                .unwrap();
            meta.insert("indent_level", 0i64).unwrap();
            meta.insert("parent", "").unwrap();
            write_block_text(&meta, "raw twin residue").unwrap();
            let (props, prop_keys) = prop_containers::node_prop_containers(&meta).unwrap();
            apply_prop_op(
                &props,
                &prop_keys,
                "priority",
                &PropOp::SetScalar(PropScalar::Int(3)),
            )
            .unwrap();
            doc.commit();
        }
        assert_eq!(
            duplicate_block_ids(&server.doc_for_note_mut(note).await).len(),
            1,
            "fixture setup must actually produce a standing twin for A \
             before the race"
        );

        // Pad the tree with MANY additional single-node (non-twin) blocks
        // so `twin_winners_for`'s doc-wide scan and `tombstone_duplicate_
        // twins`'s own scan take long enough in wall-clock time to give the
        // concurrent `record_local` writers below (spawned alongside the
        // heal/import race) a REAL chance to land their write on block A's
        // about-to-be-tombstoned node between heal's plan-fork read of A's
        // props and its tombstone call. With only A's twin present, that
        // window is a handful of CPU instructions wide and effectively
        // unhittable by scheduling luck alone — created AFTER A (so the
        // scan processes A early and still has to plow through every pad
        // node before `twin_winners_for` returns and `tombstone_duplicate_
        // twins` runs), this widens it to real, hittable microseconds.
        const PAD_BLOCKS: u32 = 1000;
        {
            let doc = server.doc_for_note_mut(note).await;
            let tree = doc.get_tree("blocks");
            for i in 0..PAD_BLOCKS {
                let mut pad_bid = [0xf0u8; 16];
                pad_bid[14..16].copy_from_slice(&(i as u16).to_be_bytes());
                let node = tree.create(TreeParentId::Root).unwrap();
                let meta = tree.get_meta(node).unwrap();
                meta.insert("block_id", hex_id(&pad_bid).as_str()).unwrap();
                meta.insert("indent_level", 0i64).unwrap();
                meta.insert("parent", "").unwrap();
                write_block_text(&meta, "pad").unwrap();
            }
            doc.commit();
        }

        // A peer concurrently authors a genuinely NEW block (disjoint from
        // A) on the SAME note — the concurrent inbound import this races
        // against the heal.
        let pdev = DeviceId::from_bytes([0x7f; 16]);
        let peer = LoroEngine::new(pdev, Arc::new(Hlc::new(pdev)));
        peer.record_local(OpPayload::BlockUpsert {
            block_id: C_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "Peer concurrent block C".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
        let peer_frame = peer.export_doc_update(note, None).await.unwrap();

        // Race: `heal_disjoint_twins()` (sweeping every resident note,
        // including this one) concurrently with `import_doc_update` for the
        // SAME note, PLUS N concurrent `record_local` edits to block A's
        // props — mirroring
        // `record_local_races_apply_import_for_same_note_preserves_local_edits`.
        // The STATIC pre-race props (`status`/`priority` above) alone don't
        // discriminate the reverted lock: `heal_disjoint_twins`'s plan and
        // `apply_import`'s own plan (`peer_genuine_block_changes` →
        // `twin_winners_for`) are both PURE functions of the same
        // (unchanging) twin set, so either racer reasserts them identically
        // regardless of interleaving — there's no genuinely racing WRITE for
        // a static fixture to catch. A CONCURRENT write can land on the twin
        // node about to be tombstoned AFTER heal's plan-fork already
        // captured a stale snapshot: with the per-note lock intact, `heal_
        // disjoint_twins` holds it across its whole plan→tombstone→reassert
        // body, so no `record_local` for this note can land mid-sequence;
        // with the lock reverted, heal never blocks the other lock holders
        // and the drop window is real (tesela-xh4 REVIEW REJECT,
        // 2026-07-02).
        const N: u8 = 20;
        let mut local_set = tokio::task::JoinSet::new();
        for i in 0..N {
            let server = server.clone();
            local_set.spawn(async move {
                server
                    .record_local(OpPayload::BlockPropertySet {
                        note_id: note,
                        block_id: A_BID_BYTES,
                        key: "race_tag".into(),
                        value: PropOp::AddToList(PropScalar::Text(format!("local-{i}"))),
                    })
                    .await
                    .map(|_| ())
            });
        }
        let heal_task = {
            let server = server.clone();
            tokio::spawn(async move { server.heal_disjoint_twins().await })
        };
        let import_task = {
            let server = server.clone();
            tokio::spawn(async move { server.import_doc_update(note, &peer_frame).await })
        };
        while let Some(res) = local_set.join_next().await {
            res.expect("race_local task must not panic")
                .expect("concurrent record_local must not error");
        }
        heal_task.await.expect("heal task must not panic");
        import_task
            .await
            .expect("import task must not panic")
            .expect("concurrent import_doc_update must not error");

        {
            let docs = server.inner.docs.read().await;
            let doc = docs.get(&note).expect("note resident after the race");
            assert!(
                duplicate_block_ids(doc).is_empty(),
                "heal_disjoint_twins racing a concurrent apply_import for the \
                 SAME note must still converge to a single surviving node per \
                 block_id — a leftover twin means the two sequences \
                 interleaved (the tesela-4ju REVIEW REJECT finding this test \
                 guards)"
            );
        }

        let a = block_text(&server, note, A_BID_BYTES).await;
        let c = block_text(&server, note, C_BID_BYTES).await;
        assert!(
            a.is_some(),
            "block A must survive the heal, not vanish entirely"
        );
        assert_eq!(
            c.as_deref(),
            Some("Peer concurrent block C"),
            "the concurrent import's genuinely new block C must land intact, \
             not be corrupted/dropped by the racing heal"
        );
        assert_eq!(
            block_prop_scalar(&server, note, A_BID_BYTES, "status").await,
            Some(PropScalar::Text("doing".into())),
            "the pre-existing A twin's scalar property must survive the racing heal"
        );
        assert_eq!(
            block_prop_scalar(&server, note, A_BID_BYTES, "priority").await,
            Some(PropScalar::Int(3)),
            "the fabricated A twin's distinct scalar property must be reasserted \
             onto the survivor by the racing heal"
        );
        let mut tags: Vec<String> = block_prop_list(&server, note, A_BID_BYTES, "race_tag")
            .await
            .into_iter()
            .map(|s| match s {
                PropScalar::Text(t) => t,
                other => format!("{other:?}"),
            })
            .collect();
        tags.sort();
        let mut expected: Vec<String> = (0..N).map(|i| format!("local-{i}")).collect();
        expected.sort();
        assert_eq!(
            tags, expected,
            "every concurrent record_local AddToList onto block A must survive \
             a racing heal_disjoint_twins for the SAME note — a dropped entry \
             here means the local edit landed between heal's plan-fork \
             (twin_winners_for) and its twin tombstone/reassert and got \
             silently discarded (the tesela-xh4 REVIEW REJECT finding this \
             test guards)"
        );
    }
}
