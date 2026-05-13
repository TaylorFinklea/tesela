//! Oplog retention: GC ops once all known peers have ack'd past them.
//!
//! Phase 1 stub. The retention sweep runs as a method on the engine
//! (`SyncEngine::run_retention_sweep`) once the engine implementation
//! exists; this module holds the policy constants.

/// Safety lag applied to retention. Even after all peers ack, ops within
/// this window of wall-clock past are retained so a peer that briefly
/// dropped offline can still ack.
///
/// Default: 24 hours of wall-clock equivalent.
pub const DEFAULT_RETENTION_SAFETY_LAG_MILLIS: i64 = 24 * 60 * 60 * 1000;
