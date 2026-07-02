//! MCP tool definitions and handlers
//!
//! Tool vocabulary (tesela-cmdd.3): the hand-written tools below
//! (`search_notes`/`get_note`/`create_note`/`list_notes`/`get_backlinks`/
//! `get_daily_note`) are genuinely MCP-only — they have no manifest
//! counterpart and keep their existing names/handlers so nothing that
//! already depends on them (the integration tests, any live MCP client)
//! breaks. Every OTHER tool `tools/list` advertises is generated straight
//! from the checked-in command manifest (`web/src/lib/command-manifest.json`,
//! tesela-cmdd.2's ONE extraction point — same `include_str!` pattern as
//! `crates/tesela-server/src/routes/commands.rs`), so the id vocabulary is
//! shared and adding a manifest command auto-exposes it here without a
//! second hand-copied list. See `MANIFEST_OPT_OUT_CATEGORIES` for the
//! explicit opt-out.

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tesela_core::{
    daily::DailyNoteConfig,
    db::SqliteIndex,
    note::NoteId,
    storage::filesystem::FsNoteStore,
    traits::plugin::PluginRegistry,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
};

use crate::mosaic_engine::create_note_via_engine;

const COMMAND_MANIFEST_JSON: &str =
    include_str!("../../../web/src/lib/command-manifest.json");

/// Just the fields `tools/list` generation needs — a subset of the Rust
/// `CommandManifestEntry` (`crates/tesela-server/src/routes/commands.rs`)
/// and TS `CommandManifestEntry` (`web/src/lib/command-registry.svelte.ts`).
/// Not reusing either type directly: this crate doesn't depend on
/// `tesela-server`, and duplicating the shape here (not the JSON) keeps the
/// ONE checked-in manifest as the single source of truth.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestCommandEntry {
    pub id: String,
    pub label: String,
    pub category: String,
    pub takes_arg: bool,
    pub arg_prompt: Option<String>,
}

static COMMAND_MANIFEST: LazyLock<Vec<ManifestCommandEntry>> = LazyLock::new(|| {
    serde_json::from_str(COMMAND_MANIFEST_JSON)
        .expect("web/src/lib/command-manifest.json must parse as Vec<ManifestCommandEntry>")
});

/// Manifest categories with NO server-side representation to auto-expose:
/// `pane`/`tab` commands (vsplit, move-left, tabnew, …) manipulate a live
/// browser window's layout, which `tesela-mcp` — a headless process
/// operating directly on the mosaic store, not a live UI session — has no
/// concept of at all. Every other category concerns note/mosaic content
/// addressable well enough (an id/slug arg, or a deterministic no-arg
/// lookup) to be worth listing even before every one has an MCP handler
/// (see `call`'s fallback for the "listed, not yet wired" case).
static MANIFEST_OPT_OUT_CATEGORIES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| HashSet::from(["pane", "tab"]));

/// `true` when `entry` should auto-expose as an MCP tool — every manifest
/// command whose args-shape is expressible (today: 0 or 1 string arg, which
/// is all of them) except the explicit category opt-out above.
pub fn is_mcp_expressible(entry: &ManifestCommandEntry) -> bool {
    !MANIFEST_OPT_OUT_CATEGORIES.contains(entry.category.as_str())
}

/// The MCP `inputSchema` for a manifest command's args-shape: no properties
/// for a no-arg command, or a single required string `arg` (described by
/// `arg_prompt`) for a one-arg command.
fn manifest_tool_schema(entry: &ManifestCommandEntry) -> Value {
    if entry.takes_arg {
        json!({
            "type": "object",
            "properties": {
                "arg": {
                    "type": "string",
                    "description": entry.arg_prompt.clone().unwrap_or_default()
                }
            },
            "required": ["arg"]
        })
    } else {
        json!({ "type": "object", "properties": {} })
    }
}

/// Generates the `tools/list` entries for every manifest command that
/// auto-exposes (`is_mcp_expressible`). Takes the manifest as a parameter
/// (rather than reading the static `COMMAND_MANIFEST` directly) so the
/// auto-expose/opt-out behavior is unit-testable against synthetic entries
/// without touching the checked-in JSON.
pub fn generate_manifest_tools(manifest: &[ManifestCommandEntry]) -> Vec<Value> {
    manifest
        .iter()
        .filter(|entry| is_mcp_expressible(entry))
        .map(|entry| {
            json!({
                "name": entry.id,
                "description": entry.label,
                "inputSchema": manifest_tool_schema(entry)
            })
        })
        .collect()
}

pub struct ToolRegistry {
    pub store: Arc<FsNoteStore>,
    pub index: Arc<SqliteIndex>,
    pub daily_config: DailyNoteConfig,
    pub registry: Arc<PluginRegistry>,
    /// Mosaic root — needed to lock it and open the Loro engine directly for
    /// writes (tesela-ows.3): `create_note` must go through the engine like
    /// the CLI's `cmd_new`, not a raw `FsNoteStore` write that never syncs.
    pub mosaic: PathBuf,
}

/// Returns the MCP tools/list response.
/// Ids of the genuinely-MCP-only tools below — no manifest command covers
/// full-text search, id/title-fuzzy note lookup, or the other note-store
/// operations these wrap, so they stay hand-written. Exposed so tests (and
/// `generate_manifest_tools` callers) can assert no manifest id collides
/// with one of these.
pub const HAND_WRITTEN_TOOL_NAMES: &[&str] = &[
    "search_notes",
    "get_note",
    "create_note",
    "list_notes",
    "get_backlinks",
    "get_daily_note",
];

fn hand_written_tools() -> Vec<Value> {
    vec![
        json!({
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
        }),
        json!({
            "name": "get_note",
            "description": "Get a note by ID or title",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Note ID" },
                    "title": { "type": "string", "description": "Note title (fuzzy match)" }
                }
            }
        }),
        json!({
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
        }),
        json!({
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
        }),
        json!({
            "name": "get_backlinks",
            "description": "Get all notes that link to a given note",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Note ID to find backlinks for" }
                },
                "required": ["id"]
            }
        }),
        json!({
            "name": "get_daily_note",
            "description": "Get or create the daily note for a given date",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "date": { "type": "string", "description": "Date in YYYY-MM-DD format (defaults to today)" }
                }
            }
        }),
    ]
}

/// The MCP `tools/list` response: the hand-written, genuinely-MCP-only
/// tools followed by every manifest command that auto-exposes (tesela-cmdd.3).
pub fn list_tools() -> Value {
    let mut tools = hand_written_tools();
    tools.extend(generate_manifest_tools(&COMMAND_MANIFEST));
    json!({ "tools": tools })
}

impl ToolRegistry {
    pub fn new(
        store: Arc<FsNoteStore>,
        index: Arc<SqliteIndex>,
        registry: Arc<PluginRegistry>,
        mosaic: PathBuf,
    ) -> Self {
        Self {
            store,
            index,
            daily_config: DailyNoteConfig::default(),
            registry,
            mosaic,
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
            _ if COMMAND_MANIFEST.iter().any(|c| c.id == name) => Err(format!(
                "tool '{}' is listed via the command manifest but has no MCP execution handler yet",
                name
            )),
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
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&results).expect("serializing a Vec<serde_json::Value> is infallible (no IO, all Values serialize)") }]
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

        // Engine-only-writes (tesela-ows.3, mirrors tesela-ows.2's CLI fix):
        // a raw `FsNoteStore` write never syncs and gets reverted by the
        // engine's next materialize — worse for MCP than the CLI since an
        // agent invokes this invisibly. Lock the mosaic and hydrate through
        // the Loro engine instead; fails loudly if tesela-server/the desktop
        // already holds the lock rather than bypassing it.
        let slug = create_note_via_engine(&self.mosaic, title, &tags, content)
            .await
            .map_err(|e| e.to_string())?;

        let note = self
            .store
            .get(&NoteId::new(&slug))
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("note '{}' not found after create", title))?;

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
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&results).expect("serializing a Vec<serde_json::Value> is infallible (no IO, all Values serialize)") }]
        }))
    }

    async fn get_backlinks(&self, params: Value) -> Result<Value, String> {
        let id = params["id"].as_str().ok_or("Missing required field: id")?;

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
            "content": [{ "type": "text", "text": serde_json::to_string_pretty(&results).expect("serializing a Vec<serde_json::Value> is infallible (no IO, all Values serialize)") }]
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

#[cfg(test)]
mod manifest_tool_tests {
    use super::*;
    use std::collections::HashSet;

    fn entry(id: &str, category: &str, takes_arg: bool, arg_prompt: Option<&str>) -> ManifestCommandEntry {
        ManifestCommandEntry {
            id: id.to_string(),
            label: format!("{id} label"),
            category: category.to_string(),
            takes_arg,
            arg_prompt: arg_prompt.map(str::to_string),
        }
    }

    #[test]
    fn opted_out_category_is_not_expressible() {
        assert!(!is_mcp_expressible(&entry("vsplit", "pane", false, None)));
        assert!(!is_mcp_expressible(&entry("tabnew", "tab", false, None)));
    }

    #[test]
    fn other_categories_are_expressible() {
        for category in ["navigate", "editor", "create", "derived", "ambient", "tile"] {
            assert!(
                is_mcp_expressible(&entry("x", category, false, None)),
                "{category} should auto-expose"
            );
        }
    }

    #[test]
    fn adding_a_manifest_command_auto_exposes_it() {
        // Acceptance: adding a manifest command with an expressible args-shape
        // auto-exposes, no second hand-copied list required.
        let manifest = vec![entry("new-command", "navigate", false, None)];
        let tools = generate_manifest_tools(&manifest);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "new-command");
        assert_eq!(tools[0]["description"], "new-command label");
    }

    #[test]
    fn a_manifest_command_can_explicitly_opt_out_via_category() {
        let manifest = vec![entry("close-pane", "pane", false, None)];
        assert!(generate_manifest_tools(&manifest).is_empty());
    }

    #[test]
    fn no_arg_command_gets_an_empty_object_schema() {
        let manifest = vec![entry("daily", "navigate", false, None)];
        let tools = generate_manifest_tools(&manifest);
        assert_eq!(tools[0]["inputSchema"]["type"], "object");
        assert_eq!(tools[0]["inputSchema"]["properties"], json!({}));
        assert!(tools[0]["inputSchema"]["required"].is_null());
    }

    #[test]
    fn takes_arg_command_gets_a_required_string_arg_schema() {
        let manifest = vec![entry("jump", "tile", true, Some("note slug or id"))];
        let tools = generate_manifest_tools(&manifest);
        assert_eq!(tools[0]["inputSchema"]["required"], json!(["arg"]));
        assert_eq!(
            tools[0]["inputSchema"]["properties"]["arg"]["description"],
            "note slug or id"
        );
    }

    #[test]
    fn list_tools_ids_trace_to_the_real_manifest() {
        // The real checked-in manifest has "jump" (tile, takes an arg) and
        // "daily" (navigate, no arg); both should be listed, "vsplit"
        // (pane) should not.
        let tools = list_tools();
        let names: HashSet<&str> = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains("jump"), "manifest command 'jump' should be listed");
        assert!(names.contains("daily"), "manifest command 'daily' should be listed");
        assert!(!names.contains("vsplit"), "pane command 'vsplit' should opt out");
        assert!(!names.contains("tabnew"), "tab command 'tabnew' should opt out");
    }

    #[test]
    fn list_tools_has_no_id_collisions_between_hand_written_and_manifest() {
        let tools = list_tools();
        let names: Vec<&str> = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        let unique: HashSet<&str> = names.iter().copied().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "duplicate tool name in tools/list: {:?}",
            names
        );
        for name in HAND_WRITTEN_TOOL_NAMES {
            assert!(names.contains(name), "hand-written tool {name} missing from list_tools()");
        }
    }
}
