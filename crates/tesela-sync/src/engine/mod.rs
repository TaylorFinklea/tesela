//! The `SyncEngine` trait and supporting types.

pub mod applied;
pub mod cursor;
pub mod sqlite_engine;

pub use applied::AppliedChanges;
pub use cursor::{LocalCursor, PeerCursor};
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

/// The core sync engine trait. Implementations: [`SqliteEngine`].
#[async_trait]
pub trait SyncEngine: Send + Sync {
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
}
