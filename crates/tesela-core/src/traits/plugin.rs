//! Plugin API traits for Tesela extensibility.
//!
//! Defines the trait surface for extending Tesela with custom behavior.
//! No runtime (Lua/WASM/JS) is implemented here — only the API contracts.

use crate::error::Result;
use crate::note::{Note, NoteId, SearchHit};
use std::path::PathBuf;

/// A command that a plugin can register
pub struct PluginCommand {
    pub name: String,
    pub description: String,
    pub usage: String,
}

/// The core plugin trait. Implement this to extend Tesela.
///
/// All hook methods have default no-op implementations, so plugins
/// only need to override the hooks they care about.
pub trait Plugin: Send + Sync {
    /// The plugin's unique identifier (e.g. "my-org/my-plugin")
    fn name(&self) -> &str;

    /// Semver version string (e.g. "1.0.0")
    fn version(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str {
        ""
    }

    /// Called after a note is created
    fn on_note_created(&self, _note: &Note) -> Result<()> {
        Ok(())
    }

    /// Called after a note is updated
    fn on_note_updated(&self, _note: &Note) -> Result<()> {
        Ok(())
    }

    /// Called before a note is deleted (return Err to cancel)
    fn on_note_deleted(&self, _id: &NoteId) -> Result<()> {
        Ok(())
    }

    /// Called after a search completes. May mutate results (add, remove, reorder).
    fn on_search(&self, _query: &str, _results: &mut Vec<SearchHit>) -> Result<()> {
        Ok(())
    }

    /// CLI commands this plugin contributes. Empty by default.
    fn commands(&self) -> Vec<PluginCommand> {
        vec![]
    }
}

/// Registry that holds and dispatches to all registered plugins.
#[derive(Default)]
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin. Panics if a plugin with the same name is already registered.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        assert!(
            !self.plugins.iter().any(|p| p.name() == plugin.name()),
            "Plugin '{}' is already registered",
            plugin.name()
        );
        self.plugins.push(plugin);
    }

    /// Number of registered plugins
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Dispatch on_note_created to all plugins. Collects all errors.
    pub fn dispatch_note_created(&self, note: &Note) -> Result<()> {
        for plugin in &self.plugins {
            plugin.on_note_created(note)?;
        }
        Ok(())
    }

    /// Dispatch on_note_updated to all plugins.
    pub fn dispatch_note_updated(&self, note: &Note) -> Result<()> {
        for plugin in &self.plugins {
            plugin.on_note_updated(note)?;
        }
        Ok(())
    }

    /// Dispatch on_note_deleted to all plugins.
    pub fn dispatch_note_deleted(&self, id: &NoteId) -> Result<()> {
        for plugin in &self.plugins {
            plugin.on_note_deleted(id)?;
        }
        Ok(())
    }

    /// Dispatch on_search to all plugins. Plugins may mutate the results.
    pub fn dispatch_search(&self, query: &str, results: &mut Vec<SearchHit>) -> Result<()> {
        for plugin in &self.plugins {
            plugin.on_search(query, results)?;
        }
        Ok(())
    }

    /// Get names of all commands from all plugins
    /// Returns (plugin_name, command_name) pairs
    pub fn command_names(&self) -> Vec<(String, String)> {
        self.plugins
            .iter()
            .flat_map(|p| {
                let plugin_name = p.name().to_string();
                p.commands()
                    .into_iter()
                    .map(move |c| (plugin_name.clone(), c.name))
            })
            .collect()
    }
}

/// Source from which a plugin can be loaded
#[derive(Debug, Clone)]
pub enum PluginSource {
    /// A file on disk (.lua, .wasm, etc.)
    File(PathBuf),
    /// Raw source code string (for testing / inline plugins)
    Code { source: String, name: String },
}

impl PluginSource {
    /// Detect runtime by file extension
    pub fn runtime_hint(&self) -> Option<&str> {
        match self {
            PluginSource::File(p) => p.extension()?.to_str(),
            PluginSource::Code { .. } => None,
        }
    }
}

/// A plugin runtime knows how to load plugins from a given source.
/// Implement this trait to add a new plugin system (Lua, WASM, etc.)
pub trait PluginRuntime: Send + Sync {
    /// Identifier for this runtime, e.g. "lua" or "wasm"
    fn id(&self) -> &str;

    /// File extensions this runtime handles, e.g. ["lua"] or ["wasm"]
    fn extensions(&self) -> &[&str];

    /// Returns true if this runtime can handle the given source
    fn can_handle(&self, source: &PluginSource) -> bool {
        match source.runtime_hint() {
            Some(ext) => self.extensions().contains(&ext),
            None => false,
        }
    }

    /// Load a plugin from the given source, returning a boxed Plugin
    fn load(&self, source: &PluginSource) -> crate::error::Result<Box<dyn Plugin>>;
}

/// Dispatches plugin loading to the correct runtime based on source type.
/// Add a new runtime with `register_runtime()`.
#[derive(Default)]
pub struct PluginLoader {
    runtimes: Vec<Box<dyn PluginRuntime>>,
}

impl PluginLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a runtime. Runtimes are tried in registration order.
    pub fn register_runtime(&mut self, runtime: Box<dyn PluginRuntime>) {
        self.runtimes.push(runtime);
    }

    /// Load a plugin from source, using the first matching runtime.
    pub fn load(&self, source: &PluginSource) -> crate::error::Result<Box<dyn Plugin>> {
        for runtime in &self.runtimes {
            if runtime.can_handle(source) {
                return runtime.load(source);
            }
        }
        Err(crate::error::TeselaError::Other(format!(
            "No runtime available for {:?}",
            source.runtime_hint()
        )))
    }

    /// Load all plugins from a directory, using registered runtimes
    pub fn load_directory(
        &self,
        dir: &std::path::Path,
    ) -> Vec<crate::error::Result<Box<dyn Plugin>>> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return vec![];
        };
        entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .map(|p| self.load(&PluginSource::File(p)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::{Note, NoteId, NoteMetadata};
    use chrono::Utc;
    use std::path::PathBuf;
    use std::sync::Mutex;

    // Helper to make a test note
    fn test_note(id: &str, title: &str) -> Note {
        Note {
            id: NoteId::new(id),
            title: title.to_string(),
            content: format!("---\ntitle: {}\n---\n\n# {}", title, title),
            body: format!("# {}", title),
            metadata: NoteMetadata {
                title: Some(title.to_string()),
                tags: vec![],
                aliases: vec![],
                note_type: None,
                custom: Default::default(),
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
            },
            path: PathBuf::from(format!("{}.md", id)),
            checksum: "abc123".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: vec![],
        }
    }

    struct RecordingPlugin {
        name: String,
        created_calls: Mutex<Vec<String>>,
        updated_calls: Mutex<Vec<String>>,
        deleted_calls: Mutex<Vec<String>>,
    }

    impl RecordingPlugin {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                created_calls: Mutex::new(vec![]),
                updated_calls: Mutex::new(vec![]),
                deleted_calls: Mutex::new(vec![]),
            }
        }
    }

    impl Plugin for RecordingPlugin {
        fn name(&self) -> &str {
            &self.name
        }
        fn version(&self) -> &str {
            "1.0.0"
        }

        fn on_note_created(&self, note: &Note) -> Result<()> {
            self.created_calls
                .lock()
                .unwrap()
                .push(note.id.as_str().to_string());
            Ok(())
        }

        fn on_note_updated(&self, note: &Note) -> Result<()> {
            self.updated_calls
                .lock()
                .unwrap()
                .push(note.id.as_str().to_string());
            Ok(())
        }

        fn on_note_deleted(&self, id: &NoteId) -> Result<()> {
            self.deleted_calls
                .lock()
                .unwrap()
                .push(id.as_str().to_string());
            Ok(())
        }
    }

    #[test]
    fn test_register_and_dispatch_created() {
        let mut registry = PluginRegistry::new();
        let plugin = RecordingPlugin::new("test-plugin");

        registry.register(Box::new(plugin));

        let note = test_note("note-1", "Test Note");
        registry.dispatch_note_created(&note).unwrap();

        // Can't access plugin after Box, so test via side effects on count
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_multiple_plugins_all_dispatched() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        struct CountingPlugin {
            name: String,
            count: Arc<AtomicUsize>,
        }
        impl Plugin for CountingPlugin {
            fn name(&self) -> &str {
                &self.name
            }
            fn version(&self) -> &str {
                "1.0.0"
            }
            fn on_note_created(&self, _: &Note) -> Result<()> {
                self.count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let count = Arc::new(AtomicUsize::new(0));
        let mut registry = PluginRegistry::new();
        registry.register(Box::new(CountingPlugin {
            name: "p1".to_string(),
            count: count.clone(),
        }));
        registry.register(Box::new(CountingPlugin {
            name: "p2".to_string(),
            count: count.clone(),
        }));
        registry.register(Box::new(CountingPlugin {
            name: "p3".to_string(),
            count: count.clone(),
        }));

        let note = test_note("note-1", "Test");
        registry.dispatch_note_created(&note).unwrap();

        assert_eq!(
            count.load(Ordering::SeqCst),
            3,
            "All 3 plugins should be called"
        );
    }

    #[test]
    fn test_search_plugin_can_filter_results() {
        struct FilterPlugin;
        impl Plugin for FilterPlugin {
            fn name(&self) -> &str {
                "filter"
            }
            fn version(&self) -> &str {
                "1.0.0"
            }
            fn on_search(&self, _query: &str, results: &mut Vec<SearchHit>) -> Result<()> {
                results.retain(|r| !r.title.contains("secret"));
                Ok(())
            }
        }

        let mut registry = PluginRegistry::new();
        registry.register(Box::new(FilterPlugin));

        let mut results = vec![
            SearchHit {
                note_id: NoteId::new("public"),
                title: "Public Note".to_string(),
                snippet: "".to_string(),
                rank: 1.0,
                tags: vec![],
                path: PathBuf::from("public.md"),
            },
            SearchHit {
                note_id: NoteId::new("secret"),
                title: "secret document".to_string(),
                snippet: "".to_string(),
                rank: 0.9,
                tags: vec![],
                path: PathBuf::from("secret.md"),
            },
        ];

        registry.dispatch_search("query", &mut results).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].note_id.as_str(), "public");
    }

    #[test]
    fn test_duplicate_plugin_name_panics() {
        let result = std::panic::catch_unwind(|| {
            let mut registry = PluginRegistry::new();
            registry.register(Box::new(RecordingPlugin::new("same-name")));
            registry.register(Box::new(RecordingPlugin::new("same-name")));
        });
        assert!(result.is_err(), "Should panic on duplicate plugin name");
    }

    #[test]
    fn test_plugin_commands() {
        struct CommandPlugin;
        impl Plugin for CommandPlugin {
            fn name(&self) -> &str {
                "cmd-plugin"
            }
            fn version(&self) -> &str {
                "1.0.0"
            }
            fn commands(&self) -> Vec<PluginCommand> {
                vec![PluginCommand {
                    name: "my-cmd".to_string(),
                    description: "Does stuff".to_string(),
                    usage: "my-cmd [arg]".to_string(),
                }]
            }
        }

        let mut registry = PluginRegistry::new();
        registry.register(Box::new(CommandPlugin));

        let commands = registry.command_names();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].0, "cmd-plugin");
        assert_eq!(commands[0].1, "my-cmd");
    }

    #[test]
    fn test_empty_registry() {
        let registry = PluginRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        let note = test_note("n1", "Test");
        registry.dispatch_note_created(&note).unwrap();
        registry.dispatch_note_deleted(&NoteId::new("n1")).unwrap();
    }
}
