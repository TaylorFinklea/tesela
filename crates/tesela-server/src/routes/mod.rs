mod agenda;
mod attachments;
mod calendar;
mod commands;
mod data_ops;
mod history;
mod keymap;
mod notes;
pub mod peer_sync;
mod relay;
mod search;
mod search_query;
mod sync;
mod tags;
mod transcription;
mod types;
mod views;
pub mod ws;

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

use crate::state::AppState;

const EXPECTED_GROUP_HEADER: &str = "x-tesela-expected-group";

pub fn build(state: AppState) -> Router {
    let state = Arc::new(state);
    let app = Router::new()
        .route("/health", get(health))
        .route("/info", get(info))
        .route("/attachments", post(attachments::post_attachment))
        .route("/attachments/{*path}", get(attachments::get_attachment))
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
        .route("/loro/notes/{id}/snapshot", get(notes::get_loro_snapshot))
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
        .route("/blocks/move-subtree", post(notes::move_block_subtree))
        .route("/blocks/recur-bump", post(notes::recur_bump))
        .route("/blocks/set-property", post(notes::set_block_property))
        .route("/blocks/clear-property", post(notes::clear_block_property))
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
        .route(
            "/sync/peer/peers/{device_id_hex}",
            axum::routing::delete(peer_sync::remove_peer),
        )
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
        // tesela-ra7 P0.3c — show-side recovery phrase for the web/desktop UI.
        .route("/sync/recovery-phrase", get(peer_sync::get_recovery_phrase))
        .route("/search", get(search::search_notes))
        .route("/agenda", post(agenda::post_agenda))
        .route("/search/query", post(search_query::execute))
        .route("/calendar/marks", get(calendar::marks))
        .route("/tags", get(tags::list_tags))
        // Saved-views registry (spec 2026-06-10) — thin wrappers over the
        // engine's synced views doc; WS `views_changed` fires on any write.
        .route("/views", get(views::list_views).post(views::create_view))
        .route("/views/reorder", post(views::reorder_views))
        .route(
            "/views/{id}",
            axum::routing::put(views::update_view).delete(views::delete_view),
        )
        .route("/types", get(types::list_types))
        .route("/types/{name}", get(types::get_type))
        .route("/types/{name}/nodes", get(types::list_typed_nodes))
        .route("/types/{name}/blocks", get(types::list_typed_blocks))
        .route("/properties", get(types::list_properties))
        // tesela-cmdd.2 — command manifest (id/verb/label/glyph/category/
        // shortcut/chord/surfaces/keywords/args-shape, no closures), embedded
        // from the checked-in web/src/lib/command-manifest.json.
        .route("/commands", get(commands::list_commands))
        // tesela-cmdd.4 — keybinding + leader-tree user config over stable
        // command ids (rebinds/hides/group-label overrides), server-
        // persisted like preferences so it survives reload on a second
        // browser hitting this same server.
        .route(
            "/keymap-config",
            get(keymap::get_keymap_config).put(keymap::put_keymap_config),
        )
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
        // Provable backups: last/next backup, contents manifest summary,
        // scheduler cadence. On-disk truth + in-process scheduler state.
        .route("/backup/status", get(data_ops::backup_status))
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
        // Dictation P2 — live streaming transcription session (binary
        // PCM in, committed/tentative partial frames out). Registered
        // after the body-limit layer like /ws; WS frames aren't bodies.
        .route("/transcription/stream", get(transcription::stream_ws));

    // Optional static-file serving for the desktop (Tauri) shell: when
    // TESELA_STATIC_DIR points at a built SvelteKit `/g` bundle, serve it
    // as a FALLBACK (after every API route), with an index.html SPA
    // fallback for client-side routes (`/g`, `/p/..`). Unset — the
    // standalone server + vite-dev case — keeps today's behavior (no
    // static serving). Because the embedded server then serves BOTH the
    // API and the UI on the same loopback origin, the desktop webview
    // needs no CORS handling and the existing same-origin WS URL works.
    let app = match std::env::var("TESELA_STATIC_DIR") {
        Ok(dir) if !dir.trim().is_empty() => {
            let dir = dir.trim().to_string();
            let index = std::path::PathBuf::from(&dir).join("index.html");
            app.fallback_service(
                tower_http::services::ServeDir::new(&dir)
                    .fallback(tower_http::services::ServeFile::new(index)),
            )
        }
        _ => app,
    };

    // CORS only for the standalone / dev server. The desktop embed serves API +
    // UI on ONE origin, so it needs no CORS — and permissive `*` on an
    // unauthenticated loopback API is the textbook DNS-rebinding target (any
    // site could read JSON from 127.0.0.1:<port>). `TESELA_STATIC_DIR` set ⇒
    // embedded ⇒ skip it.
    let app = if std::env::var_os("TESELA_STATIC_DIR").is_none() {
        app.layer(CorsLayer::permissive())
    } else {
        app
    };

    app
        // An attached client may bind any HTTP operation to the physical
        // group it verified over WebSocket. The guard holds the group's read
        // lock through the complete handler, making validation atomic with
        // the read or mutation instead of opening another check/use race.
        .layer(from_fn_with_state(Arc::clone(&state), expected_group_gate))
        // Request/response tracing for live sync-delivery visibility.
        // make_span at INFO emits method+uri; on_response at INFO emits
        // status+latency under that span — one INFO line per request at
        // our default RUST_LOG=info.
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state)
}

async fn expected_group_gate(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if state
        .group_transition_pending_restart
        .load(std::sync::atomic::Ordering::Acquire)
        && restart_pending_blocks(path)
    {
        return restart_pending_response();
    }

    // A request that replaces the group cannot also hold the current group's
    // read lease through its handler: that would self-deadlock when the route
    // acquires the write lock. Group selection/pairing is intentionally an
    // unbound control-plane operation; attached data-plane calls are bound.
    let replaces_group = request.method() == Method::POST && matches!(path, "/sync/peer/pair-code");
    if replaces_group {
        if request.headers().contains_key(EXPECTED_GROUP_HEADER) {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "group_replacement_cannot_be_bound",
                })),
            )
                .into_response();
        }
        return next.run(request).await;
    }

    let expected = match request.headers().get(EXPECTED_GROUP_HEADER) {
        Some(raw_expected) => {
            let Ok(raw_expected) = raw_expected.to_str() else {
                return invalid_expected_group("expected group header is not UTF-8");
            };
            let normalized = raw_expected.trim().to_ascii_lowercase();
            let Ok(bytes) = hex::decode(&normalized) else {
                return invalid_expected_group("expected group header is not hex");
            };
            if bytes.len() != 16 {
                return invalid_expected_group("expected group header must encode 16 bytes");
            }
            Some((normalized, bytes))
        }
        None => None,
    };

    // Every data-plane handler takes a read lease, including legacy/web calls
    // without an identity header. The header adds an exact-group comparison;
    // it is not what creates transition serialization. Pairing holds the
    // matching write lease while it durably adopts and publishes a new group.
    let identity = Arc::clone(&state.group_identity).read_owned().await;
    if state
        .group_transition_pending_restart
        .load(std::sync::atomic::Ordering::Acquire)
        && restart_pending_blocks(path)
    {
        return restart_pending_response();
    }
    if let Some((normalized_expected, expected_bytes)) = expected {
        if expected_bytes.as_slice() != identity.group_id.as_bytes() {
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "mosaic_group_mismatch",
                    "expected_group_id_hex": normalized_expected,
                    "current_group_id_hex": hex::encode(identity.group_id.as_bytes()),
                })),
            )
                .into_response();
        }
    }

    let response = next.run(request).await;
    drop(identity);
    response
}

fn restart_pending_blocks(path: &str) -> bool {
    !matches!(path, "/health" | "/mosaics/current" | "/server/restart")
}

fn restart_pending_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "group_transition_pending_restart",
            "restart_required": true,
        })),
    )
        .into_response()
}

fn invalid_expected_group(message: &'static str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "invalid_expected_group",
            "message": message,
        })),
    )
        .into_response()
}

async fn health() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

/// GET /info — read-only device metadata for the web client. Returns the
/// canonical display name (`device_display_name()`: TESELA_DEVICE_NAME env →
/// hostname → "Tesela device") so remote-cursor presence frames can label this
/// device on every peer. Unauthenticated, like `/health`.
async fn info() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({ "device_name": crate::device_display_name() })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_restart_gate_allows_only_observation_and_restart_controls() {
        for path in ["/health", "/mosaics/current", "/server/restart"] {
            assert!(
                !restart_pending_blocks(path),
                "control path blocked: {path}"
            );
        }
        for path in [
            "/notes",
            "/loro/notes/daily/snapshot",
            "/sync/peer/pair-code",
            "/mosaics/switch",
            "/ws",
        ] {
            assert!(restart_pending_blocks(path), "data path escaped: {path}");
        }
    }
}
