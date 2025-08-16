//! Configuration management for Tesela
//!
//! This module handles loading, parsing, and managing configuration from TOML files
//! and environment variables.

use crate::core::error::{Result, TeselaError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// General application settings
    pub general: GeneralConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Editor configuration
    pub editor: EditorConfig,
    /// Search configuration
    pub search: SearchConfig,
    /// UI configuration
    pub ui: UiConfig,
}

/// General application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Default mosaic path
    pub default_mosaic: Option<PathBuf>,
    /// Auto-save interval in seconds (0 to disable)
    pub auto_save_interval: u64,
    /// Enable debug logging
    pub debug: bool,
    /// Log level (trace, debug, info, warn, error)
    pub log_level: String,
    /// Date format for note creation
    pub date_format: String,
    /// Time format for note creation
    pub time_format: String,
    /// Default note template
    pub default_template: Option<String>,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Notes directory name
    pub notes_dir: String,
    /// Attachments directory name
    pub attachments_dir: String,
    /// Templates directory name
    pub templates_dir: String,
    /// Archive directory name
    pub archive_dir: String,
    /// Allowed note file extensions
    pub note_extensions: Vec<String>,
    /// Maximum attachment size in bytes
    pub max_attachment_size: u64,
    /// Enable automatic backups
    pub enable_backups: bool,
    /// Backup directory
    pub backup_dir: String,
    /// Number of backups to keep
    pub backup_count: usize,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database file name
    pub db_file: String,
    /// Maximum number of connections
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connect_timeout: u64,
    /// Enable write-ahead logging
    pub enable_wal: bool,
    /// Enable foreign key constraints
    pub enable_foreign_keys: bool,
    /// Cache size in pages
    pub cache_size: i32,
    /// Enable auto-vacuum
    pub auto_vacuum: bool,
}

/// Editor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// External editor command
    pub external_editor: Option<String>,
    /// Tab size
    pub tab_size: usize,
    /// Use spaces instead of tabs
    pub use_spaces: bool,
    /// Word wrap
    pub word_wrap: bool,
    /// Show line numbers
    pub line_numbers: bool,
    /// Highlight current line
    pub highlight_current_line: bool,
    /// Auto-close brackets
    pub auto_close_brackets: bool,
    /// Spell check
    pub spell_check: bool,
}

/// Search configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Maximum search results
    pub max_results: usize,
    /// Search result context lines
    pub context_lines: usize,
    /// Enable fuzzy search
    pub fuzzy_search: bool,
    /// Fuzzy search threshold (0.0 to 1.0)
    pub fuzzy_threshold: f32,
    /// Index update interval in seconds
    pub index_update_interval: u64,
    /// Enable incremental indexing
    pub incremental_indexing: bool,
    /// Excluded directories from search
    pub excluded_dirs: Vec<String>,
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Default theme (light, dark, auto)
    pub theme: String,
    /// Custom theme file path
    pub custom_theme: Option<PathBuf>,
    /// Font family
    pub font_family: String,
    /// Font size
    pub font_size: u16,
    /// Enable animations
    pub animations: bool,
    /// Show status bar
    pub show_status_bar: bool,
    /// Show sidebar
    pub show_sidebar: bool,
    /// Sidebar width in pixels
    pub sidebar_width: u32,
    /// Recent files count
    pub recent_files_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            storage: StorageConfig::default(),
            database: DatabaseConfig::default(),
            editor: EditorConfig::default(),
            search: SearchConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            default_mosaic: None,
            auto_save_interval: 60,
            debug: false,
            log_level: "info".to_string(),
            date_format: "%Y-%m-%d".to_string(),
            time_format: "%H:%M:%S".to_string(),
            default_template: None,
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            notes_dir: "notes".to_string(),
            attachments_dir: "attachments".to_string(),
            templates_dir: "templates".to_string(),
            archive_dir: "archive".to_string(),
            note_extensions: vec!["md".to_string(), "markdown".to_string()],
            max_attachment_size: 100 * 1024 * 1024, // 100MB
            enable_backups: true,
            backup_dir: "backups".to_string(),
            backup_count: 10,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            db_file: "tesela.db".to_string(),
            max_connections: 5,
            connect_timeout: 30,
            enable_wal: true,
            enable_foreign_keys: true,
            cache_size: 2000,
            auto_vacuum: true,
        }
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            external_editor: None,
            tab_size: 4,
            use_spaces: true,
            word_wrap: true,
            line_numbers: true,
            highlight_current_line: true,
            auto_close_brackets: true,
            spell_check: false,
        }
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_results: 100,
            context_lines: 2,
            fuzzy_search: true,
            fuzzy_threshold: 0.6,
            index_update_interval: 60,
            incremental_indexing: true,
            excluded_dirs: vec![".git".to_string(), "node_modules".to_string()],
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "auto".to_string(),
            custom_theme: None,
            font_family: "system".to_string(),
            font_size: 14,
            animations: true,
            show_status_bar: true,
            show_sidebar: true,
            sidebar_width: 250,
            recent_files_count: 10,
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| TeselaError::file_op_with_source("Failed to read config file", e))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| TeselaError::config(format!("Failed to parse config: {}", e)))?;

        config.validate()?;

        Ok(config)
    }

    /// Load configuration from a file or create default
    pub fn load_or_default(path: &Path) -> Self {
        match Self::load(path) {
            Ok(config) => config,
            Err(e) => {
                tracing::warn!("Failed to load config from {:?}: {}", path, e);
                Self::default()
            }
        }
    }

    /// Save configuration to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| TeselaError::config(format!("Failed to serialize config: {}", e)))?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                TeselaError::file_op_with_source("Failed to create config directory", e)
            })?;
        }

        fs::write(path, content)
            .map_err(|e| TeselaError::file_op_with_source("Failed to write config file", e))?;

        Ok(())
    }

    /// Merge with environment variables
    pub fn merge_env(&mut self) {
        // Override with environment variables
        if let Ok(val) = std::env::var("TESELA_DEBUG") {
            self.general.debug = val.parse().unwrap_or(false);
        }

        if let Ok(val) = std::env::var("TESELA_LOG_LEVEL") {
            self.general.log_level = val;
        }

        if let Ok(val) = std::env::var("TESELA_DEFAULT_MOSAIC") {
            self.general.default_mosaic = Some(PathBuf::from(val));
        }

        if let Ok(val) = std::env::var("TESELA_EDITOR") {
            self.editor.external_editor = Some(val);
        }

        if let Ok(val) = std::env::var("TESELA_THEME") {
            self.ui.theme = val;
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate log level
        match self.general.log_level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => {
                return Err(TeselaError::validation(format!(
                    "Invalid log level: {}",
                    self.general.log_level
                )))
            }
        }

        // Validate theme
        match self.ui.theme.as_str() {
            "light" | "dark" | "auto" => {}
            _ => {
                return Err(TeselaError::validation(format!(
                    "Invalid theme: {}",
                    self.ui.theme
                )))
            }
        }

        // Validate fuzzy threshold
        if self.search.fuzzy_threshold < 0.0 || self.search.fuzzy_threshold > 1.0 {
            return Err(TeselaError::validation(format!(
                "Fuzzy threshold must be between 0.0 and 1.0, got {}",
                self.search.fuzzy_threshold
            )));
        }

        // Validate max connections
        if self.database.max_connections == 0 {
            return Err(TeselaError::validation(
                "Database max_connections must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the default config file path
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .map(|p| p.join("tesela").join("config.toml"))
            .unwrap_or_else(|| PathBuf::from("tesela.toml"))
    }

    /// Get the config directory
    pub fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("tesela"))
    }
}

/// Builder pattern for configuration
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    pub fn with_mosaic(mut self, path: PathBuf) -> Self {
        self.config.general.default_mosaic = Some(path);
        self
    }

    pub fn with_theme(mut self, theme: String) -> Self {
        self.config.ui.theme = theme;
        self
    }

    pub fn with_editor(mut self, editor: String) -> Self {
        self.config.editor.external_editor = Some(editor);
        self
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.config.general.debug = debug;
        self
    }

    pub fn build(self) -> Config {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.log_level, "info");
        assert_eq!(config.storage.notes_dir, "notes");
        assert_eq!(config.ui.theme, "auto");
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        config.general.log_level = "invalid".to_string();
        assert!(config.validate().is_err());

        config.general.log_level = "debug".to_string();
        config.search.fuzzy_threshold = 1.5;
        assert!(config.validate().is_err());

        config.search.fuzzy_threshold = 0.7;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config = Config::default();
        config.save(&config_path).unwrap();

        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.general.log_level, config.general.log_level);
        assert_eq!(loaded.storage.notes_dir, config.storage.notes_dir);
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .with_mosaic(PathBuf::from("/test/mosaic"))
            .with_theme("dark".to_string())
            .with_editor("vim".to_string())
            .with_debug(true)
            .build();

        assert_eq!(
            config.general.default_mosaic,
            Some(PathBuf::from("/test/mosaic"))
        );
        assert_eq!(config.ui.theme, "dark");
        assert_eq!(config.editor.external_editor, Some("vim".to_string()));
        assert!(config.general.debug);
    }

    #[test]
    fn test_merge_env() {
        std::env::set_var("TESELA_DEBUG", "true");
        std::env::set_var("TESELA_LOG_LEVEL", "debug");
        std::env::set_var("TESELA_THEME", "dark");

        let mut config = Config::default();
        config.merge_env();

        assert!(config.general.debug);
        assert_eq!(config.general.log_level, "debug");
        assert_eq!(config.ui.theme, "dark");

        // Clean up
        std::env::remove_var("TESELA_DEBUG");
        std::env::remove_var("TESELA_LOG_LEVEL");
        std::env::remove_var("TESELA_THEME");
    }
}
