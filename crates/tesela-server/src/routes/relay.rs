//! Routes related to the WAN sync relay.
//!
//! - `GET /sync/relay/status`  — live status the web settings page polls.
//! - `GET /sync/relay/config`  — what's currently in the mosaic config.
//! - `PUT /sync/relay/config`  — write a new `[sync.relay]` block to
//!   the mosaic's `config.toml`. Takes effect on next server boot —
//!   the response carries `restart_required: true` so the UI can offer
//!   a one-click `/server/restart` after save.
//! - `DELETE /sync/relay/config` — remove the block (returns to
//!   LAN-only sync on next boot).

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use tesela_core::config::{Config, RelayConfig};

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

/// Wire shape for `GET`/`PUT /sync/relay/config`. Mirrors
/// [`RelayConfig`] but uses `Option` so `PUT` callers can omit fields
/// to fall back to defaults, and `GET` can return "no relay configured"
/// without a separate response shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfigDto {
    /// Base URL of the relay (scheme + host + port). `None` on GET
    /// means no `[sync.relay]` block is present.
    #[serde(default)]
    pub url: Option<String>,
    /// Poll interval in milliseconds. Defaults to 5000 on PUT when
    /// omitted; GET always returns the effective value.
    #[serde(default)]
    pub poll_interval_ms: Option<u64>,
}

/// PUT response — echoes the saved config plus a hint that the server
/// must be restarted before the new relay handle is brought up.
#[derive(Debug, Serialize)]
pub struct RelayConfigPutResponse {
    pub url: Option<String>,
    pub poll_interval_ms: Option<u64>,
    /// Always `true` — the relay handle is established at boot, so a
    /// config change requires a restart to take effect. The UI uses
    /// this to surface a one-click "Restart server" affordance.
    pub restart_required: bool,
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RelayConfigDto>, (StatusCode, String)> {
    let path = state.mosaic_root.join(".tesela").join("config.toml");
    let cfg = load_or_default(&path)?;
    Ok(Json(match cfg.sync.relay {
        Some(r) => RelayConfigDto {
            url: Some(r.url),
            poll_interval_ms: Some(r.poll_interval_ms),
        },
        None => RelayConfigDto {
            url: None,
            poll_interval_ms: None,
        },
    }))
}

pub async fn put_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RelayConfigDto>,
) -> Result<Json<RelayConfigPutResponse>, (StatusCode, String)> {
    let raw_url = req.url.as_deref().unwrap_or("").trim();
    if raw_url.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "`url` must be a non-empty string (DELETE /sync/relay/config to unset)".to_string(),
        ));
    }
    // Cheap sanity-check — catches typos like "relay.example.com"
    // (missing scheme) at save time rather than at next-boot relay
    // bring-up where the error message disappears into the log.
    let parsed = reqwest::Url::parse(raw_url).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("`url` is not a valid URL: {e}"),
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("relay URL scheme must be http or https (got `{}`)", parsed.scheme()),
        ));
    }
    let url_canonical = parsed.to_string();
    let url_canonical = url_canonical.trim_end_matches('/').to_string();

    let poll_interval_ms = req.poll_interval_ms.unwrap_or(5_000);
    if poll_interval_ms < 250 {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "poll_interval_ms must be at least 250ms (got {poll_interval_ms})"
            ),
        ));
    }

    let path = state.mosaic_root.join(".tesela").join("config.toml");
    let mut cfg = load_or_default(&path)?;
    cfg.sync.relay = Some(RelayConfig {
        url: url_canonical.clone(),
        poll_interval_ms,
    });
    cfg.save(&path).map_err(server_error)?;

    Ok(Json(RelayConfigPutResponse {
        url: Some(url_canonical),
        poll_interval_ms: Some(poll_interval_ms),
        restart_required: true,
    }))
}

pub async fn delete_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RelayConfigPutResponse>, (StatusCode, String)> {
    let path = state.mosaic_root.join(".tesela").join("config.toml");
    let mut cfg = load_or_default(&path)?;
    cfg.sync.relay = None;
    cfg.save(&path).map_err(server_error)?;
    Ok(Json(RelayConfigPutResponse {
        url: None,
        poll_interval_ms: None,
        restart_required: true,
    }))
}

fn load_or_default(path: &std::path::Path) -> Result<Config, (StatusCode, String)> {
    if path.exists() {
        Config::load(path).map_err(server_error)
    } else {
        Ok(Config::default())
    }
}

fn server_error<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}
