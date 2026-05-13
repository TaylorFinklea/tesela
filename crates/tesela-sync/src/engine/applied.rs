//! Records of which canonical rows changed during an apply pass.

use serde::{Deserialize, Serialize};

/// Set of canonical row identifiers that changed as a result of applying
/// a batch of ops. The caller uses this to drive derived-table rebuild
/// and UI invalidation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedChanges {
    /// Notes that were created, updated, or deleted.
    pub note_ids: Vec<[u8; 16]>,
    /// Blocks that were created, updated, moved, or deleted.
    pub block_ids: Vec<[u8; 16]>,
    /// Attachments that were created, updated, or deleted. Phase 2.
    pub attachment_ids: Vec<[u8; 16]>,
    /// Number of ops in the batch that were duplicates (skipped).
    pub deduped: u32,
    /// Number of ops that parked (newer schema, no translator chain).
    pub parked: u32,
    /// Number of ops that applied successfully.
    pub applied: u32,
}

impl AppliedChanges {
    /// Merge another `AppliedChanges` into this one (idempotent, sorted-dedup).
    pub fn extend(&mut self, other: AppliedChanges) {
        self.note_ids.extend(other.note_ids);
        self.note_ids.sort();
        self.note_ids.dedup();
        self.block_ids.extend(other.block_ids);
        self.block_ids.sort();
        self.block_ids.dedup();
        self.attachment_ids.extend(other.attachment_ids);
        self.attachment_ids.sort();
        self.attachment_ids.dedup();
        self.deduped += other.deduped;
        self.parked += other.parked;
        self.applied += other.applied;
    }
}
