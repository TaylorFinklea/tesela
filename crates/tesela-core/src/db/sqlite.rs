//! SQLite+FTS5 implementation of SearchIndex and LinkGraph traits

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Row, Sqlite};
use std::path::Path;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tracing::debug;

use super::queries;
use super::schema;
use crate::block::ParsedBlock;
use crate::error::{Result, TeselaError};
use crate::link::{Link, LinkType};
use crate::note::{Note, NoteId, SearchHit};
use crate::traits::link_graph::LinkGraph;
use crate::traits::search_index::SearchIndex;

/// One note's cached [`crate::block::parse_blocks`] output, keyed
/// externally by note_id in `SqliteIndex::blocks_cache`. `body_hash` is a
/// SHA-256 hex digest of the exact `body` text that produced `blocks` — a
/// hit requires that hash to match the freshly-fetched row's body hash, so
/// a stale read is structurally impossible: whenever the note's body
/// changes at all (even whitespace), the hash changes and the cache
/// misses, forcing a fresh parse. Keying on content instead of a
/// timestamp/version/generation counter means there is no "did I remember
/// to bump this" invalidation step to forget.
struct CachedBlocks {
    body_hash: String,
    blocks: Arc<Vec<ParsedBlock>>,
}

fn db_err(msg: &str, e: sqlx::Error) -> TeselaError {
    TeselaError::Database {
        message: format!("{}: {}", msg, e),
        source: None,
    }
}

/// A single type's per-property override, parsed from
/// `property_overrides.{Prop}` FLOW YAML. All fields optional; absent
/// fields fall back to the global Property page's config.
///
/// `hide_choices` is the new alias for legacy `hidden_{Prop}` — both feed
/// the same subtract step (§3.3/§5.4).
///
/// `pub` (not just crate-visible) so the shared override-resolution
/// conformance fixture (`tests/property_override_conformance.rs`) can drive
/// this SAME production merge — mirrors how `query::block_matches_typed` is
/// `pub` for `tests/query_conformance.rs`.
#[derive(Debug, Clone, Default)]
pub struct PropOverride {
    /// REPLACE the property's choice list for this type's instances.
    choices: Option<Vec<String>>,
    /// Per-type default value.
    default: Option<String>,
    /// Per-type visibility (`on_new`/`on_set`/`hidden`).
    show: Option<crate::types::Visibility>,
    /// Choices to subtract after the (possibly replaced) list is built.
    hide_choices: Vec<String>,
}

/// Parse one override object (`{choices: [...], show: "on_new", default: "todo",
/// hide_choices: [...]}`) from a JSON value. Unknown/malformed fields are
/// ignored rather than erroring — a Tag page is user content.
fn parse_prop_override(v: &serde_json::Value) -> PropOverride {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return PropOverride::default(),
    };
    let str_array = |val: &serde_json::Value| -> Vec<String> {
        val.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    PropOverride {
        choices: obj.get("choices").map(str_array),
        default: obj.get("default").and_then(|d| d.as_str().map(String::from)),
        show: obj
            .get("show")
            .and_then(|s| s.as_str())
            .and_then(|s| match s {
                "on_new" => Some(crate::types::Visibility::OnNew),
                "on_set" => Some(crate::types::Visibility::OnSet),
                "hidden" => Some(crate::types::Visibility::Hidden),
                _ => None,
            }),
        hide_choices: obj
            .get("hide_choices")
            .map(str_array)
            .unwrap_or_default(),
    }
}

/// Build the resolved override map for a tag by walking its rows in
/// child→parent order. Keys are `lower(prop)`; **first-insert-wins** so a
/// child override beats a parent's (same precedence as the name dedup, but
/// a distinct pass because the name dedup discards parent rows — §3.5).
///
/// `rows` is `(property_overrides_json, hidden_{Prop} pairs)` in the chain
/// walk order (child first). Each `hidden_{Prop}` legacy key is folded into
/// the same property's `hide_choices` subtract list.
pub fn build_overrides(
    rows: &[(String, Vec<(String, Vec<String>)>)],
) -> std::collections::HashMap<String, PropOverride> {
    let mut map: std::collections::HashMap<String, PropOverride> =
        std::collections::HashMap::new();
    for (overrides_json, hidden_pairs) in rows {
        // property_overrides.{Prop}
        if let Ok(serde_json::Value::Object(obj)) =
            serde_json::from_str::<serde_json::Value>(overrides_json)
        {
            for (prop, val) in &obj {
                let key = prop.to_ascii_lowercase();
                // first-insert-wins: child rows come first.
                map.entry(key).or_insert_with(|| parse_prop_override(val));
            }
        }
        // Legacy hidden_{Prop}: alias for property_overrides.{Prop}.hide_choices.
        // Merge into hide_choices regardless of who set the override fields —
        // both child and parent hidden_ lists subtract (it's additive subtract,
        // not a replace), but we still honor first-insert-wins for the OTHER
        // fields by only touching hide_choices.
        for (prop, hidden) in hidden_pairs {
            let key = prop.to_ascii_lowercase();
            let entry = map.entry(key).or_default();
            for h in hidden {
                if !entry.hide_choices.contains(h) {
                    entry.hide_choices.push(h.clone());
                }
            }
        }
    }
    map
}

/// Per-resolve source of `hidden_{Prop}` legacy pairs — intentionally empty.
///
/// Legacy `hidden_{Prop}: [choices]` frontmatter keys are folded into the
/// matching property's `hide_choices` at INDEX time (`index_type_info`), so
/// the cached `property_overrides_json` is already the single source of
/// subtract lists by the time the resolver runs. This keeps the Rust resolver
/// (kanban / views) in lockstep with the TS registry (chips), which reads
/// `hidden_{Prop}` straight from frontmatter. The shim stays so the chain-walk
/// call sites keep `build_overrides`'s `(json, pairs)` shape; the pairs are
/// already merged upstream.
fn legacy_hidden_pairs() -> Vec<(String, Vec<String>)> {
    Vec::new()
}

/// Apply a resolved override to a single `PropertyDef`, in place. Mirrors
/// §3.3 precedence exactly: choices REPLACE → then SUBTRACT hide_choices;
/// default override wins; show override wins, else derive from
/// `hide_by_default`.
///
/// `hide_by_default` is the property's global flag (from `property_defs`),
/// used only for the derivation fallback.
pub fn apply_override(
    def: &mut crate::types::PropertyDef,
    over: Option<&PropOverride>,
    hide_by_default: bool,
) {
    // choices: REPLACE then SUBTRACT.
    if let Some(o) = over {
        if let Some(replaced) = &o.choices {
            def.values = Some(replaced.clone());
        }
        if !o.hide_choices.is_empty() {
            if let Some(vals) = &mut def.values {
                let hidden: std::collections::HashSet<&str> =
                    o.hide_choices.iter().map(|s| s.as_str()).collect();
                vals.retain(|c| !hidden.contains(c.as_str()));
            }
        }
        if let Some(d) = &o.default {
            def.default = Some(d.clone());
        }
    }
    // show: override wins; else derive from hide_by_default
    // (on_new when shown by default, hidden otherwise).
    def.show = Some(match over.and_then(|o| o.show) {
        Some(v) => v,
        None => {
            if hide_by_default {
                crate::types::Visibility::Hidden
            } else {
                crate::types::Visibility::OnNew
            }
        }
    });
}

/// SQLite-backed search index and link graph.
///
/// SQLite is treated as a **cache** of the filesystem. If the database file
/// is lost, `rebuild_from_notes()` reconstructs it from the on-disk notes.
pub struct SqliteIndex {
    pool: Pool<Sqlite>,
    /// Per-note `parse_blocks` cache for `execute_block_query`'s no-tag-
    /// filter path (tesela-sclr.2). Keyed by note_id; overwritten in place
    /// on every reparse, so it never grows past one entry per note
    /// currently in `notes`. See `CachedBlocks` for the invalidation
    /// contract. Explicitly evicted in `remove_note`.
    blocks_cache: Mutex<std::collections::HashMap<String, CachedBlocks>>,
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

        Ok(Self {
            pool,
            blocks_cache: Mutex::new(std::collections::HashMap::new()),
        })
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

        Ok(Self {
            pool,
            blocks_cache: Mutex::new(std::collections::HashMap::new()),
        })
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
    /// Uses INSERT .. ON CONFLICT DO UPDATE (never INSERT OR REPLACE) to preserve
    /// the SQLite rowid. The content FTS5 table (`content=notes, content_rowid=rowid`)
    /// references notes by rowid; INSERT OR REPLACE silently changes the rowid
    /// (delete + re-insert), causing SQLITE_CORRUPT_VTAB (267) on the next search
    /// because the FTS5 index holds the old rowid. ON CONFLICT DO UPDATE mutates the
    /// existing row in place (rowid kept, UPDATE triggers fired). It must also stay
    /// ONE atomic statement: the create handler's reindex races the fs-watcher's
    /// reindex of the same just-written file, and a two-statement UPDATE-then-INSERT
    /// let the loser die with `UNIQUE constraint failed: notes.id`.
    /// `created_at` is intentionally absent from the UPDATE arm — the original
    /// insert's value is preserved.
    pub async fn upsert_note(&self, note: &Note) -> Result<()> {
        let tags_json = serde_json::to_string(&note.metadata.tags).map_err(TeselaError::Json)?;

        sqlx::query(
            r#"
            INSERT INTO notes (
                id, title, body, content, path, checksum, created_at, modified_at, tags, note_type
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                body = excluded.body,
                content = excluded.content,
                path = excluded.path,
                checksum = excluded.checksum,
                modified_at = excluded.modified_at,
                tags = excluded.tags,
                note_type = excluded.note_type
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
        .map_err(|e| db_err("Failed to upsert note", e))?;

        Ok(())
    }

    /// Remove a note from the index.
    pub async fn remove_note(&self, id: &NoteId) -> Result<()> {
        // Clear cached type definitions for this note explicitly. The FK
        // (migration 005) cascades these deletes on its own, but we do it
        // here too so the cleanup intent is local to the operation and
        // survives any future schema drift. Without this, a `note_id=NULL`
        // row would linger in the type-def tables (old SET NULL FK) and
        // keep surfacing in `GET /types` and the resolver.
        sqlx::query("DELETE FROM tag_defs WHERE note_id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to remove tag defs for note", e))?;
        sqlx::query("DELETE FROM property_defs WHERE note_id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to remove property defs for note", e))?;

        sqlx::query("DELETE FROM notes WHERE id = ?")
            .bind(id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to remove note", e))?;

        // A deleted note no longer appears in the `candidate_notes` scan
        // that `execute_block_query` builds its cache from, so a lingering
        // entry could never be served as a wrong answer — but it would sit
        // there as dead weight for the note's lifetime otherwise. Evict it.
        self.blocks_cache
            .lock()
            .expect("blocks_cache mutex poisoned")
            .remove(id.as_str());

        Ok(())
    }

    /// Index type system info: if note is a Tag or Property page, cache its definition.
    async fn index_type_info(&self, note: &Note) -> Result<()> {
        // Clear any cached type defs for this note first. This covers the
        // type-change path (Tag → not-typed, Tag → Property, Property →
        // Tag, etc.) so a reclassified note doesn't leave a stale row
        // under the old type. The match below re-inserts a fresh row when
        // the new type is Tag/Property; the `_ =>` arm leaves both tables
        // empty for the note.
        sqlx::query("DELETE FROM tag_defs WHERE note_id = ?")
            .bind(note.id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to clear old tag def", e))?;
        sqlx::query("DELETE FROM property_defs WHERE note_id = ?")
            .bind(note.id.as_str())
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("Failed to clear old property def", e))?;

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
                // Per-type override map (FLOW YAML inline map → JSON object),
                // keyed by property name. We ALSO fold any legacy
                // `hidden_{Prop}: [choices]` frontmatter keys into the
                // matching property's `hide_choices` here, at index time, so
                // the Rust resolver (kanban / views) subtracts the same
                // choices the web registry (chips) does — which reads
                // `hidden_{Prop}` straight from frontmatter (spec §3.3 +
                // locked decision 4: the two engines must agree). After this
                // fold the cached map is the single source of subtract lists,
                // so `legacy_hidden_pairs()` at resolve time is empty.
                let mut overrides_val = note
                    .metadata
                    .custom
                    .get("property_overrides")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                if !overrides_val.is_object() {
                    overrides_val = serde_json::json!({});
                }
                {
                    let obj = overrides_val.as_object_mut().expect("just ensured object");
                    for (key, val) in &note.metadata.custom {
                        let Some(prop) = key.strip_prefix("hidden_") else {
                            continue;
                        };
                        if prop.is_empty() {
                            continue;
                        }
                        let hidden: Vec<String> = val
                            .as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        if hidden.is_empty() {
                            continue;
                        }
                        // Merge into an existing override entry if one matches
                        // case-insensitively (the resolver keys by lower()).
                        let entry_key = obj
                            .keys()
                            .find(|k| k.eq_ignore_ascii_case(prop))
                            .cloned()
                            .unwrap_or_else(|| prop.to_string());
                        let entry = obj
                            .entry(entry_key)
                            .or_insert_with(|| serde_json::json!({}));
                        if !entry.is_object() {
                            *entry = serde_json::json!({});
                        }
                        let hc = entry
                            .as_object_mut()
                            .expect("just ensured object")
                            .entry("hide_choices".to_string())
                            .or_insert_with(|| serde_json::json!([]));
                        if let Some(arr) = hc.as_array_mut() {
                            for h in hidden {
                                if !arr.iter().any(|x| x.as_str() == Some(h.as_str())) {
                                    arr.push(serde_json::Value::String(h));
                                }
                            }
                        }
                    }
                }
                let overrides_json =
                    serde_json::to_string(&overrides_val).unwrap_or_else(|_| "{}".to_string());
                let plural = note
                    .metadata
                    .custom
                    .get("plural")
                    .and_then(|v| v.as_str().map(String::from));

                sqlx::query(
                    "INSERT OR REPLACE INTO tag_defs (id, name, extends, icon, color, properties_json, property_overrides_json, plural, note_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(note.id.as_str())
                .bind(&note.title)
                .bind(&extends)
                .bind(&icon)
                .bind(&color)
                .bind(&props_json)
                .bind(&overrides_json)
                .bind(&plural)
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
                // hide_by_default — read server-side now (Phase 1) so the Rust
                // resolver can derive the 3-state `show` (parity with the TS
                // registry, which has always read it from frontmatter).
                let hide_by_default = note
                    .metadata
                    .custom
                    .get("hide_by_default")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let description = note
                    .metadata
                    .custom
                    .get("description")
                    .and_then(|v| v.as_str().map(String::from));

                sqlx::query(
                    "INSERT OR REPLACE INTO property_defs (id, name, value_type, choices_json, default_value, multiple_values, hide_empty, hide_by_default, description, note_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(note.id.as_str())
                .bind(&note.title)
                .bind(&value_type)
                .bind(&choices_json)
                .bind(&default_value)
                .bind(multiple)
                .bind(hide_empty)
                .bind(hide_by_default)
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

        // Re-insert all notes. Mirror `reindex` (upsert + index_type_info)
        // rather than a bare `upsert_note`, so Tag/Property pages repopulate
        // `tag_defs`/`property_defs` — otherwise a bulk rebuild leaves the
        // property registry EMPTY, and the typed query matcher (L5) +
        // `GET /properties` fall back to untyped/no-defs. The top-level
        // `DELETE FROM notes` already cascades those tables clear (FK
        // migration 005), and `index_type_info` self-clears per note, so this
        // is a clean repopulation.
        for note in notes {
            self.upsert_note(note).await?;
            self.index_type_info(note).await?;
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

    /// Build the `lowercased-name → ValueType` map the typed query matcher
    /// (L5) consults so property comparisons are typed (numeric/date/bool)
    /// rather than string-guessed. One small `SELECT` per query execution;
    /// callers build it once and pass it to `block_matches_typed`.
    async fn property_type_map(
        &self,
    ) -> Result<std::collections::HashMap<String, crate::property::ValueType>> {
        Ok(self
            .get_all_property_defs()
            .await?
            .into_iter()
            .map(|d| {
                (
                    d.name.to_ascii_lowercase(),
                    crate::property::ValueType::parse(&d.value_type),
                )
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
        // Per-row override JSON in child→parent order, fed to build_overrides.
        let mut override_rows: Vec<(String, Vec<(String, Vec<String>)>)> = Vec::new();
        let mut current_name = name.to_string();
        let mut icon = "📄".to_string();
        let mut color = "#808080".to_string();
        let mut plural = String::new();
        let mut depth = 0;

        loop {
            if depth > 10 {
                break;
            } // prevent infinite loops
            depth += 1;

            let row = sqlx::query(
                "SELECT name, extends, icon, color, plural, properties_json, property_overrides_json FROM tag_defs WHERE LOWER(name) = LOWER(?)"
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
                        plural = row
                            .try_get::<Option<String>, _>("plural")
                            .ok()
                            .flatten()
                            .unwrap_or_default();
                    }
                    let props_str: String = row.get("properties_json");
                    let props: Vec<String> = serde_json::from_str(&props_str).unwrap_or_default();
                    // Prepend parent properties (parent first, child overrides)
                    all_property_names.extend(props);

                    let overrides_json: String = row.get("property_overrides_json");
                    override_rows.push((overrides_json, legacy_hidden_pairs()));

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

        // SEPARATE override pass (§3.5): child→parent, first-insert-wins.
        let overrides = build_overrides(&override_rows);

        // Resolve property definitions from property_defs table
        let mut resolved_props = Vec::new();
        for prop_name in &all_property_names {
            let prop_row = sqlx::query(
                "SELECT name, value_type, choices_json, default_value, multiple_values, hide_empty, hide_by_default, description FROM property_defs WHERE LOWER(name) = LOWER(?)"
            )
            .bind(prop_name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("Failed to resolve property", e))?;

            let over = overrides.get(&prop_name.to_ascii_lowercase());
            match prop_row {
                Some(row) => {
                    let choices_str: Option<String> = row.get("choices_json");
                    let hide_by_default: bool = row.get("hide_by_default");
                    let mut def = crate::types::PropertyDef {
                        name: row.get("name"),
                        value_type: row.get("value_type"),
                        values: choices_str.and_then(|s| serde_json::from_str(&s).ok()),
                        default: row.get("default_value"),
                        required: false,
                        ..Default::default()
                    };
                    apply_override(&mut def, over, hide_by_default);
                    resolved_props.push(def);
                }
                None => {
                    // Property page doesn't exist yet — show as text.
                    // An override (choices/default/show) still applies (§3.5c).
                    let mut def = crate::types::PropertyDef {
                        name: prop_name.clone(),
                        value_type: "text".to_string(),
                        values: None,
                        default: None,
                        required: false,
                        ..Default::default()
                    };
                    apply_override(&mut def, over, false);
                    resolved_props.push(def);
                }
            }
        }

        let plural = if plural.trim().is_empty() {
            name.to_string()
        } else {
            plural
        };
        Ok(Some(crate::types::TypeDefinition {
            name: name.to_string(),
            description: String::new(),
            icon,
            color,
            plural,
            properties: resolved_props,
        }))
    }

    /// Get all tag definitions from the cache.
    pub async fn get_all_tag_defs(&self) -> Result<Vec<crate::types::TypeDefinition>> {
        use sqlx::Row;
        let rows = sqlx::query(
            "SELECT name, extends, icon, color, plural, properties_json, property_overrides_json FROM tag_defs ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("Failed to get tag defs", e))?;

        let mut result = Vec::new();
        for row in &rows {
            let props_str: String = row.get("properties_json");
            let prop_names: Vec<String> = serde_json::from_str(&props_str).unwrap_or_default();

            // This function lists each tag's OWN direct properties (no extends
            // walk, unlike get_resolved_tag_def). Apply the same separate-pass
            // override merge over just this row's overrides — consistent
            // child-wins semantics, just with a single-element chain.
            let overrides_json: String = row.get("property_overrides_json");
            let overrides =
                build_overrides(&[(overrides_json, legacy_hidden_pairs())]);

            // Resolve each property name against property_defs for full schema
            let mut resolved_props = Vec::new();
            for pname in &prop_names {
                let prop_row = sqlx::query(
                    "SELECT name, value_type, choices_json, default_value, hide_by_default FROM property_defs WHERE LOWER(name) = LOWER(?)"
                )
                .bind(pname)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| db_err("Failed to resolve property in get_all_tag_defs", e))?;

                let over = overrides.get(&pname.to_ascii_lowercase());
                match prop_row {
                    Some(pr) => {
                        let choices_str: Option<String> = pr.get("choices_json");
                        let hide_by_default: bool = pr.get("hide_by_default");
                        let mut def = crate::types::PropertyDef {
                            name: pr.get("name"),
                            value_type: pr.get("value_type"),
                            values: choices_str.and_then(|s| serde_json::from_str(&s).ok()),
                            default: pr.get("default_value"),
                            required: false,
                            ..Default::default()
                        };
                        apply_override(&mut def, over, hide_by_default);
                        resolved_props.push(def);
                    }
                    None => {
                        let mut def = crate::types::PropertyDef {
                            name: pname.clone(),
                            value_type: "text".to_string(),
                            values: None,
                            default: None,
                            required: false,
                            ..Default::default()
                        };
                        apply_override(&mut def, over, false);
                        resolved_props.push(def);
                    }
                }
            }

            let name: String = row.get("name");
            let plural = row
                .try_get::<Option<String>, _>("plural")
                .ok()
                .flatten()
                .filter(|p| !p.trim().is_empty())
                .unwrap_or_else(|| name.clone());
            result.push(crate::types::TypeDefinition {
                name,
                description: String::new(),
                icon: row.get("icon"),
                color: row.get("color"),
                plural,
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
        // DSL-embedded `ORDER BY` wins over the external param so a
        // saved-query note can carry its own sort spec; the external
        // `sort` arg remains the fallback for ad-hoc callers that
        // want to override without modifying the DSL.
        let effective_sort = query.sort.as_deref().or(sort);
        apply_sort(&mut items, effective_sort);
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
            let prop_rows =
                sqlx::query("SELECT property_name, value FROM block_properties WHERE block_id = ?")
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
                    .map(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case("task")))
                    .unwrap_or(false);
                let has_status = props.contains_key("status");
                has_task_tag || has_status
            };
            let kind = if is_task {
                AgendaRowKind::Task
            } else {
                AgendaRowKind::Event
            };

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
            let rec = recurrence_str.as_deref().and_then(recurrence::parse);
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
        use crate::query::{block_matches_typed, Kind, QueryItem, QueryOp};

        // L5: typed-comparison registry — built once, consulted per block.
        let types = self.property_type_map().await?;

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
            let mut blocks = self.parsed_blocks_cached(note_id, body);
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
                if !block_matches_typed(block, query, &types) {
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

    /// `parse_blocks(note_id, body)`, served from `blocks_cache` when a
    /// prior call already parsed this exact `body` for this `note_id`.
    ///
    /// Correctness contract: the cache key is a SHA-256 digest of `body`
    /// itself, not a timestamp, version counter, or "was this note
    /// touched" flag. A hit therefore requires the input to be
    /// byte-identical to what produced the cached output — there is no
    /// window where an edited note can still serve its pre-edit parse.
    /// Any change to the note's body (including a no-op-looking edit like
    /// re-saving unchanged text) either hits with the *same* hash and
    /// returns an identical reparse, or misses and reparses; either way
    /// the result matches what a full reparse would give right now.
    /// Property-only edits (frontmatter, tags) don't touch `body`, so
    /// they naturally keep hitting — that's not a staleness risk because
    /// `parse_blocks` doesn't consume frontmatter/tags in the first
    /// place. Deletion is handled separately by `remove_note` evicting
    /// the entry (dead weight cleanup, not a correctness requirement:
    /// a deleted note no longer appears in the candidate rows this is
    /// called from).
    ///
    /// Always returns an **owned** `Vec` — the cached copy lives behind an
    /// `Arc` so concurrent callers share the parse, but each caller gets
    /// its own clone to mutate per-query (`execute_block_query` stamps
    /// the current `parent_note_type` on every call) without corrupting
    /// what's cached.
    fn parsed_blocks_cached(&self, note_id: &str, body: &str) -> Vec<ParsedBlock> {
        use crate::block::parse_blocks;

        let body_hash = format!("{:x}", Sha256::digest(body.as_bytes()));

        if let Some(entry) = self
            .blocks_cache
            .lock()
            .expect("blocks_cache mutex poisoned")
            .get(note_id)
        {
            if entry.body_hash == body_hash {
                return (*entry.blocks).clone();
            }
        }

        let fresh = Arc::new(parse_blocks(note_id, body));
        self.blocks_cache
            .lock()
            .expect("blocks_cache mutex poisoned")
            .insert(
                note_id.to_string(),
                CachedBlocks {
                    body_hash,
                    blocks: Arc::clone(&fresh),
                },
            );
        (*fresh).clone()
    }

    /// Execute a `kind:page` query. Loads all notes (corpus is small) and
    /// filters in-memory using the same `block_matches` semantics applied to
    /// a synthetic "page block" (tags + properties from frontmatter).
    async fn execute_page_query(
        &self,
        query: &crate::query::ParsedQuery,
    ) -> Result<Vec<crate::query::QueryItem>> {
        use crate::query::{block_matches_typed, Kind, QueryItem};
        use std::collections::HashMap;

        // L5: typed-comparison registry — built once, consulted per page-block.
        let types = self.property_type_map().await?;

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
                bid: None,
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
            if !block_matches_typed(&pseudo, query, &types) {
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
            .map(|(_, v)| normalize_sort_value(v))
            .unwrap_or_default(),
    }
}

/// Strip a fully-wrapping `[[…]]` from a property value before using it
/// as a sort key, then lowercase. Without this, legacy inline-link
/// dates (`[[2026-05-24]]`) and the new bare-ISO form (`2026-05-24`)
/// compare as different strings — `[` sorts before `2` in ASCII — so a
/// mixed-format mosaic gets nonsensical ordering. Only fully-wrapped
/// values are unwrapped; mid-string brackets (`see [[Project]] notes`)
/// stay intact since they're real content, not a link wrapper.
fn normalize_sort_value(value: &str) -> String {
    let trimmed = value.trim();
    if let Some(inner) = trimmed
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
    {
        inner.to_ascii_lowercase()
    } else {
        trimmed.to_ascii_lowercase()
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

    /// Concurrent upserts of the SAME brand-new note id must both succeed.
    ///
    /// The HTTP create handler's reindex races the fs-watcher's reindex of the
    /// file the handler just wrote. A two-statement UPDATE-then-INSERT upsert
    /// lets both writers observe rows_affected == 0 and take the INSERT branch;
    /// the loser dies with `UNIQUE constraint failed: notes.id` (the
    /// intermittent POST /notes 500). The upsert must be one atomic statement.
    #[tokio::test]
    async fn concurrent_upsert_of_same_new_note_both_succeed() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        for i in 0..50 {
            let note = make_test_note(&format!("race-{i}"), "Race", "body", &[]);
            let (a, b) = tokio::join!(index.upsert_note(&note), index.upsert_note(&note));
            a.unwrap();
            b.unwrap();
        }
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
        assert_eq!(
            rows.len(),
            2,
            "expected 2 rows (done excluded): got {rows:?}"
        );
        assert!(
            rows.iter()
                .any(|r| r.kind == crate::query::AgendaRowKind::Task
                    && r.occurrence_date == "2026-05-22"),
            "task row missing"
        );
        assert!(
            rows.iter()
                .any(|r| r.kind == crate::query::AgendaRowKind::Event
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
        let rows = index
            .agenda_blocks("2026-05-22", "2026-05-22", false)
            .await
            .unwrap();
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
        let rows = index
            .agenda_blocks("2026-04-15", "2026-04-15", false)
            .await
            .unwrap();
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
        let rows = index
            .agenda_blocks("2026-05-19", "2026-05-26", false)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].occurrence_date, "2026-05-20",
            "anchor should be scheduled date"
        );
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

        assert_eq!(
            rows.len(),
            3,
            "count 3 should yield exactly 3 rows: got {rows:?}"
        );
    }

    // ────────────────────────────────────────────────────────────────
    // apply_sort: wiki-link normalization
    // ────────────────────────────────────────────────────────────────

    fn make_query_item(text: &str, props: &[(&str, &str)]) -> crate::query::QueryItem {
        use crate::query::{Kind, QueryItem};
        let mut p: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for (k, v) in props {
            p.insert((*k).to_string(), (*v).to_string());
        }
        QueryItem {
            block_id: Some(format!("note:{text}")),
            page_id: "note".to_string(),
            title: "note".to_string(),
            text: text.to_string(),
            parent_breadcrumb: vec![],
            kind: Kind::Block,
            primary_tag: None,
            properties: p,
            page_note_type: None,
        }
    }

    /// Regression: wrapped + bare versions of the **same** date used
    /// to be treated as different keys because the sort compared raw
    /// strings (`"[[2026-05-24]]"` ≠ `"2026-05-24"`). With wrapper
    /// stripping they tie on date and fall to stable input order.
    #[test]
    fn apply_sort_treats_wrapped_and_bare_same_date_as_equal() {
        let mut items = vec![
            make_query_item("bare-a", &[("scheduled", "2026-05-24")]),
            make_query_item("wrapped", &[("scheduled", "[[2026-05-24]]")]),
            make_query_item("bare-b", &[("scheduled", "2026-05-24")]),
            make_query_item("older", &[("scheduled", "2026-05-23")]),
        ];
        apply_sort(&mut items, Some("scheduled desc"));
        let order: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
        // The three 05-24 entries tie; stable sort preserves their
        // input order. `older` (05-23) must come last under DESC.
        assert_eq!(order, vec!["bare-a", "wrapped", "bare-b", "older"]);
    }

    /// Three distinct dates still sort by value (this passed before the
    /// fix because the dates differ; kept as a regression guard for
    /// normal ordering).
    #[test]
    fn apply_sort_strips_wiki_link_wrappers_on_date_values() {
        let mut items = vec![
            make_query_item("a", &[("scheduled", "[[2026-05-24]]")]),
            make_query_item("b", &[("scheduled", "2026-05-23")]),
            make_query_item("c", &[("scheduled", "2026-05-22")]),
        ];
        apply_sort(&mut items, Some("scheduled"));
        let order: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(order, vec!["c", "b", "a"]);
    }

    /// Same regression on the DESC path.
    #[test]
    fn apply_sort_strips_wiki_link_wrappers_descending() {
        let mut items = vec![
            make_query_item("a", &[("scheduled", "2026-05-22")]),
            make_query_item("b", &[("scheduled", "[[2026-05-24]]")]),
            make_query_item("c", &[("scheduled", "2026-05-23")]),
        ];
        apply_sort(&mut items, Some("scheduled desc"));
        let order: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(order, vec!["b", "c", "a"]);
    }

    /// Non-wrapped values still sort as plain case-insensitive strings —
    /// the wrapper-strip path must not change behavior for ordinary
    /// property values like `status: todo` / `status: doing`.
    #[test]
    fn apply_sort_keeps_plain_values_unchanged() {
        let mut items = vec![
            make_query_item("a", &[("status", "todo")]),
            make_query_item("b", &[("status", "doing")]),
            make_query_item("c", &[("status", "blocked")]),
        ];
        apply_sort(&mut items, Some("status"));
        let order: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(order, vec!["c", "b", "a"]); // blocked, doing, todo
    }

    /// A property value that *contains* `[[…]]` mid-string (not the
    /// whole value) must keep its brackets — only fully-wrapped values
    /// are unwrapped, since mid-string brackets are real content.
    #[test]
    fn apply_sort_only_strips_fully_wrapped_values() {
        let mut items = vec![
            make_query_item("a", &[("status", "see [[Project]] notes")]),
            make_query_item("b", &[("status", "blocked")]),
        ];
        apply_sort(&mut items, Some("status"));
        // "blocked" < "see [[Project]] notes" lexically — verify the
        // mid-string brackets weren't blindly stripped.
        let order: Vec<&str> = items.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(order, vec!["b", "a"]);
    }

    // ────────────────────────────────────────────────────────────────
    // type-def cache hygiene (regression: stale tag_defs / property_defs)
    //
    // Bug: `remove_note` only deleted from `notes`; with the old
    // `ON DELETE SET NULL` FK, the cached type_def row survived with
    // `note_id=NULL` and kept surfacing in `GET /types` and the
    // resolver. The `_ =>` arm in `index_type_info` had the same
    // failure mode when a note's `note_type` changed away from
    // Tag/Property.
    // ────────────────────────────────────────────────────────────────

    /// Deleting a Tag/Property page must remove its cached
    /// type-definition row. Without the explicit `DELETE` in
    /// `remove_note` (and the CASCADE FK from migration 005), a
    /// `note_id=NULL` ghost would linger in the cache and surface in
    /// `get_all_tag_defs` / `get_all_property_defs`.
    #[tokio::test]
    async fn remove_note_clears_cached_type_defs() {
        use crate::traits::search_index::SearchIndex as _;
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // Index a Tag page and a Property page.
        let mut tag_note = make_test_note("rm-tag", "TaskTag", "- a tag page", &[]);
        tag_note.metadata.note_type = Some("Tag".to_string());
        index.reindex(&tag_note).await.unwrap();

        let mut prop_note = make_test_note("rm-prop", "Status", "- a property page", &[]);
        prop_note.metadata.note_type = Some("Property".to_string());
        index.reindex(&prop_note).await.unwrap();

        // Sanity: both cached rows exist.
        let tag_defs = index.get_all_tag_defs().await.unwrap();
        assert_eq!(tag_defs.len(), 1, "tag_defs should hold the cached Tag row");
        assert_eq!(tag_defs[0].name, "TaskTag");
        let prop_defs = index.get_all_property_defs().await.unwrap();
        assert_eq!(
            prop_defs.len(),
            1,
            "property_defs should hold the cached Property row"
        );
        assert_eq!(prop_defs[0].name, "Status");

        // Remove the Tag page → its cached row must be gone.
        index.remove_note(&NoteId::new("rm-tag")).await.unwrap();
        let tag_defs = index.get_all_tag_defs().await.unwrap();
        assert!(
            tag_defs.is_empty(),
            "tag_defs should be empty after remove_note; got {tag_defs:?}"
        );
        // Property cache must be untouched.
        let prop_defs = index.get_all_property_defs().await.unwrap();
        assert_eq!(
            prop_defs.len(),
            1,
            "property_defs should still hold the Property row"
        );

        // Remove the Property page → its cached row must be gone too.
        index.remove_note(&NoteId::new("rm-prop")).await.unwrap();
        let prop_defs = index.get_all_property_defs().await.unwrap();
        assert!(
            prop_defs.is_empty(),
            "property_defs should be empty after remove_note; got {prop_defs:?}"
        );
    }

    /// A bulk `rebuild_from_notes` (the startup + relay-bootstrap path)
    /// must repopulate `property_defs`/`tag_defs`, not just the notes
    /// table — otherwise the property registry is empty after a rebuild
    /// and server-side typed queries (L5) + `GET /properties` see no defs.
    /// Regression guard for the L5-followup fix.
    #[tokio::test]
    async fn rebuild_from_notes_repopulates_property_and_tag_defs() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // A Property page declaring a number type (like the builtin `points`)
        // and a Tag page, alongside a plain note.
        let mut points = make_test_note("points", "points", "- Points property.", &[]);
        points.metadata.note_type = Some("Property".to_string());
        points
            .metadata
            .custom
            .insert("value_type".to_string(), serde_json::json!("number"));
        let mut task = make_test_note("task", "Task", "- Task tag.", &[]);
        task.metadata.note_type = Some("Tag".to_string());
        let plain = make_test_note("n1", "Note One", "- hello", &[]);

        index
            .rebuild_from_notes(&[points, task, plain])
            .await
            .unwrap();

        let defs = index.get_all_property_defs().await.unwrap();
        assert_eq!(
            defs.len(),
            1,
            "rebuild must register the Property page; got {defs:?}"
        );
        assert_eq!(defs[0].name, "points");
        assert_eq!(defs[0].value_type, "number");

        let tags = index.get_all_tag_defs().await.unwrap();
        assert_eq!(tags.len(), 1, "rebuild must register the Tag page; got {tags:?}");
        assert_eq!(tags[0].name, "Task");
    }

    /// A note's `note_type` flipping away from Tag/Property (or
    /// across the Tag ↔ Property boundary) must drop the old cached
    /// row. A re-tag with a new name must also leave exactly one
    /// row — no ghost under the old name.
    #[tokio::test]
    async fn reindex_clears_stale_type_defs_on_type_change() {
        use crate::traits::search_index::SearchIndex as _;
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // Start as a Tag page named "OldName".
        let mut note = make_test_note("rt-1", "OldName", "- body", &[]);
        note.metadata.note_type = Some("Tag".to_string());
        index.reindex(&note).await.unwrap();
        let tag_defs = index.get_all_tag_defs().await.unwrap();
        assert_eq!(tag_defs.len(), 1);
        assert_eq!(tag_defs[0].name, "OldName");

        // Reindex with a different type — the stale tag row must be
        // gone, and no property row should appear.
        note.metadata.note_type = Some("Project".to_string());
        index.reindex(&note).await.unwrap();
        let tag_defs = index.get_all_tag_defs().await.unwrap();
        assert!(
            tag_defs.is_empty(),
            "tag_defs should be empty after Tag → Project; got {tag_defs:?}"
        );
        let prop_defs = index.get_all_property_defs().await.unwrap();
        assert!(
            prop_defs.is_empty(),
            "property_defs should be empty after Tag → Project; got {prop_defs:?}"
        );

        // Flip to Property — property_defs gets the row, tag_defs
        // stays empty.
        note.metadata.note_type = Some("Property".to_string());
        index.reindex(&note).await.unwrap();
        let prop_defs = index.get_all_property_defs().await.unwrap();
        assert_eq!(prop_defs.len(), 1);
        assert_eq!(prop_defs[0].name, "OldName");
        let tag_defs = index.get_all_tag_defs().await.unwrap();
        assert!(tag_defs.is_empty());

        // Now flip back to a Tag with a NEW title — the property
        // cache must be cleared, and the tag cache must contain
        // exactly one row, under the new name. No ghost under
        // "OldName".
        note.title = "NewName".to_string();
        note.metadata.note_type = Some("Tag".to_string());
        index.reindex(&note).await.unwrap();
        let tag_defs = index.get_all_tag_defs().await.unwrap();
        assert_eq!(
            tag_defs.len(),
            1,
            "rename should leave exactly one tag_defs row; got {tag_defs:?}"
        );
        assert_eq!(tag_defs[0].name, "NewName");
        let prop_defs = index.get_all_property_defs().await.unwrap();
        assert!(
            prop_defs.is_empty(),
            "property_defs should be cleared on Property → Tag; got {prop_defs:?}"
        );
    }

    // ────────────────────────────────────────────────────────────────
    // Per-type property configuration (spec 2026-06-22, Phase 1).
    //
    // One global Status Property page shared by Task + Project, each
    // carrying its own `property_overrides.Status` (choices / show /
    // default). The resolver must REPLACE choices per type, derive/honor
    // `show`, apply per-type default, and leave un-overridden tags
    // identical to the global config.
    // ────────────────────────────────────────────────────────────────

    /// Build a Tag page note with an inline `property_overrides` map and a
    /// `tag_properties` membership list, the way the FLOW-YAML frontmatter
    /// parses into `custom`.
    fn make_tag_note(
        id: &str,
        title: &str,
        tag_properties: &[&str],
        overrides: serde_json::Value,
    ) -> Note {
        let mut note = make_test_note(id, title, "- a tag page", &[]);
        note.metadata.note_type = Some("Tag".to_string());
        note.metadata.custom.insert(
            "tag_properties".to_string(),
            serde_json::json!(tag_properties),
        );
        if !overrides.is_null() {
            note.metadata
                .custom
                .insert("property_overrides".to_string(), overrides);
        }
        note
    }

    /// Build a select Property page note.
    fn make_select_prop(id: &str, title: &str, choices: &[&str], default: Option<&str>) -> Note {
        let mut note = make_test_note(id, title, "- a property page", &[]);
        note.metadata.note_type = Some("Property".to_string());
        note.metadata
            .custom
            .insert("value_type".to_string(), serde_json::json!("select"));
        note.metadata
            .custom
            .insert("choices".to_string(), serde_json::json!(choices));
        if let Some(d) = default {
            note.metadata
                .custom
                .insert("default".to_string(), serde_json::json!(d));
        }
        note
    }

    fn status_of<'a>(
        def: &'a crate::types::TypeDefinition,
    ) -> &'a crate::types::PropertyDef {
        def.properties
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case("Status"))
            .expect("Status property missing from resolved type")
    }

    /// Core acceptance: Task + Project share one global Status, each
    /// resolving to its own per-type choices; Task's show==on_new + default.
    #[tokio::test]
    async fn per_type_status_override_replaces_choices() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // Global Status property — the fallback list.
        let status = make_select_prop(
            "status",
            "Status",
            &["backlog", "todo", "doing", "in-review", "done", "canceled"],
            Some("todo"),
        );
        index.reindex(&status).await.unwrap();

        let task = make_tag_note(
            "task",
            "Task",
            &["Status"],
            serde_json::json!({
                "Status": {"choices": ["todo", "doing", "done", "blocked"], "show": "on_new", "default": "todo"}
            }),
        );
        index.reindex(&task).await.unwrap();

        let project = make_tag_note(
            "project",
            "Project",
            &["Status"],
            serde_json::json!({
                "Status": {"choices": ["planned", "active", "shipped"]}
            }),
        );
        index.reindex(&project).await.unwrap();

        let task_def = index.get_resolved_tag_def("Task").await.unwrap().unwrap();
        let ts = status_of(&task_def);
        assert_eq!(
            ts.values.as_deref(),
            Some(&["todo", "doing", "done", "blocked"].map(String::from)[..]),
            "Task Status choices must be REPLACED by the override"
        );
        assert_eq!(ts.show, Some(crate::types::Visibility::OnNew));
        assert_eq!(ts.default.as_deref(), Some("todo"));

        let proj_def = index.get_resolved_tag_def("Project").await.unwrap().unwrap();
        let ps = status_of(&proj_def);
        assert_eq!(
            ps.values.as_deref(),
            Some(&["planned", "active", "shipped"].map(String::from)[..]),
            "Project Status choices must be REPLACED by the override"
        );
        // No `show` override → derived from hide_by_default (false) → on_new.
        assert_eq!(ps.show, Some(crate::types::Visibility::OnNew));

        // get_all_tag_defs must mirror get_resolved_tag_def for these.
        let all = index.get_all_tag_defs().await.unwrap();
        let all_task = all.iter().find(|t| t.name == "Task").unwrap();
        assert_eq!(
            status_of(all_task).values.as_deref(),
            Some(&["todo", "doing", "done", "blocked"].map(String::from)[..])
        );
        let all_proj = all.iter().find(|t| t.name == "Project").unwrap();
        assert_eq!(
            status_of(all_proj).values.as_deref(),
            Some(&["planned", "active", "shipped"].map(String::from)[..])
        );
    }

    /// Legacy `hidden_{Prop}` frontmatter must subtract on the RUST side
    /// too (folded into `hide_choices` at index time) so kanban/views agree
    /// with the web chips (spec §3.3 + locked decision 4). Before the
    /// index-time fold the Rust resolver ignored `hidden_` and the engines
    /// diverged — the major the Phase-1 review caught.
    #[tokio::test]
    async fn legacy_hidden_prop_subtracts_on_rust_side() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let status = make_select_prop("status", "Status", &["todo", "doing", "done"], Some("todo"));
        index.reindex(&status).await.unwrap();

        // Task uses the LEGACY `hidden_Status: [done]` form, no property_overrides.
        let mut task = make_test_note("task", "Task", "- a tag page", &[]);
        task.metadata.note_type = Some("Tag".to_string());
        task.metadata
            .custom
            .insert("tag_properties".to_string(), serde_json::json!(["Status"]));
        task.metadata
            .custom
            .insert("hidden_Status".to_string(), serde_json::json!(["done"]));
        index.reindex(&task).await.unwrap();

        let def = index.get_resolved_tag_def("Task").await.unwrap().unwrap();
        assert_eq!(
            status_of(&def).values.as_deref(),
            Some(&["todo", "doing"].map(String::from)[..]),
            "legacy hidden_Status must subtract `done` on the Rust resolver"
        );
    }

    /// REPLACE (property_overrides.choices) THEN SUBTRACT (legacy hidden_,
    /// matched case-insensitively against the override entry) compose on Rust.
    #[tokio::test]
    async fn replace_then_subtract_legacy_hidden_on_rust_side() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let status = make_select_prop("status", "Status", &["a", "b"], None);
        index.reindex(&status).await.unwrap();

        let mut task = make_test_note("task", "Task", "- a tag page", &[]);
        task.metadata.note_type = Some("Tag".to_string());
        task.metadata
            .custom
            .insert("tag_properties".to_string(), serde_json::json!(["Status"]));
        task.metadata.custom.insert(
            "property_overrides".to_string(),
            serde_json::json!({"Status": {"choices": ["todo", "doing", "done", "blocked"]}}),
        );
        // lowercase `hidden_status` must fold into the "Status" override entry.
        task.metadata
            .custom
            .insert("hidden_status".to_string(), serde_json::json!(["blocked"]));
        index.reindex(&task).await.unwrap();

        let def = index.get_resolved_tag_def("Task").await.unwrap().unwrap();
        assert_eq!(
            status_of(&def).values.as_deref(),
            Some(&["todo", "doing", "done"].map(String::from)[..]),
            "choices REPLACE to [todo,doing,done,blocked], then hidden_ subtracts blocked"
        );
    }

    /// §3.5(a): an override applies regardless of WHICH ancestor's
    /// `tag_properties` lists the property. Here the parent (Root) lists
    /// Status; the child (Task) only carries the override.
    #[tokio::test]
    async fn override_applies_regardless_of_listing_ancestor() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        index
            .reindex(&make_select_prop("status", "Status", &["a", "b", "c"], None))
            .await
            .unwrap();

        // Parent declares Status membership; no override.
        let parent = make_tag_note("root-tag", "Root Tag", &["Status"], serde_json::Value::Null);
        index.reindex(&parent).await.unwrap();

        // Child lists no extra props but overrides Status choices.
        let mut child = make_tag_note(
            "task",
            "Task",
            &[],
            serde_json::json!({"Status": {"choices": ["x", "y"]}}),
        );
        child
            .metadata
            .custom
            .insert("extends".to_string(), serde_json::json!("Root Tag"));
        index.reindex(&child).await.unwrap();

        let def = index.get_resolved_tag_def("Task").await.unwrap().unwrap();
        assert_eq!(
            status_of(&def).values.as_deref(),
            Some(&["x", "y"].map(String::from)[..]),
            "child override must apply to a property inherited from the parent"
        );
    }

    /// §3.5(b): an override for a property NOT in the resolved membership
    /// set is ignored — it never appears in the resolved type.
    #[tokio::test]
    async fn override_for_non_member_prop_ignored() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        index
            .reindex(&make_select_prop("status", "Status", &["a", "b"], None))
            .await
            .unwrap();

        let task = make_tag_note(
            "task",
            "Task",
            &["Status"],
            serde_json::json!({
                "Status": {"choices": ["a"]},
                "Priority": {"choices": ["p1", "p2"]}
            }),
        );
        index.reindex(&task).await.unwrap();

        let def = index.get_resolved_tag_def("Task").await.unwrap().unwrap();
        assert!(
            def.properties
                .iter()
                .all(|p| !p.name.eq_ignore_ascii_case("Priority")),
            "an override for a non-member property must not introduce it"
        );
    }

    /// §3.5(c): an override for a property with NO global Property page
    /// still applies its choices/default to the text-stub PropertyDef.
    #[tokio::test]
    async fn override_applies_to_propless_text_stub() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        // No Status Property page indexed at all.
        let task = make_tag_note(
            "task",
            "Task",
            &["Status"],
            serde_json::json!({
                "Status": {"choices": ["todo", "done"], "default": "todo", "show": "on_set"}
            }),
        );
        index.reindex(&task).await.unwrap();

        let def = index.get_resolved_tag_def("Task").await.unwrap().unwrap();
        let s = status_of(&def);
        assert_eq!(s.value_type, "text", "stub keeps text type (no Property page)");
        assert_eq!(
            s.values.as_deref(),
            Some(&["todo", "done"].map(String::from)[..]),
            "override choices apply even to a text stub"
        );
        assert_eq!(s.default.as_deref(), Some("todo"));
        assert_eq!(s.show, Some(crate::types::Visibility::OnSet));
    }

    /// A no-override tag resolves byte-identical to before: choices/default
    /// untouched from the global Property page, `show` derived (on_new since
    /// hide_by_default defaults false).
    #[tokio::test]
    async fn no_override_tag_uses_global_config_unchanged() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        index
            .reindex(&make_select_prop(
                "status",
                "Status",
                &["backlog", "todo", "done"],
                Some("todo"),
            ))
            .await
            .unwrap();

        // Tag with Status membership and NO property_overrides.
        let person = make_tag_note("person", "Person", &["Status"], serde_json::Value::Null);
        index.reindex(&person).await.unwrap();

        let def = index.get_resolved_tag_def("Person").await.unwrap().unwrap();
        let s = status_of(&def);
        assert_eq!(
            s.values.as_deref(),
            Some(&["backlog", "todo", "done"].map(String::from)[..]),
            "no override → global choices unchanged"
        );
        assert_eq!(s.default.as_deref(), Some("todo"), "global default unchanged");
        assert_eq!(
            s.show,
            Some(crate::types::Visibility::OnNew),
            "no show override + hide_by_default=false → on_new"
        );
    }

    /// `hide_by_default=true` on the global Property derives `show: hidden`
    /// when the type carries no `show` override.
    #[tokio::test]
    async fn hide_by_default_derives_hidden_show() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let mut status = make_select_prop("status", "Status", &["a", "b"], None);
        status
            .metadata
            .custom
            .insert("hide_by_default".to_string(), serde_json::json!(true));
        index.reindex(&status).await.unwrap();

        let task = make_tag_note("task", "Task", &["Status"], serde_json::Value::Null);
        index.reindex(&task).await.unwrap();

        let def = index.get_resolved_tag_def("Task").await.unwrap().unwrap();
        assert_eq!(
            status_of(&def).show,
            Some(crate::types::Visibility::Hidden),
            "hide_by_default=true with no show override → hidden"
        );
    }

    /// Replace-then-subtract: `choices` override REPLACES, then
    /// `hide_choices` SUBTRACTS from the replaced list (§3.3 precedence).
    #[tokio::test]
    async fn choices_replace_then_subtract_hide_choices() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        index
            .reindex(&make_select_prop(
                "status",
                "Status",
                &["g1", "g2", "g3"],
                None,
            ))
            .await
            .unwrap();

        let task = make_tag_note(
            "task",
            "Task",
            &["Status"],
            serde_json::json!({
                "Status": {"choices": ["todo", "doing", "done", "blocked"], "hide_choices": ["blocked"]}
            }),
        );
        index.reindex(&task).await.unwrap();

        let def = index.get_resolved_tag_def("Task").await.unwrap().unwrap();
        assert_eq!(
            status_of(&def).values.as_deref(),
            Some(&["todo", "doing", "done"].map(String::from)[..]),
            "hide_choices must subtract from the REPLACED list"
        );
    }

    // -----------------------------------------------------------------------
    // execute_block_query parsed-blocks cache invalidation (tesela-sclr.2)
    // -----------------------------------------------------------------------

    /// Flatten a `QueryResult` into its items' display text, in group
    /// order — enough to assert presence/absence without caring about
    /// grouping/sort for these cache tests.
    fn item_texts(result: &crate::query::QueryResult) -> Vec<String> {
        result
            .groups
            .iter()
            .flat_map(|g| g.items.iter().map(|i| i.text.clone()))
            .collect()
    }

    fn find_item<'a>(
        result: &'a crate::query::QueryResult,
        text: &str,
    ) -> Option<&'a crate::query::QueryItem> {
        result
            .groups
            .iter()
            .flat_map(|g| g.items.iter())
            .find(|i| i.text == text)
    }

    /// A note edit (body changes, same note_id) must invalidate the
    /// per-note parsed-blocks cache. A stale cache would keep matching
    /// `-has:status` against the PRE-edit block even after the block
    /// gained a `status::` property.
    #[tokio::test]
    async fn parsed_blocks_cache_reflects_note_edit() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let note = make_test_note("cache-edit-1", "Edit Note", "- buy milk", &[]);
        index.reindex(&note).await.unwrap();

        let query = crate::query::parse_query("kind:block -has:status");
        let result = index.execute_query(&query, None, None).await.unwrap();
        assert!(
            item_texts(&result).contains(&"buy milk".to_string()),
            "expected untriaged block before edit: {:?}",
            item_texts(&result)
        );

        // Same note_id, edited body — a status:: line is added to the
        // block. This must be a cache MISS (different body hash).
        let mut edited = note.clone();
        edited.body = "- buy milk\n  status:: done".to_string();
        edited.content = format!("# Edit Note\n\n{}", edited.body);
        index.reindex(&edited).await.unwrap();

        let result = index.execute_query(&query, None, None).await.unwrap();
        assert!(
            !item_texts(&result).contains(&"buy milk".to_string()),
            "block now has status:: done but still matched -has:status — \
             stale cache served the pre-edit parse: {:?}",
            item_texts(&result)
        );
    }

    /// A deleted note's blocks must never resurface from the cache. Also
    /// exercises `remove_note`'s explicit `blocks_cache` eviction.
    #[tokio::test]
    async fn parsed_blocks_cache_evicted_on_note_delete() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let keep = make_test_note("cache-keep-1", "Keep Note", "- keep me", &[]);
        let gone = make_test_note("cache-gone-1", "Gone Note", "- remove me", &[]);
        index.reindex(&keep).await.unwrap();
        index.reindex(&gone).await.unwrap();

        let query = crate::query::parse_query("kind:block -has:status");
        let result = index.execute_query(&query, None, None).await.unwrap();
        let texts = item_texts(&result);
        assert!(texts.contains(&"keep me".to_string()));
        assert!(texts.contains(&"remove me".to_string()));

        index
            .remove_note(&NoteId::new("cache-gone-1"))
            .await
            .unwrap();

        let result = index.execute_query(&query, None, None).await.unwrap();
        let texts = item_texts(&result);
        assert!(texts.contains(&"keep me".to_string()));
        assert!(
            !texts.contains(&"remove me".to_string()),
            "deleted note's block still served after delete: {texts:?}"
        );
    }

    /// A property-only reindex (note_type changes, `body` text untouched)
    /// keeps the same body hash — the parsed-blocks cache correctly HITS
    /// — but the query must still see the new note_type, because
    /// `parent_note_type` is stamped fresh from the current SQL row on
    /// every call rather than being baked into the cached blocks.
    #[tokio::test]
    async fn parsed_blocks_cache_reflects_note_type_change_without_body_edit() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let mut note = make_test_note("cache-prop-1", "Some Page", "- a stray block", &[]);
        index.reindex(&note).await.unwrap();

        let query = crate::query::parse_query("kind:block on:system-pages");
        let result = index.execute_query(&query, None, None).await.unwrap();
        assert!(
            item_texts(&result).is_empty(),
            "not a system page yet, should match nothing: {:?}",
            item_texts(&result)
        );

        // Body is byte-identical — only note_type (frontmatter-derived)
        // changes. This is exactly the case where the cache SHOULD hit.
        assert_eq!(note.body, "- a stray block");
        note.metadata.note_type = Some("Tag".to_string());
        index.reindex(&note).await.unwrap();

        let result = index.execute_query(&query, None, None).await.unwrap();
        assert!(
            item_texts(&result).contains(&"a stray block".to_string()),
            "note_type change not reflected after a body-unchanged reindex \
             (parent_note_type must never be cached): {:?}",
            item_texts(&result)
        );
    }

    /// Moving a block to a different parent changes its computed
    /// breadcrumb. A stale cache (serving the pre-move parse) would keep
    /// reporting the OLD parent even after the note was reindexed with
    /// the new structure.
    #[tokio::test]
    async fn parsed_blocks_cache_reflects_block_moved() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let mut note = make_test_note(
            "cache-move-1",
            "Move Note",
            "- Project A\n  - subtask one\n- Project B",
            &[],
        );
        index.reindex(&note).await.unwrap();

        let query = crate::query::parse_query("kind:block -has:status");
        let result = index.execute_query(&query, None, None).await.unwrap();
        let sub = find_item(&result, "subtask one").expect("subtask present before move");
        assert_eq!(
            sub.parent_breadcrumb,
            vec!["Move Note".to_string(), "Project A".to_string()]
        );

        // Same note_id, body reorganized so "subtask one" now nests under
        // "Project B" instead of "Project A".
        note.body = "- Project A\n- Project B\n  - subtask one".to_string();
        note.content = format!("# Move Note\n\n{}", note.body);
        index.reindex(&note).await.unwrap();

        let result = index.execute_query(&query, None, None).await.unwrap();
        let sub = find_item(&result, "subtask one").expect("subtask still present after move");
        assert_eq!(
            sub.parent_breadcrumb,
            vec!["Move Note".to_string(), "Project B".to_string()],
            "breadcrumb still reflects the PRE-move parent — stale cache"
        );
    }

    /// A cache HIT (second call, nothing changed) must return exactly the
    /// same items as the cache MISS (first call) — guards against the
    /// cached `Arc<Vec<ParsedBlock>>` being mutated in place by the
    /// per-query `parent_note_type` stamping instead of cloned out.
    #[tokio::test]
    async fn parsed_blocks_cache_hit_matches_cache_miss() {
        let index = SqliteIndex::open_in_memory().await.unwrap();
        let note = make_test_note(
            "cache-stable-1",
            "Stable Note",
            "- steady block\n  - child block",
            &[],
        );
        index.reindex(&note).await.unwrap();

        let query = crate::query::parse_query("kind:block -has:status");
        let first = index.execute_query(&query, None, None).await.unwrap();
        let second = index.execute_query(&query, None, None).await.unwrap();

        assert_eq!(
            item_texts(&first),
            item_texts(&second),
            "cache-hit query diverged from the cache-miss query"
        );
        let child = find_item(&second, "child block").expect("child block present");
        assert_eq!(
            child.parent_breadcrumb,
            vec!["Stable Note".to_string(), "steady block".to_string()]
        );
    }
}
