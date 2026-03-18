//! Filesystem-based note storage

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::config::StorageConfig;
use crate::daily::{self, DailyNoteConfig};
use crate::error::{Result, TeselaError};
use crate::note::{Note, NoteId};
use crate::storage::markdown::{generate_frontmatter, parse_frontmatter, sanitize_filename};
use crate::traits::note_store::NoteStore;

/// Filesystem-backed note store
pub struct FsNoteStore {
    root: PathBuf,
    config: StorageConfig,
    daily_config: DailyNoteConfig,
}

impl FsNoteStore {
    pub fn new(root: PathBuf, config: StorageConfig) -> Self {
        Self {
            root,
            config,
            daily_config: DailyNoteConfig::default(),
        }
    }

    pub fn with_daily_config(mut self, daily_config: DailyNoteConfig) -> Self {
        self.daily_config = daily_config;
        self
    }

    /// Open a mosaic root, loading config from root/.tesela/config.toml if present
    pub fn open(root: PathBuf) -> Result<Self> {
        let config_path = root.join(".tesela").join("config.toml");
        let config = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| TeselaError::file_op_with_source("Failed to read config", e))?;
            let full_config: crate::config::Config = toml::from_str(&content)
                .map_err(|e| TeselaError::config(format!("Failed to parse config: {}", e)))?;
            full_config.storage
        } else {
            StorageConfig::default()
        };
        Ok(Self::new(root, config))
    }

    fn notes_dir(&self) -> PathBuf {
        self.root.join(&self.config.notes_dir)
    }

    fn ensure_notes_dir(&self) -> Result<()> {
        let dir = self.notes_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .map_err(|e| TeselaError::file_op_with_source("Failed to create notes dir", e))?;
        }
        Ok(())
    }

    fn calculate_checksum(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    fn parse_note_from_file(&self, path: &Path) -> Result<Note> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| TeselaError::file_op_with_source("Failed to read note file", e))?;

        self.parse_note_from_content(path, &content)
    }

    fn parse_note_from_content(&self, path: &Path, content: &str) -> Result<Note> {
        let (metadata, body) = parse_frontmatter(content)?;

        let title = metadata
            .title
            .clone()
            .unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Untitled")
                    .to_string()
            });

        let id = NoteId::from_filename(
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown"),
        );

        let checksum = Self::calculate_checksum(content.as_bytes());

        let file_metadata = std::fs::metadata(path).ok();
        let created_at = file_metadata
            .as_ref()
            .and_then(|m| m.created().ok())
            .and_then(|t| {
                chrono::DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
            .unwrap_or_else(Utc::now);

        let modified_at = file_metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| {
                chrono::DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
            .unwrap_or_else(Utc::now);

        let rel_path = path
            .strip_prefix(&self.root)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| path.to_path_buf());

        Ok(Note {
            id,
            title,
            content: content.to_string(),
            body,
            metadata,
            path: rel_path,
            checksum,
            created_at,
            modified_at,
            attachments: vec![],
        })
    }

    fn find_note_path(&self, id: &NoteId) -> Option<PathBuf> {
        let notes_dir = self.notes_dir();
        if !notes_dir.exists() {
            return None;
        }
        // Try common extensions
        for ext in &self.config.note_extensions {
            let path = notes_dir.join(format!("{}.{}", id.as_str(), ext));
            if path.exists() {
                return Some(path);
            }
        }
        None
    }

    fn is_note_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            self.config
                .note_extensions
                .contains(&ext.to_lowercase())
        } else {
            false
        }
    }
}

#[async_trait]
impl NoteStore for FsNoteStore {
    async fn get(&self, id: &NoteId) -> Result<Option<Note>> {
        match self.find_note_path(id) {
            Some(path) => Ok(Some(self.parse_note_from_file(&path)?)),
            None => Ok(None),
        }
    }

    async fn get_by_title(&self, title: &str) -> Result<Option<Note>> {
        let notes_dir = self.notes_dir();
        if !notes_dir.exists() {
            return Ok(None);
        }

        for entry in WalkDir::new(&notes_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && self.is_note_file(path) {
                if let Ok(note) = self.parse_note_from_file(path) {
                    if note.title.eq_ignore_ascii_case(title) {
                        return Ok(Some(note));
                    }
                }
            }
        }

        Ok(None)
    }

    async fn create(&self, title: &str, content: &str, tags: &[&str]) -> Result<Note> {
        self.ensure_notes_dir()?;

        let safe_name = sanitize_filename(title);
        let filename = format!("{}.md", safe_name);
        let path = self.notes_dir().join(&filename);

        if path.exists() {
            return Err(TeselaError::Validation {
                message: format!("Note '{}' already exists", title),
            });
        }

        let now = Utc::now();
        let frontmatter = generate_frontmatter(title, tags, now, &Default::default());
        let full_content = if content.is_empty() {
            format!("{}\n# {}\n\n", frontmatter, title)
        } else {
            format!("{}\n{}", frontmatter, content)
        };

        std::fs::write(&path, &full_content)
            .map_err(|e| TeselaError::file_op_with_source("Failed to write note", e))?;

        self.parse_note_from_content(&path, &full_content)
    }

    async fn update(&self, note: &Note) -> Result<()> {
        let path = self.root.join(&note.path);
        std::fs::write(&path, &note.content)
            .map_err(|e| TeselaError::file_op_with_source("Failed to update note", e))?;
        Ok(())
    }

    async fn delete(&self, id: &NoteId) -> Result<()> {
        match self.find_note_path(id) {
            Some(path) => {
                std::fs::remove_file(&path)
                    .map_err(|e| TeselaError::file_op_with_source("Failed to delete note", e))?;
                Ok(())
            }
            None => Err(TeselaError::NoteNotFound {
                identifier: id.to_string(),
            }),
        }
    }

    async fn list(
        &self,
        tag_filter: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Note>> {
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
                match self.parse_note_from_file(path) {
                    Ok(note) => {
                        if let Some(tag) = tag_filter {
                            if note.metadata.tags.iter().any(|t| t == tag) {
                                notes.push(note);
                            }
                        } else {
                            notes.push(note);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load note {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort by modified_at descending
        notes.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));

        Ok(notes.into_iter().skip(offset).take(limit).collect())
    }

    async fn daily_note(
        &self,
        date: Option<NaiveDate>,
        config: &DailyNoteConfig,
    ) -> Result<Note> {
        self.ensure_notes_dir()?;

        let date = date.unwrap_or_else(|| chrono::Local::now().date_naive());
        let filename = daily::daily_note_filename(date, config);
        let path = self.notes_dir().join(&filename);

        if path.exists() {
            return self.parse_note_from_file(&path);
        }

        let content = daily::daily_note_content(date, config);
        std::fs::write(&path, &content)
            .map_err(|e| TeselaError::file_op_with_source("Failed to create daily note", e))?;

        self.parse_note_from_content(&path, &content)
    }

    async fn mosaic_root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store() -> (FsNoteStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = FsNoteStore::new(temp_dir.path().to_path_buf(), StorageConfig::default());
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_create_and_get_note() {
        let (store, _tmp) = create_test_store();

        let note = store.create("Test Note", "", &["test"]).await.unwrap();
        assert_eq!(note.title, "Test Note");
        assert!(note.metadata.tags.contains(&"test".to_string()));

        let fetched = store.get(&note.id).await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.title, "Test Note");
    }

    #[tokio::test]
    async fn test_update_note() {
        let (store, _tmp) = create_test_store();

        let mut note = store.create("Update Me", "", &[]).await.unwrap();
        let new_content = "---\ntitle: \"Update Me\"\n---\n\nUpdated body!";
        note.content = new_content.to_string();

        store.update(&note).await.unwrap();

        let fetched = store.get(&note.id).await.unwrap().unwrap();
        assert!(fetched.content.contains("Updated body!"));
    }

    #[tokio::test]
    async fn test_delete_note() {
        let (store, _tmp) = create_test_store();

        let note = store.create("Delete Me", "", &[]).await.unwrap();
        assert!(store.get(&note.id).await.unwrap().is_some());

        store.delete(&note.id).await.unwrap();
        assert!(store.get(&note.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_notes() {
        let (store, _tmp) = create_test_store();

        store.create("Note A", "", &["alpha"]).await.unwrap();
        store.create("Note B", "", &["beta"]).await.unwrap();
        store.create("Note C", "", &["alpha"]).await.unwrap();

        let all = store.list(None, 100, 0).await.unwrap();
        assert_eq!(all.len(), 3);

        let alpha = store.list(Some("alpha"), 100, 0).await.unwrap();
        assert_eq!(alpha.len(), 2);
    }

    #[tokio::test]
    async fn test_daily_note_creates_and_reuses() {
        let (store, _tmp) = create_test_store();
        let date = NaiveDate::from_ymd_opt(2026, 3, 18).unwrap();
        let config = DailyNoteConfig::default();

        let note1 = store.daily_note(Some(date), &config).await.unwrap();
        assert!(note1.content.contains("2026-03-18"));
        assert!(note1.metadata.tags.contains(&"daily".to_string()));

        let note2 = store.daily_note(Some(date), &config).await.unwrap();
        assert_eq!(note1.id, note2.id);
        assert_eq!(note1.content, note2.content);
    }

    #[tokio::test]
    async fn test_list_with_offset_and_limit() {
        let (store, _tmp) = create_test_store();

        for i in 0..5 {
            store
                .create(&format!("Note {}", i), "", &[])
                .await
                .unwrap();
        }

        let page = store.list(None, 2, 1).await.unwrap();
        assert_eq!(page.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_returns_error() {
        let (store, _tmp) = create_test_store();
        let result = store.delete(&NoteId::new("nonexistent")).await;
        assert!(result.is_err());
    }
}
