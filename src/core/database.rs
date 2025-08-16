//! Database layer for Tesela
//!
//! This module provides SQLite database operations with FTS5 full-text search support.
//! It handles persistent storage of note metadata, indexing, and search capabilities.

use crate::core::error::{Result, TeselaError};
use crate::core::storage::{Attachment, Link, LinkType, Note, NoteMetadata};
use chrono::{DateTime, Utc};
use serde_json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::{Row, SqliteConnection};
use std::collections::HashMap;
use std::str::FromStr;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Database file path
    pub db_path: std::path::PathBuf,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connect_timeout: u64,
    /// Enable write-ahead logging
    pub enable_wal: bool,
    /// Enable foreign key constraints
    pub enable_foreign_keys: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            db_path: std::path::PathBuf::from("tesela.db"),
            max_connections: 5,
            connect_timeout: 30,
            enable_wal: true,
            enable_foreign_keys: true,
        }
    }
}

/// Main database manager
pub struct Database {
    pool: SqlitePool,
    config: DatabaseConfig,
}

impl Database {
    /// Create a new database instance
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        let options =
            SqliteConnectOptions::from_str(config.db_path.to_str().unwrap_or("tesela.db"))
                .map_err(|e| {
                    TeselaError::database_with_source("Failed to create database options", e)
                })?
                .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(std::time::Duration::from_secs(config.connect_timeout))
            .connect_with(options)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to connect to database", e))?;

        let db = Self { pool, config };

        // Initialize database schema
        db.initialize().await?;

        Ok(db)
    }

    /// Initialize the database schema
    pub async fn initialize(&self) -> Result<()> {
        // Enable WAL mode if configured (must be done outside transaction)
        if self.config.enable_wal {
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&self.pool)
                .await
                .map_err(|e| TeselaError::database_with_source("Failed to enable WAL mode", e))?;
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to begin transaction", e))?;

        // Enable foreign keys if configured
        if self.config.enable_foreign_keys {
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    TeselaError::database_with_source("Failed to enable foreign keys", e)
                })?;
        }

        // Create notes table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                body TEXT NOT NULL,
                metadata TEXT NOT NULL,
                path TEXT NOT NULL,
                checksum TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                modified_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create notes table", e))?;

        // Create FTS5 virtual table for full-text search
        sqlx::query(
            r#"
            CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
                id UNINDEXED,
                title,
                body,
                content='notes',
                content_rowid='rowid',
                tokenize='porter unicode61'
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create FTS table", e))?;

        // Create trigger to keep FTS table in sync
        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS notes_fts_insert AFTER INSERT ON notes
            BEGIN
                INSERT INTO notes_fts(id, title, body) VALUES (new.id, new.title, new.body);
            END
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create FTS insert trigger", e))?;

        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS notes_fts_update AFTER UPDATE ON notes
            BEGIN
                UPDATE notes_fts SET title = new.title, body = new.body WHERE id = new.id;
            END
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create FTS update trigger", e))?;

        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS notes_fts_delete AFTER DELETE ON notes
            BEGIN
                DELETE FROM notes_fts WHERE id = old.id;
            END
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create FTS delete trigger", e))?;

        // Create tags table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create tags table", e))?;

        // Create note_tags junction table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS note_tags (
                note_id TEXT NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (note_id, tag_id),
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
                FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create note_tags table", e))?;

        // Create links table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_note_id TEXT NOT NULL,
                target TEXT NOT NULL,
                link_type TEXT NOT NULL,
                text TEXT,
                position INTEGER,
                FOREIGN KEY (source_note_id) REFERENCES notes(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create links table", e))?;

        // Create attachments table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS attachments (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                size INTEGER NOT NULL,
                checksum TEXT NOT NULL,
                path TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to create attachments table", e))?;

        // Create note_attachments junction table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS note_attachments (
                note_id TEXT NOT NULL,
                attachment_id TEXT NOT NULL,
                PRIMARY KEY (note_id, attachment_id),
                FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
                FOREIGN KEY (attachment_id) REFERENCES attachments(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            TeselaError::database_with_source("Failed to create note_attachments table", e)
        })?;

        // Create indices for better query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_notes_created_at ON notes(created_at DESC)")
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to create index", e))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_notes_modified_at ON notes(modified_at DESC)")
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to create index", e))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_note_id)")
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to create index", e))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_links_target ON links(target)")
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to create index", e))?;

        tx.commit()
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to commit transaction", e))?;

        Ok(())
    }

    /// Insert or update a note
    pub async fn upsert_note(&self, note: &Note) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to begin transaction", e))?;

        // Serialize metadata to JSON
        let metadata_json =
            serde_json::to_string(&note.metadata).map_err(|e| TeselaError::Json(e))?;

        // Insert or replace note
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO notes (
                id, title, content, body, metadata, path, checksum, created_at, modified_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&note.id)
        .bind(&note.title)
        .bind(&note.content)
        .bind(&note.body)
        .bind(&metadata_json)
        .bind(note.path.to_str().unwrap_or(""))
        .bind(&note.checksum)
        .bind(note.created_at.timestamp())
        .bind(note.modified_at.timestamp())
        .execute(&mut *tx)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to insert note", e))?;

        // Update tags
        self.update_note_tags(&mut tx, &note.id, &note.metadata.tags)
            .await?;

        // Update attachments
        self.update_note_attachments(&mut tx, &note.id, &note.attachments)
            .await?;

        tx.commit()
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to commit transaction", e))?;

        Ok(())
    }

    /// Get a note by ID
    pub async fn get_note(&self, id: &str) -> Result<Option<Note>> {
        let row = sqlx::query(
            r#"
            SELECT id, title, content, body, metadata, path, checksum, created_at, modified_at
            FROM notes
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to fetch note", e))?;

        if let Some(row) = row {
            let metadata: NoteMetadata =
                serde_json::from_str(row.get("metadata")).map_err(|e| TeselaError::Json(e))?;

            let created_at =
                DateTime::from_timestamp(row.get("created_at"), 0).unwrap_or_else(Utc::now);
            let modified_at =
                DateTime::from_timestamp(row.get("modified_at"), 0).unwrap_or_else(Utc::now);

            let attachments = self.get_note_attachments(id).await?;

            Ok(Some(Note {
                id: row.get("id"),
                title: row.get("title"),
                content: row.get("content"),
                body: row.get("body"),
                metadata,
                path: std::path::PathBuf::from(row.get::<String, _>("path")),
                checksum: row.get("checksum"),
                created_at,
                modified_at,
                attachments,
            }))
        } else {
            Ok(None)
        }
    }

    /// Delete a note
    pub async fn delete_note(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to delete note", e))?;

        Ok(())
    }

    /// Search notes using full-text search
    pub async fn search_notes(&self, query: &str, limit: i32, offset: i32) -> Result<Vec<Note>> {
        let rows = sqlx::query(
            r#"
            SELECT n.id, n.title, n.content, n.body, n.metadata, n.path, n.checksum,
                   n.created_at, n.modified_at,
                   rank
            FROM notes n
            JOIN notes_fts ON notes_fts.id = n.id
            WHERE notes_fts MATCH ?
            ORDER BY rank
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(query)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to search notes", e))?;

        let mut notes = Vec::new();
        for row in rows {
            let metadata: NoteMetadata =
                serde_json::from_str(row.get("metadata")).map_err(|e| TeselaError::Json(e))?;

            let created_at =
                DateTime::from_timestamp(row.get("created_at"), 0).unwrap_or_else(Utc::now);
            let modified_at =
                DateTime::from_timestamp(row.get("modified_at"), 0).unwrap_or_else(Utc::now);

            let attachments = self.get_note_attachments(row.get("id")).await?;

            notes.push(Note {
                id: row.get("id"),
                title: row.get("title"),
                content: row.get("content"),
                body: row.get("body"),
                metadata,
                path: std::path::PathBuf::from(row.get::<String, _>("path")),
                checksum: row.get("checksum"),
                created_at,
                modified_at,
                attachments,
            });
        }

        Ok(notes)
    }

    /// Get notes by tag
    pub async fn get_notes_by_tag(&self, tag: &str) -> Result<Vec<Note>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT n.id, n.title, n.content, n.body, n.metadata, n.path,
                   n.checksum, n.created_at, n.modified_at
            FROM notes n
            JOIN note_tags nt ON n.id = nt.note_id
            JOIN tags t ON nt.tag_id = t.id
            WHERE t.name = ?
            ORDER BY n.modified_at DESC
            "#,
        )
        .bind(tag)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to fetch notes by tag", e))?;

        let mut notes = Vec::new();
        for row in rows {
            let metadata: NoteMetadata =
                serde_json::from_str(row.get("metadata")).map_err(|e| TeselaError::Json(e))?;

            let created_at =
                DateTime::from_timestamp(row.get("created_at"), 0).unwrap_or_else(Utc::now);
            let modified_at =
                DateTime::from_timestamp(row.get("modified_at"), 0).unwrap_or_else(Utc::now);

            let attachments = self.get_note_attachments(row.get("id")).await?;

            notes.push(Note {
                id: row.get("id"),
                title: row.get("title"),
                content: row.get("content"),
                body: row.get("body"),
                metadata,
                path: std::path::PathBuf::from(row.get::<String, _>("path")),
                checksum: row.get("checksum"),
                created_at,
                modified_at,
                attachments,
            });
        }

        Ok(notes)
    }

    /// Get all tags with their usage counts
    pub async fn get_tags_with_counts(&self) -> Result<HashMap<String, usize>> {
        let rows = sqlx::query(
            r#"
            SELECT t.name, COUNT(nt.note_id) as count
            FROM tags t
            LEFT JOIN note_tags nt ON t.id = nt.tag_id
            GROUP BY t.name
            ORDER BY count DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to fetch tags", e))?;

        let mut tags = HashMap::new();
        for row in rows {
            let name: String = row.get("name");
            let count: i64 = row.get("count");
            tags.insert(name, count as usize);
        }

        Ok(tags)
    }

    /// Insert or update links for a note
    pub async fn update_note_links(&self, note_id: &str, links: &[Link]) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to begin transaction", e))?;

        // Delete existing links
        sqlx::query("DELETE FROM links WHERE source_note_id = ?")
            .bind(note_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to delete links", e))?;

        // Insert new links
        for link in links {
            let link_type = match link.link_type {
                LinkType::Internal => "internal",
                LinkType::External => "external",
                LinkType::Attachment => "attachment",
            };

            sqlx::query(
                r#"
                INSERT INTO links (source_note_id, target, link_type, text, position)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(note_id)
            .bind(&link.target)
            .bind(link_type)
            .bind(&link.text)
            .bind(link.position as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to insert link", e))?;
        }

        tx.commit()
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to commit transaction", e))?;

        Ok(())
    }

    /// Get backlinks (notes that link to this note)
    pub async fn get_backlinks(&self, note_id: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT source_note_id
            FROM links
            WHERE target = ? AND link_type = 'internal'
            "#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to fetch backlinks", e))?;

        Ok(rows
            .into_iter()
            .map(|row| row.get("source_note_id"))
            .collect())
    }

    /// Rebuild the search index
    pub async fn rebuild_index(&self) -> Result<()> {
        sqlx::query("INSERT INTO notes_fts(notes_fts) VALUES('rebuild')")
            .execute(&self.pool)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to rebuild index", e))?;

        Ok(())
    }

    // Helper methods

    async fn update_note_tags(
        &self,
        tx: &mut SqliteConnection,
        note_id: &str,
        tags: &[String],
    ) -> Result<()> {
        // Delete existing tags
        sqlx::query("DELETE FROM note_tags WHERE note_id = ?")
            .bind(note_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to delete tags", e))?;

        // Insert new tags
        for tag in tags {
            // Get or create tag
            let tag_id = sqlx::query_scalar::<_, i64>(
                "INSERT OR IGNORE INTO tags (name) VALUES (?); SELECT id FROM tags WHERE name = ?",
            )
            .bind(tag)
            .bind(tag)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to get/create tag", e))?;

            // Link tag to note
            sqlx::query("INSERT INTO note_tags (note_id, tag_id) VALUES (?, ?)")
                .bind(note_id)
                .bind(tag_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| TeselaError::database_with_source("Failed to link tag", e))?;
        }

        Ok(())
    }

    async fn update_note_attachments(
        &self,
        tx: &mut SqliteConnection,
        note_id: &str,
        attachments: &[Attachment],
    ) -> Result<()> {
        // Delete existing attachments
        sqlx::query("DELETE FROM note_attachments WHERE note_id = ?")
            .bind(note_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to delete attachments", e))?;

        // Insert new attachments
        for attachment in attachments {
            // Insert or ignore attachment
            sqlx::query(
                r#"
                INSERT OR IGNORE INTO attachments (
                    id, filename, mime_type, size, checksum, path, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&attachment.id)
            .bind(&attachment.filename)
            .bind(&attachment.mime_type)
            .bind(attachment.size as i64)
            .bind(&attachment.checksum)
            .bind(attachment.path.to_str().unwrap_or(""))
            .bind(Utc::now().timestamp())
            .execute(&mut *tx)
            .await
            .map_err(|e| TeselaError::database_with_source("Failed to insert attachment", e))?;

            // Link attachment to note
            sqlx::query("INSERT INTO note_attachments (note_id, attachment_id) VALUES (?, ?)")
                .bind(note_id)
                .bind(&attachment.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| TeselaError::database_with_source("Failed to link attachment", e))?;
        }

        Ok(())
    }

    async fn get_note_attachments(&self, note_id: &str) -> Result<Vec<Attachment>> {
        let rows = sqlx::query(
            r#"
            SELECT a.id, a.filename, a.mime_type, a.size, a.checksum, a.path
            FROM attachments a
            JOIN note_attachments na ON a.id = na.attachment_id
            WHERE na.note_id = ?
            "#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| TeselaError::database_with_source("Failed to fetch attachments", e))?;

        let mut attachments = Vec::new();
        for row in rows {
            attachments.push(Attachment {
                id: row.get("id"),
                filename: row.get("filename"),
                mime_type: row.get("mime_type"),
                size: row.get::<i64, _>("size") as u64,
                checksum: row.get("checksum"),
                path: std::path::PathBuf::from(row.get::<String, _>("path")),
                note_ids: vec![note_id.to_string()],
            });
        }

        Ok(attachments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_database() -> (Database, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = DatabaseConfig {
            db_path: temp_dir.path().join("test.db"),
            ..Default::default()
        };
        let db = Database::new(config).await.unwrap();
        (db, temp_dir)
    }

    #[tokio::test]
    async fn test_database_initialization() {
        let (_db, _temp_dir) = create_test_database().await;
        // Database should be initialized without errors
    }

    #[tokio::test]
    async fn test_note_crud() {
        let (db, _temp_dir) = create_test_database().await;

        let note = Note {
            id: "test-note".to_string(),
            title: "Test Note".to_string(),
            content: "# Test Note\n\nContent".to_string(),
            body: "Content".to_string(),
            metadata: NoteMetadata {
                tags: vec!["test".to_string(), "example".to_string()],
                ..Default::default()
            },
            path: std::path::PathBuf::from("notes/test-note.md"),
            checksum: "abc123".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: Vec::new(),
        };

        // Insert note
        db.upsert_note(&note).await.unwrap();

        // Get note
        let retrieved = db.get_note("test-note").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.title, "Test Note");
        assert_eq!(retrieved.metadata.tags, vec!["test", "example"]);

        // Update note
        let mut updated_note = note.clone();
        updated_note.title = "Updated Test Note".to_string();
        db.upsert_note(&updated_note).await.unwrap();

        let retrieved = db.get_note("test-note").await.unwrap().unwrap();
        assert_eq!(retrieved.title, "Updated Test Note");

        // Delete note
        db.delete_note("test-note").await.unwrap();
        let retrieved = db.get_note("test-note").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_full_text_search() {
        let (db, _temp_dir) = create_test_database().await;

        // Insert test notes
        let notes = vec![
            Note {
                id: "note1".to_string(),
                title: "Rust Programming".to_string(),
                content: "Content about Rust".to_string(),
                body: "Learn about ownership and borrowing in Rust".to_string(),
                metadata: NoteMetadata::default(),
                path: std::path::PathBuf::from("note1.md"),
                checksum: "123".to_string(),
                created_at: Utc::now(),
                modified_at: Utc::now(),
                attachments: Vec::new(),
            },
            Note {
                id: "note2".to_string(),
                title: "Python Tutorial".to_string(),
                content: "Content about Python".to_string(),
                body: "Python is great for data science and machine learning".to_string(),
                metadata: NoteMetadata::default(),
                path: std::path::PathBuf::from("note2.md"),
                checksum: "456".to_string(),
                created_at: Utc::now(),
                modified_at: Utc::now(),
                attachments: Vec::new(),
            },
        ];

        for note in &notes {
            db.upsert_note(note).await.unwrap();
        }

        // Search for "Rust"
        let results = db.search_notes("Rust", 10, 0).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "note1");

        // Search for "Python"
        let results = db.search_notes("Python", 10, 0).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "note2");
    }

    #[tokio::test]
    async fn test_tags() {
        let (db, _temp_dir) = create_test_database().await;

        let note1 = Note {
            id: "note1".to_string(),
            title: "Note 1".to_string(),
            content: "Content 1".to_string(),
            body: "Body 1".to_string(),
            metadata: NoteMetadata {
                tags: vec!["rust".to_string(), "programming".to_string()],
                ..Default::default()
            },
            path: std::path::PathBuf::from("note1.md"),
            checksum: "123".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: Vec::new(),
        };

        let note2 = Note {
            id: "note2".to_string(),
            title: "Note 2".to_string(),
            content: "Content 2".to_string(),
            body: "Body 2".to_string(),
            metadata: NoteMetadata {
                tags: vec!["rust".to_string(), "async".to_string()],
                ..Default::default()
            },
            path: std::path::PathBuf::from("note2.md"),
            checksum: "456".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: Vec::new(),
        };

        // Insert notes
        db.upsert_note(&note1).await.unwrap();
        db.upsert_note(&note2).await.unwrap();

        // Get notes by tag "rust"
        let rust_notes = db.get_notes_by_tag("rust").await.unwrap();
        assert_eq!(rust_notes.len(), 2);

        // Get notes by tag "programming"
        let prog_notes = db.get_notes_by_tag("programming").await.unwrap();
        assert_eq!(prog_notes.len(), 1);
        assert_eq!(prog_notes[0].id, "note1");

        // Get tags with counts
        let tags = db.get_tags_with_counts().await.unwrap();
        assert_eq!(tags.get("rust"), Some(&2));
        assert_eq!(tags.get("programming"), Some(&1));
        assert_eq!(tags.get("async"), Some(&1));
    }

    #[tokio::test]
    async fn test_backlinks() {
        let (db, _temp_dir) = create_test_database().await;

        // Create notes
        let note1 = Note {
            id: "note1".to_string(),
            title: "Note 1".to_string(),
            content: "Links to [[note2]]".to_string(),
            body: "Links to note2".to_string(),
            metadata: NoteMetadata::default(),
            path: std::path::PathBuf::from("note1.md"),
            checksum: "123".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: Vec::new(),
        };

        db.upsert_note(&note1).await.unwrap();

        // Update links
        let links = vec![Link {
            link_type: LinkType::Internal,
            target: "note2".to_string(),
            text: "note2".to_string(),
            position: 0,
        }];

        db.update_note_links("note1", &links).await.unwrap();

        // Get backlinks for note2
        let backlinks = db.get_backlinks("note2").await.unwrap();
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0], "note1");
    }

    #[tokio::test]
    async fn test_rebuild_index() {
        let (db, _temp_dir) = create_test_database().await;

        // Should succeed without error
        db.rebuild_index().await.unwrap();
    }
}
