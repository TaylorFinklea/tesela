use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use tesela_core::{
    traits::note_store::NoteStore,
    types::{PropertyDef, TypeDefinition},
};

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

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

/// Get a specific tag with resolved properties (walks extends chain)
pub async fn get_type(
    Path(name): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<TypeDefinition>> {
    let resolved = s.index.get_resolved_tag_def(&name).await?;
    match resolved {
        Some(t) => Ok(Json(t)),
        None => Err(AppError::NotFound(format!("Tag not found: {}", name))),
    }
}

/// List all nodes (notes) tagged with a specific type
pub async fn list_typed_nodes(
    Path(name): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<tesela_core::Note>>> {
    // Fetch all notes with this tag
    let notes = s.store.list(Some(&name), usize::MAX, 0).await?;
    Ok(Json(notes))
}

/// List all property definitions (from DB cache, populated by indexing Property pages)
pub async fn list_properties(State(s): State<Arc<AppState>>) -> AppResult<Json<Vec<PropertyDef>>> {
    let props = s.index.get_all_property_defs().await?;
    Ok(Json(props))
}
