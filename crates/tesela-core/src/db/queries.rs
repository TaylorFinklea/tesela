//! Row mapping helpers for SQLite queries

use chrono::{DateTime, Utc};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use std::path::PathBuf;

use crate::error::{Result, TeselaError};
use crate::note::{Note, NoteId, NoteMetadata, SearchHit};

fn db_err(e: sqlx::Error) -> TeselaError {
    TeselaError::Database {
        message: e.to_string(),
        source: None,
    }
}

/// Map a database row to a Note
pub fn row_to_note(row: &SqliteRow) -> Result<Note> {
    let id: String = row.try_get("id").map_err(db_err)?;
    let title: String = row.try_get("title").map_err(db_err)?;
    let body: String = row.try_get("body").map_err(db_err)?;
    let content: String = row.try_get("content").map_err(db_err)?;
    let path: String = row.try_get("path").map_err(db_err)?;
    let checksum: String = row.try_get("checksum").map_err(db_err)?;
    let created_str: String = row.try_get("created_at").map_err(db_err)?;
    let modified_str: String = row.try_get("modified_at").map_err(db_err)?;
    let tags_json: String = row.try_get("tags").map_err(db_err)?;

    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    let created_at = created_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());
    let modified_at = modified_str
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    Ok(Note {
        id: NoteId::new(id),
        title: title.clone(),
        content,
        body,
        metadata: NoteMetadata {
            title: None,
            tags,
            aliases: vec![],
            custom: Default::default(),
            created: Some(created_at),
            modified: Some(modified_at),
        },
        path: PathBuf::from(path),
        checksum,
        created_at,
        modified_at,
        attachments: vec![],
    })
}

/// Map a search result row to a SearchHit
pub fn row_to_search_hit(row: &SqliteRow) -> Result<SearchHit> {
    let id: String = row.try_get("id").map_err(db_err)?;
    let title: String = row.try_get("title").map_err(db_err)?;
    let snippet: String = row.try_get("snippet").map_err(db_err)?;
    let rank: f64 = row.try_get("rank").map_err(db_err)?;
    let tags_json: String = row.try_get("tags").map_err(db_err)?;
    let path: String = row.try_get("path").map_err(db_err)?;

    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

    Ok(SearchHit {
        note_id: NoteId::new(id),
        title,
        snippet,
        rank,
        tags,
        path: PathBuf::from(path),
    })
}
