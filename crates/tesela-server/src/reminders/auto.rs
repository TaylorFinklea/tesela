//! Phase 12.1 slice 3.4 — auto-sync triggers.
//!
//! The "Sync now" button is fine but the daily-driver win is making the
//! user not have to think about it. Three triggers fire `sync_all`
//! automatically:
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

#[cfg(target_os = "macos")]
pub fn start_triggers(
    auto: Arc<AutoSync>,
    store: Arc<dyn NoteStore>,
    note_events: broadcast::Sender<NoteEvent>,
) {
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
}

#[cfg(not(target_os = "macos"))]
pub fn start_triggers(
    _auto: Arc<AutoSync>,
    _store: Arc<dyn NoteStore>,
    _note_events: broadcast::Sender<NoteEvent>,
) {
    // sync_all returns "platform unsupported" on non-macOS; firing
    // auto-triggers would just record that error every 5 minutes.
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
