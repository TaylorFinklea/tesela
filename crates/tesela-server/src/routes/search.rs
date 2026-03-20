use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use tesela_core::{note::SearchHit, traits::search_index::SearchIndex};

use crate::{error::AppResult, state::AppState};

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn search_notes(
    Query(q): Query<SearchQuery>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<SearchHit>>> {
    let limit = q.limit.unwrap_or(20);
    let offset = q.offset.unwrap_or(0);
    let hits = s.index.search(&q.q, limit, offset).await?;
    Ok(Json(hits))
}
