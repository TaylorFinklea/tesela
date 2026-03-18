//! Note types for Tesela

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique identifier for a note
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NoteId(String);

impl NoteId {
    pub fn new(s: impl Into<String>) -> Self {
        NoteId(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_filename(filename: &str) -> Self {
        let stem = std::path::Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename);
        NoteId(stem.to_string())
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for NoteId {
    fn from(s: String) -> Self {
        NoteId(s)
    }
}

impl From<&str> for NoteId {
    fn from(s: &str) -> Self {
        NoteId(s.to_string())
    }
}

/// Represents a note with its metadata and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Unique identifier for the note (usually filename without extension)
    pub id: NoteId,
    /// Note title
    pub title: String,
    /// Full content including frontmatter
    pub content: String,
    /// Parsed body content (without frontmatter)
    pub body: String,
    /// Frontmatter metadata
    pub metadata: NoteMetadata,
    /// File path relative to mosaic root
    pub path: PathBuf,
    /// SHA256 checksum of the content
    pub checksum: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last modification timestamp
    pub modified_at: DateTime<Utc>,
    /// List of attachments
    pub attachments: Vec<Attachment>,
}

/// Note metadata extracted from frontmatter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoteMetadata {
    /// Note title from frontmatter (overrides extracted title)
    pub title: Option<String>,
    /// Tags associated with the note
    pub tags: Vec<String>,
    /// Note aliases for easier searching
    pub aliases: Vec<String>,
    /// Custom key-value pairs
    pub custom: HashMap<String, serde_json::Value>,
    /// Creation date from frontmatter
    pub created: Option<DateTime<Utc>>,
    /// Last modified date from frontmatter
    pub modified: Option<DateTime<Utc>>,
}

/// Represents an attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    /// Unique identifier
    pub id: String,
    /// Original filename
    pub filename: String,
    /// MIME type
    pub mime_type: String,
    /// File size in bytes
    pub size: u64,
    /// SHA256 checksum
    pub checksum: String,
    /// Path relative to attachments directory
    pub path: PathBuf,
    /// Associated note IDs
    pub note_ids: Vec<NoteId>,
}

/// A search result hit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub note_id: NoteId,
    pub title: String,
    pub snippet: String,
    pub rank: f64,
    pub tags: Vec<String>,
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_id_new() {
        let id = NoteId::new("my-note");
        assert_eq!(id.as_str(), "my-note");
    }

    #[test]
    fn test_note_id_display() {
        let id = NoteId::new("hello-world");
        assert_eq!(format!("{}", id), "hello-world");
    }

    #[test]
    fn test_note_id_from_string() {
        let id: NoteId = "test-note".into();
        assert_eq!(id.as_str(), "test-note");

        let id: NoteId = String::from("owned-note").into();
        assert_eq!(id.as_str(), "owned-note");
    }

    #[test]
    fn test_note_id_from_filename() {
        let id = NoteId::from_filename("my-note.md");
        assert_eq!(id.as_str(), "my-note");

        let id = NoteId::from_filename("document.markdown");
        assert_eq!(id.as_str(), "document");

        let id = NoteId::from_filename("no-extension");
        assert_eq!(id.as_str(), "no-extension");
    }

    #[test]
    fn test_note_id_equality() {
        let a = NoteId::new("test");
        let b = NoteId::new("test");
        let c = NoteId::new("other");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_note_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(NoteId::new("a"));
        set.insert(NoteId::new("b"));
        set.insert(NoteId::new("a")); // duplicate
        assert_eq!(set.len(), 2);
    }
}
