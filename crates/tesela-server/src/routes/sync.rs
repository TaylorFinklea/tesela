//! Phase 12.1 — Apple Reminders sync endpoints.
//!
//! Web client never talks to EventKit directly. These routes are the
//! single choke point for triggering pushes, pulls, and the combined
//! sync. Each returns a structured outcome so the UI can show
//! created/updated/error counts.

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::error::AppResult;
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
/// chance to clobber them.
pub async fn sync(State(state): State<Arc<AppState>>) -> AppResult<Json<SyncOutcome>> {
    let store: Arc<dyn NoteStore> = Arc::clone(&state.store) as Arc<dyn NoteStore>;
    let outcome = reminders::sync_all(store).await?;
    Ok(Json(outcome))
}
