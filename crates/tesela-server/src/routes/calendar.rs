use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use tesela_core::{query::CalendarMarks, traits::search_index::SearchIndex};

use crate::{error::AppResult, state::AppState};

#[derive(Deserialize)]
pub struct MarksRange {
    /// Inclusive start date (`YYYY-MM-DD`).
    pub from: String,
    /// Inclusive end date (`YYYY-MM-DD`).
    pub to: String,
}

/// GET /calendar/marks?from=YYYY-MM-DD&to=YYYY-MM-DD
/// Returns per-day marker counts for the rail's mini calendar (Phase 9.2).
pub async fn marks(
    Query(r): Query<MarksRange>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<CalendarMarks>> {
    let m = s.index.calendar_marks(&r.from, &r.to).await?;
    Ok(Json(m))
}
