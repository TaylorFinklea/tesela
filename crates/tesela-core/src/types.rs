//! Type system for Tesela — defines page types and their property schemas.
//!
//! Types are loaded from `.tesela/types.toml` in the mosaic directory.
//! A page declares its type via `type: "task"` in frontmatter.
//! Blocks can declare type via `#task` tag or `type:: task` property.
//! Type names are lowercase by default so tagging stays lowercase
//! (`#ritual`, `[[domain]]`); the UI may title-case them for display.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[cfg(test)]
use ts_rs::TS;

/// A type definition (e.g., Task, Project, Person)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct TypeDefinition {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_color")]
    pub color: String,
    /// Plural display name (e.g. `Tasks` for the `Task` type). Falls back to
    /// `name` when the Tag page declares no `plural:` frontmatter. Used
    /// wherever a type is labelled in the plural (e.g. the tag-page header).
    #[serde(default)]
    pub plural: String,
    #[serde(default)]
    pub properties: Vec<PropertyDef>,
}

fn default_icon() -> String {
    "📄".to_string()
}
fn default_color() -> String {
    "#808080".to_string()
}

/// Per-type visibility for a property (Anytype/Logseq-DB 3-state model).
///
/// - `OnNew` — auto-seeded onto a new block of the type (and its default
///   applied); always shown. Legacy equivalent: `hide_by_default = false`.
/// - `OnSet` — settable but not auto-seeded; shown only when it has a
///   value (per-type `hide_empty` semantics).
/// - `Hidden` — never seeded; available in `/p` but hidden when empty.
///   Legacy equivalent: `hide_by_default = true`.
///
/// Serializes as the same `on_new` / `on_set` / `hidden` strings used in
/// `property_overrides.{Prop}.show` FLOW YAML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    OnNew,
    OnSet,
    Hidden,
}

/// A property definition within a type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct PropertyDef {
    pub name: String,
    #[serde(default = "default_value_type")]
    pub value_type: String,
    #[serde(default)]
    pub values: Option<Vec<String>>,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub required: bool,
    /// Per-type visibility, resolved from the type's `property_overrides`
    /// (`show`) or derived from `hide_by_default` when no override exists.
    /// `None` only when this PropertyDef was produced outside the per-type
    /// resolver (e.g. `get_all_property_defs`).
    #[serde(default)]
    pub show: Option<Visibility>,
    /// If true, the property is hidden from the block by default. The user
    /// must expand the block's "show properties" affordance (chevron) to see
    /// it. Inspired by Logseq DB's per-tag-property "Hide by default" toggle.
    #[serde(default)]
    pub hide_by_default: bool,
    /// If true, the property only renders when its value is non-empty. Empty
    /// property lines are suppressed. Defaults to true (most users want this).
    #[serde(default = "default_true")]
    pub hide_empty: bool,
}

fn default_value_type() -> String {
    "text".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for PropertyDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            value_type: default_value_type(),
            values: None,
            default: None,
            required: false,
            show: None,
            hide_by_default: false,
            hide_empty: true,
        }
    }
}

/// Container for all type definitions loaded from types.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypesFile {
    #[serde(default)]
    pub types: Vec<TypeDefinition>,
}

/// Registry of all available types in the mosaic
#[derive(Debug, Clone)]
pub struct TypeRegistry {
    pub types: Vec<TypeDefinition>,
}

impl TypeRegistry {
    /// Load types from `.tesela/types.toml` in the mosaic directory.
    /// Returns an empty registry if the file doesn't exist.
    pub fn load(mosaic_path: &Path) -> Self {
        let types_path = mosaic_path.join(".tesela").join("types.toml");
        if types_path.exists() {
            match std::fs::read_to_string(&types_path) {
                Ok(content) => match toml::from_str::<TypesFile>(&content) {
                    Ok(file) => return TypeRegistry { types: file.types },
                    Err(e) => {
                        tracing::warn!("Failed to parse types.toml: {}", e);
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read types.toml: {}", e);
                }
            }
        }
        // No types.toml: the built-in `.md` pages (Task/Project/Person/
        // Priority/Status/…) are seeded at server boot and indexed into the
        // DB, which is the runtime source of truth for types (surfaced via
        // get_all_tag_defs). Return an empty registry rather than a second
        // hardcoded copy that silently drifts from those pages.
        TypeRegistry { types: Vec::new() }
    }

    /// Get a type definition by name (case-insensitive)
    pub fn get(&self, name: &str) -> Option<&TypeDefinition> {
        self.types
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn load_without_types_toml_is_empty() {
        // No types.toml → empty registry (the built-in pages indexed into the
        // DB are the source of truth; the fallback must not carry a second,
        // drifting copy). See get_all_tag_defs in the server's /types route.
        let dir = TempDir::new().unwrap();
        let registry = TypeRegistry::load(dir.path());
        assert!(registry.types.is_empty());
        assert!(registry.get("Task").is_none());
    }

    #[test]
    fn load_reads_types_toml_when_present() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".tesela")).unwrap();
        fs::write(
            dir.path().join(".tesela").join("types.toml"),
            "[[types]]\nname = \"task\"\ndescription = \"\"\nicon = \"\"\ncolor = \"\"\n",
        )
        .unwrap();
        let registry = TypeRegistry::load(dir.path());
        assert_eq!(registry.types.len(), 1);
        assert!(registry.get("Task").is_some());
        assert!(registry.get("nonexistent").is_none());
    }
}
