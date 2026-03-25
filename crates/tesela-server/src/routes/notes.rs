use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use tesela_core::{
    daily::DailyNoteConfig,
    link::{GraphEdge, Link},
    note::NoteId,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    Note,
};

use crate::{
    error::{AppError, AppResult},
    state::{AppState, WsEvent},
};

#[derive(Deserialize)]
pub struct ListQuery {
    pub tag: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Deserialize)]
pub struct CreateNoteReq {
    pub title: String,
    pub content: String,
    pub tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateNoteReq {
    /// Full note content (including frontmatter). The store writes this directly to disk.
    pub content: String,
}

pub async fn list_notes(
    Query(q): Query<ListQuery>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Note>>> {
    let limit = q.limit.unwrap_or(100);
    let offset = q.offset.unwrap_or(0);
    let notes = s.store.list(q.tag.as_deref(), limit, offset).await?;
    Ok(Json(notes))
}

pub async fn get_note(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Note>> {
    let note_id = NoteId::new(&id);
    let note = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", id)))?;
    Ok(Json(note))
}

#[derive(Deserialize)]
pub struct DailyQuery {
    pub date: Option<String>,  // optional ISO date "2026-03-30"
}

pub async fn get_daily_note(
    Query(q): Query<DailyQuery>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Note>> {
    let config = DailyNoteConfig::default();
    let date = q.date.and_then(|d| {
        // Parse "YYYY-MM-DD" without pulling in chrono directly
        let parts: Vec<&str> = d.split('-').collect();
        if parts.len() == 3 {
            if let (Ok(y), Ok(m), Ok(d)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                return chrono::NaiveDate::from_ymd_opt(y, m, d);
            }
        }
        None
    });
    let note = s.store.daily_note(date, &config).await?;
    Ok(Json(note))
}

pub async fn create_note(
    State(s): State<Arc<AppState>>,
    Json(req): Json<CreateNoteReq>,
) -> AppResult<Json<Note>> {
    let tags: Vec<&str> = req
        .tags
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(String::as_str)
        .collect();
    let note = s.store.create(&req.title, &req.content, &tags).await?;
    s.index.reindex(&note).await?;
    ensure_tag_pages(&s, &note).await;
    let _ = s.ws_tx.send(WsEvent::NoteCreated { note: note.clone() });
    Ok(Json(note))
}

pub async fn update_note(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
    Json(req): Json<UpdateNoteReq>,
) -> AppResult<Json<Note>> {
    let note_id = NoteId::new(&id);
    let mut note = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found: {}", id)))?;
    note.content = req.content;
    s.store.update(&note).await?;
    // Re-read to get fresh parsed metadata and checksum
    let updated = s
        .store
        .get(&note_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Note not found after update: {}", id)))?;
    s.index.reindex(&updated).await?;
    ensure_tag_pages(&s, &updated).await;
    let _ = s.ws_tx.send(WsEvent::NoteUpdated { note: updated.clone() });
    Ok(Json(updated))
}

pub async fn delete_note(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<StatusCode> {
    let note_id = NoteId::new(&id);
    s.store.delete(&note_id).await?;
    s.index.remove(&note_id).await?;
    let _ = s.ws_tx.send(WsEvent::NoteDeleted { id });
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_backlinks(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Link>>> {
    let note_id = NoteId::new(&id);
    let links = s.index.get_backlinks(&note_id).await?;
    Ok(Json(links))
}

pub async fn get_forward_links(
    Path(id): Path<String>,
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<Link>>> {
    let note_id = NoteId::new(&id);
    let links = s.index.get_forward_links(&note_id).await?;
    Ok(Json(links))
}

pub async fn get_all_edges(
    State(s): State<Arc<AppState>>,
) -> AppResult<Json<Vec<GraphEdge>>> {
    let edges = s.index.get_all_edges().await?;
    Ok(Json(edges))
}

/// Auto-create tag pages for any tags in the note that don't have a corresponding page.
/// Scans both frontmatter tags AND inline #tags in the body.
async fn ensure_tag_pages(s: &Arc<AppState>, note: &Note) {
    // Collect tags from frontmatter AND inline body text
    let mut all_tags: Vec<String> = note.metadata.tags.clone();
    // Extract inline #tags from body
    let tag_re = regex::Regex::new(r"#([A-Za-z][A-Za-z0-9_-]*)").unwrap();
    for cap in tag_re.captures_iter(&note.body) {
        let tag = cap[1].to_string();
        if !all_tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
            all_tags.push(tag);
        }
    }

    for tag in &all_tags {
        if tag == "daily" { continue; }

        let tag_id = NoteId::new(tag.to_lowercase());
        match s.store.get(&tag_id).await {
            Ok(Some(_)) => {} // Page already exists
            Ok(None) => {
                // Auto-create tag page
                let content = format!(
                    "---\ntitle: \"{}\"\ntype: \"Tag\"\nextends: \"Root Tag\"\ntag_properties: []\ntags: []\n---\n- Tag properties are inherited by all nodes using the tag.\n",
                    tag
                );
                match s.store.create(tag, &content, &[]).await {
                    Ok(tag_note) => {
                        let _ = s.index.reindex(&tag_note).await;
                        let _ = s.ws_tx.send(WsEvent::NoteCreated { note: tag_note });
                        tracing::info!("Auto-created tag page: {}", tag);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to auto-create tag page '{}': {}", tag, e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to check tag page '{}': {}", tag, e);
            }
        }
    }
}

