//! Error handling for Tesela
//!
//! This module provides a comprehensive error handling system using `thiserror`
//! for automatic error type derivation and `anyhow` for error context chaining.

use std::path::PathBuf;
use thiserror::Error;

/// Main error type for Tesela operations
#[derive(Error, Debug)]
pub enum TeselaError {
    /// Errors related to file operations
    #[error("File operation failed: {message}")]
    FileOperation {
        message: String,
        #[source]
        source: Option<std::io::Error>,
    },

    /// Note not found
    #[error("Note not found: {identifier}")]
    NoteNotFound { identifier: String },

    /// Multiple notes matched when expecting a single note
    #[error("Multiple notes matched '{query}': {matches:?}")]
    MultipleNotesMatched { query: String, matches: Vec<String> },

    /// Mosaic not initialized
    #[error("No mosaic found. Run 'tesela init' first to create one.")]
    MosaicNotInitialized,

    /// Invalid mosaic structure
    #[error("Invalid mosaic structure at {path}: {reason}")]
    InvalidMosaic { path: PathBuf, reason: String },

    /// Database errors
    #[error("Database error: {message}")]
    Database {
        message: String,
        #[source]
        source: Option<sqlx::Error>,
    },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Parsing errors (Markdown, YAML, etc.)
    #[error("Failed to parse {format}: {message}")]
    ParseError { format: String, message: String },

    /// Attachment errors
    #[error("Attachment error: {message}")]
    Attachment { message: String },

    /// Index errors
    #[error("Index error: {message}")]
    Index { message: String },

    /// Search errors
    #[error("Search error: {message}")]
    Search { message: String },

    /// Validation errors
    #[error("Validation failed: {message}")]
    Validation { message: String },

    /// Template errors
    #[error("Template error: {message}")]
    Template { message: String },

    /// Permission errors
    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    /// Network errors (for future sync features)
    #[error("Network error: {message}")]
    Network { message: String },

    /// Generic I/O errors
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// UTF-8 conversion errors
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),

    /// JSON serialization/deserialization errors
    #[error(transparent)]
    Json(#[from] serde_json::Error),

    /// TOML serialization/deserialization errors
    #[error(transparent)]
    Toml(#[from] toml::de::Error),

    /// Other errors with context
    #[error("{0}")]
    Other(String),
}

/// Result type alias for Tesela operations
pub type Result<T> = std::result::Result<T, TeselaError>;

impl TeselaError {
    /// Create a file operation error with context
    pub fn file_op(message: impl Into<String>) -> Self {
        TeselaError::FileOperation {
            message: message.into(),
            source: None,
        }
    }

    /// Create a file operation error with an IO error source
    pub fn file_op_with_source(message: impl Into<String>, source: std::io::Error) -> Self {
        TeselaError::FileOperation {
            message: message.into(),
            source: Some(source),
        }
    }

    /// Create a database error with context
    pub fn database(message: impl Into<String>) -> Self {
        TeselaError::Database {
            message: message.into(),
            source: None,
        }
    }

    /// Create a database error with a sqlx error source
    pub fn database_with_source(message: impl Into<String>, source: sqlx::Error) -> Self {
        TeselaError::Database {
            message: message.into(),
            source: Some(source),
        }
    }

    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        TeselaError::Configuration {
            message: message.into(),
        }
    }

    /// Create a parse error
    pub fn parse(format: impl Into<String>, message: impl Into<String>) -> Self {
        TeselaError::ParseError {
            format: format.into(),
            message: message.into(),
        }
    }

    /// Create an attachment error
    pub fn attachment(message: impl Into<String>) -> Self {
        TeselaError::Attachment {
            message: message.into(),
        }
    }

    /// Create an index error
    pub fn index(message: impl Into<String>) -> Self {
        TeselaError::Index {
            message: message.into(),
        }
    }

    /// Create a search error
    pub fn search(message: impl Into<String>) -> Self {
        TeselaError::Search {
            message: message.into(),
        }
    }

    /// Create a validation error
    pub fn validation(message: impl Into<String>) -> Self {
        TeselaError::Validation {
            message: message.into(),
        }
    }

    /// Check if this is a "not found" type error
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            TeselaError::NoteNotFound { .. } | TeselaError::MosaicNotInitialized
        )
    }
}

/// Extension trait for adding context to Results
pub trait ResultExt<T> {
    /// Add context to an error
    fn context(self, msg: impl Into<String>) -> Result<T>;

    /// Add context with a closure (lazy evaluation)
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T> for std::result::Result<T, E>
where
    E: Into<TeselaError>,
{
    fn context(self, msg: impl Into<String>) -> Result<T> {
        self.map_err(|e| {
            let base_error = e.into();
            TeselaError::Other(format!("{}: {}", msg.into(), base_error))
        })
    }

    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let base_error = e.into();
            TeselaError::Other(format!("{}: {}", f(), base_error))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = TeselaError::NoteNotFound {
            identifier: "test-note".to_string(),
        };
        assert_eq!(err.to_string(), "Note not found: test-note");
    }

    #[test]
    fn test_error_is_not_found() {
        let err1 = TeselaError::NoteNotFound {
            identifier: "test".to_string(),
        };
        assert!(err1.is_not_found());

        let err2 = TeselaError::MosaicNotInitialized;
        assert!(err2.is_not_found());

        let err3 = TeselaError::Other("test".to_string());
        assert!(!err3.is_not_found());
    }

    #[test]
    fn test_error_context() {
        let result: std::result::Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));

        let with_context = result.context("Failed to read configuration");
        assert!(with_context.is_err());
        let err_msg = with_context.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to read configuration"));
    }

    #[test]
    fn test_file_op_helpers() {
        let err1 = TeselaError::file_op("Cannot write file");
        assert!(matches!(err1, TeselaError::FileOperation { .. }));

        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err2 = TeselaError::file_op_with_source("Cannot write file", io_err);
        assert!(matches!(
            err2,
            TeselaError::FileOperation {
                source: Some(_),
                ..
            }
        ));
    }
}
