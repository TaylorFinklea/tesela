mod agenda;
mod calendar;
mod data_ops;
mod history;
mod notes;
pub mod peer_sync;
mod relay;
mod search;
mod search_query;
mod sync;
mod tags;
mod transcription;
mod types;
pub mod ws;

use std::sync::Arc;

use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::state::AppState;

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/notes", get(notes::list_notes).post(notes::create_note))
        .route("/notes/daily", get(notes::get_daily_note))
        .route(
            "/notes/{id}",
            get(notes::get_note)
                .put(notes::update_note)
                .delete(notes::delete_note),
        )
        .route("/notes/{id}/blocks", post(notes::upsert_blocks))
        .route(
            "/notes/{id}/blocks/{bid}",
            axum::routing::delete(notes::delete_block),
        )
        .route("/loro/index", get(notes::get_loro_index))
        .route(
            "/loro/notes/{id}/snapshot",
            get(notes::get_loro_snapshot),
        )
        .route("/notes/{id}/backlinks", get(notes::get_backlinks))
        .route("/notes/{id}/links", get(notes::get_forward_links))
        .route("/notes/{id}/unlinked", get(notes::get_unlinked))
        .route("/tags/rename", post(notes::rename_tag))
        .route("/tags/resolve", post(notes::resolve_tag))
        .route("/tags/{slug}/usage", get(notes::get_tag_usage))
        .route(
            "/tags/{slug}/cleanup-references",
            post(notes::cleanup_tag_references),
        )
        .route("/notes/{id}/versions", get(history::list_versions))
        .route(
            "/notes/{id}/versions/{version_id}",
            get(history::get_version),
        )
        .route("/links", get(notes::get_all_edges))
        .route("/blocks/recur-bump", post(notes::recur_bump))
        .route("/blocks/set-property", post(notes::set_block_property))
        .route("/sync/reminders/push", post(sync::push))
        .route("/sync/reminders/pull", post(sync::pull))
        .route("/sync/reminders", post(sync::sync))
        .route("/sync/reminders/status", get(sync::status))
        // Phase 1.5 multi-device sync
        .route("/sync/peer/device", get(peer_sync::get_device))
        .route(
            "/sync/peer/peers",
            get(peer_sync::list_peers).post(peer_sync::add_peer),
        )
        .route("/sync/peer/peers/{device_id_hex}", axum::routing::delete(peer_sync::remove_peer))
        .route("/sync/peer/produce", post(peer_sync::produce))
        .route("/sync/peer/envelope", post(peer_sync::receive_envelope))
        .route("/sync/peer/now", post(peer_sync::sync_now))
        .route("/sync/peer/status", get(peer_sync::status))
        // WAN relay status — drives the web settings page.
        .route("/sync/relay/status", get(relay::status))
        .route(
            "/sync/relay/config",
            get(relay::get_config)
                .put(relay::put_config)
                .delete(relay::delete_config),
        )
        // Phase 2.1 mDNS LAN discovery
        .route("/sync/peer/discovered", get(peer_sync::discovered))
        // Phase 2.2 pairing-code key exchange
        .route("/sync/peer/pairing-code", get(peer_sync::get_pairing_code))
        .route("/sync/peer/pair-code", post(peer_sync::pair_with_code))
        // Phase 2.5 — 6-character short-code lookup (in-memory TTL'd map).
        // The joining device types the short code shown under the QR;
        // GET resolves it to the long base64url code, then the existing
        // pair_with_code path takes over.
        .route(
            "/sync/peer/short-code/{code}",
            get(peer_sync::lookup_pairing_short_code),
        )
        .route("/search", get(search::search_notes))
        .route("/agenda", post(agenda::post_agenda))
        .route("/search/query", post(search_query::execute))
        .route("/calendar/marks", get(calendar::marks))
        .route("/tags", get(tags::list_tags))
        .route("/types", get(types::list_types))
        .route("/types/{name}", get(types::get_type))
        .route("/types/{name}/nodes", get(types::list_typed_nodes))
        .route("/types/{name}/blocks", get(types::list_typed_blocks))
        .route("/properties", get(types::list_properties))
        // Phase 13 — backup/export/import management (drives the web Settings UI)
        .route(
            "/backups",
            get(data_ops::list_backups).post(data_ops::run_backup),
        )
        .route("/backups/{name}/verify", post(data_ops::verify_backup))
        .route("/backups/{name}/restore", post(data_ops::restore_backup))
        .route("/backups/prune", post(data_ops::prune_backups))
        .route("/backups/keygen", post(data_ops::keygen))
        .route("/backups/key-status", get(data_ops::key_status))
        .route(
            "/backup-config",
            get(data_ops::get_backup_config).put(data_ops::put_backup_config),
        )
        .route("/export", post(data_ops::run_export))
        .route("/imports/obsidian", post(data_ops::import_obsidian))
        .route("/imports/logseq", post(data_ops::import_logseq))
        .route("/imports/logseq/plan", post(data_ops::plan_logseq))
        .route("/imports/logseq/apply", post(data_ops::apply_logseq))
        .route("/imports/org", post(data_ops::import_org))
        .route("/pick-folder", post(data_ops::pick_folder))
        .route("/mosaics/current", get(data_ops::get_current_mosaic))
        .route(
            "/mosaics/discovered",
            get(data_ops::list_discovered_mosaics),
        )
        .route("/mosaics", post(data_ops::create_mosaic))
        .route("/mosaics/switch", post(data_ops::switch_mosaic))
        .route("/server/restart", post(data_ops::restart_server))
        // Phase 25 — transcription model management
        .route("/transcription/models", get(transcription::list_models))
        .route(
            "/transcription/models/{id}/download",
            post(transcription::download_model),
        )
        .route(
            "/transcription/models/{id}",
            axum::routing::delete(transcription::delete_model),
        )
        .route(
            "/transcription/models/{id}/activate",
            post(transcription::activate_model),
        )
        .route("/transcription/active", get(transcription::get_active))
        .route("/transcription/transcribe", post(transcription::transcribe))
        .layer(axum::extract::DefaultBodyLimit::max(200 * 1024 * 1024))
        .route("/ws", get(ws::ws_handler))
        .layer(CorsLayer::permissive())
        // Request/response tracing for live sync-delivery visibility.
        // make_span at INFO emits method+uri; on_response at INFO emits
        // status+latency under that span — one INFO line per request at
        // our default RUST_LOG=info.
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(Arc::new(state))
}

async fn health() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}
