//! Keybinding + leader-tree user config (tesela-cmdd.4).
//!
//! `GET`/`PUT /keymap-config` — server-persisted config over stable command
//! ids: per-command shortcut/chord rebinds, per-surface hides, and
//! leader-tree bucket-label overrides. Mirrors the `relay-config`/
//! `backup-config` idiom (a small JSON blob under the mosaic's `.tesela/`
//! dir, GET returns the effective/default value, PUT overwrites it) rather
//! than the synced-views CRDT doc: unlike saved views, THIS config's
//! conflict/reserved-key validation (`checkRebind`) needs the live web
//! command registry, which only exists client-side, so the server is
//! intentionally a dumb blob store — it round-trips whatever shape the
//! client sends without interpreting `overrides`/`group_labels` at all.
//!
//! For the WEB client specifically (unlike iOS, which carries its own local
//! sync engine), "survives reload on a second device" just means "another
//! browser pointed at this same tesela-server" — this file, read by any
//! client hitting this server, is sufficient.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// Wire shape for `GET`/`PUT /keymap-config`. Field names match the web
/// `KeymapConfig` type verbatim (snake_case `group_labels` — no camelCase
/// translation layer at the boundary, same convention as `ViewRecord`/
/// `CommandManifestEntry`).
///
/// Each override is kept as an opaque `serde_json::Value` rather than a
/// typed `{shortcut, chord, hidden}` struct DELIBERATELY: the web store's
/// tri-state channels (key ABSENT = inherit default; `null` = explicitly
/// unbound; a value = rebound) can't round-trip through a naively-typed
/// `Option<Option<T>>` field — standard serde collapses a present `null`
/// and an absent key to the same `None`, silently turning "explicitly
/// unbound" back into "no override" on the next GET. Since the server
/// never interprets these fields anyway (validation needs the live web
/// command registry, which is client-only), passing them through as raw
/// JSON preserves the exact shape the client sent, tri-state included.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KeymapConfigDto {
    #[serde(default)]
    pub overrides: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub group_labels: HashMap<String, String>,
}

fn config_path(state: &AppState) -> std::path::PathBuf {
    state.mosaic_root.join(".tesela").join("keymap-config.json")
}

fn server_error<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

/// `GET /keymap-config` — the stored config, or an empty default (no
/// overrides, no group-label overrides) when nothing has been saved yet.
pub async fn get_keymap_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<KeymapConfigDto>, (StatusCode, String)> {
    let path = config_path(&state);
    match tokio::fs::read_to_string(&path).await {
        Ok(raw) => {
            let cfg: KeymapConfigDto = serde_json::from_str(&raw).map_err(server_error)?;
            Ok(Json(cfg))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Json(KeymapConfigDto::default())),
        Err(e) => Err(server_error(e)),
    }
}

/// `PUT /keymap-config` — overwrite the stored config with the full body
/// (the client always sends the whole config, mirroring `wholeConfig()` on
/// the web store). Echoes back what was saved.
pub async fn put_keymap_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<KeymapConfigDto>,
) -> Result<Json<KeymapConfigDto>, (StatusCode, String)> {
    let path = config_path(&state);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(server_error)?;
    }
    let raw = serde_json::to_string_pretty(&req).map_err(server_error)?;
    tokio::fs::write(&path, raw).await.map_err(server_error)?;
    Ok(Json(req))
}
