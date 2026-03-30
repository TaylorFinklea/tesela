use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;

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
    let notes = s.store.list(Some(&name), usize::MAX, 0).await?;
    Ok(Json(notes))
}

#[derive(Deserialize)]
pub struct TypedBlocksQuery {
    /// Single filter (backward compat)
    pub filter_property: Option<String>,
    pub filter_value: Option<String>,
    /// Multi-filter: JSON array of {"property":"...","value":"..."}
    pub filters: Option<String>,
    pub sort_by: Option<String>,
    pub sort_dir: Option<String>,
}

#[derive(Deserialize)]
struct PropertyFilter {
    property: String,
    value: String,
}

/// List all blocks tagged with a specific type, with DB-indexed properties.
/// Supports single filter (?filter_property=status&filter_value=todo)
/// or multi-filter (?filters=[{"property":"status","value":"todo"},{"property":"priority","value":"high"}])
pub async fn list_typed_blocks(
    Path(name): Path<String>,
    Query(q): Query<TypedBlocksQuery>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<tesela_core::block::ParsedBlock>>> {
    let mut blocks = s.index.get_typed_blocks(&name).await?;

    // Collect all filters
    let mut active_filters: Vec<PropertyFilter> = Vec::new();

    // Single filter (backward compat)
    if let (Some(prop), Some(val)) = (&q.filter_property, &q.filter_value) {
        active_filters.push(PropertyFilter {
            property: prop.clone(),
            value: val.clone(),
        });
    }

    // Multi-filter JSON
    if let Some(filters_json) = &q.filters {
        if let Ok(parsed) = serde_json::from_str::<Vec<PropertyFilter>>(filters_json) {
            active_filters.extend(parsed);
        }
    }

    // Apply all filters (AND logic — block must match every filter)
    for filter in &active_filters {
        let prop_lower = filter.property.to_lowercase();
        let val_lower = filter.value.to_lowercase();
        blocks.retain(|b| {
            b.properties.iter().any(|(k, v)| {
                k.to_lowercase() == prop_lower && v.to_lowercase() == val_lower
            })
        });
    }

    // Sort by property
    if let Some(sort_prop) = &q.sort_by {
        let prop_lower = sort_prop.to_lowercase();
        let ascending = q.sort_dir.as_deref() != Some("desc");
        blocks.sort_by(|a, b| {
            let va = a.properties.iter()
                .find(|(k, _)| k.to_lowercase() == prop_lower)
                .map(|(_, v)| v.as_str()).unwrap_or("");
            let vb = b.properties.iter()
                .find(|(k, _)| k.to_lowercase() == prop_lower)
                .map(|(_, v)| v.as_str()).unwrap_or("");
            if ascending { va.cmp(vb) } else { vb.cmp(va) }
        });
    }

    Ok(Json(blocks))
}

/// List all property definitions (from DB cache, populated by indexing Property pages)
pub async fn list_properties(State(s): State<Arc<AppState>>) -> AppResult<Json<Vec<PropertyDef>>> {
    let props = s.index.get_all_property_defs().await?;
    Ok(Json(props))
}
