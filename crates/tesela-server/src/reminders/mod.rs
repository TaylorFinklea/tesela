//! Phase 12.1 — Apple Reminders bidirectional sync.
//!
//! Architecture:
//!   * Web client never talks to EventKit directly — all interaction goes
//!     through this server module.
//!   * EventKit is macOS-only, so the real implementation lives behind
//!     `#[cfg(target_os = "macos")]`. Other platforms compile a stub
//!     module whose entry points return a "platform unsupported" error.
//!   * v1 was push-only. Slice 2 (this) adds pull, plus a combined
//!     `sync_all` that does pull-then-push so external edits don't get
//!     clobbered by an immediate push.
//!
//! Identity model:
//!   * Each Tesela Task block that has a `deadline::` (or `scheduled::`)
//!     property is eligible for sync.
//!   * On first push we create an `EKReminder` and store its
//!     `calendarItemIdentifier` on the block as `apple_reminder_id::`.
//!   * Subsequent pushes look the EKReminder up by that identifier and
//!     update fields in place. Missing identifier → recreate.
//!
//! Conflict resolution (slice 2):
//!   * Each successful sync writes `apple_reminder_synced_at::` on the
//!     block (RFC 3339 UTC).
//!   * On pull, if `EKReminder.lastModifiedDate > synced_at` the user
//!     edited it in Reminders.app since our last sync — pull wins.
//!     Otherwise the EK side hasn't changed since we last agreed, so we
//!     skip the diff (Tesela's value stays).
//!   * The combined `sync_all` pulls first, then pushes. This ordering
//!     means:
//!       - User-only-Tesela edits: pull no-ops (lastModified unchanged),
//!         then push writes Tesela → EK.
//!       - User-only-Reminders edits: pull writes EK → Tesela, then
//!         push is a near no-op (values agree again).
//!       - Concurrent edits: EK wins per field. Documented limitation.
//!
//! Property mapping (v2):
//!   * Block text                  ↔ `EKReminder.title`
//!   * `status:: done`             ↔ `EKReminder.completed`
//!   * `deadline:: [[YYYY-MM-DD]]` ↔ `EKReminder.dueDateComponents`
//!     (date-only; time-of-day round-trip is slice 3).
//!   * `priority:: high|medium|low` ↔ `EKReminder.priority` (1, 5, 9)
//!     EventKit uses `0` for none. Pull maps 1-4 → high, 5 → medium,
//!     6-9 → low so any priority round-trips losslessly through the
//!     three-bucket Tesela model.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PushOutcome {
    /// Block ids that were created in Reminders for the first time.
    pub created: Vec<String>,
    /// Block ids whose Reminders item was updated in place.
    pub updated: Vec<String>,
    /// Block ids the sync touched. Same as created ∪ updated when the
    /// sync ran cleanly.
    pub synced: Vec<String>,
    /// Per-block error messages — non-fatal failures so partial progress
    /// is still recorded.
    pub errors: Vec<PushError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushError {
    pub block_id: String,
    pub message: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PullOutcome {
    /// Block ids whose properties were updated by the pull.
    pub updated: Vec<String>,
    /// EKReminder ids that have no matching Tesela block. They were
    /// either created in Reminders.app directly or had their
    /// `apple_reminder_id::` link severed. v2 leaves these alone — a
    /// future "import as task" feature can decide what to do with them.
    pub orphans: Vec<String>,
    /// Per-reminder error messages — non-fatal so partial progress is
    /// still recorded.
    pub errors: Vec<PullError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullError {
    pub reminder_id: String,
    pub message: String,
}

/// Combined pull + push outcome returned by the unified `/sync/reminders`
/// endpoint that the "Sync now" button hits.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SyncOutcome {
    pub pull: PullOutcome,
    pub push: PushOutcome,
}

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::{pull_all, push_all, sync_all};

pub mod auto;

#[cfg(not(target_os = "macos"))]
pub async fn push_all(
    _store: std::sync::Arc<dyn tesela_core::traits::note_store::NoteStore>,
) -> anyhow::Result<PushOutcome> {
    anyhow::bail!("Apple Reminders sync is only available on macOS")
}

#[cfg(not(target_os = "macos"))]
pub async fn pull_all(
    _store: std::sync::Arc<dyn tesela_core::traits::note_store::NoteStore>,
) -> anyhow::Result<PullOutcome> {
    anyhow::bail!("Apple Reminders sync is only available on macOS")
}

#[cfg(not(target_os = "macos"))]
pub async fn sync_all(
    _store: std::sync::Arc<dyn tesela_core::traits::note_store::NoteStore>,
) -> anyhow::Result<SyncOutcome> {
    anyhow::bail!("Apple Reminders sync is only available on macOS")
}
