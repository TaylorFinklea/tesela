//! Storage layer for Tesela
//!
//! This module handles all file system operations including:
//! - Note storage and retrieval
//! - Attachment management
//! - Markdown parsing with frontmatter
//! - File type detection and validation

use crate::core::error::{Result, TeselaError};
use chrono::{DateTime, Utc};
use gray_matter::engine::YAML;
use gray_matter::Matter;
use pulldown_cmark::{Event, Parser, Tag};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Represents a note with its metadata and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// Unique identifier for the note (usually filename without extension)
    pub id: String,
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
    pub note_ids: Vec<String>,
}

/// Extracted link from markdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// Type of link (internal, external, attachment)
    pub link_type: LinkType,
    /// Target of the link
    pub target: String,
    /// Link text
    pub text: String,
    /// Position in the document
    pub position: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LinkType {
    Internal,
    External,
    Attachment,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Root directory of the mosaic
    pub mosaic_root: PathBuf,
    /// Notes directory name
    pub notes_dir: String,
    /// Attachments directory name
    pub attachments_dir: String,
    /// Allowed file extensions for notes
    pub note_extensions: Vec<String>,
    /// Maximum attachment size in bytes
    pub max_attachment_size: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            mosaic_root: PathBuf::from("."),
            notes_dir: "notes".to_string(),
            attachments_dir: "attachments".to_string(),
            note_extensions: vec!["md".to_string(), "markdown".to_string()],
            max_attachment_size: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// Main storage manager
pub struct Storage {
    config: StorageConfig,
    matter_parser: Matter<YAML>,
}

impl Storage {
    /// Create a new storage instance
    pub fn new(config: StorageConfig) -> Self {
        Self {
            config,
            matter_parser: Matter::<YAML>::new(),
        }
    }

    /// Get the notes directory path
    pub fn notes_dir(&self) -> PathBuf {
        self.config.mosaic_root.join(&self.config.notes_dir)
    }

    /// Get the attachments directory path
    pub fn attachments_dir(&self) -> PathBuf {
        self.config.mosaic_root.join(&self.config.attachments_dir)
    }

    /// Parse a note from file content
    pub fn parse_note(&self, path: &Path, content: &str) -> Result<Note> {
        let file_metadata = fs::metadata(path)
            .map_err(|e| TeselaError::file_op_with_source("Failed to get file metadata", e))?;

        let created_at = file_metadata
            .created()
            .ok()
            .and_then(|t| {
                DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
            .unwrap_or_else(Utc::now);

        let modified_at = file_metadata
            .modified()
            .ok()
            .and_then(|t| {
                DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
            .unwrap_or_else(Utc::now);

        // Parse frontmatter
        let parsed = self.matter_parser.parse(content);
        let metadata = self.extract_metadata(&parsed.data)?;

        // Extract title
        let title = metadata
            .title
            .clone()
            .or_else(|| self.extract_title_from_markdown(&parsed.content))
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            });

        // Calculate checksum
        let checksum = self.calculate_checksum(content.as_bytes());

        // Extract ID from filename
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| TeselaError::parse("filename", "Invalid filename"))?
            .to_string();

        // Parse attachments from content
        let attachments = self.extract_attachments(&parsed.content)?;

        Ok(Note {
            id,
            title,
            content: content.to_string(),
            body: parsed.content.clone(),
            metadata,
            path: self.normalize_path(path)?,
            checksum,
            created_at,
            modified_at,
            attachments,
        })
    }

    /// Save a note to disk
    pub fn save_note(&self, note: &Note) -> Result<()> {
        let full_path = self.config.mosaic_root.join(&note.path);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| TeselaError::file_op_with_source("Failed to create directory", e))?;
        }

        // Write content
        let mut file = fs::File::create(&full_path)
            .map_err(|e| TeselaError::file_op_with_source("Failed to create note file", e))?;

        file.write_all(note.content.as_bytes())
            .map_err(|e| TeselaError::file_op_with_source("Failed to write note content", e))?;

        Ok(())
    }

    /// Load a note from disk
    pub fn load_note(&self, path: &Path) -> Result<Note> {
        let content = fs::read_to_string(path)
            .map_err(|e| TeselaError::file_op_with_source("Failed to read note file", e))?;

        self.parse_note(path, &content)
    }

    /// List all notes in the mosaic
    pub fn list_notes(&self) -> Result<Vec<Note>> {
        let notes_dir = self.notes_dir();
        if !notes_dir.exists() {
            return Ok(Vec::new());
        }

        let mut notes = Vec::new();

        for entry in WalkDir::new(&notes_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && self.is_note_file(path) {
                match self.load_note(path) {
                    Ok(note) => notes.push(note),
                    Err(e) => {
                        tracing::warn!("Failed to load note {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(notes)
    }

    /// Copy an attachment to the mosaic
    pub fn add_attachment(&self, source_path: &Path, note_id: &str) -> Result<Attachment> {
        if !source_path.exists() {
            return Err(TeselaError::file_op("Source file does not exist"));
        }

        let metadata = fs::metadata(source_path)
            .map_err(|e| TeselaError::file_op_with_source("Failed to get file metadata", e))?;

        if metadata.len() > self.config.max_attachment_size {
            return Err(TeselaError::attachment(format!(
                "File size {} exceeds maximum allowed size {}",
                metadata.len(),
                self.config.max_attachment_size
            )));
        }

        let filename = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| TeselaError::file_op("Invalid filename"))?;

        // Generate unique ID
        let id = self.generate_attachment_id(filename);

        // Determine destination path
        let dest_dir = self.attachments_dir().join(note_id);
        fs::create_dir_all(&dest_dir).map_err(|e| {
            TeselaError::file_op_with_source("Failed to create attachments directory", e)
        })?;

        let dest_path = dest_dir.join(filename);

        // Copy file
        fs::copy(source_path, &dest_path)
            .map_err(|e| TeselaError::file_op_with_source("Failed to copy attachment", e))?;

        // Calculate checksum
        let content = fs::read(&dest_path)
            .map_err(|e| TeselaError::file_op_with_source("Failed to read attachment", e))?;
        let checksum = self.calculate_checksum(&content);

        // Detect MIME type
        let mime_type = mime_guess::from_path(&dest_path)
            .first_or_octet_stream()
            .to_string();

        Ok(Attachment {
            id,
            filename: filename.to_string(),
            mime_type,
            size: metadata.len(),
            checksum,
            path: self.normalize_path(&dest_path)?,
            note_ids: vec![note_id.to_string()],
        })
    }

    /// Remove an attachment
    pub fn remove_attachment(&self, attachment: &Attachment) -> Result<()> {
        let full_path = self.config.mosaic_root.join(&attachment.path);
        if full_path.exists() {
            fs::remove_file(&full_path)
                .map_err(|e| TeselaError::file_op_with_source("Failed to remove attachment", e))?;
        }
        Ok(())
    }

    /// Extract links from markdown content
    pub fn extract_links(&self, content: &str) -> Vec<Link> {
        let mut links = Vec::new();
        let parser = Parser::new(content);
        let mut position = 0;

        for event in parser {
            if let Event::Start(Tag::Link(_link_type, dest, title)) = event {
                let link_type = if dest.starts_with("http://") || dest.starts_with("https://") {
                    LinkType::External
                } else if dest.starts_with("attachment:") || dest.starts_with("file:") {
                    LinkType::Attachment
                } else {
                    LinkType::Internal
                };

                links.push(Link {
                    link_type,
                    target: dest.to_string(),
                    text: title.to_string(),
                    position,
                });
            }
            position += 1;
        }

        links
    }

    // Helper methods

    fn extract_metadata(&self, data: &Option<gray_matter::Pod>) -> Result<NoteMetadata> {
        let mut metadata = NoteMetadata::default();

        if let Some(gray_matter::Pod::Hash(map)) = data {
            // Extract title
            if let Some(gray_matter::Pod::String(title)) = map.get("title") {
                metadata.title = Some(title.clone());
            }

            // Extract tags
            if let Some(gray_matter::Pod::Array(tags)) = map.get("tags") {
                metadata.tags = tags
                    .iter()
                    .filter_map(|t| {
                        if let gray_matter::Pod::String(s) = t {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
            }

            // Extract aliases
            if let Some(gray_matter::Pod::Array(aliases)) = map.get("aliases") {
                metadata.aliases = aliases
                    .iter()
                    .filter_map(|a| {
                        if let gray_matter::Pod::String(s) = a {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
            }
        }

        Ok(metadata)
    }

    fn extract_title_from_markdown(&self, content: &str) -> Option<String> {
        let parser = Parser::new(content);

        for event in parser {
            if let Event::Start(Tag::Heading(..)) = event {
                // Next event should be the heading text
                let mut title = String::new();
                let parser = Parser::new(content);
                let mut in_heading = false;

                for event in parser {
                    match event {
                        Event::Start(Tag::Heading(..)) if !in_heading => {
                            in_heading = true;
                        }
                        Event::Text(text) if in_heading => {
                            title.push_str(&text);
                        }
                        Event::End(Tag::Heading(..)) if in_heading => {
                            return Some(title.trim().to_string());
                        }
                        _ => {}
                    }
                }
            }
        }

        None
    }

    fn extract_attachments(&self, _content: &str) -> Result<Vec<Attachment>> {
        // TODO: Implement attachment extraction from markdown
        // This will parse attachment links and references in the content
        Ok(Vec::new())
    }

    fn calculate_checksum(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    fn normalize_path(&self, path: &Path) -> Result<PathBuf> {
        // Make path relative to mosaic root
        if path.is_absolute() {
            path.strip_prefix(&self.config.mosaic_root)
                .map(|p| p.to_path_buf())
                .map_err(|_| TeselaError::file_op("Failed to normalize path"))
        } else {
            Ok(path.to_path_buf())
        }
    }

    fn is_note_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                return self
                    .config
                    .note_extensions
                    .contains(&ext_str.to_lowercase());
            }
        }
        false
    }

    fn generate_attachment_id(&self, filename: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();

        format!("{}_{}", timestamp, filename.replace(' ', "_"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_storage() -> (Storage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig {
            mosaic_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let storage = Storage::new(config);

        // Create directories
        fs::create_dir_all(storage.notes_dir()).unwrap();
        fs::create_dir_all(storage.attachments_dir()).unwrap();

        (storage, temp_dir)
    }

    #[test]
    fn test_parse_note_with_frontmatter() {
        let (storage, temp_dir) = create_test_storage();

        let content = r#"---
title: Test Note
tags: [test, example]
aliases: [test-note, sample]
---

# Test Note

This is a test note with frontmatter."#;

        let note_path = temp_dir.path().join("notes").join("test.md");
        fs::write(&note_path, content).unwrap();

        let note = storage.parse_note(&note_path, content).unwrap();

        assert_eq!(note.title, "Test Note");
        assert_eq!(note.metadata.tags, vec!["test", "example"]);
        assert_eq!(note.metadata.aliases, vec!["test-note", "sample"]);
        assert!(note.body.contains("This is a test note"));
    }

    #[test]
    fn test_parse_note_without_frontmatter() {
        let (storage, temp_dir) = create_test_storage();

        let content = r#"# My Note

This is a simple note without frontmatter."#;

        let note_path = temp_dir.path().join("notes").join("simple.md");
        fs::write(&note_path, content).unwrap();

        let note = storage.parse_note(&note_path, content).unwrap();

        assert_eq!(note.title, "My Note");
        assert!(note.metadata.tags.is_empty());
        assert!(note.body.contains("simple note"));
    }

    #[test]
    fn test_save_and_load_note() {
        let (storage, temp_dir) = create_test_storage();

        let note = Note {
            id: "test-note".to_string(),
            title: "Test Note".to_string(),
            content: "# Test Note\n\nContent".to_string(),
            body: "Content".to_string(),
            metadata: NoteMetadata::default(),
            path: PathBuf::from("notes/test-note.md"),
            checksum: "abc123".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: Vec::new(),
        };

        storage.save_note(&note).unwrap();

        let note_path = temp_dir.path().join("notes").join("test-note.md");
        assert!(note_path.exists());

        let loaded = storage.load_note(&note_path).unwrap();
        assert_eq!(loaded.title, note.title);
        assert_eq!(loaded.content, note.content);
    }

    #[test]
    fn test_add_attachment() {
        let (storage, temp_dir) = create_test_storage();

        // Create a test file to attach
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, b"Test attachment content").unwrap();

        let attachment = storage.add_attachment(&test_file, "note-123").unwrap();

        assert_eq!(attachment.filename, "test.txt");
        assert_eq!(attachment.mime_type, "text/plain");
        assert_eq!(attachment.size, 23);
        assert_eq!(attachment.note_ids, vec!["note-123"]);

        // Check that file was copied
        let dest_path = temp_dir
            .path()
            .join("attachments")
            .join("note-123")
            .join("test.txt");
        assert!(dest_path.exists());
    }

    #[test]
    fn test_extract_links() {
        let (storage, _) = create_test_storage();

        let content = r#"
Here is an [internal link](other-note.md) and an
[external link](https://example.com) and an
[attachment](attachment:image.png).
"#;

        let links = storage.extract_links(content);

        assert_eq!(links.len(), 3);
        assert_eq!(links[0].link_type, LinkType::Internal);
        assert_eq!(links[1].link_type, LinkType::External);
        assert_eq!(links[2].link_type, LinkType::Attachment);
    }

    #[test]
    fn test_checksum_calculation() {
        let (storage, _) = create_test_storage();

        let data = b"Hello, World!";
        let checksum = storage.calculate_checksum(data);

        // SHA256 of "Hello, World!"
        assert_eq!(
            checksum,
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
        );
    }

    #[test]
    fn test_list_notes() {
        let (storage, temp_dir) = create_test_storage();

        // Create some test notes
        let notes_dir = temp_dir.path().join("notes");
        fs::write(notes_dir.join("note1.md"), "# Note 1").unwrap();
        fs::write(notes_dir.join("note2.md"), "# Note 2").unwrap();
        fs::write(notes_dir.join("note3.txt"), "Not a note").unwrap(); // Should be ignored

        let notes = storage.list_notes().unwrap();

        assert_eq!(notes.len(), 2);
        assert!(notes.iter().any(|n| n.title == "Note 1"));
        assert!(notes.iter().any(|n| n.title == "Note 2"));
    }

    #[test]
    fn test_attachment_size_limit() {
        let (mut storage, temp_dir) = create_test_storage();
        storage.config.max_attachment_size = 10; // Set very small limit

        let test_file = temp_dir.path().join("large.txt");
        fs::write(&test_file, b"This is more than 10 bytes").unwrap();

        let result = storage.add_attachment(&test_file, "note-123");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TeselaError::Attachment { .. }
        ));
    }
}
