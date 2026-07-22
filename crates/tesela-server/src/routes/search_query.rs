use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use tesela_core::query::{parse_query, QueryContext, QueryPage, QueryResult};

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
    // The synced directory is the sole authority for Node RHS resolution;
    // SQLite remains a rebuildable query projection and receives it only as
    // additive matcher context.
    let context = QueryContext {
        pages: s
            .sync_engine
            .page_directory_list()
            .await
            .into_iter()
            .map(|entry| QueryPage {
                page_id: entry.page_id,
                slug: entry.slug,
                title: entry.title,
                aliases: entry.aliases,
                deleted: entry.deleted || entry.conflict,
            })
            .collect(),
    };
    let result = s
        .index
        .execute_query_with_context(
            &parsed,
            body.group.as_deref(),
            body.sort.as_deref(),
            &context,
        )
        .await?;
    Ok(Json(result))
}
