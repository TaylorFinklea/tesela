//! Note types for Tesela

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[cfg(test)]
use ts_rs::TS;

/// Unique identifier for a note
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
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

/// Immutable identity used by typed page relations.
///
/// It is deliberately separate from slug-keyed [`NoteId`] and from the
/// legacy Loro stream address. New identities are deterministically backfilled
/// from the current address, then persisted in note authority and never
/// recomputed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(
    test,
    ts(type = "string", export, export_to = "../../../web/src/lib/types/")
)]
#[serde(transparent)]
pub struct PageId(Uuid);

impl PageId {
    /// Fixed Tesela namespace for deterministic legacy backfill.
    pub const NAMESPACE: Uuid = Uuid::from_bytes([
        0x74, 0x65, 0x73, 0x65, 0x6c, 0x61, 0x5f, 0x70, 0x61, 0x67, 0x65, 0x5f, 0x69, 0x64, 0x5f,
        0x31,
    ]);

    pub fn from_legacy_doc_id(doc_id: &[u8; 16]) -> Self {
        Self(Uuid::new_v5(&Self::NAMESPACE, doc_id))
    }

    pub fn parse(value: &str) -> Option<Self> {
        Uuid::parse_str(value).ok().map(Self)
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for PageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for PageId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// System-wide stable sync document id for a note slug.
///
/// Two clients deriving an id for the same slug must address the same Loro
/// document, so every writer uses this BLAKE3 truncation rather than carrying
/// a private copy of the algorithm.
pub fn stable_uuid_from_slug(slug: &str) -> [u8; 16] {
    let hash = blake3::hash(slug.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&hash.as_bytes()[..16]);
    out
}

/// Represents a note with its metadata and content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
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
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct NoteMetadata {
    /// Note title from frontmatter (overrides extracted title)
    pub title: Option<String>,
    /// Tags associated with the note
    pub tags: Vec<String>,
    /// Note aliases for easier searching
    pub aliases: Vec<String>,
    /// Page type (e.g., "Task", "Project", "Person")
    pub note_type: Option<String>,
    /// Custom key-value pairs
    #[cfg_attr(test, ts(type = "Record<string, unknown>"))]
    pub custom: HashMap<String, serde_json::Value>,
    /// Creation date from frontmatter
    pub created: Option<DateTime<Utc>>,
    /// Last modified date from frontmatter
    pub modified: Option<DateTime<Utc>>,
}

/// Represents an attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
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

/// A historical version of a note. Created on every successful PUT (Phase 9.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct NoteVersion {
    /// Auto-incrementing primary key.
    pub id: i64,
    pub note_id: NoteId,
    /// Per-note monotonically increasing version number (1-based).
    pub version_number: i64,
    /// Snapshot of `notes.content` after this PUT.
    pub content: String,
    /// What `notes.content` looked like before this PUT (for `+N/-M` diff).
    pub prev_content: Option<String>,
    /// ISO timestamp of when the row was written.
    pub created_at: String,
}

/// A search result hit
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
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

    #[test]
    fn stable_note_id_matches_blake3_128_bit_vector() {
        assert_eq!(
            stable_uuid_from_slug("abc"),
            [
                0x64, 0x37, 0xb3, 0xac, 0x38, 0x46, 0x51, 0x33, 0xff, 0xb6, 0x3b, 0x75, 0x27, 0x3a,
                0x8d, 0xb5,
            ]
        );
    }

    #[test]
    fn page_id_backfill_is_uuid_v5_and_deterministic() {
        let doc = stable_uuid_from_slug("abc");
        let first = PageId::from_legacy_doc_id(&doc);
        assert_eq!(first, PageId::from_legacy_doc_id(&doc));
        assert_eq!(first.as_uuid().get_version(), Some(uuid::Version::Sha1));
        assert_eq!(PageId::parse(&first.to_string()), Some(first));
    }
}
