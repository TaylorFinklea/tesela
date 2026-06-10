//! Saved-views registry routes (saved-views spec, 2026-06-10).
//!
//! Thin wrappers over the engine's synced views-registry doc
//! (`tesela_sync::VIEWS_DOC_ID`): `views_list` / `views_upsert` /
//! `views_delete` on `Arc<dyn SyncEngine>`. The registry rides the SAME
//! relay/delta/snapshot streams note docs do, so a write here converges to
//! every device with no extra plumbing; these routes only add HTTP shape,
//! DSL validation, and the WS fan-out (`WsEvent::ViewsChanged` + the
//! views-doc binary delta, mirroring `update_note`'s post-write tail).
//!
//! Builtins (the seeded Inbox) are editable but never deletable — the
//! engine enforces the guard; `delete_view` pre-checks so the client gets
//! a precise 400 instead of a generic engine error.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use tesela_core::query::{parse_query, BoolExpr};
use tesela_sync::ViewRecord;

use crate::{
    error::{AppError, AppResult},
    state::{AppState, WsEvent},
};

/// Allowed `display_mode` values (spec: `display.mode: list|table|kanban`).
const DISPLAY_MODES: [&str; 3] = ["list", "table", "kanban"];

/// Validate a view's query DSL. `parse_query` is deliberately TOTAL and
/// liberal (unrecognized syntax is dropped silently, matching the TS
/// parser), so "unparseable" here means: a non-empty input from which the
/// parser recognized NO predicates — saving it would silently create a
/// match-everything view. The error message names the offending DSL so the
/// view editor can surface it. Carve-outs: a lone `kind:…` selector and a
/// bare `ORDER BY` clause are valid queries with an empty predicate tree.
fn validate_dsl(dsl: &str) -> Result<(), AppError> {
    let trimmed = dsl.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(
            "view dsl must not be empty".to_string(),
        ));
    }
    let parsed = parse_query(trimmed);
    let no_predicates = parsed.expr == BoolExpr::default();
    let mentions_kind = trimmed.to_ascii_lowercase().contains("kind:");
    if no_predicates && parsed.sort.is_none() && !mentions_kind {
        return Err(AppError::Validation(format!(
            "unparseable query DSL: no predicates recognized in \"{trimmed}\" \
             (use key:value filters like status:todo, tag:project, -has:scheduled)"
        )));
    }
    Ok(())
}

fn validate_display_mode(mode: &str) -> Result<(), AppError> {
    if DISPLAY_MODES.contains(&mode) {
        return Ok(());
    }
    Err(AppError::Validation(format!(
        "invalid display_mode '{mode}': expected one of {}",
        DISPLAY_MODES.join("|")
    )))
}

/// Look one view up by id (the registry holds 6–12 entries; a list scan
/// is the engine's own access pattern).
async fn find_view(s: &AppState, id: &str) -> Option<ViewRecord> {
    s.sync_engine
        .views_list()
        .await
        .into_iter()
        .find(|v| v.id == id)
}

/// Post-write fan-out, mirroring `update_note`'s tail: a text
/// `WsEvent::ViewsChanged` carrying the full ordered registry (web
/// invalidates without a refetch) plus the views-doc cursor-free delta as
/// a binary frame so live `.relay` device sockets converge in <1s without
/// waiting on the relay poll. Best-effort — the relay tick still carries
/// the change if either send is missed.
async fn notify_views_changed(s: &AppState) {
    let views = s.sync_engine.views_list().await;
    let _ = s.ws_tx.send(WsEvent::ViewsChanged { views });
    if let Some(delta) = s
        .sync_engine
        .export_doc_update(tesela_sync::VIEWS_DOC_ID, None)
        .await
    {
        match tesela_sync::encode_loro_relay_payload(&[tesela_sync::LoroDocUpdate {
            doc: tesela_sync::VIEWS_DOC_ID,
            update_bytes: delta,
        }]) {
            Ok(frame) => {
                let _ = s.ws_delta_tx.send(crate::state::WsDelta {
                    origin: None,
                    frame,
                });
            }
            Err(e) => tracing::warn!("ws: encode views-doc delta failed: {e}"),
        }
    }
}

/// `GET /views` — all saved views, sorted by `(order, id)` (the engine's
/// deterministic ordering; ties break by id).
pub async fn list_views(State(s): State<Arc<AppState>>) -> Json<Vec<ViewRecord>> {
    Json(s.sync_engine.views_list().await)
}

#[derive(Deserialize)]
pub struct CreateViewReq {
    /// Optional explicit id (the iOS/web clients normally omit it and the
    /// server mints a UUID). Creating over an existing id is refused —
    /// updates go through `PUT /views/{id}`.
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub dsl: String,
    /// Sort position; defaults to appending after the current last view.
    #[serde(default)]
    pub order: Option<i64>,
    /// "list" (default) | "table" | "kanban".
    #[serde(default)]
    pub display_mode: Option<String>,
    #[serde(default)]
    pub display_group_by: Option<String>,
    #[serde(default)]
    pub display_show_done: Option<bool>,
}

/// `POST /views` — create a saved view. The server mints the id unless one
/// is provided; `builtin` is always false (builtins are seeded by the
/// engine, never created over HTTP — an HTTP-created builtin would be
/// undeletable by the guard).
pub async fn create_view(
    State(s): State<Arc<AppState>>,
    Json(req): Json<CreateViewReq>,
) -> AppResult<(StatusCode, Json<ViewRecord>)> {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation(
            "view name must not be empty".to_string(),
        ));
    }
    validate_dsl(&req.dsl)?;
    let display_mode = match req.display_mode.as_deref().map(str::trim) {
        Some(m) if !m.is_empty() => {
            validate_display_mode(m)?;
            m.to_string()
        }
        _ => "list".to_string(),
    };

    let existing = s.sync_engine.views_list().await;
    let id = match req.id.as_deref().map(str::trim) {
        Some(id) if !id.is_empty() => {
            if existing.iter().any(|v| v.id == id) {
                return Err(AppError::Validation(format!(
                    "view '{id}' already exists — update it via PUT /views/{id}"
                )));
            }
            id.to_string()
        }
        _ => uuid::Uuid::new_v4().to_string(),
    };
    // Append after the current last view (order steps of 10 leave gaps for
    // client-side inserts between reorders).
    let order = req
        .order
        .unwrap_or_else(|| existing.iter().map(|v| v.order).max().unwrap_or(-10) + 10);

    let record = ViewRecord {
        id,
        name,
        dsl: req.dsl.trim().to_string(),
        order,
        builtin: false,
        display_mode,
        display_group_by: req.display_group_by.filter(|g| !g.trim().is_empty()),
        display_show_done: req.display_show_done,
    };
    s.sync_engine
        .views_upsert(record.clone())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("views_upsert: {e}")))?;
    notify_views_changed(&s).await;
    Ok((StatusCode::CREATED, Json(record)))
}

#[derive(Deserialize)]
pub struct UpdateViewReq {
    /// All fields optional — omitted fields keep their stored value (the
    /// engine's field-level LWW writes per field anyway).
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub dsl: Option<String>,
    #[serde(default)]
    pub order: Option<i64>,
    #[serde(default)]
    pub display_mode: Option<String>,
    /// Empty string clears the grouping key.
    #[serde(default)]
    pub display_group_by: Option<String>,
    #[serde(default)]
    pub display_show_done: Option<bool>,
}

/// `PUT /views/{id}` — update name/dsl/display/order on an existing view
/// (404 for an unknown id). Builtins are editable here — only deletion is
/// guarded — and `builtin` itself is not settable (sticky in the engine).
pub async fn update_view(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
    Json(req): Json<UpdateViewReq>,
) -> AppResult<Json<ViewRecord>> {
    let mut record = find_view(&s, &id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("view not found: {id}")))?;

    if let Some(name) = req.name {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::Validation(
                "view name must not be empty".to_string(),
            ));
        }
        record.name = name;
    }
    if let Some(dsl) = req.dsl {
        validate_dsl(&dsl)?;
        record.dsl = dsl.trim().to_string();
    }
    if let Some(order) = req.order {
        record.order = order;
    }
    if let Some(mode) = req.display_mode {
        let mode = mode.trim().to_string();
        validate_display_mode(&mode)?;
        record.display_mode = mode;
    }
    if let Some(group_by) = req.display_group_by {
        let group_by = group_by.trim().to_string();
        record.display_group_by = (!group_by.is_empty()).then_some(group_by);
    }
    if let Some(show_done) = req.display_show_done {
        record.display_show_done = Some(show_done);
    }

    s.sync_engine
        .views_upsert(record.clone())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("views_upsert: {e}")))?;
    notify_views_changed(&s).await;
    Ok(Json(record))
}

/// `DELETE /views/{id}` — remove a user view. 404 for an unknown id; 400
/// for a builtin (editable, never deletable — the UI offers "reset to
/// default" instead). The engine enforces the same guard; the pre-check
/// here just maps it to a precise status + message.
pub async fn delete_view(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    let existing = find_view(&s, &id)
        .await
        .ok_or_else(|| AppError::NotFound(format!("view not found: {id}")))?;
    if existing.builtin {
        return Err(AppError::Validation(format!(
            "view '{id}' is builtin and cannot be deleted — builtins are \
             editable; reset it to its default instead"
        )));
    }
    let removed = s
        .sync_engine
        .views_delete(&id)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("views_delete: {e}")))?;
    if !removed {
        // Raced with a concurrent delete between the lookup and the call.
        return Err(AppError::NotFound(format!("view not found: {id}")));
    }
    notify_views_changed(&s).await;
    Ok(Json(json!({ "deleted": true, "id": id })))
}

/// `POST /views/reorder` — body is the bare JSON array of view ids in
/// their new order; each listed view's `order` is reassigned to its index
/// (steps of 10). Every id must exist — an unknown id rejects the whole
/// reorder so a stale switcher can't scramble the registry. Views omitted
/// from the array keep their current `order` (the switcher submits the
/// full list). Responds with the full re-sorted registry.
pub async fn reorder_views(
    State(s): State<Arc<AppState>>,
    Json(ids): Json<Vec<String>>,
) -> AppResult<Json<Vec<ViewRecord>>> {
    if ids.is_empty() {
        return Err(AppError::Validation(
            "reorder requires at least one view id".to_string(),
        ));
    }
    let existing = s.sync_engine.views_list().await;
    let unknown: Vec<&str> = ids
        .iter()
        .map(String::as_str)
        .filter(|id| !existing.iter().any(|v| v.id == *id))
        .collect();
    if !unknown.is_empty() {
        return Err(AppError::Validation(format!(
            "unknown view id(s) in reorder: {}",
            unknown.join(", ")
        )));
    }
    for (idx, id) in ids.iter().enumerate() {
        let new_order = (idx as i64 + 1) * 10;
        // `unwrap` is safe: every id was just checked against `existing`.
        let mut record = existing.iter().find(|v| v.id == *id).unwrap().clone();
        if record.order == new_order {
            continue;
        }
        record.order = new_order;
        s.sync_engine
            .views_upsert(record)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("views_upsert: {e}")))?;
    }
    notify_views_changed(&s).await;
    Ok(Json(s.sync_engine.views_list().await))
}
