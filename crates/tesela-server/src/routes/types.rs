use std::sync::Arc;

use axum::{extract::State, Json};

use tesela_core::types::TypeDefinition;

use crate::state::AppState;

pub async fn list_types(State(s): State<Arc<AppState>>) -> Json<Vec<TypeDefinition>> {
    Json(s.type_registry.types.clone())
}
