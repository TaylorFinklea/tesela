//! Phase 12.1 slice 3.4 — auto-sync triggers.
//!
//! **Default OFF since 2026-06-09** (audit A10 / product decision #3):
//! the triggers below only arm when `TESELA_REMINDERS_AUTOSYNC` is set
//! to a non-empty value. The audit found every sync rewrites each
//! candidate's `apple_reminder_synced_at::` (fresh `Utc::now()` even
//! when nothing changed), the fs-watcher emits `Updated` for those
//! writes, and the edit-debounce trigger fires another sync ~30s later
//! — a permanent self-retrigger loop, plus per-cycle EventKit
//! saveReminder commits (iCloud churn) and pull-side fail-open clobber
//! risk. The manual "Sync now" route (`POST /sync/reminders`) stays
//! fully functional; it calls [`AutoSync::run_once`] directly and never
//! touches this gate.
//!
//! When armed, three triggers fire `sync_all` automatically:
//!
//! 1. **Startup**: 10 seconds after the server starts (delay so the
//!    initial index has settled).
//! 2. **Interval**: every 5 minutes after that — this is what catches
//!    "user ticked something on iPhone" without needing an EventKit
//!    change-notification observer (which needs a CFRunLoop, see the
//!    deferred work note in `mod.rs`).
//! 3. **Edit-driven**: subscribes to the indexer's `NoteEvent` stream
//!    and triggers a sync 30 seconds after the last edit. Debounced
//!    so a flurry of edits during a typing session collapses to one
//!    sync at the end.
//!
//! All three triggers serialize through a single `Mutex` so EventKit
//! never sees overlapping calls. Each run records its outcome in
//! `LastSync` for the Settings UI to display.
//!
//! On non-macOS platforms `start_triggers` is a no-op — `sync_all`
//! itself returns "platform unsupported" there, and we don't want the
//! triggers polluting `LastSync.error` every 5 minutes with the same
//! message.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, info, warn};

use tesela_core::indexer::NoteEvent;
use tesela_core::traits::note_store::NoteStore;

use crate::reminders::{self, SyncOutcome};

/// What `GET /sync/reminders/status` returns. The Settings UI uses
/// this to show "last synced 12 minutes ago via interval" plus the
/// last outcome counts.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LastSync {
    pub at: Option<DateTime<Utc>>,
    /// One of: `startup`, `interval`, `edit`, `manual`. `None` before
    /// the first sync ever runs.
    pub trigger: Option<String>,
    pub outcome: Option<SyncOutcome>,
    /// Set when the last sync errored. Cleared on the next successful
    /// run.
    pub error: Option<String>,
}

pub struct AutoSync {
    last: Mutex<LastSync>,
    /// Held for the lifetime of a single `sync_all` invocation. Other
    /// triggers acquire it before running, so an in-progress sync
    /// blocks the next trigger rather than racing it. EventKit isn't
    /// reentrant on a single `EKEventStore`.
    in_flight: Mutex<()>,
}

impl AutoSync {
    pub fn new() -> Self {
        Self {
            last: Mutex::new(LastSync::default()),
            in_flight: Mutex::new(()),
        }
    }

    /// Run a sync, recording the outcome under the given trigger label.
    /// Used by both auto-triggers and the manual `/sync/reminders`
    /// route so the Settings UI sees a unified last-sync record.
    pub async fn run_once(
        &self,
        store: Arc<dyn NoteStore>,
        trigger: &str,
    ) -> anyhow::Result<SyncOutcome> {
        let _guard = self.in_flight.lock().await;
        let result = reminders::sync_all(store).await;
        let mut last = self.last.lock().await;
        last.at = Some(Utc::now());
        last.trigger = Some(trigger.to_string());
        match &result {
            Ok(o) => {
                last.outcome = Some(o.clone());
                last.error = None;
            }
            Err(e) => {
                last.outcome = None;
                last.error = Some(e.to_string());
            }
        }
        result
    }

    pub async fn snapshot(&self) -> LastSync {
        self.last.lock().await.clone()
    }
}

/// Opt-in env flag for the AUTOMATIC sync triggers. Mirrors the other
/// boolean env toggles in `main.rs` (`TESELA_LORO_RESEED` shape): any
/// non-empty value enables; unset/empty disables. Default OFF per the
/// 2026-06-09 decision — see the module docs for why.
#[cfg(any(target_os = "macos", test))]
pub const AUTOSYNC_ENV: &str = "TESELA_REMINDERS_AUTOSYNC";

/// Pure flag predicate, split out so the default-OFF contract is unit-
/// testable without env mutation.
#[cfg(any(target_os = "macos", test))]
fn env_flag_enabled(value: Option<&str>) -> bool {
    matches!(value, Some(v) if !v.is_empty())
}

#[cfg(target_os = "macos")]
fn autosync_enabled() -> bool {
    env_flag_enabled(std::env::var(AUTOSYNC_ENV).ok().as_deref())
}

/// Arm the automatic triggers. Returns whether they actually armed —
/// `false` when the `TESELA_REMINDERS_AUTOSYNC` opt-in is absent (the
/// default) or on non-macOS.
#[cfg(target_os = "macos")]
pub fn start_triggers(
    auto: Arc<AutoSync>,
    store: Arc<dyn NoteStore>,
    note_events: broadcast::Sender<NoteEvent>,
) -> bool {
    if !autosync_enabled() {
        info!(
            "reminders auto-sync: triggers disabled (default; set \
             {AUTOSYNC_ENV}=1 to enable). Manual sync via POST \
             /sync/reminders still works."
        );
        return false;
    }
    let startup_delay = Duration::from_secs(10);
    let interval_period = Duration::from_secs(300);
    let edit_debounce = Duration::from_secs(30);

    // (1) Startup
    let auto_s = auto.clone();
    let store_s = store.clone();
    tokio::spawn(async move {
        tokio::time::sleep(startup_delay).await;
        info!("reminders auto-sync: startup trigger");
        if let Err(e) = auto_s.run_once(store_s, "startup").await {
            warn!("auto-sync startup failed: {e}");
        }
    });

    // (2) Periodic interval
    let auto_i = auto.clone();
    let store_i = store.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval_period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // discard the immediate first tick
        loop {
            ticker.tick().await;
            debug!("reminders auto-sync: interval trigger");
            if let Err(e) = auto_i.run_once(store_i.clone(), "interval").await {
                warn!("auto-sync interval failed: {e}");
            }
        }
    });

    // (3) Edit-driven (debounced)
    //
    // KNOWN LOOP when armed (audit A10, 2026-06-09): `push_all` stamps a
    // fresh `apple_reminder_synced_at::` on every pushed candidate and
    // writes the note files via `store.update`, the Indexer watcher emits
    // `Updated` for those self-originated writes (no suppression), and
    // this trigger fires another sync ~30s later — forever, on any mosaic
    // with ≥1 reminder-bearing task. The Milestone 3 fix is re-routing the
    // reminders writebacks through the sync engine (`record_local`, like
    // every other writer) with change-detection so a no-op sync writes
    // nothing and self-originated events are attributable; until then the
    // default-OFF gate above is what contains the loop. Do NOT re-enable
    // by default without that re-route.
    let auto_e = auto;
    let store_e = store;
    let mut rx = note_events.subscribe();
    tokio::spawn(async move {
        loop {
            // Wait for the first event in a quiet window.
            let ev = match rx.recv().await {
                Ok(e) => e,
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            };
            if !is_relevant(&ev) {
                continue;
            }

            // Drain further events until we go `edit_debounce` quiet.
            // Each new event resets the timer.
            loop {
                match tokio::time::timeout(edit_debounce, rx.recv()).await {
                    Ok(Ok(next)) => {
                        if !is_relevant(&next) {
                            // ignored events don't reset the timer
                        }
                    }
                    Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                        // pretend it was a relevant event; safer to
                        // re-debounce than to miss a sync window
                    }
                    Ok(Err(broadcast::error::RecvError::Closed)) => return,
                    Err(_) => {
                        // Timed out → quiet for `edit_debounce`. Fire.
                        debug!("reminders auto-sync: edit trigger");
                        if let Err(e) = auto_e.run_once(store_e.clone(), "edit").await {
                            warn!("auto-sync edit-driven failed: {e}");
                        }
                        break;
                    }
                }
            }
        }
    });
    true
}

#[cfg(not(target_os = "macos"))]
pub fn start_triggers(
    _auto: Arc<AutoSync>,
    _store: Arc<dyn NoteStore>,
    _note_events: broadcast::Sender<NoteEvent>,
) -> bool {
    // sync_all returns "platform unsupported" on non-macOS; firing
    // auto-triggers would just record that error every 5 minutes.
    false
}

/// Heuristic: skip the sync if the event is for a note that obviously
/// can't affect Reminders (e.g. a Tag page edit). The simple check is
/// "did the body change in a way that could reach a Task block." We
/// punt on a precise check — the cost of an unnecessary sync is one
/// EventKit fetch + push that completes in milliseconds when nothing
/// has changed. The benefit of skipping (e.g. on Tag edits) is small.
/// Trigger on Created/Updated; ignore Deleted (a deleted note can't
/// have a syncable Task block).
fn is_relevant(ev: &NoteEvent) -> bool {
    matches!(ev, NoteEvent::Created(_) | NoteEvent::Updated(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Audit A10 (decision 2026-06-09 #3): the automatic triggers must
    /// default OFF. Without the opt-in env flag, `start_triggers` arms
    /// nothing — no startup sync, no 5-minute interval, no edit-debounce
    /// loop (whose self-retrigger ran a sync every ~30s forever). The
    /// manual `POST /sync/reminders` route is unaffected (it calls
    /// `AutoSync::run_once` directly).
    #[tokio::test]
    async fn triggers_do_not_arm_when_flag_unset() {
        std::env::remove_var(AUTOSYNC_ENV);
        let auto = Arc::new(AutoSync::new());
        let store: Arc<dyn NoteStore> = Arc::new(
            tesela_core::storage::filesystem::FsNoteStore::new(
                std::path::PathBuf::from("/nonexistent-test-mosaic"),
                tesela_core::config::StorageConfig::default(),
            ),
        );
        let (tx, _) = broadcast::channel::<NoteEvent>(4);
        let armed = start_triggers(auto, store, tx);
        assert!(
            !armed,
            "auto-sync triggers must NOT arm without TESELA_REMINDERS_AUTOSYNC"
        );
    }

    /// The opt-in flag parsing: unset and empty are OFF; any non-empty
    /// value is ON.
    #[test]
    fn autosync_env_gate_parsing() {
        assert!(!env_flag_enabled(None), "unset → off (the default)");
        assert!(!env_flag_enabled(Some("")), "empty → off");
        assert!(env_flag_enabled(Some("1")));
        assert!(env_flag_enabled(Some("true")));
    }
}
