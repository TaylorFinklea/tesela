use std::sync::Arc;

use axum::{extract::State, Json};

use tesela_core::types::{PropertyDef, TypeDefinition};

use crate::{error::AppResult, state::AppState};

/// List all tag definitions (from DB cache, populated by indexing Tag pages)
pub async fn list_types(State(s): State<Arc<AppState>>) -> AppResult<Json<Vec<TypeDefinition>>> {
    // Try DB cache first, fall back to TypeRegistry (from types.toml)
    let db_types = s.index.get_all_tag_defs().await?;
    if db_types.is_empty() {
        Ok(Json(s.type_registry.types.clone()))
    } else {
        Ok(Json(db_types))
    }
}

/// List all property definitions (from DB cache, populated by indexing Property pages)
pub async fn list_properties(State(s): State<Arc<AppState>>) -> AppResult<Json<Vec<PropertyDef>>> {
    let props = s.index.get_all_property_defs().await?;
    Ok(Json(props))
}
