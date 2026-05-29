//! The `SyncEngine` trait and supporting types.

pub mod applied;
pub mod cursor;
pub mod loro_engine;

pub use applied::AppliedChanges;
pub use cursor::{LocalCursor, PeerCursor};
pub use loro_engine::LoroEngine;

use crate::device::DeviceId;
use crate::error::SyncResult;
use crate::oplog::op::{ContentHash, EncodedOp, OpPayload};
use crate::oplog::parked::ParkReason;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Summary of a parked-op replay attempt.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplayReport {
    /// Ops that successfully applied after replay.
    pub applied: u32,
    /// Ops still parked (e.g. translator chain still missing).
    pub still_parked: u32,
}

/// Snapshot of the parked-op queue for the UI banner.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParkedSummary {
    /// Total number of parked ops on this device.
    pub count: u32,
    /// Earliest `parked_at` (wall-clock millis), if any.
    pub oldest_parked_at_millis: Option<i64>,
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

    /// Park an op the local schema cannot understand. Exposed for tests
    /// and admin tooling.
    async fn park_op(&self, op: EncodedOp, reason: ParkReason) -> SyncResult<()>;

    /// Replay parked ops after a schema upgrade.
    async fn replay_parked(&self) -> SyncResult<ReplayReport>;

    /// Snapshot the parked-op queue for the UI banner.
    async fn parked_summary(&self) -> SyncResult<ParkedSummary>;

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
    /// (idempotent + commutative). Returns the count applied. Default 0.
    async fn apply_relay_updates(&self, _updates: &[([u8; 16], Vec<u8>)]) -> usize {
        0
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
