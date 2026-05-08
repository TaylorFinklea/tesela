//! Phase 12.1 — Apple Reminders bidirectional sync.
//!
//! Architecture:
//!   * Web client never talks to EventKit directly — all interaction goes
//!     through this server module.
//!   * EventKit is macOS-only, so the real implementation lives behind
//!     `#[cfg(target_os = "macos")]`. Other platforms compile a stub
//!     module whose `push_all` returns a "platform unsupported" error.
//!   * v1 is push-only (Tesela → Reminders). Pull is Slice 2.
//!
//! Identity model:
//!   * Each Tesela Task block that has a `deadline::` (or `scheduled::`)
//!     property is eligible for sync.
//!   * On first push we create an `EKReminder` and store its
//!     `calendarItemIdentifier` on the block as `apple_reminder_id::`.
//!   * Subsequent pushes look the EKReminder up by that identifier and
//!     update fields in place. Missing identifier → recreate.
//!
//! Property mapping (v1):
//!   * Block text                  → `EKReminder.title`
//!   * `status:: done`             → `EKReminder.completed = true`
//!   * `deadline:: [[YYYY-MM-DD]]` → `EKReminder.dueDateComponents`
//!     (date-only; time-of-day round-trip is Slice 2).
//!   * `priority:: high|medium|low` → `EKReminder.priority` (1, 5, 9)
//!     EventKit uses `0` for none.

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

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "macos")]
pub use darwin::push_all;

#[cfg(not(target_os = "macos"))]
pub async fn push_all(_store_dir: &std::path::Path) -> anyhow::Result<PushOutcome> {
    anyhow::bail!("Apple Reminders sync is only available on macOS")
}
