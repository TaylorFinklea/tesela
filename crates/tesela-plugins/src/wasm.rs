//! WASM plugin runtime stub.
//!
//! This module is a placeholder that shows exactly what needs to be
//! implemented to add WASM plugin support. To implement:
//!
//! 1. Add wasmtime or extism to Cargo.toml:
//!    extism = "1"
//!
//! 2. Implement WasmPlugin::from_file() to load a .wasm binary
//!    and look up exported functions: on_note_created, on_note_updated, etc.
//!
//! 3. Implement the Plugin trait for WasmPlugin using host function calls.
//!    With extism, this looks like:
//!    `self.plugin.lock().unwrap().call::<Json<NoteInput>, ()>("on_note_created", Json(input))?`
//!
//! 4. Register WasmRuntime in your PluginLoader:
//!    `loader.register_runtime(Box::new(WasmRuntime))`
//!
//! The rest of the system (PluginLoader, PluginRegistry) requires zero changes.

use tesela_core::{
    error::Result,
    note::{Note, NoteId, SearchHit},
    traits::plugin::{Plugin, PluginRuntime, PluginSource},
};

/// A plugin loaded from a WASM binary.
/// Currently a stub -- see module docs for implementation guide.
pub struct WasmPlugin {
    name: String,
    version: String,
}

impl Plugin for WasmPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        &self.version
    }

    fn on_note_created(&self, _note: &Note) -> Result<()> {
        // TODO: serialize note to JSON, call WASM export "on_note_created"
        Ok(())
    }

    fn on_note_updated(&self, _note: &Note) -> Result<()> {
        // TODO: serialize note to JSON, call WASM export "on_note_updated"
        Ok(())
    }

    fn on_note_deleted(&self, _id: &NoteId) -> Result<()> {
        // TODO: serialize id to string, call WASM export "on_note_deleted"
        Ok(())
    }

    fn on_search(&self, _query: &str, _results: &mut Vec<SearchHit>) -> Result<()> {
        // TODO: serialize query + results to JSON, call WASM export "on_search",
        //       deserialize returned JSON back into results
        Ok(())
    }
}

/// Runtime that loads .wasm files.
/// Currently a stub -- see module docs for implementation guide.
pub struct WasmRuntime;

impl PluginRuntime for WasmRuntime {
    fn id(&self) -> &str {
        "wasm"
    }
    fn extensions(&self) -> &[&str] {
        &["wasm"]
    }

    fn load(&self, source: &PluginSource) -> Result<Box<dyn Plugin>> {
        // TODO: load WASM binary, instantiate module, extract name/version from exports
        let name = match source {
            PluginSource::File(p) => p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("wasm-plugin")
                .to_string(),
            PluginSource::Code { name, .. } => name.clone(),
        };
        Ok(Box::new(WasmPlugin {
            name,
            version: "0.0.0-stub".to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_runtime_id() {
        let rt = WasmRuntime;
        assert_eq!(rt.id(), "wasm");
        assert_eq!(rt.extensions(), &["wasm"]);
    }

    #[test]
    fn test_wasm_stub_loads() {
        let rt = WasmRuntime;
        let source = PluginSource::Code {
            source: String::new(),
            name: "stub-plugin".to_string(),
        };
        let plugin = rt.load(&source).unwrap();
        assert_eq!(plugin.name(), "stub-plugin");
    }
}
