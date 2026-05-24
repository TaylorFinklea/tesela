//! HTTP handlers. Stage 2a ships only the health probe; the spec'd
//! endpoint set lands across stages 3a–3d so each batch can land its
//! own conformance tests as it arrives.

use axum::extract::State;
use axum::Json;

use crate::state::AppState;

/// Lightweight liveness check. Operators wire this into their load
/// balancer / Docker healthcheck.
pub async fn health(State(_state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "tesela-relay" }))
}
