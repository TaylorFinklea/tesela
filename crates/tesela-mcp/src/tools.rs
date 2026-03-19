//! MCP tool definitions and handlers

use serde_json::{json, Value};
use std::sync::Arc;
use tesela_core::{
    daily::DailyNoteConfig,
    db::SqliteIndex,
    note::NoteId,
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    traits::plugin::PluginRegistry,
};

pub struct ToolRegistry {
    pub store: Arc<FsNoteStore>,
    pub index: Arc<SqliteIndex>,
    pub daily_config: DailyNoteConfig,
    pub registry: Arc<PluginRegistry>,
}

/// Returns the MCP tools/list response.
pub fn list_tools() -> Value {
    json!({
        "tools": [
            {
                "name": "search_notes",
                "description": "Full-text search through notes",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "description": "Max results (default 10)" }
                    },
                    "required": ["query"]
                }
            },
            {
                "name": "get_note",
                "description": "Get a note by ID or title",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Note ID" },
                        "title": { "type": "string", "description": "Note title (fuzzy match)" }
                    }
                }
            },
            {
                "name": "create_note",
                "description": "Create a new note",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" },
                        "content": { "type": "string" },
                        "tags": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["title"]
                }
            },
            {
                "name": "list_notes",
                "description": "List notes with optional filters",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tag": { "type": "string", "description": "Filter by tag" },
                        "limit": { "type": "integer", "description": "Max results (default 20)" },
                        "offset": { "type": "integer", "description": "Pagination offset" }
                    }
                }
            },
            {
                "name": "get_backlinks",
                "description": "Get all notes that link to a given note",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string", "description": "Note ID to find backlinks for" }
                    },
                    "required": ["id"]
                }
            },
            {
                "name": "get_daily_note",
                "description": "Get or create the daily note for a given date",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "date": { "type": "string", "description": "Date in YYYY-MM-DD format (defaults to today)" }
                    }
                }
            }
        ]
    })
}

impl ToolRegistry {
    pub fn new(store: Arc<FsNoteStore>, index: Arc<SqliteIndex>, registry: Arc<PluginRegistry>) -> Self {
        Self {
            store,
            index,
            daily_config: DailyNoteConfig::default(),
            registry,
        }
    }

    pub async fn call(&self, name: &str, params: Option<Value>) -> Result<Value, String> {
        let params = params.unwrap_or(json!({}));
        match name {
            "search_notes" => self.search_notes(params).await,
            "get_note" => self.get_note(params).await,
            "create_note" => self.create_note(params).await,
            "list_notes" => self.list_notes(params).await,
            "get_backlinks" => self.get_backlinks(params).await,
            "get_daily_note" => self.get_daily_note(params).await,
            _ => Err(format!("Unknown tool: {}", name)),
        }
    }

    async fn search_notes(&self, params: Value) -> Result<Value, String> {
        let query = params["query"]
            .as_str()
            .ok_or("Missing required field: query")?;
        let limit = params["limit"].as_u64().unwrap_or(10) as usize;

        let hits = self
            .index
            .search(query, limit, 0)
            .await
            .map_err(|e| e.to_string())?;

        let results: Vec<Value> = hits
            .iter()
            .map(|h| {
                json!({
                    "id": h.note_id.as_str(),
                    "title": h.title,
                    "snippet": h.snippet,
                    "tags": h.tags,
                })
            })
            .collect();

        Ok(json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&results).unwrap() }]
        }))
    }

    async fn get_note(&self, params: Value) -> Result<Value, String> {
        let note = if let Some(id) = params["id"].as_str() {
            self.store
                .get(&NoteId::new(id))
                .await
                .map_err(|e| e.to_string())?
        } else if let Some(title) = params["title"].as_str() {
            self.store
                .get_by_title(title)
                .await
                .map_err(|e| e.to_string())?
        } else {
            return Err("Provide either 'id' or 'title'".to_string());
        };

        match note {
            Some(n) => Ok(json!({
                "content": [{ "type": "text", "text": format!(
                    "# {}\n\nID: {}\nTags: {}\n\n{}",
                    n.title, n.id, n.metadata.tags.join(", "), n.body
                )}]
            })),
            None => Ok(json!({
                "content": [{ "type": "text", "text": "Note not found" }],
                "isError": true
            })),
        }
    }

    async fn create_note(&self, params: Value) -> Result<Value, String> {
        let title = params["title"]
            .as_str()
            .ok_or("Missing required field: title")?;
        let content = params["content"].as_str().unwrap_or("");
        let tags: Vec<&str> = params["tags"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        let note = self
            .store
            .create(title, content, &tags)
            .await
            .map_err(|e| e.to_string())?;

        // Index the new note
        let _ = self.index.reindex(&note).await;

        if let Err(e) = self.registry.dispatch_note_created(&note) {
            tracing::warn!("Plugin hook on_note_created failed: {}", e);
        }

        Ok(json!({
            "content": [{ "type": "text", "text": format!("Created note '{}' with ID: {}", note.title, note.id) }]
        }))
    }

    async fn list_notes(&self, params: Value) -> Result<Value, String> {
        let tag = params["tag"].as_str();
        let limit = params["limit"].as_u64().unwrap_or(20) as usize;
        let offset = params["offset"].as_u64().unwrap_or(0) as usize;

        let notes = self
            .store
            .list(tag, limit, offset)
            .await
            .map_err(|e| e.to_string())?;

        let results: Vec<Value> = notes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id.as_str(),
                    "title": n.title,
                    "tags": n.metadata.tags,
                    "created": n.created_at.to_rfc3339(),
                    "modified": n.modified_at.to_rfc3339(),
                })
            })
            .collect();

        Ok(json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&results).unwrap() }]
        }))
    }

    async fn get_backlinks(&self, params: Value) -> Result<Value, String> {
        let id = params["id"]
            .as_str()
            .ok_or("Missing required field: id")?;

        let links = self
            .index
            .get_backlinks(&NoteId::new(id))
            .await
            .map_err(|e| e.to_string())?;

        let results: Vec<Value> = links
            .iter()
            .map(|l| {
                json!({
                    "source": l.target,
                    "text": l.text,
                })
            })
            .collect();

        Ok(json!({
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&results).unwrap() }]
        }))
    }

    async fn get_daily_note(&self, params: Value) -> Result<Value, String> {
        let date = params["date"]
            .as_str()
            .map(|d| {
                chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
                    .map_err(|e| format!("Invalid date '{}': {}", d, e))
            })
            .transpose()?;

        let note = self
            .store
            .daily_note(date, &self.daily_config)
            .await
            .map_err(|e| e.to_string())?;

        Ok(json!({
            "content": [{ "type": "text", "text": format!(
                "# {}\n\nID: {}\nPath: {}\n\n{}",
                note.title, note.id, note.path.display(), note.body
            )}]
        }))
    }
}
