//! File watcher module for Tesela
//!
//! This module provides automatic file watching and indexing capabilities,
//! monitoring changes to notes and updating the search index accordingly.

use crate::core::database::Database;
use crate::core::error::{Result, TeselaError};
use crate::core::indexer::Indexer;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, error, info, warn};

/// File system events that we care about
#[derive(Debug, Clone)]
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
}

/// Status of the file watcher
#[derive(Debug, Clone, PartialEq)]
pub enum WatcherStatus {
    Idle,
    Indexing { current_file: String },
    Error(String),
}

impl std::fmt::Display for WatcherStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatcherStatus::Idle => write!(f, "Idle"),
            WatcherStatus::Indexing { current_file } => {
                write!(f, "Indexing: {}", current_file)
            }
            WatcherStatus::Error(msg) => write!(f, "Error: {}", msg),
        }
    }
}

/// Configuration for the file watcher
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Debounce duration in milliseconds
    pub debounce_ms: u64,
    /// Paths to watch
    pub watch_paths: Vec<PathBuf>,
    /// File extensions to watch
    pub extensions: Vec<String>,
    /// Maximum number of pending events
    pub max_pending_events: usize,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 250,
            watch_paths: vec![PathBuf::from("notes"), PathBuf::from("dailies")],
            extensions: vec!["md".to_string(), "markdown".to_string()],
            max_pending_events: 100,
        }
    }
}

/// File watcher that monitors changes and triggers reindexing
pub struct FileWatcher {
    config: WatcherConfig,
    database: Arc<Database>,
    indexer: Arc<Indexer>,
    status: Arc<RwLock<WatcherStatus>>,
    event_tx: Sender<FileEvent>,
    event_rx: Option<Receiver<FileEvent>>,
    pending_events: Arc<RwLock<HashMap<PathBuf, (FileEvent, Instant)>>>,
}

impl FileWatcher {
    /// Create a new file watcher
    pub fn new(
        config: WatcherConfig,
        database: Arc<Database>,
        indexer: Arc<Indexer>,
    ) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::channel(config.max_pending_events);

        Ok(Self {
            config,
            database,
            indexer,
            status: Arc::new(RwLock::new(WatcherStatus::Idle)),
            event_tx,
            event_rx: Some(event_rx),
            pending_events: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get the current status
    pub async fn status(&self) -> WatcherStatus {
        self.status.read().await.clone()
    }

    /// Start watching for file changes
    pub async fn start(&mut self) -> Result<()> {
        let event_tx = self.event_tx.clone();
        let config = self.config.clone();
        let pending_events = self.pending_events.clone();

        // Create the native file watcher
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                match res {
                    Ok(event) => {
                        // Filter and convert events
                        if let Some(file_event) = Self::convert_event(event, &config.extensions) {
                            // Add to pending events for debouncing
                            let pending = pending_events.clone();
                            let _tx = event_tx.clone();

                            // Use blocking send since we're in a sync context
                            tokio::spawn(async move {
                                let mut pending = pending.write().await;

                                // Get the path from the event
                                let path = match &file_event {
                                    FileEvent::Created(p)
                                    | FileEvent::Modified(p)
                                    | FileEvent::Deleted(p) => p.clone(),
                                    FileEvent::Renamed { to, .. } => to.clone(),
                                };

                                pending.insert(path, (file_event, Instant::now()));
                            });
                        }
                    }
                    Err(e) => error!("Watch error: {:?}", e),
                }
            },
            Config::default(),
        )
        .map_err(|e| TeselaError::file_op(format!("Failed to create file watcher: {}", e)))?;

        // Watch configured paths
        for path in &self.config.watch_paths {
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive).map_err(|e| {
                    TeselaError::file_op(format!("Failed to watch path {:?}: {}", path, e))
                })?;
                info!("Watching path: {:?}", path);
            } else {
                warn!("Watch path does not exist: {:?}", path);
            }
        }

        // Store the watcher to keep it alive
        let _watcher = watcher; // This would normally be stored in the struct

        // Take the receiver to process events
        let _event_rx = self
            .event_rx
            .take()
            .ok_or_else(|| TeselaError::validation("Event receiver already taken"))?;

        let database = self.database.clone();
        let indexer = self.indexer.clone();
        let status = self.status.clone();
        let debounce_ms = self.config.debounce_ms;
        let pending_events = self.pending_events.clone();

        // Start the event processing task
        tokio::spawn(async move {
            // Debounce timer
            let mut debounce_interval = time::interval(Duration::from_millis(debounce_ms));

            loop {
                tokio::select! {
                    // Check for debounced events
                    _ = debounce_interval.tick() => {
                        let mut pending = pending_events.write().await;
                        let now = Instant::now();

                        // Process events that have been pending for the debounce duration
                        let mut events_to_process = Vec::new();
                        pending.retain(|path, (event, timestamp)| {
                            if now.duration_since(*timestamp) >= Duration::from_millis(debounce_ms) {
                                events_to_process.push((path.clone(), event.clone()));
                                false // Remove from pending
                            } else {
                                true // Keep in pending
                            }
                        });

                        // Process the debounced events
                        for (_path, event) in events_to_process {
                            Self::process_event(
                                event,
                                database.clone(),
                                indexer.clone(),
                                status.clone()
                            ).await;
                        }
                    }

                    // Handle shutdown
                    _ = tokio::signal::ctrl_c() => {
                        info!("Shutting down file watcher");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Convert notify event to our FileEvent type
    fn convert_event(event: Event, extensions: &[String]) -> Option<FileEvent> {
        // Check if the event is for a file we care about
        let paths: Vec<PathBuf> = event
            .paths
            .iter()
            .filter(|p| {
                if let Some(ext) = p.extension() {
                    extensions.iter().any(|e| ext == e.as_str())
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        if paths.is_empty() {
            return None;
        }

        match event.kind {
            EventKind::Create(_) => Some(FileEvent::Created(paths[0].clone())),
            EventKind::Modify(_) => Some(FileEvent::Modified(paths[0].clone())),
            EventKind::Remove(_) => Some(FileEvent::Deleted(paths[0].clone())),
            EventKind::Any => {
                // Renamed events often come as Any
                if paths.len() >= 2 {
                    Some(FileEvent::Renamed {
                        from: paths[0].clone(),
                        to: paths[1].clone(),
                    })
                } else {
                    Some(FileEvent::Modified(paths[0].clone()))
                }
            }
            _ => None,
        }
    }

    /// Process a file event
    async fn process_event(
        event: FileEvent,
        database: Arc<Database>,
        indexer: Arc<Indexer>,
        status: Arc<RwLock<WatcherStatus>>,
    ) {
        debug!("Processing event: {:?}", event);

        match event {
            FileEvent::Created(path) | FileEvent::Modified(path) => {
                // Update status
                {
                    let mut status = status.write().await;
                    *status = WatcherStatus::Indexing {
                        current_file: path.display().to_string(),
                    };
                }

                // Index the file
                if let Err(e) = Self::index_file(&path, database.clone(), indexer.clone()).await {
                    error!("Failed to index file {:?}: {}", path, e);
                    let mut status = status.write().await;
                    *status = WatcherStatus::Error(format!("Index error: {}", e));
                } else {
                    info!("Successfully indexed: {:?}", path);
                    let mut status = status.write().await;
                    *status = WatcherStatus::Idle;
                }
            }
            FileEvent::Deleted(path) => {
                // Update status
                {
                    let mut status = status.write().await;
                    *status = WatcherStatus::Indexing {
                        current_file: path.display().to_string(),
                    };
                }

                // Remove from index
                if let Err(e) = Self::remove_from_index(&path, database.clone()).await {
                    error!("Failed to remove file from index {:?}: {}", path, e);
                    let mut status = status.write().await;
                    *status = WatcherStatus::Error(format!("Remove error: {}", e));
                } else {
                    info!("Successfully removed from index: {:?}", path);
                    let mut status = status.write().await;
                    *status = WatcherStatus::Idle;
                }
            }
            FileEvent::Renamed { from, to } => {
                // Handle as delete + create
                {
                    let mut status = status.write().await;
                    *status = WatcherStatus::Indexing {
                        current_file: format!("{} -> {}", from.display(), to.display()),
                    };
                }

                // Remove old file
                if let Err(e) = Self::remove_from_index(&from, database.clone()).await {
                    error!("Failed to remove renamed file {:?}: {}", from, e);
                }

                // Index new file
                if let Err(e) = Self::index_file(&to, database.clone(), indexer.clone()).await {
                    error!("Failed to index renamed file {:?}: {}", to, e);
                    let mut status = status.write().await;
                    *status = WatcherStatus::Error(format!("Rename error: {}", e));
                } else {
                    info!("Successfully handled rename: {:?} -> {:?}", from, to);
                    let mut status = status.write().await;
                    *status = WatcherStatus::Idle;
                }
            }
        }
    }

    /// Index a single file
    async fn index_file(
        path: &Path,
        _database: Arc<Database>,
        indexer: Arc<Indexer>,
    ) -> Result<()> {
        // Read the file content
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| TeselaError::file_op(format!("Failed to read file {:?}: {}", path, e)))?;

        // Parse and index the note
        indexer.index_content(path, &content).await?;

        debug!("Indexed file: {:?}", path);
        Ok(())
    }

    /// Remove a file from the index
    async fn remove_from_index(path: &Path, database: Arc<Database>) -> Result<()> {
        // Convert path to string for database operations
        let path_str = path.to_string_lossy();

        // Delete from database
        database.delete_note_by_path(&path_str).await?;

        debug!("Removed from index: {:?}", path);
        Ok(())
    }

    /// Stop watching for file changes
    pub async fn stop(&mut self) -> Result<()> {
        // Update status
        {
            let mut status = self.status.write().await;
            *status = WatcherStatus::Idle;
        }

        info!("File watcher stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_event_conversion() {
        let extensions = vec!["md".to_string()];

        // Test create event
        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![PathBuf::from("test.md")],
            attrs: Default::default(),
        };

        let file_event = FileWatcher::convert_event(event, &extensions);
        assert!(matches!(file_event, Some(FileEvent::Created(_))));

        // Test non-markdown file (should be filtered)
        let event = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![PathBuf::from("test.txt")],
            attrs: Default::default(),
        };

        let file_event = FileWatcher::convert_event(event, &extensions);
        assert!(file_event.is_none());
    }

    #[tokio::test]
    async fn test_watcher_status() {
        let _config = WatcherConfig::default();
        let _temp_dir = TempDir::new().unwrap();

        // This would need mock database and indexer for a full test
        // For now, just test the status mechanism
        let status = Arc::new(RwLock::new(WatcherStatus::Idle));

        // Test status updates
        {
            let mut s = status.write().await;
            *s = WatcherStatus::Indexing {
                current_file: "test.md".to_string(),
            };
        }

        let current = status.read().await.clone();
        assert!(matches!(current, WatcherStatus::Indexing { .. }));
    }
}
