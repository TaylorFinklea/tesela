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

const DEFAULT_MAX_CONNECTIONS: u32 = 5;
const IN_MEMORY_MAX_CONNECTIONS: u32 = 1;

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
            .max_connections(DEFAULT_MAX_CONNECTIONS)
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
            .max_connections(IN_MEMORY_MAX_CONNECTIONS)
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

        for (idx, (name, statements)) in schema::MIGRATIONS.iter().enumerate() {
            let version = (idx + 1) as i64;

            // Check if migration was already applied
            let applied: Option<i64> =
                sqlx::query_scalar("SELECT version FROM schema_migrations WHERE version = ?")
                    .bind(version)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| db_err("Failed to check migration status", e))?;

            if applied.is_some() {
                debug!("Migration {} already applied, skipping", name);
                continue;
            }

            debug!("Applying migration: {}", name);

            for statement in *statements {
                sqlx::query(statement).execute(pool).await.map_err(|e| {
                    db_err(
                        &format!("Failed to apply migration {}: {}", name, statement),
                        e,
                    )
                })?;
            }

            // Record migration
            sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
                .bind(version)
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
                modified_at = ?, tags = ?, note_type = ?
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
        .bind(note.metadata.note_type.as_deref())
        .bind(note.id.as_str())
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("Failed to update note", e))?;

        // If no row was modified, the note is new — INSERT it.
        if updated.rows_affected() == 0 {
            sqlx::query(
                r#"
                INSERT INTO notes (
                    id, title, body, content, path, checksum, created_at, modified_at, tags, note_type
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            .bind(note.metadata.note_type.as_deref())
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

    /// Index type system info: if note is a Tag or Property page, cache its definition.
    async fn index_type_info(&self, note: &Note) -> Result<()> {
        match note.metadata.note_type.as_deref() {
            Some("Tag") => {
                // Extract tag_properties from frontmatter custom fields
                let props_json = note
                    .metadata
                    .custom
                    .get("tag_properties")
                    .and_then(|v| serde_json::to_string(v).ok())
                    .unwrap_or_else(|| "[]".to_string());
                let extends = note
                    .metadata
                    .custom
                    .get("extends")
                    .and_then(|v| v.as_str().map(String::from));
                let icon = note
                    .metadata
                    .custom
                    .get("icon")
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "📄".to_string());
                let color = note
                    .metadata
                    .custom
                    .get("color")
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "#808080".to_string());

                sqlx::query(
                    "INSERT OR REPLACE INTO tag_defs (id, name, extends, icon, color, properties_json, note_id) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(note.id.as_str())
                .bind(&note.title)
                .bind(&extends)
                .bind(&icon)
                .bind(&color)
                .bind(&props_json)
                .bind(note.id.as_str())
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("Failed to index tag def", e))?;
            }
            Some("Property") => {
                let value_type = note
                    .metadata
                    .custom
                    .get("value_type")
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| "text".to_string());
                let choices_json = note
                    .metadata
                    .custom
                    .get("choices")
                    .and_then(|v| serde_json::to_string(v).ok());
                let default_value = note
                    .metadata
                    .custom
                    .get("default")
                    .and_then(|v| v.as_str().map(String::from));
                let multiple = note
                    .metadata
                    .custom
                    .get("multiple_values")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let hide_empty = note
                    .metadata
                    .custom
                    .get("hide_empty")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let description = note
                    .metadata
                    .custom
                    .get("description")
                    .and_then(|v| v.as_str().map(String::from));

                sqlx::query(
                    "INSERT OR REPLACE INTO property_defs (id, name, value_type, choices_json, default_value, multiple_values, hide_empty, description, note_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(note.id.as_str())
                .bind(&note.title)
                .bind(&value_type)
                .bind(&choices_json)
                .bind(&default_value)
                .bind(multiple)
                .bind(hide_empty)
                .bind(&description)
                .bind(note.id.as_str())
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("Failed to index property def", e))?;
            }
            _ => {}
        }

        // Index block-level properties into block_properties table
        self.index_block_properties(note).await?;

        Ok(())
    }

    /// Parse blocks from note body and index their properties.
    async fn index_block_properties(&self, note: &Note) -> Result<()> {
        use crate::block::parse_blocks;

        // Delete existing block properties for this note
        sqlx::query("DELETE FROM block_properties WHERE note_id = ?")
            .bind(note.id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to delete old block properties", e))?;

        // Parse blocks and insert properties
        let blocks = parse_blocks(note.id.as_str(), &note.body);
        for block in &blocks {
            for (key, value) in &block.properties {
                sqlx::query(
                    "INSERT OR REPLACE INTO block_properties (block_id, note_id, property_id, property_name, value) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(&block.id)
                .bind(note.id.as_str())
                .bind(format!("{}:{}", key.to_lowercase(), block.id)) // property_id = key:block_id
                .bind(key)
                .bind(value)
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("Failed to index block property", e))?;
            }
        }

        Ok(())
    }

    /// Take a consistent snapshot of the database into `target` via
    /// SQLite's `VACUUM INTO`. Unlike a raw `fs::copy` of `tesela.db`,
    /// this is safe while the database is open in WAL mode — `VACUUM
    /// INTO` produces a self-contained, fully-merged copy. Used by the
    /// backup pipeline.
    pub async fn vacuum_into(&self, target: &std::path::Path) -> Result<()> {
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TeselaError::Database {
                message: format!("create snapshot parent dir: {}", e),
                source: None,
            })?;
        }
        if target.exists() {
            std::fs::remove_file(target).map_err(|e| TeselaError::Database {
                message: format!("clear existing snapshot {}: {}", target.display(), e),
                source: None,
            })?;
        }
        let target_str = target
            .to_str()
            .ok_or_else(|| TeselaError::Database {
                message: format!("snapshot path is not valid UTF-8: {}", target.display()),
                source: None,
            })?
            .replace('\'', "''");
        let stmt = format!("VACUUM INTO '{}'", target_str);
        sqlx::query(&stmt)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("VACUUM INTO failed", e))?;
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

    /// Get all property definitions from the cache.
    pub async fn get_all_property_defs(&self) -> Result<Vec<crate::types::PropertyDef>> {
        use sqlx::Row;
        let rows = sqlx::query("SELECT name, value_type, choices_json, default_value, multiple_values, hide_empty, description FROM property_defs ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_err("Failed to get property defs", e))?;

        Ok(rows
            .iter()
            .map(|row| {
                let choices_str: Option<String> = row.get("choices_json");
                let choices: Option<Vec<String>> =
                    choices_str.and_then(|s| serde_json::from_str(&s).ok());
                crate::types::PropertyDef {
                    name: row.get("name"),
                    value_type: row.get("value_type"),
                    values: choices,
                    default: row.get("default_value"),
                    required: false,
                    ..Default::default()
                }
            })
            .collect())
    }

    /// Get a single tag definition with resolved property schemas (walks extends chain).
    pub async fn get_resolved_tag_def(
        &self,
        name: &str,
    ) -> Result<Option<crate::types::TypeDefinition>> {
        use sqlx::Row;

        // Collect properties by walking the extends chain (child → parent → root)
        let mut all_property_names: Vec<String> = Vec::new();
        let mut current_name = name.to_string();
        let mut icon = "📄".to_string();
        let mut color = "#808080".to_string();
        let mut depth = 0;

        loop {
            if depth > 10 {
                break;
            } // prevent infinite loops
            depth += 1;

            let row = sqlx::query(
                "SELECT name, extends, icon, color, properties_json FROM tag_defs WHERE LOWER(name) = LOWER(?)"
            )
            .bind(&current_name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("Failed to get tag def", e))?;

            match row {
                Some(row) => {
                    if depth == 1 {
                        icon = row.get("icon");
                        color = row.get("color");
                    }
                    let props_str: String = row.get("properties_json");
                    let props: Vec<String> = serde_json::from_str(&props_str).unwrap_or_default();
                    // Prepend parent properties (parent first, child overrides)
                    all_property_names.extend(props);

                    let extends: Option<String> = row.get("extends");
                    match extends {
                        Some(parent) if !parent.is_empty() => current_name = parent,
                        _ => break,
                    }
                }
                None => break,
            }
        }

        if depth == 0 {
            return Ok(None);
        }

        // Deduplicate (child properties take precedence)
        let mut seen = std::collections::HashSet::new();
        all_property_names.retain(|p| seen.insert(p.clone()));

        // Resolve property definitions from property_defs table
        let mut resolved_props = Vec::new();
        for prop_name in &all_property_names {
            let prop_row = sqlx::query(
                "SELECT name, value_type, choices_json, default_value, multiple_values, hide_empty, description FROM property_defs WHERE LOWER(name) = LOWER(?)"
            )
            .bind(prop_name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("Failed to resolve property", e))?;

            match prop_row {
                Some(row) => {
                    let choices_str: Option<String> = row.get("choices_json");
                    resolved_props.push(crate::types::PropertyDef {
                        name: row.get("name"),
                        value_type: row.get("value_type"),
                        values: choices_str.and_then(|s| serde_json::from_str(&s).ok()),
                        default: row.get("default_value"),
                        required: false,
                        ..Default::default()
                    });
                }
                None => {
                    // Property page doesn't exist yet — show as text
                    resolved_props.push(crate::types::PropertyDef {
                        name: prop_name.clone(),
                        value_type: "text".to_string(),
                        values: None,
                        default: None,
                        required: false,
                        ..Default::default()
                    });
                }
            }
        }

        Ok(Some(crate::types::TypeDefinition {
            name: name.to_string(),
            description: String::new(),
            icon,
            color,
            properties: resolved_props,
        }))
    }

    /// Get all tag definitions from the cache.
    pub async fn get_all_tag_defs(&self) -> Result<Vec<crate::types::TypeDefinition>> {
        use sqlx::Row;
        let rows = sqlx::query(
            "SELECT name, extends, icon, color, properties_json FROM tag_defs ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to get tag defs", e))?;

        let mut result = Vec::new();
        for row in &rows {
            let props_str: String = row.get("properties_json");
            let prop_names: Vec<String> = serde_json::from_str(&props_str).unwrap_or_default();

            // Resolve each property name against property_defs for full schema
            let mut resolved_props = Vec::new();
            for pname in &prop_names {
                let prop_row = sqlx::query(
                    "SELECT name, value_type, choices_json, default_value FROM property_defs WHERE LOWER(name) = LOWER(?)"
                )
                .bind(pname)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| db_err("Failed to resolve property in get_all_tag_defs", e))?;

                match prop_row {
                    Some(pr) => {
                        let choices_str: Option<String> = pr.get("choices_json");
                        resolved_props.push(crate::types::PropertyDef {
                            name: pr.get("name"),
                            value_type: pr.get("value_type"),
                            values: choices_str.and_then(|s| serde_json::from_str(&s).ok()),
                            default: pr.get("default_value"),
                            required: false,
                            ..Default::default()
                        });
                    }
                    None => {
                        resolved_props.push(crate::types::PropertyDef {
                            name: pname.clone(),
                            value_type: "text".to_string(),
                            values: None,
                            default: None,
                            required: false,
                            ..Default::default()
                        });
                    }
                }
            }

            result.push(crate::types::TypeDefinition {
                name: row.get("name"),
                description: String::new(),
                icon: row.get("icon"),
                color: row.get("color"),
                properties: resolved_props,
            });
        }
        Ok(result)
    }

    /// Get all blocks tagged with a specific type, with their properties from the DB index.
    pub async fn get_typed_blocks(&self, tag_name: &str) -> Result<Vec<crate::block::ParsedBlock>> {
        use sqlx::Row;

        // Find notes containing the tag name anywhere in body text (matches
        // inline `#TagName` AND `tags:: TagName` continuation syntax) OR in
        // frontmatter tags array. Phase 11 — relaxed from `%#TagName%` to
        // `%TagName%` so blocks tagged via the canonical `tags::` continuation
        // line (rather than the legacy `#tag` token) are included. The
        // `block.tags.iter().any(...)` check below filters precisely.
        let notes =
            sqlx::query("SELECT id, title, body FROM notes WHERE body LIKE ? OR tags LIKE ?")
                .bind(format!("%{}%", tag_name))
                .bind(format!("%\"{}%", tag_name))
                .fetch_all(&self.pool)
                .await
                .map_err(|e| db_err("Failed to get typed notes", e))?;

        let mut result = Vec::new();
        for row in &notes {
            let note_id: String = row.get("id");
            let body: String = row.get("body");
            let blocks = crate::block::parse_blocks(&note_id, &body);
            for mut block in blocks {
                if block.tags.iter().any(|t| t.eq_ignore_ascii_case(tag_name)) {
                    // Enrich with property values from DB index (more reliable than re-parsing)
                    let prop_rows = sqlx::query(
                        "SELECT property_name, value FROM block_properties WHERE block_id = ?",
                    )
                    .bind(&block.id)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| db_err("Failed to get block properties", e))?;

                    block.properties.clear();
                    for pr in &prop_rows {
                        let key: String = pr.get("property_name");
                        let value: Option<String> = pr.get("value");
                        if let Some(v) = value {
                            block.properties.insert(key, v);
                        }
                    }
                    result.push(block);
                }
            }
        }

        Ok(result)
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
        self.upsert_note(note).await?;
        self.index_type_info(note).await?;
        Ok(())
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

    async fn execute_query(
        &self,
        query: &crate::query::ParsedQuery,
        group: Option<&str>,
        sort: Option<&str>,
    ) -> Result<crate::query::QueryResult> {
        use crate::query::{Kind, QueryResult};
        let mut items = match query.kind {
            Kind::Block => self.execute_block_query(query).await?,
            Kind::Page => self.execute_page_query(query).await?,
        };
        apply_sort(&mut items, sort);
        let groups = apply_group(items, group);
        Ok(QueryResult { groups })
    }

    async fn record_version(
        &self,
        note_id: &NoteId,
        prev_content: Option<&str>,
        new_content: &str,
        cap: usize,
    ) -> Result<i64> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_err("Failed to begin tx for record_version", e))?;

        // Compute the next version number for this note.
        let next: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(version_number), 0) + 1 FROM note_versions WHERE note_id = ?",
        )
        .bind(note_id.as_str())
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_err("Failed to compute next version_number", e))?;

        sqlx::query(
            r#"INSERT INTO note_versions (note_id, version_number, content, prev_content)
               VALUES (?, ?, ?, ?)"#,
        )
        .bind(note_id.as_str())
        .bind(next)
        .bind(new_content)
        .bind(prev_content)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err("Failed to insert note version", e))?;

        // Prune oldest beyond cap. Inline the cap into the SQL since SQLite
        // doesn't accept LIMIT params on subqueries reliably across versions.
        if cap > 0 {
            let prune_sql = format!(
                r#"DELETE FROM note_versions
                   WHERE note_id = ?
                     AND id NOT IN (
                       SELECT id FROM note_versions
                       WHERE note_id = ?
                       ORDER BY version_number DESC
                       LIMIT {}
                     )"#,
                cap
            );
            sqlx::query(&prune_sql)
                .bind(note_id.as_str())
                .bind(note_id.as_str())
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err("Failed to prune old note versions", e))?;
        }

        tx.commit()
            .await
            .map_err(|e| db_err("Failed to commit record_version tx", e))?;
        Ok(next)
    }

    async fn list_versions(
        &self,
        note_id: &NoteId,
        limit: usize,
    ) -> Result<Vec<crate::note::NoteVersion>> {
        use crate::note::NoteVersion;
        let rows = sqlx::query(
            r#"SELECT id, note_id, version_number, content, prev_content, created_at
               FROM note_versions
               WHERE note_id = ?
               ORDER BY version_number DESC
               LIMIT ?"#,
        )
        .bind(note_id.as_str())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to list note versions", e))?;
        Ok(rows
            .into_iter()
            .map(|row| NoteVersion {
                id: row.get("id"),
                note_id: NoteId::from(row.get::<String, _>("note_id")),
                version_number: row.get("version_number"),
                content: row.get("content"),
                prev_content: row.try_get("prev_content").ok().flatten(),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    async fn get_version(&self, version_id: i64) -> Result<Option<crate::note::NoteVersion>> {
        use crate::note::NoteVersion;
        let row = sqlx::query(
            r#"SELECT id, note_id, version_number, content, prev_content, created_at
               FROM note_versions WHERE id = ?"#,
        )
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_err("Failed to get note version", e))?;
        Ok(row.map(|row| NoteVersion {
            id: row.get("id"),
            note_id: NoteId::from(row.get::<String, _>("note_id")),
            version_number: row.get("version_number"),
            content: row.get("content"),
            prev_content: row.try_get("prev_content").ok().flatten(),
            created_at: row.get("created_at"),
        }))
    }

    async fn calendar_marks(&self, from: &str, to: &str) -> Result<crate::query::CalendarMarks> {
        use crate::query::{extract_iso_date, CalendarMarks, DayMarkers};
        use std::collections::HashMap;
        let mut days: HashMap<String, DayMarkers> = HashMap::new();

        // Block markers: scan block_properties for deadline/scheduled rows whose
        // values contain an ISO date in the [from, to] range. The values may
        // be wiki-wrapped (`[[2026-04-15]]`) — `extract_iso_date` handles it.
        let rows = sqlx::query(
            r#"SELECT property_name, value FROM block_properties
               WHERE property_name IN ('deadline', 'scheduled')
                 AND value IS NOT NULL"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to fetch calendar block markers", e))?;
        for row in &rows {
            let property_name: String = row.get("property_name");
            let value: Option<String> = row.try_get("value").ok().flatten();
            let Some(v) = value else { continue };
            let Some(date) = extract_iso_date(&v) else {
                continue;
            };
            if date.as_str() < from || date.as_str() > to {
                continue;
            }
            let entry = days.entry(date).or_default();
            match property_name.as_str() {
                "deadline" => entry.tasks += 1,
                "scheduled" => entry.events += 1,
                _ => {}
            }
        }

        // Note markers: daily notes use `YYYY-MM-DD` as their id.
        let note_rows = sqlx::query(
            r#"SELECT id FROM notes WHERE id >= ? AND id <= ?
               AND id GLOB '????-??-??'"#,
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to fetch calendar note markers", e))?;
        for row in &note_rows {
            let id: String = row.get("id");
            days.entry(id).or_default().notes = true;
        }

        Ok(CalendarMarks { days })
    }

    async fn agenda_blocks(
        &self,
        from: &str,
        to: &str,
        include_done: bool,
    ) -> Result<Vec<crate::query::AgendaRow>> {
        use crate::query::{extract_iso_date, AgendaField, AgendaRow, AgendaRowKind};
        use crate::recurrence;
        use chrono::NaiveDate;

        let today = chrono::Local::now().date_naive();
        let from_date = NaiveDate::parse_from_str(from, "%Y-%m-%d").map_err(|e| {
            crate::error::TeselaError::Database {
                message: format!("agenda_blocks: invalid from date '{}': {}", from, e),
                source: None,
            }
        })?;
        let to_date = NaiveDate::parse_from_str(to, "%Y-%m-%d").map_err(|e| {
            crate::error::TeselaError::Database {
                message: format!("agenda_blocks: invalid to date '{}': {}", to, e),
                source: None,
            }
        })?;

        // Fetch all block_id + note_id pairs that have a scheduled or deadline
        // property. We'll collect all properties for each matching block in a
        // second pass. The broad fetch (no date-range filter) lets us handle
        // recurring blocks whose anchor pre-dates the window but whose
        // projected occurrences land inside it.
        let candidate_ids: Vec<(String, String)> = {
            let rows = sqlx::query(
                r#"SELECT DISTINCT block_id, note_id
                   FROM block_properties
                   WHERE property_name IN ('deadline', 'scheduled')"#,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_err("Failed to fetch agenda candidate block ids", e))?;
            rows.iter()
                .map(|r| {
                    let block_id: String = r.get("block_id");
                    let note_id: String = r.get("note_id");
                    (block_id, note_id)
                })
                .collect()
        };

        // For each candidate block, load all its properties and the note body
        // (to recover display text). We use parse_blocks to get the text field
        // but rely on the indexed block_properties for properties (more reliable).
        //
        // Batch the notes we need so we don't spam individual SELECTs.
        let note_ids: Vec<String> = {
            let mut ids: Vec<String> = candidate_ids.iter().map(|(_, n)| n.clone()).collect();
            ids.sort();
            ids.dedup();
            ids
        };

        // note_id -> body mapping
        let mut note_bodies: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for note_id in &note_ids {
            let row = sqlx::query("SELECT body FROM notes WHERE id = ?")
                .bind(note_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| db_err("Failed to fetch note body for agenda", e))?;
            if let Some(row) = row {
                let body: String = row.get("body");
                note_bodies.insert(note_id.clone(), body);
            }
        }

        // block_id -> {property_name -> value}
        let mut block_props: std::collections::HashMap<
            String,
            std::collections::HashMap<String, String>,
        > = std::collections::HashMap::new();
        for (block_id, _) in &candidate_ids {
            let prop_rows = sqlx::query(
                "SELECT property_name, value FROM block_properties WHERE block_id = ?",
            )
            .bind(block_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_err("Failed to fetch block properties for agenda", e))?;
            let mut props = std::collections::HashMap::new();
            for pr in &prop_rows {
                let key: String = pr.get("property_name");
                let value: Option<String> = pr.get("value");
                if let Some(v) = value {
                    props.insert(key, v);
                }
            }
            block_props.insert(block_id.clone(), props);
        }

        // Helper: parse a dated property value into (NaiveDate, Option<time_str>).
        // Handles bare "YYYY-MM-DD" and "YYYY-MM-DD HH:MM" forms as well as
        // wiki-wrapped "[[YYYY-MM-DD]]" legacy form.
        let parse_dated_value = |value: &str| -> Option<(NaiveDate, Option<String>)> {
            // Extract the ISO date portion (strips [[ ]] if present).
            let date_str = extract_iso_date(value)?;
            let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").ok()?;
            // Look for an HH:MM time token after the date.
            let rest = value[value.find(&date_str[..]).unwrap_or(0) + 10..].trim();
            let time = if rest.len() >= 5
                && rest.as_bytes()[2] == b':'
                && rest[..2].chars().all(|c| c.is_ascii_digit())
                && rest[3..5].chars().all(|c| c.is_ascii_digit())
            {
                Some(rest[..5].to_string())
            } else {
                None
            };
            Some((date, time))
        };

        let mut rows: Vec<AgendaRow> = Vec::new();

        for (block_id, note_id) in &candidate_ids {
            let props = match block_props.get(block_id) {
                Some(p) => p,
                None => continue,
            };

            // Determine anchor date + time + which field it came from:
            // prefer `scheduled` (the "when am I doing it" answer), fall
            // back to `deadline` only when `scheduled` is absent. The
            // `field` rides along on the AgendaRow so clients can split
            // the Overdue bucket (a missed deadline is semantically
            // different from a missed planned-do date).
            let (anchor_date, anchor_time, field) = {
                if let Some(p) = props.get("scheduled").and_then(|v| parse_dated_value(v)) {
                    (p.0, p.1, AgendaField::Scheduled)
                } else if let Some(p) = props.get("deadline").and_then(|v| parse_dated_value(v)) {
                    (p.0, p.1, AgendaField::Deadline)
                } else {
                    continue;
                }
            };

            // Status and done-filtering.
            let status = props.get("status").cloned();
            if !include_done && status.as_deref() == Some("done") {
                continue;
            }

            // Determine kind. A block is a Task if:
            //   - it has a `tags` property containing "Task" (case-insensitive), OR
            //   - it has a `status` property (todo/in-progress/done/etc.).
            // Everything else is an Event.
            let is_task = {
                let has_task_tag = props
                    .get("tags")
                    .map(|v| {
                        v.split(',')
                            .any(|t| t.trim().eq_ignore_ascii_case("task"))
                    })
                    .unwrap_or(false);
                let has_status = props.contains_key("status");
                has_task_tag || has_status
            };
            let kind = if is_task { AgendaRowKind::Task } else { AgendaRowKind::Event };

            // Block text: parse from body if we have it, otherwise use empty.
            let block_text: String = note_bodies
                .get(note_id)
                .map(|body| {
                    crate::block::parse_blocks(note_id, body)
                        .into_iter()
                        .find(|b| &b.id == block_id)
                        .map(|b| b.text.clone())
                        .unwrap_or_default()
                })
                .unwrap_or_default();

            // Recurrence setup.
            let recurrence_str = props.get("recurring").cloned();
            let rec = recurrence_str
                .as_deref()
                .and_then(|s| recurrence::parse(s));
            let done_so_far_start: u32 = props
                .get("recurrence_done")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            // Closure to push a row.
            let push_row = |rows: &mut Vec<AgendaRow>,
                            date: NaiveDate,
                            time: Option<String>,
                            is_anchor: bool| {
                rows.push(AgendaRow {
                    block_id: block_id.clone(),
                    source_note_id: note_id.clone(),
                    occurrence_date: date.format("%Y-%m-%d").to_string(),
                    occurrence_time: time,
                    kind,
                    overdue: date < today,
                    recurrence: recurrence_str.clone(),
                    is_anchor,
                    text: block_text.clone(),
                    status: status.clone(),
                    field,
                });
            };

            match rec {
                None => {
                    // Non-recurring: emit only if anchor falls in window.
                    if anchor_date >= from_date && anchor_date <= to_date {
                        push_row(&mut rows, anchor_date, anchor_time.clone(), true);
                    }
                }
                Some(ref rec) => {
                    // Recurring: emit anchor if in window, then walk forward.
                    if anchor_date >= from_date && anchor_date <= to_date {
                        push_row(&mut rows, anchor_date, anchor_time.clone(), true);
                    }
                    let mut current = anchor_date;
                    let mut done_so_far = done_so_far_start;
                    loop {
                        let next = recurrence::advance(rec, current, done_so_far);
                        let next = match next {
                            None => break,
                            Some(d) if d > to_date => break,
                            Some(d) => d,
                        };
                        done_so_far += 1;
                        if next >= from_date {
                            push_row(&mut rows, next, anchor_time.clone(), false);
                        }
                        current = next;
                    }
                }
            }
        }

        rows.sort_by(|a, b| {
            a.occurrence_date
                .cmp(&b.occurrence_date)
                .then_with(|| a.occurrence_time.cmp(&b.occurrence_time))
                .then_with(|| a.block_id.cmp(&b.block_id))
        });

        Ok(rows)
    }
}

// ---------------------------------------------------------------------------
// Query execution helpers (Phase 9.1)
// ---------------------------------------------------------------------------

impl SqliteIndex {
    /// Execute a `kind:block` query. Strategy: pull a candidate set of notes
    /// from SQL using the most selective tag filter (or all notes if none),
    /// parse blocks, then refine in-memory with [`crate::query::block_matches`].
    async fn execute_block_query(
        &self,
        query: &crate::query::ParsedQuery,
    ) -> Result<Vec<crate::query::QueryItem>> {
        use crate::block::parse_blocks;
        use crate::query::{block_matches, Kind, QueryItem, QueryOp};

        // Pick the first positive `tag:` filter as the broad SQL prefilter.
        // Negative tag filters and other property filters refine in-memory.
        let prefilter_tag: Option<&str> = query
            .filters
            .iter()
            .find(|f| f.key == "tag" && f.op == QueryOp::Eq)
            .map(|f| f.value.as_str());

        let candidate_notes: Vec<(String, String, String, Option<String>)> =
            if let Some(tag) = prefilter_tag {
                // Pre-filter is intentionally over-inclusive — `block_matches`
                // refines below. `body LIKE '%<tag>%'` catches both legacy
                // `#<tag>` inline syntax AND the `tags:: <tag>` continuation-line
                // syntax used by block-level tags (e.g. projects.md where the
                // block has `tags:: Task` but the note frontmatter does not).
                sqlx::query(
                    "SELECT id, title, body, note_type FROM notes WHERE body LIKE ? OR tags LIKE ?",
                )
                .bind(format!("%{}%", tag))
                .bind(format!("%\"{}%", tag))
                .fetch_all(&self.pool)
                .await
                .map_err(|e| db_err("Failed to fetch candidate notes for block query", e))?
                .into_iter()
                .map(|row| {
                    (
                        row.get("id"),
                        row.get("title"),
                        row.get("body"),
                        row.try_get::<Option<String>, _>("note_type").ok().flatten(),
                    )
                })
                .collect()
            } else {
                sqlx::query("SELECT id, title, body, note_type FROM notes")
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| db_err("Failed to fetch all notes for block query", e))?
                    .into_iter()
                    .map(|row| {
                        (
                            row.get("id"),
                            row.get("title"),
                            row.get("body"),
                            row.try_get::<Option<String>, _>("note_type").ok().flatten(),
                        )
                    })
                    .collect()
            };

        let mut out = Vec::new();
        for (note_id, note_title, body, page_note_type) in &candidate_notes {
            let mut blocks = parse_blocks(note_id, body);
            // Enrich every block with its containing page's note_type so
            // DSL predicates that depend on parent metadata (`on:system-
            // pages`, `on:daily-page`'s fallback branch) can run inside
            // `block_matches` without re-fetching the note row at filter
            // time. Cheap (a clone per block) and keeps the matcher pure.
            for b in blocks.iter_mut() {
                b.parent_note_type = page_note_type.clone();
            }
            // Refine each block in-memory.
            for (idx, block) in blocks.iter().enumerate() {
                if !block_matches(block, query) {
                    continue;
                }
                // Walk back through earlier blocks at lower indent_level to
                // build the parent breadcrumb. The page title is the first
                // element; ancestor block texts follow in outer-to-inner order.
                let mut breadcrumb = vec![note_title.clone()];
                let mut crumbs = Vec::new();
                let mut cursor = idx;
                let target_indent = block.indent_level;
                while cursor > 0 && target_indent > 0 {
                    cursor -= 1;
                    if blocks[cursor].indent_level < target_indent {
                        crumbs.push(blocks[cursor].text.clone());
                        if blocks[cursor].indent_level == 0 {
                            break;
                        }
                    }
                }
                crumbs.reverse();
                breadcrumb.extend(crumbs);

                let primary_tag = block.tags.first().cloned();
                out.push(QueryItem {
                    block_id: Some(block.id.clone()),
                    page_id: note_id.clone(),
                    title: note_title.clone(),
                    text: if block.text.is_empty() {
                        block.raw_text.lines().next().unwrap_or("").to_string()
                    } else {
                        block.text.clone()
                    },
                    parent_breadcrumb: breadcrumb,
                    kind: Kind::Block,
                    primary_tag,
                    properties: block.properties.clone(),
                    page_note_type: page_note_type.clone(),
                });
            }
        }
        Ok(out)
    }

    /// Execute a `kind:page` query. Loads all notes (corpus is small) and
    /// filters in-memory using the same `block_matches` semantics applied to
    /// a synthetic "page block" (tags + properties from frontmatter).
    async fn execute_page_query(
        &self,
        query: &crate::query::ParsedQuery,
    ) -> Result<Vec<crate::query::QueryItem>> {
        use crate::block::ParsedBlock;
        use crate::query::{block_matches, Kind, QueryItem};
        use std::collections::HashMap;

        // SELECT id, title, tags, note_type, plus full content for property parsing.
        let rows = sqlx::query(
            "SELECT id, title, tags, note_type, content FROM notes ORDER BY modified_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to fetch notes for page query", e))?;

        let mut out = Vec::new();
        for row in &rows {
            let id: String = row.get("id");
            let title: String = row.get("title");
            let tags_json: String = row.get("tags");
            let note_type: Option<String> = row.try_get("note_type").ok().flatten();
            let content: String = row.get("content");

            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            let mut props: HashMap<String, String> = HashMap::new();
            // Pull properties from frontmatter — naive line-by-line parse looking
            // for `key: value` between `---` fences.
            if let Some(fm) = extract_frontmatter(&content) {
                for line in fm.lines() {
                    if let Some((k, v)) = line.split_once(':') {
                        let k = k.trim();
                        let v = v.trim().trim_matches('"');
                        if !k.is_empty() && !v.is_empty() {
                            // YAML uses `type:`; metadata API exposes it as
                            // `note_type`. Alias on insert so DSL filters that
                            // reference `note_type:` resolve correctly.
                            let canonical = if k == "type" { "note_type" } else { k };
                            props.insert(canonical.to_string(), v.to_string());
                        }
                    }
                }
            }
            if let Some(nt) = &note_type {
                props.insert("note_type".to_string(), nt.clone());
            }

            // Synthetic page-block for matcher. inherited_tags is empty for pages.
            // inline/trailing tags are treated as empty here — page-level tags
            // come from frontmatter, not from positional `#tag` tokens in body.
            let pseudo = ParsedBlock {
                id: id.clone(),
                text: title.clone(),
                raw_text: title.clone(),
                tags: tags.clone(),
                inline_tags: vec![],
                trailing_tags: vec![],
                inherited_tags: vec![],
                properties: props.clone(),
                indent_level: 0,
                note_id: id.clone(),
                // Page-kind rows don't have a "parent" — the row IS the
                // page — so leave None. `on:*` predicates that depend
                // on this field don't make sense for page queries.
                parent_note_type: None,
            };
            if !block_matches(&pseudo, query) {
                continue;
            }
            out.push(QueryItem {
                block_id: None,
                page_id: id.clone(),
                title: title.clone(),
                text: title,
                parent_breadcrumb: vec![],
                kind: Kind::Page,
                primary_tag: tags.first().cloned(),
                properties: props,
                page_note_type: note_type,
            });
        }
        Ok(out)
    }
}

/// Extract the YAML frontmatter body (between the two `---` fences) from a
/// note's full content. Returns `None` if there is no frontmatter.
fn extract_frontmatter(content: &str) -> Option<&str> {
    if !content.starts_with("---") {
        return None;
    }
    let after_first = content.get(3..)?.trim_start_matches('\n');
    let end = after_first.find("\n---")?;
    Some(&after_first[..end])
}

/// Sort `items` in place by a comma-separated `key [asc|desc]` list. Property
/// keys map to the row's `properties` map; `title` and `text` map to the row
/// fields directly. Unknown keys are ignored.
fn apply_sort(items: &mut [crate::query::QueryItem], sort: Option<&str>) {
    let Some(s) = sort else {
        return;
    };
    let mut keys: Vec<(String, bool)> = Vec::new(); // (key, desc)
    for tok in s.split(',') {
        let mut parts = tok.split_whitespace();
        let Some(key) = parts.next() else { continue };
        let desc = matches!(parts.next(), Some(d) if d.eq_ignore_ascii_case("desc"));
        keys.push((key.to_ascii_lowercase(), desc));
    }
    if keys.is_empty() {
        return;
    }
    items.sort_by(|a, b| {
        for (k, desc) in &keys {
            let av = field(a, k);
            let bv = field(b, k);
            let ord = av.cmp(&bv);
            if ord != std::cmp::Ordering::Equal {
                return if *desc { ord.reverse() } else { ord };
            }
        }
        std::cmp::Ordering::Equal
    });
}

fn field(item: &crate::query::QueryItem, key: &str) -> String {
    match key {
        "title" => item.title.to_ascii_lowercase(),
        "text" => item.text.to_ascii_lowercase(),
        other => item
            .properties
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(other))
            .map(|(_, v)| v.to_ascii_lowercase())
            .unwrap_or_default(),
    }
}

/// Bucket `items` by a property/metadata key. When `group` is `None`, returns
/// a single `QueryGroup` with key `""` containing all items.
fn apply_group(
    items: Vec<crate::query::QueryItem>,
    group: Option<&str>,
) -> Vec<crate::query::QueryGroup> {
    use crate::query::QueryGroup;
    use std::collections::BTreeMap;

    let Some(g) = group else {
        let count = items.len() as u32;
        return vec![QueryGroup {
            key: String::new(),
            count,
            items,
        }];
    };
    // BTreeMap to keep group order stable across calls.
    let mut buckets: BTreeMap<String, Vec<crate::query::QueryItem>> = BTreeMap::new();
    for item in items {
        let key = item
            .properties
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(g))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        buckets.entry(key).or_default().push(item);
    }
    buckets
        .into_iter()
        .map(|(key, items)| {
            let count = items.len() as u32;
            QueryGroup { key, count, items }
        })
        .collect()
}

#[async_trait]
impl LinkGraph for SqliteIndex {
    async fn get_backlinks(&self, id: &NoteId) -> Result<Vec<Link>> {
        let rows = sqlx::query(
            r#"
            SELECT source_id AS target, link_text, position, link_type
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
                note_type: None,
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

    // -----------------------------------------------------------------------
    // agenda_blocks tests (Task 2)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn agenda_blocks_returns_dated_blocks_in_window() {
        use crate::traits::search_index::SearchIndex as _;
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // A task: scheduled 2026-05-22, status todo.
        let task_note = make_test_note(
            "agenda-t1",
            "Task Note",
            "- buy milk\n  scheduled:: 2026-05-22\n  tags:: Task\n  status:: todo",
            &[],
        );
        // An event: scheduled 2026-05-23 14:00 (no status = event).
        let event_note = make_test_note(
            "agenda-t2",
            "Event Note",
            "- party\n  scheduled:: 2026-05-23 14:00",
            &[],
        );
        // A done task scheduled on 2026-05-22 — should be excluded when include_done=false.
        let done_note = make_test_note(
            "agenda-t3",
            "Done Note",
            "- done chore\n  scheduled:: 2026-05-22\n  tags:: Task\n  status:: done",
            &[],
        );

        index.reindex(&task_note).await.unwrap();
        index.reindex(&event_note).await.unwrap();
        index.reindex(&done_note).await.unwrap();

        let rows = index
            .agenda_blocks("2026-05-22", "2026-05-25", false)
            .await
            .unwrap();

        // done task excluded
        assert_eq!(rows.len(), 2, "expected 2 rows (done excluded): got {rows:?}");
        assert!(
            rows.iter().any(|r| r.kind == crate::query::AgendaRowKind::Task
                && r.occurrence_date == "2026-05-22"),
            "task row missing"
        );
        assert!(
            rows.iter().any(|r| r.kind == crate::query::AgendaRowKind::Event
                && r.occurrence_time == Some("14:00".to_string())),
            "event row missing"
        );
    }

    #[tokio::test]
    async fn agenda_blocks_projects_recurring_forward() {
        use crate::traits::search_index::SearchIndex as _;
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // Weekly recurring task, anchor 2026-05-22 (a Friday).
        let note = make_test_note(
            "agenda-r1",
            "Recurring Note",
            "- weekly review\n  scheduled:: 2026-05-22\n  recurring:: weekly\n  tags:: Task\n  status:: todo",
            &[],
        );
        index.reindex(&note).await.unwrap();

        let rows = index
            .agenda_blocks("2026-05-22", "2026-06-12", false)
            .await
            .unwrap();

        let dates: Vec<&str> = rows.iter().map(|r| r.occurrence_date.as_str()).collect();
        assert_eq!(
            dates,
            vec!["2026-05-22", "2026-05-29", "2026-06-05", "2026-06-12"],
            "projected dates wrong"
        );
        assert!(rows[0].is_anchor, "first row should be anchor");
        assert!(!rows[1].is_anchor, "second row should not be anchor");
        assert!(!rows[2].is_anchor, "third row should not be anchor");
        assert!(!rows[3].is_anchor, "fourth row should not be anchor");
    }

    #[tokio::test]
    async fn agenda_blocks_field_is_scheduled_when_scheduled_set() {
        use crate::traits::search_index::SearchIndex as _;
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let note = make_test_note(
            "agenda-fs",
            "Field-Scheduled Note",
            "- shop\n  scheduled:: 2026-05-22\n  tags:: Task\n  status:: todo",
            &[],
        );
        index.reindex(&note).await.unwrap();
        let rows = index.agenda_blocks("2026-05-22", "2026-05-22", false).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].field, crate::query::AgendaField::Scheduled);
    }

    #[tokio::test]
    async fn agenda_blocks_field_is_deadline_when_only_deadline_set() {
        use crate::traits::search_index::SearchIndex as _;
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let note = make_test_note(
            "agenda-fd",
            "Field-Deadline Note",
            "- file taxes\n  deadline:: 2026-04-15\n  tags:: Task\n  status:: todo",
            &[],
        );
        index.reindex(&note).await.unwrap();
        let rows = index.agenda_blocks("2026-04-15", "2026-04-15", false).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].field, crate::query::AgendaField::Deadline);
    }

    #[tokio::test]
    async fn agenda_blocks_field_prefers_scheduled_when_both_set() {
        // When a block carries both deadline and scheduled, the agenda
        // anchors on scheduled (the "when am I doing it" answer), so
        // `field` reports Scheduled. Mirrors the anchor-selection rule
        // in `agenda_blocks` so clients can trust `field` for UI splits.
        use crate::traits::search_index::SearchIndex as _;
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let note = make_test_note(
            "agenda-fb",
            "Both Note",
            "- big project\n  scheduled:: 2026-05-20\n  deadline:: 2026-05-25\n  tags:: Task\n  status:: todo",
            &[],
        );
        index.reindex(&note).await.unwrap();
        let rows = index.agenda_blocks("2026-05-19", "2026-05-26", false).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].occurrence_date, "2026-05-20", "anchor should be scheduled date");
        assert_eq!(rows[0].field, crate::query::AgendaField::Scheduled);
    }

    #[tokio::test]
    async fn agenda_blocks_respects_recurrence_count() {
        use crate::traits::search_index::SearchIndex as _;
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // `recurring:: weekly count 3` — series has exactly 3 occurrences.
        let note = make_test_note(
            "agenda-c1",
            "Count Note",
            "- counted task\n  scheduled:: 2026-05-22\n  recurring:: weekly count 3\n  recurrence_done:: 0\n  tags:: Task\n  status:: todo",
            &[],
        );
        index.reindex(&note).await.unwrap();

        let rows = index
            .agenda_blocks("2026-05-22", "2026-12-31", false)
            .await
            .unwrap();

        assert_eq!(rows.len(), 3, "count 3 should yield exactly 3 rows: got {rows:?}");
    }
}
