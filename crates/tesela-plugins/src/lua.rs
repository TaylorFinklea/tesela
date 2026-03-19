//! Lua plugin runtime via mlua.
//!
//! Lua plugins are .lua files that define:
//!   name = "my-plugin"      -- required
//!   version = "1.0.0"       -- required
//!   description = "..."     -- optional
//!
//!   function on_note_created(note) end   -- optional
//!   function on_note_updated(note) end   -- optional
//!   function on_note_deleted(id) end     -- optional
//!   function on_search(query, results) return results end  -- optional

use mlua::prelude::*;
use std::path::Path;
use std::sync::Mutex;
use tesela_core::{
    error::{Result, TeselaError},
    note::{Note, NoteId, SearchHit},
    traits::plugin::{Plugin, PluginRuntime, PluginSource},
};

/// A plugin loaded from a Lua script.
pub struct LuaPlugin {
    lua: Mutex<Lua>,
    name: String,
    version: String,
    description: String,
}

impl LuaPlugin {
    /// Load from a file path
    pub fn from_file(path: &Path) -> Result<Self> {
        let code = std::fs::read_to_string(path).map_err(|e| TeselaError::FileOperation {
            message: format!("Cannot read plugin: {}", path.display()),
            source: Some(e),
        })?;
        let plugin_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        Self::from_code(&code, &plugin_name)
    }

    /// Load from a code string (for testing)
    pub fn from_code(code: &str, default_name: &str) -> Result<Self> {
        let lua = Lua::new();
        lua.load(code)
            .exec()
            .map_err(|e| TeselaError::Other(format!("Lua error: {}", e)))?;

        let globals = lua.globals();
        let name: String =
            globals.get::<String>("name").unwrap_or_else(|_| default_name.to_string());
        let version: String =
            globals.get::<String>("version").unwrap_or_else(|_| "0.1.0".to_string());
        let description: String = globals.get::<String>("description").unwrap_or_default();

        Ok(Self {
            lua: Mutex::new(lua),
            name,
            version,
            description,
        })
    }

    /// Convert a Note to a Lua table
    fn note_to_table(&self, lua: &Lua, note: &Note) -> LuaResult<LuaTable> {
        let t = lua.create_table()?;
        t.set("id", note.id.as_str())?;
        t.set("title", note.title.as_str())?;
        t.set("body", note.body.as_str())?;
        t.set("path", note.path.to_str().unwrap_or(""))?;
        let tags = lua.create_table()?;
        for (i, tag) in note.metadata.tags.iter().enumerate() {
            tags.set(i + 1, tag.as_str())?;
        }
        t.set("tags", tags)?;
        Ok(t)
    }

    /// Call a named Lua function with one argument, if it exists
    fn call_hook_note(&self, func_name: &str, note: &Note) -> Result<()> {
        let lua = self.lua.lock().unwrap();
        let globals = lua.globals();
        if let Ok(func) = globals.get::<LuaFunction>(func_name) {
            let table = self
                .note_to_table(&lua, note)
                .map_err(|e| TeselaError::Other(e.to_string()))?;
            func.call::<()>(table)
                .map_err(|e| TeselaError::Other(format!("{}: {}", func_name, e)))?;
        }
        Ok(())
    }
}

impl Plugin for LuaPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn description(&self) -> &str {
        &self.description
    }

    fn on_note_created(&self, note: &Note) -> Result<()> {
        self.call_hook_note("on_note_created", note)
    }

    fn on_note_updated(&self, note: &Note) -> Result<()> {
        self.call_hook_note("on_note_updated", note)
    }

    fn on_note_deleted(&self, id: &NoteId) -> Result<()> {
        let lua = self.lua.lock().unwrap();
        let globals = lua.globals();
        if let Ok(func) = globals.get::<LuaFunction>("on_note_deleted") {
            func.call::<()>(id.as_str())
                .map_err(|e| TeselaError::Other(format!("on_note_deleted: {}", e)))?;
        }
        Ok(())
    }

    fn on_search(&self, query: &str, results: &mut Vec<SearchHit>) -> Result<()> {
        let lua = self.lua.lock().unwrap();
        let globals = lua.globals();
        if let Ok(func) = globals.get::<LuaFunction>("on_search") {
            // Convert results to Lua table
            let results_table = lua
                .create_table()
                .map_err(|e| TeselaError::Other(e.to_string()))?;
            for (i, hit) in results.iter().enumerate() {
                let t = lua
                    .create_table()
                    .map_err(|e| TeselaError::Other(e.to_string()))?;
                t.set("id", hit.note_id.as_str()).ok();
                t.set("title", hit.title.as_str()).ok();
                t.set("snippet", hit.snippet.as_str()).ok();
                t.set("rank", hit.rank).ok();
                results_table.set(i + 1, t).ok();
            }

            // Call on_search(query, results) -> returns modified results table or nil
            let ret: LuaValue = func
                .call((query, results_table))
                .map_err(|e| TeselaError::Other(format!("on_search: {}", e)))?;

            // If the plugin returned a table, use it to filter/reorder
            if let LuaValue::Table(new_results) = ret {
                results.clear();
                for pair in new_results.sequence_values::<LuaTable>() {
                    let Ok(t) = pair else { continue };
                    let id: String = t.get("id").unwrap_or_default();
                    let title: String = t.get("title").unwrap_or_default();
                    let snippet: String = t.get("snippet").unwrap_or_default();
                    let rank: f64 = t.get("rank").unwrap_or(0.0);
                    results.push(SearchHit {
                        note_id: NoteId::new(id),
                        title,
                        snippet,
                        rank,
                        tags: vec![],
                        path: std::path::PathBuf::new(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Runtime that loads .lua files
pub struct LuaRuntime;

impl PluginRuntime for LuaRuntime {
    fn id(&self) -> &str {
        "lua"
    }
    fn extensions(&self) -> &[&str] {
        &["lua"]
    }

    fn load(&self, source: &PluginSource) -> Result<Box<dyn Plugin>> {
        let plugin = match source {
            PluginSource::File(path) => LuaPlugin::from_file(path)?,
            PluginSource::Code { source: code, name } => LuaPlugin::from_code(code, name)?,
        };
        Ok(Box::new(plugin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tesela_core::{
        note::{Note, NoteMetadata},
        traits::plugin::{PluginLoader, PluginSource},
    };

    fn test_note(id: &str, title: &str) -> Note {
        Note {
            id: NoteId::new(id),
            title: title.to_string(),
            content: format!("# {}", title),
            body: format!("# {}", title),
            metadata: NoteMetadata {
                title: Some(title.to_string()),
                tags: vec![],
                aliases: vec![],
                custom: Default::default(),
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
            },
            path: std::path::PathBuf::from(format!("{}.md", id)),
            checksum: "abc".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: vec![],
        }
    }

    #[test]
    fn test_lua_plugin_metadata() {
        let code = r#"
name = "my-plugin"
version = "2.0.0"
description = "A test plugin"
"#;
        let plugin = LuaPlugin::from_code(code, "fallback").unwrap();
        assert_eq!(plugin.name(), "my-plugin");
        assert_eq!(plugin.version(), "2.0.0");
        assert_eq!(plugin.description(), "A test plugin");
    }

    #[test]
    fn test_lua_on_note_created_called() {
        let code = r#"
name = "counter"
version = "1.0.0"
_count = 0
function on_note_created(note)
    _count = _count + 1
end
"#;
        let plugin = LuaPlugin::from_code(code, "counter").unwrap();
        let note = test_note("n1", "Test");
        plugin.on_note_created(&note).unwrap();
        plugin.on_note_created(&note).unwrap();

        // Verify count via Lua eval
        let lua = plugin.lua.lock().unwrap();
        let count: i32 = lua.globals().get("_count").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_lua_on_note_deleted_called() {
        let code = r#"
name = "delete-watcher"
version = "1.0.0"
last_deleted = nil
function on_note_deleted(id)
    last_deleted = id
end
"#;
        let plugin = LuaPlugin::from_code(code, "test").unwrap();
        plugin.on_note_deleted(&NoteId::new("note-abc")).unwrap();

        let lua = plugin.lua.lock().unwrap();
        let deleted: String = lua.globals().get("last_deleted").unwrap();
        assert_eq!(deleted, "note-abc");
    }

    #[test]
    fn test_lua_on_search_filters_results() {
        let code = r#"
name = "filter-plugin"
version = "1.0.0"
function on_search(query, results)
    local filtered = {}
    for _, r in ipairs(results) do
        if r.rank > 0.5 then
            table.insert(filtered, r)
        end
    end
    return filtered
end
"#;
        let plugin = LuaPlugin::from_code(code, "test").unwrap();
        let mut results = vec![
            SearchHit {
                note_id: NoteId::new("a"),
                title: "High".to_string(),
                snippet: "".to_string(),
                rank: 0.9,
                tags: vec![],
                path: Default::default(),
            },
            SearchHit {
                note_id: NoteId::new("b"),
                title: "Low".to_string(),
                snippet: "".to_string(),
                rank: 0.1,
                tags: vec![],
                path: Default::default(),
            },
        ];
        plugin.on_search("test", &mut results).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_id.as_str(), "a");
    }

    #[test]
    fn test_lua_plugin_no_hooks_is_fine() {
        let code = r#"name = "minimal" version = "1.0.0""#;
        let plugin = LuaPlugin::from_code(code, "minimal").unwrap();
        let note = test_note("n1", "Test");
        plugin.on_note_created(&note).unwrap(); // no hook defined = no-op, no error
    }

    #[test]
    fn test_plugin_loader_dispatches_lua() {
        let mut loader = PluginLoader::new();
        loader.register_runtime(Box::new(LuaRuntime));

        let code = r#"name = "loaded-plugin" version = "1.0.0""#;
        let source = PluginSource::Code {
            source: code.to_string(),
            name: "loaded-plugin".to_string(),
        };

        // Code source has no extension, so runtime_hint returns None -> can_handle returns false
        // For file-based loading, use File source. Test that LuaRuntime.load works directly:
        let plugin = LuaRuntime.load(&source).unwrap();
        assert_eq!(plugin.name(), "loaded-plugin");
    }

    #[test]
    fn test_plugin_loader_file_extension_routing() {
        let tmp = tempfile::NamedTempFile::with_suffix(".lua").unwrap();
        std::fs::write(tmp.path(), r#"name = "file-plugin" version = "1.0.0""#).unwrap();

        let mut loader = PluginLoader::new();
        loader.register_runtime(Box::new(LuaRuntime));

        let source = PluginSource::File(tmp.path().to_path_buf());
        let plugin = loader.load(&source).unwrap();
        assert_eq!(plugin.name(), "file-plugin");
    }
}
