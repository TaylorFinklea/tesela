mod notes;
mod search;
mod tags;
mod ws;

use std::sync::Arc;

use axum::{http::StatusCode, routing::get, Json, Router};
use serde_json::json;
use tower_http::cors::CorsLayer;

use crate::state::AppState;

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/notes", get(notes::list_notes).post(notes::create_note))
        .route("/notes/daily", get(notes::get_daily_note))
        .route(
            "/notes/:id",
            get(notes::get_note)
                .put(notes::update_note)
                .delete(notes::delete_note),
        )
        .route("/notes/:id/backlinks", get(notes::get_backlinks))
        .route("/notes/:id/links", get(notes::get_forward_links))
        .route("/search", get(search::search_notes))
        .route("/tags", get(tags::list_tags))
        .route("/ws", get(ws::ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(state))
}

async fn health() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}
