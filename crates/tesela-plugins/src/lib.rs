//! Tesela plugin runtimes.
//!
//! This crate provides concrete plugin runtime implementations:
//! - [`lua::LuaRuntime`]: Load plugins written in Lua 5.4
//! - [`wasm::WasmRuntime`]: Load plugins compiled to WebAssembly (stub, see wasm.rs docs)
//!
//! # Quick start
//!
//! ```rust,ignore
//! use tesela_core::traits::plugin::{PluginLoader, PluginSource, PluginRegistry};
//! use tesela_plugins::lua::LuaRuntime;
//! use tesela_plugins::wasm::WasmRuntime;
//! use std::path::PathBuf;
//!
//! let mut loader = PluginLoader::new();
//! loader.register_runtime(Box::new(LuaRuntime));   // handles .lua files
//! loader.register_runtime(Box::new(WasmRuntime));  // handles .wasm files (stub)
//!
//! // Load all plugins from ~/.config/tesela/plugins/
//! let plugin_dir = dirs::config_dir().unwrap().join("tesela/plugins");
//! let mut registry = PluginRegistry::new();
//! for result in loader.load_directory(&plugin_dir) {
//!     match result {
//!         Ok(plugin) => registry.register(plugin),
//!         Err(e) => eprintln!("Failed to load plugin: {}", e),
//!     }
//! }
//! ```

pub mod lua;
pub mod wasm;
