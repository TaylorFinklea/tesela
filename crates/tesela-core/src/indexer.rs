//! Unified filesystem watcher + search index synchronization
//!
//! The Indexer watches a notes directory for changes and keeps the SQLite
//! search index and link graph in sync with the filesystem.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};

use crate::error::Result;
use crate::link::extract_wiki_links;
use crate::traits::link_graph::LinkGraph;
use crate::traits::note_store::NoteStore;
use crate::traits::search_index::SearchIndex;

/// Configuration for the Indexer.
pub struct IndexerConfig {
    /// Debounce interval in milliseconds for filesystem events.
    pub debounce_ms: u64,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self { debounce_ms: 500 }
    }
}

/// Unified filesystem watcher that keeps the search index and link graph
/// in sync with the notes directory.
pub struct Indexer {
    store: Arc<dyn NoteStore>,
    index: Arc<dyn SearchIndex>,
    graph: Arc<dyn LinkGraph>,
    config: IndexerConfig,
}

impl Indexer {
    /// Create a new Indexer.
    pub fn new(
        store: Arc<dyn NoteStore>,
        index: Arc<dyn SearchIndex>,
        graph: Arc<dyn LinkGraph>,
    ) -> Self {
        Self {
            store,
            index,
            graph,
            config: IndexerConfig::default(),
        }
    }

    /// Create a new Indexer with custom configuration.
    pub fn with_config(
        store: Arc<dyn NoteStore>,
        index: Arc<dyn SearchIndex>,
        graph: Arc<dyn LinkGraph>,
        config: IndexerConfig,
    ) -> Self {
        Self {
            store,
            index,
            graph,
            config,
        }
    }

    /// Do an initial full index of all notes in the store.
    pub async fn initial_index(&self) -> Result<usize> {
        let notes = self.store.list(None, usize::MAX, 0).await?;
        let count = notes.len();

        for note in &notes {
            self.index.reindex(note).await?;
            let links = extract_wiki_links(&note.content);
            self.graph.update_links(&note.id, &links).await?;
        }

        debug!("Initial index complete: {} notes indexed", count);
        Ok(count)
    }

    /// Start the indexer in a background tokio task.
    ///
    /// Returns an `IndexerHandle` that can be used to shut down the watcher.
    pub async fn start(self) -> Result<IndexerHandle> {
        let root = self.store.mosaic_root().await.to_path_buf();
        let debounce = Duration::from_millis(self.config.debounce_ms);

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        let store = self.store;
        let index = self.index;
        let graph = self.graph;

        let handle = tokio::task::spawn(async move {
            let (fs_tx, mut fs_rx) = mpsc::channel::<Event>(256);

            // Set up filesystem watcher
            let watcher_result = {
                let fs_tx = fs_tx.clone();
                RecommendedWatcher::new(
                    move |res: notify::Result<Event>| {
                        if let Ok(event) = res {
                            let _ = fs_tx.blocking_send(event);
                        }
                    },
                    notify::Config::default()
                        .with_poll_interval(debounce),
                )
            };

            let mut watcher = match watcher_result {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create file watcher: {}", e);
                    return;
                }
            };

            if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
                error!("Failed to watch directory {:?}: {}", root, e);
                return;
            }

            debug!("Indexer watching {:?}", root);

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        debug!("Indexer shutting down");
                        break;
                    }
                    Some(event) = fs_rx.recv() => {
                        Self::handle_event(&store, &index, &graph, &root, event).await;
                    }
                }
            }

            // Drop watcher to stop watching
            drop(watcher);
            debug!("Indexer stopped");
        });

        Ok(IndexerHandle {
            shutdown: shutdown_tx,
            handle,
        })
    }

    /// Process a single filesystem event.
    async fn handle_event(
        store: &Arc<dyn NoteStore>,
        index: &Arc<dyn SearchIndex>,
        graph: &Arc<dyn LinkGraph>,
        root: &PathBuf,
        event: Event,
    ) {
        for path in &event.paths {
            // Only process markdown files
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "md" && ext != "markdown" {
                continue;
            }

            // Derive note ID from the file path relative to root
            let rel_path = match path.strip_prefix(root) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let note_id_str = rel_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");

            if note_id_str.is_empty() {
                continue;
            }

            let note_id = crate::note::NoteId::new(note_id_str);

            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    // Re-read the note from the store and reindex
                    match store.get(&note_id).await {
                        Ok(Some(note)) => {
                            if let Err(e) = index.reindex(&note).await {
                                warn!("Failed to reindex {:?}: {}", note_id, e);
                            }
                            let links = extract_wiki_links(&note.content);
                            if let Err(e) = graph.update_links(&note_id, &links).await {
                                warn!("Failed to update links for {:?}: {}", note_id, e);
                            }
                            debug!("Reindexed {:?}", note_id);
                        }
                        Ok(None) => {
                            debug!("Note not found in store after fs event: {:?}", note_id);
                        }
                        Err(e) => {
                            warn!("Failed to read note {:?}: {}", note_id, e);
                        }
                    }
                }
                EventKind::Remove(_) => {
                    if let Err(e) = index.remove(&note_id).await {
                        warn!("Failed to remove {:?} from index: {}", note_id, e);
                    }
                    if let Err(e) = graph.remove_links(&note_id).await {
                        warn!("Failed to remove links for {:?}: {}", note_id, e);
                    }
                    debug!("Removed {:?} from index", note_id);
                }
                _ => {}
            }
        }
    }
}

/// Handle to a running Indexer background task.
pub struct IndexerHandle {
    /// Send a value to shut down the indexer.
    pub shutdown: oneshot::Sender<()>,
    /// The background task handle.
    pub handle: tokio::task::JoinHandle<()>,
}

impl IndexerHandle {
    /// Gracefully stop the indexer.
    pub async fn stop(self) {
        let _ = self.shutdown.send(());
        let _ = self.handle.await;
    }
}
