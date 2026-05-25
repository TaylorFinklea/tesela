//! Routes related to the WAN sync relay. Currently just a status
//! endpoint the web settings page reads to show "connected to relay
//! X, last fetch N seconds ago, K envelopes pending, last error if
//! any."

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::state::AppState;
use crate::sync_relay::RelayStatus;

/// `GET /sync/relay/status`.
///
/// Returns the current relay state — `configured: false` when no
/// `[sync.relay]` block is set; otherwise the URL, cursors,
/// timestamps, and most recent error string (if any). The web
/// settings page polls this to render the sync surface live.
pub async fn status(State(s): State<Arc<AppState>>) -> Json<RelayStatus> {
    let Some(handle) = s.relay.as_ref() else {
        return Json(RelayStatus::disabled());
    };
    let state = handle.state.read().await;
    Json(RelayStatus::from_handle(handle, &state))
}
