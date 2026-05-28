//! The `SyncEngine` trait and supporting types.

pub mod applied;
pub mod cursor;
pub mod dual_engine;
pub mod loro_engine;
pub mod sqlite_engine;

pub use applied::AppliedChanges;
pub use cursor::{LocalCursor, PeerCursor};
pub use dual_engine::DualEngine;
pub use loro_engine::LoroEngine;
pub use sqlite_engine::SqliteEngine;

use crate::device::DeviceId;
use crate::error::SyncResult;
use crate::oplog::op::{ContentHash, EncodedOp, OpPayload};
use crate::oplog::parked::ParkReason;
use crate::wire::envelope::SyncEnvelope;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A batch of ops produced by [`SyncEngine::produce_changes_since`], plus
/// the cursor pointing past the last yielded op.
#[derive(Debug, Clone)]
pub struct ProducedBatch {
    /// The ops, oldest first.
    pub ops: Vec<EncodedOp>,
    /// The new cursor pointing past the last op in `ops`. If `ops` is
    /// empty, this equals the input cursor.
    pub new_cursor: PeerCursor,
}

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

/// The core sync engine trait. Implementations: [`SqliteEngine`],
/// [`LoroEngine`], [`DualEngine`].
#[async_trait]
pub trait SyncEngine: Send + Sync {
    /// Local device id. Surfaced on the trait so server code can hold an
    /// `Arc<dyn SyncEngine>` without reaching for a concrete engine —
    /// the dual-write wrapper needs this when fanning out, and several
    /// server routes use the device id for envelope addressing.
    fn device(&self) -> DeviceId;

    /// Local-side mutation entry point. `tesela-core` funnels every write
    /// here when sync is enabled. The engine appends an oplog row and
    /// returns the resulting content hash.
    async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash>;

    /// Apply incoming changes from a peer. Returns the set of canonical
    /// row identifiers that changed so callers can rebuild derived tables.
    async fn apply_changes(
        &self,
        peer: DeviceId,
        envelope: SyncEnvelope,
    ) -> SyncResult<AppliedChanges>;

    /// Produce ops authored locally with HLC strictly greater than
    /// `since`, up to `max_bytes` of postcard-encoded payload.
    async fn produce_changes_since(
        &self,
        peer: DeviceId,
        since: PeerCursor,
        max_bytes: usize,
    ) -> SyncResult<ProducedBatch>;

    /// Like [`produce_changes_since`] but filters to ops THIS device
    /// authored. Used by the WAN relay outbound tick where we must not
    /// re-publish ops we merely received from another peer (the relay
    /// fan-out would otherwise loop them back to senders).
    async fn produce_local_authored_since(
        &self,
        since: PeerCursor,
        max_bytes: usize,
    ) -> SyncResult<ProducedBatch>;

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

    /// Entries from the Loro index doc as `(note_id_hex, title, slug)`.
    /// The hybrid-model spine (cutover spec Phase 2). Default empty;
    /// LoroEngine/DualEngine override. Used by the `/loro/index` debug
    /// endpoint and, eventually, the note list + ref resolution.
    async fn index_entries(&self) -> Vec<(String, String, String)> {
        Vec::new()
    }
}
