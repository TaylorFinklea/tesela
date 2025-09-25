//! Async runtime bridge for TUI
//!
//! This module provides a bridge between the synchronous TUI event loop
//! and async database operations. It maintains a tokio runtime that can
//! execute async tasks from synchronous code.

use crate::core::{Database, DatabaseConfig};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

/// Search result from async database operation
#[derive(Debug, Clone)]
pub struct AsyncSearchResult {
    pub title: String,
    pub path: String,
    pub content: String,
    pub snippet: Option<String>,
    pub rank: f32,
    pub tags: Vec<String>,
}

/// Async runtime manager for TUI
pub struct AsyncRuntime {
    runtime: Arc<Mutex<Runtime>>,
    database: Option<Arc<Database>>,
}

impl AsyncRuntime {
    /// Create a new async runtime
    pub fn new() -> Result<Self> {
        let runtime = Runtime::new()?;

        // Initialize database if we're in a mosaic (tesela.toml exists)
        let database = if std::path::Path::new("tesela.toml").exists() {
            let rt = runtime.handle().clone();
            // Use absolute path for database
            let db_path = std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .join("tesela.db");

            let config = DatabaseConfig {
                db_path,
                max_connections: 5,
                connect_timeout: 10,
                enable_wal: true,
                enable_foreign_keys: true,
            };

            let db = rt.block_on(async move { Database::new(config).await });

            match db {
                Ok(database) => {
                    let db_arc = Arc::new(database);

                    // Clear and reindex all notes on startup to ensure database is fresh
                    let db_clone = Arc::clone(&db_arc);
                    if let Err(e) = rt.block_on(async move { db_clone.clear_all_notes().await }) {
                        eprintln!("Warning: Failed to clear notes: {}", e);
                    }

                    // Index existing notes from filesystem
                    if let Err(e) = Self::index_existing_notes(&rt, &db_arc) {
                        eprintln!("Warning: Failed to index existing notes: {}", e);
                    }

                    // Always rebuild FTS5 index to ensure search works properly
                    let db_clone = Arc::clone(&db_arc);
                    if let Err(e) = rt.block_on(async move { db_clone.rebuild_index().await }) {
                        eprintln!("Warning: Failed to rebuild search index: {}", e);
                    }

                    Some(db_arc)
                }
                Err(e) => {
                    eprintln!("Warning: Failed to initialize database: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            runtime: Arc::new(Mutex::new(runtime)),
            database,
        })
    }

    /// Index existing notes from filesystem into database
    fn index_existing_notes(rt: &tokio::runtime::Handle, db: &Arc<Database>) -> Result<()> {
        use crate::core::{Storage, StorageConfig};
        use std::path::PathBuf;

        // Index notes from both "notes" and "dailies" directories
        let mut all_notes = Vec::new();

        // Index regular notes
        let notes_config = StorageConfig {
            mosaic_root: PathBuf::from("."),
            notes_dir: "notes".to_string(),
            attachments_dir: "attachments".to_string(),
            note_extensions: vec!["md".to_string()],
            max_attachment_size: 10 * 1024 * 1024,
        };
        let notes_storage = Storage::new(notes_config);
        all_notes.extend(notes_storage.list_notes()?);

        // Index daily notes
        let dailies_config = StorageConfig {
            mosaic_root: PathBuf::from("."),
            notes_dir: "dailies".to_string(),
            attachments_dir: "attachments".to_string(),
            note_extensions: vec!["md".to_string()],
            max_attachment_size: 10 * 1024 * 1024,
        };
        let dailies_storage = Storage::new(dailies_config);
        all_notes.extend(dailies_storage.list_notes()?);

        if !all_notes.is_empty() {
            let db_clone = Arc::clone(db);
            rt.block_on(async move {
                for note in all_notes {
                    if let Err(e) = db_clone.upsert_note(&note).await {
                        eprintln!(
                            "Warning: Failed to index note {}: {}",
                            note.path.display(),
                            e
                        );
                    }
                }
            });
        }

        Ok(())
    }

    /// Execute a search query using the async database
    pub fn search_notes(&self, query: &str) -> Result<Vec<AsyncSearchResult>> {
        if let Some(db) = &self.database {
            let db = Arc::clone(db);
            let query = query.to_string();

            let runtime = self.runtime.lock().unwrap();
            let results = runtime
                .block_on(async move { db.search_notes_with_snippets(&query, 50, 0).await })?;

            Ok(results
                .into_iter()
                .map(|(note, _title_snippet, body_snippet)| AsyncSearchResult {
                    title: note.title,
                    path: note.path.to_string_lossy().to_string(),
                    content: note.content,
                    snippet: Some(body_snippet),
                    rank: 1.0, // TODO: Calculate proper rank from FTS5
                    tags: note.metadata.tags,
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Search notes by tag
    pub fn search_by_tag(&self, tag: &str) -> Result<Vec<AsyncSearchResult>> {
        if let Some(db) = &self.database {
            let db = Arc::clone(db);
            let tag = tag.to_string();

            let runtime = self.runtime.lock().unwrap();
            let notes = runtime.block_on(async move { db.get_notes_by_tag(&tag).await })?;

            Ok(notes
                .into_iter()
                .map(|note| AsyncSearchResult {
                    title: note.title.clone(),
                    path: note.path.to_string_lossy().to_string(),
                    content: note.content.clone(),
                    snippet: None,
                    rank: 1.0,
                    tags: note.metadata.tags.clone(),
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get all unique tags from the database
    pub fn get_all_tags(&self) -> Result<Vec<String>> {
        if let Some(db) = &self.database {
            let db = Arc::clone(db);

            let runtime = self.runtime.lock().unwrap();
            let tags = runtime.block_on(async move { db.get_all_tags().await })?;

            Ok(tags)
        } else {
            Ok(Vec::new())
        }
    }

    /// Search notes within a date range
    pub fn search_by_date_range(
        &self,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<AsyncSearchResult>> {
        if let Some(db) = &self.database {
            let db = Arc::clone(db);

            let runtime = self.runtime.lock().unwrap();
            let notes =
                runtime.block_on(async move { db.get_notes_by_date_range(from, to).await })?;

            Ok(notes
                .into_iter()
                .map(|note| AsyncSearchResult {
                    title: note.title.clone(),
                    path: note.path.to_string_lossy().to_string(),
                    content: note.content.clone(),
                    snippet: None,
                    rank: 1.0,
                    tags: note.metadata.tags.clone(),
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Reindex all notes (used by file watcher)
    pub fn reindex_all(&self) -> Result<()> {
        if let Some(db) = &self.database {
            let db = Arc::clone(db);

            let runtime = self.runtime.lock().unwrap();
            runtime.block_on(async move {
                // First clear the database
                db.clear_all_notes().await?;

                // Then reindex from filesystem - both notes and dailies
                let mut all_notes = Vec::new();

                // Index regular notes
                let notes_config = crate::core::StorageConfig {
                    mosaic_root: std::path::PathBuf::from("."),
                    notes_dir: "notes".to_string(),
                    attachments_dir: "attachments".to_string(),
                    note_extensions: vec!["md".to_string()],
                    max_attachment_size: 10 * 1024 * 1024, // 10MB
                };
                let notes_storage = crate::core::Storage::new(notes_config);
                all_notes.extend(notes_storage.list_notes()?);

                // Index daily notes
                let dailies_config = crate::core::StorageConfig {
                    mosaic_root: std::path::PathBuf::from("."),
                    notes_dir: "dailies".to_string(),
                    attachments_dir: "attachments".to_string(),
                    note_extensions: vec!["md".to_string()],
                    max_attachment_size: 10 * 1024 * 1024, // 10MB
                };
                let dailies_storage = crate::core::Storage::new(dailies_config);
                all_notes.extend(dailies_storage.list_notes()?);

                for note in all_notes {
                    db.upsert_note(&note).await?;
                }

                Ok::<(), anyhow::Error>(())
            })?;
        }

        Ok(())
    }

    /// Index a single note
    pub fn index_note(&self, note_path: &std::path::Path) -> Result<()> {
        if let Some(db) = &self.database {
            let db = Arc::clone(db);
            let note_path = note_path.to_path_buf();

            let runtime = self.runtime.lock().unwrap();
            runtime.block_on(async move {
                let config = crate::core::StorageConfig {
                    mosaic_root: std::path::PathBuf::from("."),
                    notes_dir: "notes".to_string(),
                    attachments_dir: "attachments".to_string(),
                    note_extensions: vec!["md".to_string()],
                    max_attachment_size: 10 * 1024 * 1024, // 10MB
                };
                let storage = crate::core::Storage::new(config);
                if let Ok(note) = storage.load_note(&note_path) {
                    db.upsert_note(&note).await?;
                }
                Ok::<(), anyhow::Error>(())
            })?;
        }

        Ok(())
    }

    /// Delete a note from the index
    pub fn delete_note(&self, note_path: &std::path::Path) -> Result<()> {
        if let Some(db) = &self.database {
            let db = Arc::clone(db);
            let note_path_str = note_path.to_string_lossy().to_string();

            let runtime = self.runtime.lock().unwrap();
            runtime.block_on(async move {
                db.delete_note_by_path(&note_path_str).await?;
                Ok::<(), anyhow::Error>(())
            })?;
        }

        Ok(())
    }

    /// Execute a combined search with multiple filters
    pub fn search_with_filters(
        &self,
        text_query: Option<&str>,
        tags: Vec<String>,
        from_date: Option<chrono::DateTime<chrono::Utc>>,
        to_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<AsyncSearchResult>> {
        if let Some(db) = &self.database {
            let db = Arc::clone(db);
            let text_query = text_query.map(|s| s.to_string());

            let runtime = self.runtime.lock().unwrap();
            let mut all_results = Vec::new();

            // Start with text search if provided
            if let Some(ref query) = text_query {
                let results = runtime
                    .block_on(async { db.search_notes_with_snippets(query, 50, 0).await })?;

                all_results = results
                    .into_iter()
                    .map(|(note, _title_snippet, body_snippet)| AsyncSearchResult {
                        title: note.title,
                        path: note.path.to_string_lossy().to_string(),
                        content: note.content,
                        snippet: Some(body_snippet),
                        rank: 1.0,
                        tags: note.metadata.tags,
                    })
                    .collect();
            }

            // Filter by tags if provided
            if !tags.is_empty() {
                all_results.retain(|r| tags.iter().any(|tag| r.tags.contains(tag)));
            }

            // Filter by date range if provided
            if from_date.is_some() || to_date.is_some() {
                let date_results = runtime
                    .block_on(async { db.get_notes_by_date_range(from_date, to_date).await })?;

                let date_paths: std::collections::HashSet<String> = date_results
                    .iter()
                    .map(|n| n.path.to_string_lossy().to_string())
                    .collect();

                if text_query.is_some() || !tags.is_empty() {
                    // Intersect with existing results
                    all_results.retain(|r| date_paths.contains(&r.path));
                } else {
                    // Use date results as primary results
                    all_results = date_results
                        .into_iter()
                        .map(|note| AsyncSearchResult {
                            title: note.title.clone(),
                            path: note.path.to_string_lossy().to_string(),
                            content: note.content.clone(),
                            snippet: None,
                            rank: 1.0,
                            tags: note.metadata.tags.clone(),
                        })
                        .collect();
                }
            }

            // Sort by rank
            all_results.sort_by(|a, b| {
                b.rank
                    .partial_cmp(&a.rank)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            Ok(all_results)
        } else {
            Ok(Vec::new())
        }
    }
}

impl Default for AsyncRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create async runtime")
    }
}
