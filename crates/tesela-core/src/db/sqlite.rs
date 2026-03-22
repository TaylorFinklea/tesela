//! SQLite+FTS5 implementation of SearchIndex and LinkGraph traits

use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Row, Sqlite};
use std::path::Path;
use std::str::FromStr;
use tracing::debug;

use super::queries;
use super::schema;
use crate::error::{Result, TeselaError};
use crate::link::{Link, LinkType};
use crate::note::{Note, NoteId, SearchHit};
use crate::traits::link_graph::LinkGraph;
use crate::traits::search_index::SearchIndex;

fn db_err(msg: &str, e: sqlx::Error) -> TeselaError {
    TeselaError::Database {
        message: format!("{}: {}", msg, e),
        source: None,
    }
}

/// SQLite-backed search index and link graph.
///
/// SQLite is treated as a **cache** of the filesystem. If the database file
/// is lost, `rebuild_from_notes()` reconstructs it from the on-disk notes.
pub struct SqliteIndex {
    pool: Pool<Sqlite>,
}

impl SqliteIndex {
    /// Open (or create) a SQLite database at the given path.
    pub async fn open(path: &Path) -> Result<Self> {
        let db_path = path.to_str().unwrap_or("tesela.db");
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path))
            .map_err(|e| db_err("Failed to parse connection string", e))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .map_err(|e| db_err("Failed to connect to database", e))?;

        Self::migrate(&pool).await?;

        Ok(Self { pool })
    }

    /// Open an in-memory SQLite database (for testing).
    pub async fn open_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .map_err(|e| db_err("Failed to parse connection string", e))?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| db_err("Failed to connect to in-memory database", e))?;

        Self::migrate(&pool).await?;

        Ok(Self { pool })
    }

    /// Run schema migrations.
    async fn migrate(pool: &Pool<Sqlite>) -> Result<()> {
        // Create migrations tracking table
        sqlx::query(schema::CREATE_MIGRATIONS_TABLE)
            .execute(pool)
            .await
            .map_err(|e| db_err("Failed to create migrations table", e))?;

        for (name, statements) in schema::MIGRATIONS {
            // Check if migration was already applied
            let applied: Option<i64> =
                sqlx::query_scalar("SELECT version FROM schema_migrations WHERE version = ?")
                    .bind(schema::SCHEMA_VERSION)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| db_err("Failed to check migration status", e))?;

            if applied.is_some() {
                debug!("Migration {} already applied, skipping", name);
                continue;
            }

            debug!("Applying migration: {}", name);

            for statement in *statements {
                sqlx::query(statement)
                    .execute(pool)
                    .await
                    .map_err(|e| db_err(&format!("Failed to apply migration {}", name), e))?;
            }

            // Record migration
            sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
                .bind(schema::SCHEMA_VERSION)
                .execute(pool)
                .await
                .map_err(|e| db_err("Failed to record migration", e))?;
        }

        Ok(())
    }

    /// Upsert a note into the index (insert or update).
    ///
    /// Uses UPDATE + INSERT instead of INSERT OR REPLACE to preserve the SQLite rowid.
    /// The content FTS5 table (`content=notes, content_rowid=rowid`) references notes by rowid;
    /// INSERT OR REPLACE silently changes the rowid (delete + re-insert), causing
    /// SQLITE_CORRUPT_VTAB (267) on the next search because the FTS5 index holds the old rowid.
    pub async fn upsert_note(&self, note: &Note) -> Result<()> {
        let tags_json = serde_json::to_string(&note.metadata.tags).map_err(TeselaError::Json)?;

        // Try to UPDATE first — this preserves the rowid so FTS5 triggers stay consistent.
        let updated = sqlx::query(
            r#"
            UPDATE notes
            SET title = ?, body = ?, content = ?, path = ?, checksum = ?,
                modified_at = ?, tags = ?
            WHERE id = ?
            "#,
        )
        .bind(&note.title)
        .bind(&note.body)
        .bind(&note.content)
        .bind(note.path.to_str().unwrap_or(""))
        .bind(&note.checksum)
        .bind(note.modified_at.to_rfc3339())
        .bind(&tags_json)
        .bind(note.id.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("Failed to update note", e))?;

        // If no row was modified, the note is new — INSERT it.
        if updated.rows_affected() == 0 {
            sqlx::query(
                r#"
                INSERT INTO notes (
                    id, title, body, content, path, checksum, created_at, modified_at, tags
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(note.id.as_str())
            .bind(&note.title)
            .bind(&note.body)
            .bind(&note.content)
            .bind(note.path.to_str().unwrap_or(""))
            .bind(&note.checksum)
            .bind(note.created_at.to_rfc3339())
            .bind(note.modified_at.to_rfc3339())
            .bind(&tags_json)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to insert note", e))?;
        }

        Ok(())
    }

    /// Remove a note from the index.
    pub async fn remove_note(&self, id: &NoteId) -> Result<()> {
        sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to remove note", e))?;

        Ok(())
    }

    /// Rebuild the entire index from a slice of notes.
    ///
    /// This is used when the database is lost or out of sync with the filesystem.
    pub async fn rebuild_from_notes(&self, notes: &[Note]) -> Result<usize> {
        // Clear existing data
        sqlx::query("DELETE FROM links")
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to clear links", e))?;

        sqlx::query("DELETE FROM notes")
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to clear notes", e))?;

        // Re-insert all notes
        for note in notes {
            self.upsert_note(note).await?;
        }

        Ok(notes.len())
    }

    /// Return all distinct tags across all indexed notes, sorted alphabetically.
    pub async fn list_tags(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT value
            FROM notes, json_each(notes.tags)
            ORDER BY value
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to list tags", e))?;

        let tags: Vec<String> = rows
            .iter()
            .map(|row| row.get::<String, _>("value"))
            .collect();
        Ok(tags)
    }

    /// Prepare an FTS5 query string with proper escaping and prefix matching.
    fn prepare_fts_query(query: &str) -> String {
        let query = query.trim();

        // Pass through boolean operators as-is
        if query.contains(" AND ") || query.contains(" OR ") || query.contains(" NOT ") {
            return query.to_string();
        }

        // Pass through phrase searches as-is
        if query.starts_with('"') && query.ends_with('"') {
            return query.to_string();
        }

        // Pass through explicit prefix searches
        if query.ends_with('*') {
            return query.to_string();
        }

        // For simple queries: escape special chars, add prefix matching on last token
        let words: Vec<&str> = query.split_whitespace().collect();
        if words.is_empty() {
            return query.to_string();
        }

        let mut parts: Vec<String> = Vec::new();
        for (i, word) in words.iter().enumerate() {
            let escaped = word.replace('"', "\"\"");
            if i == words.len() - 1 {
                // Add prefix matching on the last token
                parts.push(format!("\"{}\"*", escaped));
            } else {
                parts.push(format!("\"{}\"", escaped));
            }
        }

        parts.join(" ")
    }
}

#[async_trait]
impl SearchIndex for SqliteIndex {
    async fn search(&self, query: &str, limit: usize, offset: usize) -> Result<Vec<SearchHit>> {
        let fts_query = Self::prepare_fts_query(query);

        let rows = sqlx::query(
            r#"
            SELECT n.id, n.title, n.path, n.tags,
                   snippet(notes_fts, 2, '<b>', '</b>', '...', 32) as snippet,
                   notes_fts.rank as rank
            FROM notes_fts
            JOIN notes n ON notes_fts.id = n.id
            WHERE notes_fts MATCH ?
            ORDER BY rank
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(&fts_query)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to search notes", e))?;

        let mut results = Vec::new();
        for row in &rows {
            results.push(queries::row_to_search_hit(row)?);
        }
        Ok(results)
    }

    async fn suggest(&self, partial: &str) -> Result<Vec<String>> {
        let fts_query = format!("\"{}\"*", partial.trim().replace('"', "\"\""));

        let rows = sqlx::query(
            r#"
            SELECT DISTINCT n.title
            FROM notes_fts
            JOIN notes n ON notes_fts.id = n.id
            WHERE notes_fts MATCH ?
            ORDER BY notes_fts.rank
            LIMIT 10
            "#,
        )
        .bind(&fts_query)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to suggest", e))?;

        let suggestions: Vec<String> = rows
            .iter()
            .map(|row| row.get::<String, _>("title"))
            .collect();
        Ok(suggestions)
    }

    async fn reindex(&self, note: &Note) -> Result<()> {
        self.upsert_note(note).await
    }

    async fn remove(&self, id: &NoteId) -> Result<()> {
        self.remove_note(id).await
    }

    async fn rebuild(&self) -> Result<usize> {
        // Rebuild FTS from the notes table (in case FTS got out of sync)
        sqlx::query("INSERT INTO notes_fts(notes_fts) VALUES('rebuild')")
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to rebuild FTS index", e))?;

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM notes")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| db_err("Failed to count notes", e))?;

        Ok(count as usize)
    }
}

#[async_trait]
impl LinkGraph for SqliteIndex {
    async fn get_backlinks(&self, id: &NoteId) -> Result<Vec<Link>> {
        let rows = sqlx::query(
            r#"
            SELECT source_id, target, link_text, position, link_type
            FROM links WHERE target = ?
            "#,
        )
        .bind(id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to get backlinks", e))?;

        let mut links = Vec::new();
        for row in &rows {
            links.push(row_to_link(row)?);
        }
        Ok(links)
    }

    async fn get_forward_links(&self, id: &NoteId) -> Result<Vec<Link>> {
        let rows = sqlx::query(
            r#"
            SELECT source_id, target, link_text, position, link_type
            FROM links WHERE source_id = ?
            "#,
        )
        .bind(id.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to get forward links", e))?;

        let mut links = Vec::new();
        for row in &rows {
            links.push(row_to_link(row)?);
        }
        Ok(links)
    }

    async fn get_all_edges(&self) -> Result<Vec<crate::link::GraphEdge>> {
        use sqlx::Row;
        let rows = sqlx::query(
            "SELECT DISTINCT source_id, target FROM links WHERE link_type = 'internal'",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to get all edges", e))?;

        Ok(rows
            .iter()
            .map(|row| crate::link::GraphEdge {
                source: row.get("source_id"),
                target: row.get("target"),
            })
            .collect())
    }

    async fn update_links(&self, id: &NoteId, links: &[Link]) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_err("Failed to begin transaction", e))?;

        // Delete existing links from this source
        sqlx::query("DELETE FROM links WHERE source_id = ?")
            .bind(id.as_str())
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err("Failed to delete old links", e))?;

        // Insert new links
        for link in links {
            let link_type_str = match link.link_type {
                LinkType::Internal => "internal",
                LinkType::External => "external",
                LinkType::Attachment => "attachment",
            };

            sqlx::query(
                r#"
                INSERT INTO links (source_id, target, link_text, position, link_type)
                VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(id.as_str())
            .bind(&link.target)
            .bind(&link.text)
            .bind(link.position as i64)
            .bind(link_type_str)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err("Failed to insert link", e))?;
        }

        tx.commit()
            .await
            .map_err(|e| db_err("Failed to commit transaction", e))?;

        Ok(())
    }

    async fn remove_links(&self, id: &NoteId) -> Result<()> {
        sqlx::query("DELETE FROM links WHERE source_id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to remove links", e))?;

        Ok(())
    }
}

/// Map a link row to a Link struct.
fn row_to_link(row: &sqlx::sqlite::SqliteRow) -> Result<Link> {
    let target: String = row.try_get("target").map_err(|e| TeselaError::Database {
        message: e.to_string(),
        source: None,
    })?;
    let link_text: String = row
        .try_get("link_text")
        .map_err(|e| TeselaError::Database {
            message: e.to_string(),
            source: None,
        })?;
    let position: i64 = row.try_get("position").map_err(|e| TeselaError::Database {
        message: e.to_string(),
        source: None,
    })?;
    let link_type_str: String = row
        .try_get("link_type")
        .map_err(|e| TeselaError::Database {
            message: e.to_string(),
            source: None,
        })?;

    let link_type = match link_type_str.as_str() {
        "external" => LinkType::External,
        "attachment" => LinkType::Attachment,
        _ => LinkType::Internal,
    };

    Ok(Link {
        link_type,
        target,
        text: link_text,
        position: position as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::NoteMetadata;
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_test_note(id: &str, title: &str, body: &str, tags: &[&str]) -> Note {
        let tags: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
        Note {
            id: NoteId::new(id),
            title: title.to_string(),
            content: format!("# {}\n\n{}", title, body),
            body: body.to_string(),
            metadata: NoteMetadata {
                title: None,
                tags,
                aliases: vec![],
                custom: Default::default(),
                created: None,
                modified: None,
            },
            path: PathBuf::from(format!("notes/{}.md", id)),
            checksum: format!("checksum-{}", id),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: vec![],
        }
    }

    #[tokio::test]
    async fn test_upsert_and_search() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let note = make_test_note(
            "test-1",
            "Rust Programming",
            "Rust is a systems language",
            &["rust", "programming"],
        );
        index.upsert_note(&note).await.unwrap();

        let results = index.search("Rust", 10, 0).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].note_id.as_str(), "test-1");
    }

    #[tokio::test]
    async fn test_search_fts5_multiple_terms() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        let note1 = make_test_note(
            "note-1",
            "Rust Programming Guide",
            "Learn about ownership and borrowing in Rust",
            &["rust"],
        );
        let note2 = make_test_note(
            "note-2",
            "Python Programming Guide",
            "Python is great for data science",
            &["python"],
        );

        index.upsert_note(&note1).await.unwrap();
        index.upsert_note(&note2).await.unwrap();

        // Search for "Rust" should only match note-1
        let results = index.search("Rust", 10, 0).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_id.as_str(), "note-1");

        // Search for "Programming" should match both
        let results = index.search("Programming", 10, 0).await.unwrap();
        assert_eq!(results.len(), 2);

        // Search for "Python data" should match note-2
        let results = index.search("Python data", 10, 0).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_id.as_str(), "note-2");
    }

    #[tokio::test]
    async fn test_remove_from_index() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        let note = make_test_note("rm-1", "Removable Note", "This will be removed", &["temp"]);
        index.upsert_note(&note).await.unwrap();

        // Verify it exists
        let results = index.search("Removable", 10, 0).await.unwrap();
        assert_eq!(results.len(), 1);

        // Remove it
        index.remove_note(&NoteId::new("rm-1")).await.unwrap();

        // Verify it is gone
        let results = index.search("Removable", 10, 0).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_link_graph_forward_links() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        let note = make_test_note("src-1", "Source Note", "Links to target", &[]);
        index.upsert_note(&note).await.unwrap();

        let links = vec![
            Link {
                link_type: LinkType::Internal,
                target: "target-1".to_string(),
                text: "Target 1".to_string(),
                position: 10,
            },
            Link {
                link_type: LinkType::External,
                target: "https://example.com".to_string(),
                text: "Example".to_string(),
                position: 50,
            },
        ];

        index
            .update_links(&NoteId::new("src-1"), &links)
            .await
            .unwrap();

        let forward = index
            .get_forward_links(&NoteId::new("src-1"))
            .await
            .unwrap();
        assert_eq!(forward.len(), 2);
        assert_eq!(forward[0].target, "target-1");
        assert_eq!(forward[1].target, "https://example.com");
    }

    #[tokio::test]
    async fn test_link_graph_backlinks() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // Create two source notes that link to the same target
        let note1 = make_test_note("src-a", "Source A", "body a", &[]);
        let note2 = make_test_note("src-b", "Source B", "body b", &[]);
        index.upsert_note(&note1).await.unwrap();
        index.upsert_note(&note2).await.unwrap();

        index
            .update_links(
                &NoteId::new("src-a"),
                &[Link {
                    link_type: LinkType::Internal,
                    target: "shared-target".to_string(),
                    text: "shared".to_string(),
                    position: 0,
                }],
            )
            .await
            .unwrap();

        index
            .update_links(
                &NoteId::new("src-b"),
                &[Link {
                    link_type: LinkType::Internal,
                    target: "shared-target".to_string(),
                    text: "shared".to_string(),
                    position: 0,
                }],
            )
            .await
            .unwrap();

        let backlinks = index
            .get_backlinks(&NoteId::new("shared-target"))
            .await
            .unwrap();
        assert_eq!(backlinks.len(), 2);
    }

    #[tokio::test]
    async fn test_link_graph_update_removes_old() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        let note = make_test_note("src-u", "Updater", "body", &[]);
        index.upsert_note(&note).await.unwrap();

        // First set of links
        index
            .update_links(
                &NoteId::new("src-u"),
                &[Link {
                    link_type: LinkType::Internal,
                    target: "old-target".to_string(),
                    text: "old".to_string(),
                    position: 0,
                }],
            )
            .await
            .unwrap();

        // Update with new links (old ones should be gone)
        index
            .update_links(
                &NoteId::new("src-u"),
                &[Link {
                    link_type: LinkType::Internal,
                    target: "new-target".to_string(),
                    text: "new".to_string(),
                    position: 0,
                }],
            )
            .await
            .unwrap();

        let forward = index
            .get_forward_links(&NoteId::new("src-u"))
            .await
            .unwrap();
        assert_eq!(forward.len(), 1);
        assert_eq!(forward[0].target, "new-target");

        // Old target should have no backlinks
        let backlinks = index
            .get_backlinks(&NoteId::new("old-target"))
            .await
            .unwrap();
        assert!(backlinks.is_empty());
    }

    #[tokio::test]
    async fn test_rebuild_fts_index() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        let note1 = make_test_note("rb-1", "Rebuild Test One", "first note body", &["a"]);
        let note2 = make_test_note("rb-2", "Rebuild Test Two", "second note body", &["b"]);

        index.upsert_note(&note1).await.unwrap();
        index.upsert_note(&note2).await.unwrap();

        let count = index.rebuild().await.unwrap();
        assert_eq!(count, 2);

        // Search should still work after rebuild
        let results = index.search("Rebuild", 10, 0).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_schema_migration_idempotent() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let index1 = SqliteIndex::open(tmp.path()).await.unwrap();
        drop(index1);
        // Opening a second time should not fail (migrations already applied)
        let _index2 = SqliteIndex::open(tmp.path()).await.unwrap();
    }

    #[tokio::test]
    async fn test_rebuild_from_notes() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        let notes = vec![
            make_test_note("rfn-1", "Note One", "first", &["a"]),
            make_test_note("rfn-2", "Note Two", "second", &["b"]),
            make_test_note("rfn-3", "Note Three", "third", &["c"]),
        ];

        let count = index.rebuild_from_notes(&notes).await.unwrap();
        assert_eq!(count, 3);

        let results = index.search("Note", 10, 0).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_list_tags() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        let note1 = make_test_note("t1", "Note One", "body", &["rust", "programming"]);
        let note2 = make_test_note("t2", "Note Two", "body", &["rust", "tui"]);
        let note3 = make_test_note("t3", "Note Three", "body", &[]);

        index.upsert_note(&note1).await.unwrap();
        index.upsert_note(&note2).await.unwrap();
        index.upsert_note(&note3).await.unwrap();

        let tags = index.list_tags().await.unwrap();
        assert_eq!(tags, vec!["programming", "rust", "tui"]);
    }

    #[tokio::test]
    async fn test_suggest() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        let note = make_test_note("sug-1", "Suggestion Test", "some body text", &[]);
        index.upsert_note(&note).await.unwrap();

        let suggestions = index.suggest("Suggest").await.unwrap();
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0], "Suggestion Test");
    }
}
