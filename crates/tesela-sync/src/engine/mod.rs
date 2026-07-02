//! The `SyncEngine` trait and supporting types.

pub mod applied;
pub mod cursor;
pub mod loro_engine;

pub use applied::AppliedChanges;
pub use cursor::{LocalCursor, PeerCursor};
pub use loro_engine::LoroEngine;

use crate::device::DeviceId;
use crate::error::SyncResult;
use crate::oplog::op::{ContentHash, OpPayload};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Per-note outcome of one inbound relay batch apply
/// ([`SyncEngine::apply_relay_updates`]). Replaces the old bare `usize`
/// count, which silently swallowed per-note failures while the callers
/// advanced/acked the relay cursor past them (audit A4, 2026-06-09).
#[derive(Debug, Clone, Default)]
pub struct RelayApplyReport {
    /// Notes whose update imported cleanly (fully integrated).
    pub applied: Vec<[u8; 16]>,
    /// Notes whose update imported but was left PENDING by Loro — a causal
    /// gap (missing dependencies). The bytes are buffered in-memory only,
    /// so the caller should trigger an authoritative-snapshot catch-up for
    /// these notes or the data is lost on restart.
    pub pending: Vec<[u8; 16]>,
    /// Notes whose import errored, with the error message. A caller MUST
    /// NOT ack/advance its relay cursor past the carrying envelope without
    /// a retry/catch-up policy, or the update is skipped forever.
    pub failed: Vec<([u8; 16], String)>,
}

impl RelayApplyReport {
    /// Count of updates that imported (cleanly OR pending) — the same
    /// number the old `usize` return reported, for observability parity.
    pub fn applied_count(&self) -> usize {
        self.applied.len() + self.pending.len()
    }
}

/// The core sync engine trait. Post-flag-day (2026-05-29) the only
/// implementation is [`LoroEngine`]; the trait remains as the boundary
/// the server's `Arc<dyn SyncEngine>` and the FFI hold. The legacy
/// op-replay methods (`apply_changes` / `produce_changes_since` /
/// `produce_local_authored_since`) were removed with the SqliteEngine
/// stack — sync now flows entirely through the Loro relay-update methods
/// below.
#[async_trait]
pub trait SyncEngine: Send + Sync {
    /// Local device id. Surfaced on the trait so server code can hold an
    /// `Arc<dyn SyncEngine>` without reaching for a concrete engine —
    /// several server routes use the device id for envelope addressing.
    fn device(&self) -> DeviceId;

    /// Local-side mutation entry point. `tesela-core` funnels every write
    /// here when sync is enabled. The engine appends an oplog row and
    /// returns the resulting content hash.
    async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash>;

    /// Current cursor for ops THIS device has produced.
    async fn local_cursor(&self) -> SyncResult<LocalCursor>;

    /// Cursor we have stored for ops we have received from a given peer.
    async fn peer_cursor(&self, peer: DeviceId) -> SyncResult<PeerCursor>;

    /// Record that a peer has acknowledged ops up to `ack`. Drives
    /// oplog retention.
    async fn ack_peer(&self, peer: DeviceId, ack: PeerCursor) -> SyncResult<()>;

    /// Render a note's body from the engine's internal state. Returns
    /// `None` if the engine doesn't track this note (or doesn't support
    /// rendering — SqliteEngine's default returns None since the
    /// authoritative state lives on disk via materialize, not in the
    /// engine).
    ///
    /// Used by the `GET /api/loro/notes/:slug` debug endpoint and the
    /// per-request divergence check. LoroEngine overrides this to walk
    /// its tree; DualEngine forwards to the shadow.
    async fn render_note(&self, _note_id: [u8; 16]) -> Option<String> {
        None
    }

    /// Render the *complete* `.md` file (frontmatter + page properties +
    /// blocks) the engine would write to disk as the authoritative writer.
    /// This is the dry-run surface for the Loro cutover: what
    /// materialization WOULD emit, diffable against the live on-disk file
    /// before any write flips. Default `None`; LoroEngine overrides to
    /// include frontmatter; DualEngine forwards to the shadow.
    async fn render_note_full(&self, _note_id: [u8; 16]) -> Option<String> {
        None
    }

    /// Compute the per-note Loro updates to broadcast this relay tick:
    /// `(note_id, update_bytes, captured_vv)` for every note changed since
    /// its last broadcast. Does NOT advance the broadcast cursor — the
    /// `tick` calls [`commit_broadcast_cursors`](Self::commit_broadcast_cursors)
    /// only after a confirmed PUT, so a failed send retries. Default empty.
    async fn produce_relay_updates(&self) -> Vec<([u8; 16], Vec<u8>, Vec<u8>)> {
        Vec::new()
    }

    /// Advance + persist the broadcast cursor for notes confirmed sent
    /// (paired with `produce_relay_updates`' `captured_vv`). Default no-op.
    async fn commit_broadcast_cursors(&self, _committed: &[([u8; 16], Vec<u8>)]) {}

    /// Apply a batch of inbound per-note Loro updates from the relay
    /// (idempotent + commutative). Returns a per-note [`RelayApplyReport`]
    /// — which notes applied cleanly, which were left PENDING by Loro
    /// (causal gap), and which failed — so callers can hold the cursor /
    /// trigger a snapshot catch-up instead of silently skipping failures.
    /// Default: empty report.
    async fn apply_relay_updates(&self, _updates: &[([u8; 16], Vec<u8>)]) -> RelayApplyReport {
        RelayApplyReport::default()
    }

    /// Encoded version vector of a note's doc — a peer sends this so we
    /// export only updates newer than what it has. `None` if the doc isn't
    /// resident (or the engine doesn't track Loro docs). Surfaced on the
    /// trait (2026-05-30) so the live WS path, holding `dyn SyncEngine`, can
    /// capture a note's pre-edit version vector. Default `None`; LoroEngine
    /// overrides. Does NOT touch the relay's broadcast cursor.
    async fn doc_version(&self, _note_id: [u8; 16]) -> Option<Vec<u8>> {
        None
    }

    /// Export a note's Loro update bytes since the given encoded version
    /// vector (`None` = full compact snapshot, for a fresh-device
    /// bootstrap). `None` if the doc isn't resident or export fails. This is
    /// the **cursor-free** delta export the live WS path uses — it does NOT
    /// read or advance the relay's `broadcast_cursor`, so the WS and relay
    /// paths never contend (instant-multidevice spec, finding #3). Default
    /// `None`; LoroEngine overrides.
    async fn export_doc_update(
        &self,
        _note_id: [u8; 16],
        _since: Option<&[u8]>,
    ) -> Option<Vec<u8>> {
        None
    }

    /// Import a peer's Loro update bytes into the addressed note's doc
    /// (creating it if absent), refresh derived state, and persist. Loro
    /// merge is commutative + idempotent, so duplicate / out-of-order
    /// imports are safe. Surfaced on the trait (2026-05-30) so the live WS
    /// path can apply a single received delta. Default no-op `Ok(())`;
    /// LoroEngine overrides.
    async fn import_doc_update(&self, _note_id: [u8; 16], _bytes: &[u8]) -> SyncResult<()> {
        Ok(())
    }

    /// Apply the server's FULL snapshot as an AUTHORITATIVE re-base: a
    /// disjoint device adopts the server's lineage by unioning it in and then
    /// resolving each same-bid twin with the SAME deterministic keep-winner
    /// rule as [`import_doc_update`](Self::import_doc_update) (max-`TreeID`
    /// among non-stale tips — NOT server-wins), so the catch-up path and the WS
    /// path always pick the IDENTICAL survivor and later concurrent edits MERGE
    /// instead of forking new twins (tesela-y11, decisions.md 2026-07-01). The
    /// iOS catch-up path routes here. Default forwards to `import_doc_update`;
    /// LoroEngine overrides with the re-base.
    async fn import_authoritative_snapshot(
        &self,
        note_id: [u8; 16],
        bytes: &[u8],
    ) -> SyncResult<()> {
        self.import_doc_update(note_id, bytes).await
    }

    /// Like [`import_doc_update`](Self::import_doc_update) but RETURNS whether
    /// Loro left the imported update PENDING — i.e. it referenced ops the doc
    /// is missing (a causal gap / disjoint-lineage signal a caller can use to
    /// trigger an authoritative-snapshot catch-up). Default forwards to
    /// `import_doc_update` and reports `false`; LoroEngine overrides to surface
    /// the real `ImportStatus.pending`.
    async fn apply_doc_update_status(&self, note_id: [u8; 16], bytes: &[u8]) -> SyncResult<bool> {
        self.import_doc_update(note_id, bytes).await?;
        Ok(false)
    }

    /// Apply a single CHARACTER-LEVEL splice to one block's text: delete
    /// `utf16_delete_len` UTF-16 code units at `utf16_offset`, then insert
    /// `insert` (the two at the same offset = a replace). Offsets are UTF-16
    /// code units (matching iOS `NSRange` / JS string indices). The
    /// outbound foundation for cursor-accurate collaborative editing — a
    /// client sends the user's actual keystroke instead of re-authoring the
    /// whole block text (which Myers-diffs into DELETEs of a concurrent
    /// peer's characters → clobber). Routes through the block's `text_seq`
    /// LoroText, so concurrent splices INTERLEAVE. Returns `Ok(1)` when
    /// applied, `Ok(0)` when the block isn't found (a splice is an in-place
    /// edit). Default no-op `Ok(0)`; LoroEngine overrides.
    async fn splice_block_text(
        &self,
        _note_id: [u8; 16],
        _block_id: [u8; 16],
        _utf16_offset: u32,
        _utf16_delete_len: u32,
        _insert: &str,
    ) -> SyncResult<u32> {
        Ok(0)
    }

    /// Read a single block's current text — the engine-exact `text_seq`
    /// content (falling back to a legacy `text` register) — by note + block
    /// id. The inbound counterpart of [`splice_block_text`](Self::splice_block_text):
    /// after a remote splice is applied, the iOS client reads the MERGED block
    /// text here to reconcile the open editor (the engine is the source of
    /// truth; the editor matches it). Returns `None` for an unknown note/block
    /// or an empty block. Default `None`; LoroEngine overrides.
    async fn read_block_text(&self, _note_id: [u8; 16], _block_id: [u8; 16]) -> Option<String> {
        None
    }

    /// Mint a stable, op-anchored cursor at `utf16_offset` in a block's text,
    /// as transport bytes (Phase 1 presence). Default `None`; LoroEngine
    /// overrides. See [`LoroEngine::mint_block_cursor`].
    async fn mint_block_cursor(
        &self,
        _note_id: [u8; 16],
        _block_id: [u8; 16],
        _utf16_offset: u32,
    ) -> Option<Vec<u8>> {
        None
    }

    /// Resolve an encoded cursor to its CURRENT utf16 offset in this engine's
    /// doc. Default `None`; LoroEngine overrides.
    async fn resolve_block_cursor(&self, _note_id: [u8; 16], _cursor_bytes: &[u8]) -> Option<u32> {
        None
    }

    /// Set THIS peer's ephemeral presence under `key`, returning the broadcast
    /// delta. Default no-op (empty); LoroEngine overrides.
    fn set_local_presence(&self, _key: String, _value: Vec<u8>) -> Vec<u8> {
        Vec::new()
    }

    /// Merge a peer's presence delta (last-write-wins). Default `false`.
    fn apply_presence(&self, _bytes: &[u8]) -> bool {
        false
    }

    /// All live peers' presence as `(key, value)`. Default empty.
    fn presence_peers(&self) -> Vec<(String, Vec<u8>)> {
        Vec::new()
    }

    /// Purge presence entries past the timeout (call on a timer). Default no-op.
    fn presence_remove_outdated(&self) {}

    /// Enumerate every note id the engine tracks. Default empty.
    /// `DualEngine` overrides to return the shadow's tracked notes;
    /// `SqliteEngine` returns empty because oplog enumeration would be
    /// expensive and not what callers want (they want the shadow's
    /// view for divergence work).
    async fn tracked_note_ids(&self) -> Vec<[u8; 16]> {
        Vec::new()
    }

    /// Return the primary (authoritative) engine's view of a note's
    /// body, for divergence comparison. SqliteEngine reads the
    /// materialized markdown file; DualEngine forwards to its primary;
    /// other impls default to `None`.
    async fn primary_body(&self, _note_id: [u8; 16]) -> Option<String> {
        None
    }

    /// Entries from the Loro index doc. The hybrid-model spine (cutover
    /// spec Phase 2). Default empty; LoroEngine/DualEngine override. Used
    /// by the `/loro/index` debug endpoint and, eventually, the note
    /// list + backlinks + ref resolution.
    async fn index_entries(&self) -> Vec<IndexEntry> {
        Vec::new()
    }

    /// All saved views from the synced views registry doc, sorted by
    /// `(order, id)` — deterministic across devices. Default empty;
    /// LoroEngine overrides (saved-views spec, 2026-06-10).
    async fn views_list(&self) -> Vec<ViewRecord> {
        Vec::new()
    }

    /// Create or update a saved view in the registry (field-level LWW —
    /// concurrent edits of different fields both survive). A view that is
    /// already builtin stays builtin regardless of the record's flag, so
    /// the delete guard can't be bypassed via upsert. Default no-op;
    /// LoroEngine overrides.
    async fn views_upsert(&self, _record: ViewRecord) -> SyncResult<()> {
        Ok(())
    }

    /// Delete a saved view by id. Returns `Ok(true)` when removed,
    /// `Ok(false)` when no such view exists, and `Err` for a builtin view
    /// (builtins are editable but never deletable — enforced HERE, at the
    /// API). Default `Ok(false)`; LoroEngine overrides.
    async fn views_delete(&self, _view_id: &str) -> SyncResult<bool> {
        Ok(false)
    }

    /// Idempotently seed the built-in views (currently: Inbox). Safe to
    /// call on every boot and on every device — the builtin's FIXED view
    /// id makes concurrent seeds write the same entry, so the group
    /// converges to ONE Inbox. No-op when the entry already exists
    /// (locally seeded or received via sync), so user edits to the
    /// builtin's dsl/display are never clobbered by a reseed. Default
    /// no-op; LoroEngine overrides.
    async fn ensure_builtin_views(&self) -> SyncResult<()> {
        Ok(())
    }
}

/// One saved view in the synced views registry doc (saved-views spec,
/// 2026-06-10). The registry is ONE dedicated always-resident Loro doc
/// ([`loro_engine::VIEWS_DOC_ID`]) that syncs across devices exactly like
/// a note doc; each view is a map entry with field-level LWW.
///
/// The spec's nested `display{mode, groupBy?, showDone?}` shape is stored
/// FLAT (`display_mode` / `display_group_by` / `display_show_done`) — a
/// nested CRDT map would buy identical per-field LWW semantics at the cost
/// of one more container per view, and flat fields cross the FFI as plain
/// scalars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewRecord {
    /// Stable view id. User views mint a UUID; builtins use a FIXED
    /// constant id (e.g. [`loro_engine::INBOX_VIEW_ID`]) so concurrent
    /// seeds on two devices write the same entry and converge to one.
    pub id: String,
    /// Display name ("Inbox", "This week", …).
    pub name: String,
    /// The query DSL string the view executes
    /// (e.g. `status:backlog,todo -has:scheduled -has:deadline`).
    pub dsl: String,
    /// Sort position in the view switcher. A plain LWW integer (not a
    /// CRDT list): with 6–12 views, reorder rewrites the handful of
    /// affected `order` values — far simpler than a movable list, and
    /// ties break deterministically by `id`.
    pub order: i64,
    /// Built-in views are seeded by the engine, editable, but never
    /// deletable. Sticky: once true, an upsert cannot flip it back.
    pub builtin: bool,
    /// Result rendering: "list" | "table" | "kanban".
    pub display_mode: String,
    /// Optional grouping key (kanban columns / table groups).
    pub display_group_by: Option<String>,
    /// Optional "include done items" toggle.
    pub display_show_done: Option<bool>,
}

/// One note's entry in the Loro index doc.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// 32-char hex of the note_id.
    pub note_id: String,
    /// Note title (frontmatter `title:` or slug).
    pub title: String,
    /// Filename slug (display_alias).
    pub slug: String,
    /// All tags for the note — frontmatter `tags:` + `tags::` page
    /// property + inline `#tags`, deduped + sorted.
    pub tags: Vec<String>,
    /// Outbound `[[wiki-link]]` targets, deduped + sorted (the link
    /// graph edges originating from this note).
    pub links: Vec<String>,
}
