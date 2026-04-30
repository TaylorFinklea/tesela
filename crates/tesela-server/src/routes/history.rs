use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use tesela_core::{
    note::{NoteId, NoteVersion},
    traits::search_index::SearchIndex,
};

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

#[derive(Deserialize)]
pub struct ListVersionsParams {
    pub limit: Option<usize>,
}

/// GET /notes/:id/versions?limit=50
/// Lists historical versions for a note, newest first.
pub async fn list_versions(
    Path(id): Path<String>,
    Query(p): Query<ListVersionsParams>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<NoteVersion>>> {
    let note_id = NoteId::new(&id);
    let limit = p.limit.unwrap_or(50).min(200);
    let versions = s.index.list_versions(&note_id, limit).await?;
    Ok(Json(versions))
}

/// GET /notes/:id/versions/:version_id
/// Fetch a single historical version. Returns 404 if missing or note_id mismatch.
pub async fn get_version(
    Path((id, version_id)): Path<(String, i64)>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<NoteVersion>> {
    let note_id = NoteId::new(&id);
    let v = s
        .index
        .get_version(version_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Version not found: {}", version_id)))?;
    if v.note_id.as_str() != note_id.as_str() {
        return Err(AppError::NotFound(format!(
            "Version {} does not belong to note {}",
            version_id, id
        )));
    }
    Ok(Json(v))
}
