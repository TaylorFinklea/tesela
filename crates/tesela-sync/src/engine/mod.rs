//! The `SyncEngine` trait and supporting types.

pub mod applied;
pub mod cursor;
pub mod hydration;
pub mod loro_engine;

pub use applied::AppliedChanges;
pub use cursor::{LocalCursor, PeerCursor};
pub use hydration::{hydrate_note, EngineImportNoteWriter};
pub use loro_engine::LoroEngine;

use crate::device::DeviceId;
use crate::error::SyncResult;
use crate::oplog::op::{ContentHash, OpPayload};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Placement of a relocated block subtree relative to its destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MovePlacement {
    /// Insert immediately before the target and adopt its ancestry.
    Before,
    /// Append as the target's final child.
    Inside,
    /// Insert after the target's complete subtree and adopt its ancestry.
    After,
    /// Append to the note as a top-level subtree.
    Append,
}

/// Trusted metadata used when relocation creates a missing destination note.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RelocationNoteSeed {
    /// Optional display slug/alias stored with the note.
    pub display_alias: Option<String>,
    /// Note title.
    pub title: String,
    /// Canonical seed Markdown whose frontmatter and page properties are kept.
    pub content: String,
    /// Note creation time in Unix milliseconds, retained in the canonical
    /// request for Task 4 intent/receipt hashing. As with `NoteUpsert`, the
    /// rendered creation timestamp remains authoritative in `content`
    /// frontmatter rather than a second Loro root register.
    pub created_at_millis: i64,
}

/// Complete request for one stable-id block subtree relocation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlockRelocationRequest {
    /// Idempotency key for the relocation.
    pub move_id: [u8; 16],
    /// Current owning note of the subtree root.
    pub source_note_id: [u8; 16],
    /// Stable source note slug.
    pub source_slug: String,
    /// Stable block id at the root of the moved subtree.
    pub root_bid: [u8; 16],
    /// Destination note id.
    pub destination_note_id: [u8; 16],
    /// Stable destination note slug.
    pub destination_slug: String,
    /// Destination block id for target-relative placements.
    pub target_bid: Option<[u8; 16]>,
    /// Requested placement relative to `target_bid` or the destination note.
    pub placement: MovePlacement,
    /// Trusted seed used only when the destination note does not yet exist.
    pub destination_seed: Option<RelocationNoteSeed>,
}

/// Result class for a completed relocation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockRelocationStatus {
    /// The engine changed one or both addressed notes.
    Applied,
    /// A durable idempotent receipt answered a retry.
    Replayed,
    /// The requested same-note placement already matched authoritative state.
    NoOp,
}

/// Per-note version and change information captured by relocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelocatedNoteVersion {
    /// Stable note id.
    pub note_id: [u8; 16],
    /// Stable note slug.
    pub slug: String,
    /// Encoded version vector before relocation changed the note.
    pub pre_version: Vec<u8>,
    /// Whether this request changed the note.
    pub changed: bool,
    /// Whether this request created the note.
    pub created: bool,
}

/// Authoritative engine outcome for one subtree relocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockRelocationOutcome {
    /// Idempotency key echoed from the request.
    pub move_id: [u8; 16],
    /// How the engine satisfied the request.
    pub status: BlockRelocationStatus,
    /// Source first, then destination for cross-note moves; one entry for same-note moves.
    pub notes: Vec<RelocatedNoteVersion>,
}

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

/// One entry in the engine's durable causal-gap ledger (tesela-c7s item 2):
/// a note whose inbound relay update Loro left PENDING because it referenced
/// ops the doc is missing (a disjoint-lineage / missing-base signal). Recorded
/// STRUCTURALLY (not just `tracing::warn`'d and forgotten) so the strand is
/// observable and the engine can auto-issue a snapshot catch-up for any note
/// that stays pending past one apply pass. Cleared when a later delta OR an
/// authoritative snapshot fully integrates the note.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingImport {
    /// The note whose update is stuck behind a causal gap.
    pub note_id: [u8; 16],
    /// The engine `apply_relay_updates` pass at which this note FIRST went
    /// pending. A note whose `first_seen_pass` is strictly below the current
    /// pass has survived at least one full inbound batch still pending — the
    /// auto-heal (`notes_needing_snapshot_catchup`) trigger.
    pub first_seen_pass: u64,
    /// The most recent pass at which this note was still pending.
    pub last_seen_pass: u64,
    /// Loro PeerIDs whose ops the stuck frame carried (best-effort, decoded
    /// from the frame's blob metadata) — the "from_peer" of the missing base.
    pub from_peers: Vec<u64>,
    /// Number of times this note has been ESCALATED to an authoritative-snapshot
    /// catch-up (tesela-c7s F3). Bounds the escalation so a PERMANENTLY-gapped
    /// note (no peer ever deposits its snapshot) can't re-escalate every pass
    /// forever: escalations are spaced by exponential backoff and stop once this
    /// reaches [`MAX_CATCHUP_ATTEMPTS`], after which `catchup_exhausted` is set.
    #[serde(default)]
    pub catchup_attempts: u32,
    /// The import pass at which this note was last escalated to a catch-up —
    /// the reference point for the exponential backoff window between escalations
    /// (tesela-c7s F3). `0` before the first escalation (then `first_seen_pass`
    /// is used as the reference).
    #[serde(default)]
    pub last_catchup_pass: u64,
    /// Set once `catchup_attempts` hits [`MAX_CATCHUP_ATTEMPTS`] with no heal —
    /// the note is a PERMANENT gap. It stays in the ledger (so the sync-health
    /// surface can show it) but is EXCLUDED from further escalation. Cleared only
    /// by an actual heal (a clean delta apply or an authoritative snapshot, both
    /// of which drop the whole entry via `clear_pending_import`). tesela-c7s F3.
    #[serde(default)]
    pub catchup_exhausted: bool,
}

/// Max authoritative-snapshot catch-up escalations for one stuck note before it
/// is declared a PERMANENT gap and stops re-escalating (tesela-c7s F3). Bounds
/// the "escalate every pass forever" loop a note with no recoverable snapshot
/// anywhere in the group would otherwise drive. Deliberately small so the bound
/// is reachable in a test; with the exponential backoff below the last
/// escalation lands O(2^N) passes out, which for an active group is many
/// minutes of real time — ample room for a genuinely-recoverable note to heal
/// first.
pub const MAX_CATCHUP_ATTEMPTS: u32 = 6;

/// Cap on the backoff SHIFT so the window between escalations doesn't overflow /
/// grow without bound: escalation N is due `min(2^N, 2^CATCHUP_BACKOFF_SHIFT_CAP)`
/// passes after the previous one (tesela-c7s F3).
pub const CATCHUP_BACKOFF_SHIFT_CAP: u32 = 4;

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

    /// Relocate one complete stable-id block subtree within or across notes.
    async fn relocate_subtree(
        &self,
        _request: BlockRelocationRequest,
    ) -> SyncResult<BlockRelocationOutcome> {
        Err(crate::error::SyncError::Other(
            "block subtree relocation is unsupported".into(),
        ))
    }

    /// Record a bounded group of independent local mutations. The default is
    /// deliberately sequential; `LoroEngine` specializes unique NoteUpserts
    /// so bulk import can checkpoint the shared derived index once while each
    /// note still completes its own durable snapshot + materialization tail.
    async fn record_local_batch(
        &self,
        payloads: Vec<OpPayload>,
    ) -> Vec<SyncResult<ContentHash>> {
        let mut results = Vec::with_capacity(payloads.len());
        for payload in payloads {
            results.push(self.record_local(payload).await);
        }
        results
    }

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

    /// Heal a stranded outbound cursor after a note's full snapshot was
    /// CONFIRMED deposited to the relay (tesela-c7s item 4). Each pair is
    /// `(note_id, vv_at_snapshot_export_time)`. Only rewinds a stale-ahead /
    /// undecodable cursor down to the snapshot version so the next local edit
    /// ships an incremental delta over the ops stream again; leaves healthy
    /// cursors alone. Default no-op.
    async fn repair_broadcast_cursors_after_snapshot(&self, _committed: &[([u8; 16], Vec<u8>)]) {}

    /// Notes whose inbound relay update stayed PENDING (causal gap) past one
    /// full apply pass and so need an authoritative-snapshot catch-up to heal
    /// (tesela-c7s item 2). The relay tick fetches + imports the relay's
    /// snapshot for exactly these. Default empty.
    async fn notes_needing_snapshot_catchup(&self) -> Vec<[u8; 16]> {
        Vec::new()
    }

    /// Monotonic count of outbound STRAND ALARMS raised so far (tesela-c7s
    /// item 3): a dirty note whose broadcast cursor was stale-ahead /
    /// undecodable and had to fall back to a full snapshot instead of an
    /// incremental delta. The relay tick reads this to log the deposit-strand
    /// class when it fires. Default 0.
    async fn outbound_strand_alarm_count(&self) -> u64 {
        0
    }

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
    /// rule as [`import_doc_update`](Self::import_doc_update) (pure global-max
    /// `TreeID` — NOT server-wins), so the catch-up path and the WS
    /// path always pick the IDENTICAL survivor and later concurrent edits MERGE
    /// instead of forking new twins (tesela-y11, decisions.md 2026-07-01). The
    /// iOS catch-up path routes here. Default forwards to `import_doc_update`;
    /// LoroEngine overrides with the re-base. (Since tesela-fte the survivor is
    /// purely the global-max `TreeID` twin, so "re-base" no longer implies
    /// server-wins — the higher-`TreeID` twin's text survives, device or server.)
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
    /// tesela-ya4.4 — table column display config (hide / reorder / sort),
    /// round-trip-authoritative for a saved-view table per spec decision 4
    /// (mirrors `display_group_by`'s write-back contract). Additive: absent
    /// in older records/payloads, which deserializes to `None` (serde's
    /// built-in `Option<T>` missing-key default, reinforced here with
    /// `#[serde(default)]` for defense in depth). Stored in the CRDT views
    /// doc as a single JSON-encoded string field (same flat-scalar,
    /// whole-field-LWW shape as every other `display_*` field) rather than
    /// a nested CRDT container — see `loro_engine.rs`'s views doc comment.
    #[serde(default)]
    pub display_table_config: Option<TableColumnConfig>,
}

/// tesela-ya4.4 — one saved view's table display config: which columns are
/// hidden, an explicit column order override, and the active sort. Every
/// field defaults empty/`None` so an absent config behaves exactly like "no
/// override" (natural column resolution + no sort).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableColumnConfig {
    /// Property names hidden from the table.
    #[serde(default)]
    pub hidden: Vec<String>,
    /// Explicit column display order (property names). Columns not
    /// mentioned here render after the ordered ones, in their naturally
    /// resolved order (see `resolveTableColumns`/`applyTableConfig` on the
    /// web side).
    #[serde(default)]
    pub order: Vec<String>,
    /// Property name currently sorted by, if any.
    #[serde(default)]
    pub sort_by: Option<String>,
    /// Sort direction ("asc" | "desc"); only meaningful when `sort_by` is
    /// set. A plain string (not an enum) to match `display_mode`'s
    /// boundary-validated-string convention.
    #[serde(default)]
    pub sort_dir: Option<String>,
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

#[cfg(test)]
mod display_table_config_tests {
    use super::{TableColumnConfig, ViewRecord};

    fn sample_view() -> ViewRecord {
        ViewRecord {
            id: "v-table".to_string(),
            name: "Table".to_string(),
            dsl: "tag:task".to_string(),
            order: 10,
            builtin: false,
            display_mode: "table".to_string(),
            display_group_by: None,
            display_show_done: None,
            display_table_config: None,
        }
    }

    /// tesela-ya4.4 — an older `ViewRecord` JSON payload that predates the
    /// `display_table_config` field must still deserialize, defaulting the
    /// new field to `None` (additive-field acceptance for the shared
    /// serde shape crossing HTTP + FFI boundaries).
    #[test]
    fn view_record_json_without_table_config_field_deserializes_to_none() {
        let json = serde_json::json!({
            "id": "v-old",
            "name": "Old",
            "dsl": "tag:x",
            "order": 10,
            "builtin": false,
            "display_mode": "list",
            "display_group_by": null,
            "display_show_done": null,
        });
        let record: ViewRecord = serde_json::from_value(json).expect("deserializes without the new field");
        assert_eq!(record.display_table_config, None);
    }

    /// A populated config round-trips through JSON byte-for-byte-equivalent
    /// (field order aside) — the shape `updateView` sends/receives.
    #[test]
    fn table_column_config_json_round_trips() {
        let mut view = sample_view();
        view.display_table_config = Some(TableColumnConfig {
            hidden: vec!["notes".to_string()],
            order: vec!["priority".to_string(), "status".to_string()],
            sort_by: Some("priority".to_string()),
            sort_dir: Some("desc".to_string()),
        });
        let json = serde_json::to_string(&view).unwrap();
        let back: ViewRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, view);
    }

    /// An explicitly-empty config (`{}` — every field defaulted) also
    /// round-trips, so a client that sends a bare object to reset the
    /// override doesn't fail to parse.
    #[test]
    fn table_column_config_empty_object_deserializes_to_default() {
        let cfg: TableColumnConfig = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(cfg, TableColumnConfig::default());
    }
}
