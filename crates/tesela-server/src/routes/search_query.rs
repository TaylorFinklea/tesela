use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use tesela_core::{
    query::{parse_query, QueryResult},
    traits::search_index::SearchIndex,
};

use crate::{error::AppResult, state::AppState};

#[derive(Deserialize)]
pub struct ExecuteQueryBody {
    /// The DSL string from a Query note's `query::` property.
    pub dsl: String,
    /// Property/metadata key to group by (e.g. `"status"`). Optional.
    pub group: Option<String>,
    /// Comma-separated `key [asc|desc]` list. Optional.
    pub sort: Option<String>,
}

/// POST /search/query — execute a DSL query and return grouped results.
pub async fn execute(
    State(s): State<Arc<AppState>>,
    Json(body): Json<ExecuteQueryBody>,
) -> AppResult<Json<QueryResult>> {
    let parsed = parse_query(&body.dsl);
    let result = s
        .index
        .execute_query(&parsed, body.group.as_deref(), body.sort.as_deref())
        .await?;
    Ok(Json(result))
}
