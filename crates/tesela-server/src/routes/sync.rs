//! Phase 12.1 — Apple Reminders sync endpoints.
//!
//! Web client never talks to EventKit directly. These routes are the
//! single choke point for triggering pushes, pulls, and the combined
//! sync. Each returns a structured outcome so the UI can show
//! created/updated/error counts.

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::error::AppResult;
use crate::reminders::auto::LastSync;
use crate::reminders::{self, PullOutcome, PushOutcome, SyncOutcome};
use crate::state::AppState;
use tesela_core::traits::note_store::NoteStore;

pub async fn push(State(state): State<Arc<AppState>>) -> AppResult<Json<PushOutcome>> {
    let store: Arc<dyn NoteStore> = Arc::clone(&state.store) as Arc<dyn NoteStore>;
    let outcome = reminders::push_all(store).await?;
    Ok(Json(outcome))
}

pub async fn pull(State(state): State<Arc<AppState>>) -> AppResult<Json<PullOutcome>> {
    let store: Arc<dyn NoteStore> = Arc::clone(&state.store) as Arc<dyn NoteStore>;
    let outcome = reminders::pull_all(store).await?;
    Ok(Json(outcome))
}

/// Combined pull-then-push. The "Sync now" UI button hits this so
/// external Reminders edits flow back into Tesela before any push has a
/// chance to clobber them. Routed through `AutoSync` so the manual
/// trigger updates the same `LastSync` that the auto-triggers populate
/// — the Settings UI then shows a unified "last synced" line whether
/// the sync was kicked off by the button, an interval tick, or an
/// edit-debounce.
pub async fn sync(State(state): State<Arc<AppState>>) -> AppResult<Json<SyncOutcome>> {
    let store: Arc<dyn NoteStore> = Arc::clone(&state.store) as Arc<dyn NoteStore>;
    let outcome = state.auto_sync.run_once(store, "manual").await?;
    Ok(Json(outcome))
}

/// Returns the most recent sync state — when it ran, what triggered
/// it, and the outcome (or error). The Settings UI polls/reads this
/// to render "Synced 3 minutes ago via interval · 2 updated" etc.
pub async fn status(State(state): State<Arc<AppState>>) -> Json<LastSync> {
    Json(state.auto_sync.snapshot().await)
}
