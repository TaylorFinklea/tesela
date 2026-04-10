//! Type system for Tesela — defines page types and their property schemas.
//!
//! Types are loaded from `.tesela/types.toml` in the mosaic directory.
//! A page declares its type via `type: "Task"` in frontmatter.
//! Blocks can declare type via `#Task` tag or `type:: Task` property.

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
    #[serde(default)]
    pub properties: Vec<PropertyDef>,
}

fn default_icon() -> String {
    "📄".to_string()
}
fn default_color() -> String {
    "#808080".to_string()
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
}

fn default_value_type() -> String {
    "text".to_string()
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
        // Return default built-in types if no file
        TypeRegistry {
            types: default_types(),
        }
    }

    /// Get a type definition by name (case-insensitive)
    pub fn get(&self, name: &str) -> Option<&TypeDefinition> {
        self.types
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
    }
}

/// Built-in default types
fn default_types() -> Vec<TypeDefinition> {
    vec![
        TypeDefinition {
            name: "Task".to_string(),
            description: "A task to be completed".to_string(),
            icon: "☐".to_string(),
            color: "#FF6B6B".to_string(),
            properties: vec![
                PropertyDef {
                    name: "status".to_string(),
                    value_type: "select".to_string(),
                    values: Some(vec![
                        "backlog".to_string(),
                        "todo".to_string(),
                        "doing".to_string(),
                        "in-review".to_string(),
                        "done".to_string(),
                        "canceled".to_string(),
                    ]),
                    default: Some("todo".to_string()),
                    required: false,
                },
                PropertyDef {
                    name: "priority".to_string(),
                    value_type: "select".to_string(),
                    values: Some(vec![
                        "critical".to_string(),
                        "high".to_string(),
                        "medium".to_string(),
                        "low".to_string(),
                    ]),
                    default: Some("medium".to_string()),
                    required: false,
                },
                PropertyDef {
                    name: "deadline".to_string(),
                    value_type: "date".to_string(),
                    values: None,
                    default: None,
                    required: false,
                },
                PropertyDef {
                    name: "scheduled".to_string(),
                    value_type: "date".to_string(),
                    values: None,
                    default: None,
                    required: false,
                },
                PropertyDef {
                    name: "effort".to_string(),
                    value_type: "text".to_string(),
                    values: None,
                    default: None,
                    required: false,
                },
            ],
        },
        TypeDefinition {
            name: "Project".to_string(),
            description: "A project with multiple tasks".to_string(),
            icon: "📋".to_string(),
            color: "#4ECDC4".to_string(),
            properties: vec![
                PropertyDef {
                    name: "status".to_string(),
                    value_type: "select".to_string(),
                    values: Some(vec![
                        "planning".to_string(),
                        "active".to_string(),
                        "paused".to_string(),
                        "completed".to_string(),
                        "archived".to_string(),
                    ]),
                    default: Some("planning".to_string()),
                    required: false,
                },
                PropertyDef {
                    name: "deadline".to_string(),
                    value_type: "date".to_string(),
                    values: None,
                    default: None,
                    required: false,
                },
            ],
        },
        TypeDefinition {
            name: "Person".to_string(),
            description: "A contact or team member".to_string(),
            icon: "👤".to_string(),
            color: "#45B7D1".to_string(),
            properties: vec![
                PropertyDef {
                    name: "email".to_string(),
                    value_type: "text".to_string(),
                    values: None,
                    default: None,
                    required: false,
                },
                PropertyDef {
                    name: "team".to_string(),
                    value_type: "text".to_string(),
                    values: None,
                    default: None,
                    required: false,
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_types() {
        let registry = TypeRegistry {
            types: default_types(),
        };
        assert_eq!(registry.types.len(), 3);
        assert!(registry.get("Task").is_some());
        assert!(registry.get("Project").is_some());
        assert!(registry.get("Person").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_task_type_properties() {
        let registry = TypeRegistry {
            types: default_types(),
        };
        let task = registry.get("Task").unwrap();
        assert_eq!(task.properties.len(), 5);
        assert_eq!(task.properties[0].name, "status");
        assert_eq!(task.properties[0].value_type, "select");
    }
}
