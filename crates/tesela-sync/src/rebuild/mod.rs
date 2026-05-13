//! Derived-table rebuild dispatcher.
//!
//! When [`crate::SyncEngine::apply_changes`] returns an
//! [`crate::AppliedChanges`], the caller passes it here to refresh derived
//! tables (`links`, `notes_fts`, `block_properties`, `tag_defs`,
//! `property_defs`) from canonical state.
//!
//! Phase 1 ships the dispatcher shape only. Concrete rebuilders are wired
//! once the `Mutation` API in `tesela-core` lands and the canonical tables
//! exist.

use crate::engine::applied::AppliedChanges;
use crate::error::SyncResult;

/// Trait implemented by `tesela-core` (or wherever derived-table parsers
/// live). The sync crate calls this after each apply.
#[async_trait::async_trait]
pub trait DerivedRebuild: Send + Sync {
    /// Rebuild derived tables for the given set of touched canonical rows.
    async fn rebuild(&self, changes: &AppliedChanges) -> SyncResult<()>;
}

/// No-op rebuilder. Useful for tests that exercise the engine without
/// caring about derived state. Production wiring substitutes a real
/// implementation provided by `tesela-core`.
pub struct NoopRebuild;

#[async_trait::async_trait]
impl DerivedRebuild for NoopRebuild {
    async fn rebuild(&self, _changes: &AppliedChanges) -> SyncResult<()> {
        Ok(())
    }
}
