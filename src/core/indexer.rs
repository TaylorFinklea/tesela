//! Indexer module for Tesela
//!
//! This module provides file watching and incremental indexing capabilities.
//! It monitors the mosaic for changes and updates the search index accordingly.

use crate::core::database::Database;
use crate::core::error::{Result, TeselaError};
use crate::core::storage::Storage;
use chrono::{DateTime, Utc};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

/// Events emitted by the indexer
#[derive(Debug, Clone)]
pub enum IndexEvent {
    /// A note was indexed
    NoteIndexed { path: PathBuf, note_id: String },
    /// A note was removed from the index
    NoteRemoved { path: PathBuf, note_id: String },
    /// Index rebuild started
    RebuildStarted,
    /// Index rebuild completed
    RebuildCompleted { notes_processed: usize },
    /// An error occurred during indexing
    IndexError { path: PathBuf, error: String },
}

/// Configuration for the indexer
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    /// Debounce interval for file changes (milliseconds)
    pub debounce_interval: u64,
    /// Maximum file size to index (bytes)
    pub max_file_size: u64,
    /// Batch size for processing multiple files
    pub batch_size: usize,
    /// Whether to index hidden files
    pub index_hidden: bool,
    /// Excluded patterns (glob-style)
    pub exclude_patterns: Vec<String>,
    /// Whether to extract content from attachments
    pub extract_attachment_content: bool,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            debounce_interval: 500,
            max_file_size: 10 * 1024 * 1024, // 10MB
            batch_size: 50,
            index_hidden: false,
            exclude_patterns: vec![
                "*.tmp".to_string(),
                "*.bak".to_string(),
                ".git/**".to_string(),
                "**/.DS_Store".to_string(),
            ],
            extract_attachment_content: false,
        }
    }
}

/// Main indexer that manages file watching and incremental indexing
pub struct Indexer {
    storage: Arc<Storage>,
    database: Arc<Database>,
    config: IndexerConfig,
    event_sender: broadcast::Sender<IndexEvent>,
    _watcher: Option<RecommendedWatcher>,
    file_checksums: Arc<Mutex<HashMap<PathBuf, String>>>,
}

impl Indexer {
    /// Create a new indexer instance
    pub async fn new(
        storage: Arc<Storage>,
        database: Arc<Database>,
        config: IndexerConfig,
    ) -> Result<Self> {
        let (event_sender, _) = broadcast::channel(1000);

        Ok(Self {
            storage,
            database,
            config,
            event_sender,
            _watcher: None,
            file_checksums: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Start the file watcher
    pub fn start_watching(&mut self) -> Result<broadcast::Receiver<IndexEvent>> {
        let notes_dir = self.storage.notes_dir();

        if !notes_dir.exists() {
            return Err(TeselaError::index("Notes directory does not exist"));
        }

        let (tx, rx) = mpsc::channel();
        let event_sender = self.event_sender.clone();
        let storage = Arc::clone(&self.storage);
        let database = Arc::clone(&self.database);
        let config = self.config.clone();
        let file_checksums = Arc::clone(&self.file_checksums);

        // Create watcher
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Err(e) = tx.send(res) {
                    error!("Failed to send file system event: {}", e);
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(config.debounce_interval)),
        )
        .map_err(|e| TeselaError::index(format!("Failed to create file watcher: {}", e)))?;

        // Watch the notes directory
        watcher
            .watch(&notes_dir, RecursiveMode::Recursive)
            .map_err(|e| TeselaError::index(format!("Failed to start watching: {}", e)))?;

        // Spawn background thread to handle file system events
        let event_receiver = self.event_sender.subscribe();
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let mut debounce_map: HashMap<PathBuf, DateTime<Utc>> = HashMap::new();

                loop {
                    match rx.recv_timeout(Duration::from_millis(config.debounce_interval)) {
                        Ok(Ok(event)) => {
                            debug!("File system event: {:?}", event);

                            if let Err(e) = Self::handle_fs_event(
                                event,
                                &storage,
                                &database,
                                &config,
                                &event_sender,
                                &file_checksums,
                                &mut debounce_map,
                            )
                            .await
                            {
                                error!("Error handling file system event: {}", e);
                            }
                        }
                        Ok(Err(e)) => {
                            error!("File system watcher error: {}", e);
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            // Process debounced events
                            let now = Utc::now();
                            let mut to_process = Vec::new();

                            debounce_map.retain(|path, timestamp| {
                                if now.signed_duration_since(*timestamp).num_milliseconds()
                                    > config.debounce_interval as i64
                                {
                                    to_process.push(path.clone());
                                    false
                                } else {
                                    true
                                }
                            });

                            // Process debounced files
                            for path in to_process {
                                if let Err(e) = Self::index_file(
                                    &path,
                                    &storage,
                                    &database,
                                    &event_sender,
                                    &file_checksums,
                                )
                                .await
                                {
                                    error!("Error indexing file {}: {}", path.display(), e);
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            info!("File watcher channel disconnected");
                            break;
                        }
                    }
                }
            });
        });

        self._watcher = Some(watcher);
        Ok(event_receiver)
    }

    /// Perform a full rebuild of the index
    pub async fn rebuild_index(&self) -> Result<usize> {
        info!("Starting full index rebuild");

        let _ = self.event_sender.send(IndexEvent::RebuildStarted);

        let notes_dir = self.storage.notes_dir();
        let mut notes_processed = 0;

        // Clear existing checksums
        {
            let mut checksums = self.file_checksums.lock().unwrap();
            checksums.clear();
        }

        // Walk through all files in the notes directory
        for entry in WalkDir::new(&notes_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if !self.should_index_file(path) {
                continue;
            }

            match Self::index_file(
                path,
                &self.storage,
                &self.database,
                &self.event_sender,
                &self.file_checksums,
            )
            .await
            {
                Ok(()) => {
                    notes_processed += 1;
                    if notes_processed % self.config.batch_size == 0 {
                        debug!("Processed {} notes", notes_processed);
                    }
                }
                Err(e) => {
                    warn!("Failed to index {}: {}", path.display(), e);
                    let _ = self.event_sender.send(IndexEvent::IndexError {
                        path: path.to_path_buf(),
                        error: e.to_string(),
                    });
                }
            }
        }

        // Rebuild the FTS index
        self.database.rebuild_index().await?;

        let _ = self
            .event_sender
            .send(IndexEvent::RebuildCompleted { notes_processed });

        info!(
            "Index rebuild completed: {} notes processed",
            notes_processed
        );
        Ok(notes_processed)
    }

    /// Get event receiver for listening to index events
    pub fn subscribe_events(&self) -> broadcast::Receiver<IndexEvent> {
        self.event_sender.subscribe()
    }

    // Private helper methods

    async fn handle_fs_event(
        event: Event,
        _storage: &Storage,
        database: &Database,
        config: &IndexerConfig,
        event_sender: &broadcast::Sender<IndexEvent>,
        file_checksums: &Arc<Mutex<HashMap<PathBuf, String>>>,
        debounce_map: &mut HashMap<PathBuf, DateTime<Utc>>,
    ) -> Result<()> {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if Self::should_index_file_with_config(&path, config) {
                        // Add to debounce map
                        debounce_map.insert(path, Utc::now());
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if Self::should_index_file_with_config(&path, config) {
                        Self::remove_from_index(path, database, event_sender, file_checksums)
                            .await?;
                    }
                }
            }
            _ => {
                // Ignore other event types
            }
        }

        Ok(())
    }

    async fn index_file(
        path: &Path,
        storage: &Storage,
        database: &Database,
        event_sender: &broadcast::Sender<IndexEvent>,
        file_checksums: &Arc<Mutex<HashMap<PathBuf, String>>>,
    ) -> Result<()> {
        debug!("Indexing file: {}", path.display());

        // Load and parse the note
        let note = storage.load_note(path)?;

        // Check if file has changed using checksums
        let current_checksum = &note.checksum;
        let path_buf = path.to_path_buf();

        let needs_update = {
            let checksums = file_checksums.lock().unwrap();
            checksums
                .get(&path_buf)
                .map_or(true, |old_checksum| old_checksum != current_checksum)
        };

        if !needs_update {
            debug!("File {} hasn't changed, skipping", path.display());
            return Ok(());
        }

        // Extract links from the note content
        let links = storage.extract_links(&note.body);

        // Update database
        database.upsert_note(&note).await?;
        database.update_note_links(&note.id, &links).await?;

        // Update checksum cache
        {
            let mut checksums = file_checksums.lock().unwrap();
            checksums.insert(path_buf.clone(), current_checksum.clone());
        }

        let _ = event_sender.send(IndexEvent::NoteIndexed {
            path: path_buf,
            note_id: note.id.clone(),
        });

        debug!("Successfully indexed note: {}", note.id);
        Ok(())
    }

    async fn remove_from_index(
        path: PathBuf,
        database: &Database,
        event_sender: &broadcast::Sender<IndexEvent>,
        file_checksums: &Arc<Mutex<HashMap<PathBuf, String>>>,
    ) -> Result<()> {
        if let Some(note_id) = path.file_stem().and_then(|s| s.to_str()) {
            debug!("Removing note from index: {}", note_id);

            database.delete_note(note_id).await?;

            // Remove from checksum cache
            {
                let mut checksums = file_checksums.lock().unwrap();
                checksums.remove(&path);
            }

            let _ = event_sender.send(IndexEvent::NoteRemoved {
                path: path.clone(),
                note_id: note_id.to_string(),
            });
        }

        Ok(())
    }

    fn should_index_file(&self, path: &Path) -> bool {
        Self::should_index_file_with_config(path, &self.config)
    }

    fn should_index_file_with_config(path: &Path, config: &IndexerConfig) -> bool {
        // Must be a file
        if !path.is_file() {
            return false;
        }

        // Check file extension
        if !path
            .extension()
            .map_or(false, |ext| ext == "md" || ext == "markdown")
        {
            return false;
        }

        // Check if hidden and if we should index hidden files
        if !config.index_hidden {
            if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                if file_name.starts_with('.') {
                    return false;
                }
            }
        }

        // Check file size
        if let Ok(metadata) = path.metadata() {
            if metadata.len() > config.max_file_size {
                debug!(
                    "Skipping large file: {} ({} bytes)",
                    path.display(),
                    metadata.len()
                );
                return false;
            }
        }

        // Check exclude patterns
        let path_str = path.to_string_lossy();
        for pattern in &config.exclude_patterns {
            if glob_match(pattern, &path_str) {
                debug!(
                    "Excluding file due to pattern '{}': {}",
                    pattern,
                    path.display()
                );
                return false;
            }
        }

        true
    }
}

/// Simple glob pattern matching
fn glob_match(pattern: &str, text: &str) -> bool {
    // Simple implementation - in a real system you'd use a proper glob library
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];

            // Handle the suffix properly - ** means match any path segment
            let suffix_to_match = if suffix.starts_with('/') {
                // Remove leading slash and match against file extensions or patterns
                &suffix[1..]
            } else {
                suffix
            };

            // For ** patterns, we need to check if the text starts with prefix
            // and contains the suffix pattern somewhere after any path segments
            if text.starts_with(prefix) {
                // Handle simple file extension patterns like *.tmp
                if suffix_to_match.starts_with('*') {
                    let ext = &suffix_to_match[1..]; // Remove the *
                    return text.ends_with(ext);
                } else {
                    return text.ends_with(suffix_to_match);
                }
            }
            return false;
        }
    }

    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            return text.starts_with(prefix) && text.ends_with(suffix);
        }
    }

    pattern == text
}

/// Statistics about the indexing process
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub total_notes: usize,
    pub total_links: usize,
    pub total_tags: usize,
    pub total_attachments: usize,
    pub last_rebuild: Option<DateTime<Utc>>,
    pub index_size_bytes: u64,
}

impl Indexer {
    /// Get statistics about the current index
    pub async fn get_stats(&self) -> Result<IndexStats> {
        // This would query the database for various statistics
        // For now, return a basic implementation
        let tags = self.database.get_tags_with_counts().await?;

        Ok(IndexStats {
            total_notes: 0, // Would query database
            total_links: 0, // Would query database
            total_tags: tags.len(),
            total_attachments: 0, // Would query database
            last_rebuild: None,   // Would store in config/database
            index_size_bytes: 0,  // Would calculate database size
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::DatabaseConfig;
    use crate::core::storage::StorageConfig;
    use std::fs;
    use tempfile::TempDir;

    async fn create_test_indexer() -> (Indexer, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        let storage_config = StorageConfig {
            mosaic_root: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let storage = Arc::new(Storage::new(storage_config));

        let db_config = DatabaseConfig {
            db_path: temp_dir.path().join("test.db"),
            ..Default::default()
        };
        let database = Arc::new(Database::new(db_config).await.unwrap());

        let indexer_config = IndexerConfig::default();
        let indexer = Indexer::new(storage, database, indexer_config)
            .await
            .unwrap();

        // Create notes directory
        fs::create_dir_all(temp_dir.path().join("notes")).unwrap();
        fs::create_dir_all(temp_dir.path().join("attachments")).unwrap();

        (indexer, temp_dir)
    }

    #[tokio::test]
    async fn test_indexer_creation() {
        let (_indexer, _temp_dir) = create_test_indexer().await;
        // Indexer should be created successfully
    }

    #[tokio::test]
    async fn test_should_index_file() {
        let (_indexer, temp_dir) = create_test_indexer().await;
        let config = IndexerConfig::default();

        // Should index markdown files
        let md_file = temp_dir.path().join("test.md");
        fs::write(&md_file, "# Test").unwrap();
        assert!(Indexer::should_index_file_with_config(&md_file, &config));

        // Should not index other files
        let txt_file = temp_dir.path().join("test.txt");
        fs::write(&txt_file, "test").unwrap();
        assert!(!Indexer::should_index_file_with_config(&txt_file, &config));

        // Should not index hidden files by default
        let hidden_file = temp_dir.path().join(".hidden.md");
        fs::write(&hidden_file, "# Hidden").unwrap();
        assert!(!Indexer::should_index_file_with_config(
            &hidden_file,
            &config
        ));
    }

    #[tokio::test]
    async fn test_rebuild_index() {
        let (indexer, temp_dir) = create_test_indexer().await;

        // Create some test notes
        let notes_dir = temp_dir.path().join("notes");
        fs::write(notes_dir.join("note1.md"), "# Note 1\nContent").unwrap();
        fs::write(notes_dir.join("note2.md"), "# Note 2\nContent").unwrap();

        let result = indexer.rebuild_index().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*.md", "test.md"));
        assert!(glob_match("*.md", "note.md"));
        assert!(!glob_match("*.md", "test.txt"));

        assert!(glob_match("**/.git", "some/path/.git"));
        assert!(glob_match("**/*.tmp", "path/to/file.tmp"));
        assert!(!glob_match("**/*.tmp", "file.md"));
    }
}
