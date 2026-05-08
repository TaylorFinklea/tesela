//! Phase 12.1 — Apple Reminders sync endpoints.
//!
//! Web client never talks to EventKit directly. This route is the single
//! choke point for triggering a push (and, in slice 2, a pull). Returns
//! a structured outcome so the UI can show created/updated/error counts.

use std::sync::Arc;

use axum::{extract::State, Json};

use crate::error::AppResult;
use crate::reminders::{self, PushOutcome};
use crate::state::AppState;
use tesela_core::traits::note_store::NoteStore;

pub async fn push(State(state): State<Arc<AppState>>) -> AppResult<Json<PushOutcome>> {
    let store: Arc<dyn NoteStore> = Arc::clone(&state.store) as Arc<dyn NoteStore>;
    let outcome = reminders::push_all(store).await?;
    Ok(Json(outcome))
}
