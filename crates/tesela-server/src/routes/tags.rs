use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{error::AppResult, state::AppState};

pub async fn list_tags(State(s): State<Arc<AppState>>) -> AppResult<Json<Vec<String>>> {
    let tags = s.index.list_tags().await?;
    Ok(Json(tags))
}
