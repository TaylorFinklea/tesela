mod error;
mod routes;
mod state;

use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::broadcast;
use tracing::{info, warn};

use tesela_core::{
    config::Config,
    db::SqliteIndex,
    indexer::{Indexer, NoteEvent},
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    types::TypeRegistry,
};

use state::{AppState, WsEvent};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mosaic = find_mosaic()?;

    // Auto-backup on startup (keep last 5 daily backups)
    auto_backup(&mosaic);

    let config = Config::default();
    let db_path = mosaic.join(".tesela").join("tesela.db");
    let notes_dir = mosaic.join("notes");
    let type_registry = TypeRegistry::load(&mosaic);
    info!("Loaded {} type definitions", type_registry.types.len());

    let store = Arc::new(FsNoteStore::new(mosaic, config.storage));
    let index = Arc::new(SqliteIndex::open(&db_path).await?);

    // Wire up the Indexer (same as TUI)
    let store_dyn: Arc<dyn NoteStore> = Arc::clone(&store) as Arc<dyn NoteStore>;
    let index_dyn: Arc<dyn SearchIndex> = Arc::clone(&index) as Arc<dyn SearchIndex>;
    let graph_dyn: Arc<dyn LinkGraph> = Arc::clone(&index) as Arc<dyn LinkGraph>;

    // WebSocket broadcast channel
    let (ws_tx, _) = broadcast::channel::<WsEvent>(64);

    // Indexer notify channel — maps file-system events to WsEvents
    let (note_event_tx, _) = broadcast::channel::<NoteEvent>(64);

    let indexer =
        Indexer::new(store_dyn, index_dyn, graph_dyn).with_notify_tx(note_event_tx.clone());
    indexer.initial_index().await?;

    // Auto-create tag pages for any tags that don't have a corresponding page
    {
        let all_notes = store.list(None, usize::MAX, 0).await?;
        let existing_ids: std::collections::HashSet<String> = all_notes
            .iter()
            .map(|n| n.id.as_str().to_lowercase())
            .collect();
        for note in &all_notes {
            for tag in &note.metadata.tags {
                if tag == "daily" {
                    continue;
                }
                let tag_lower = tag.to_lowercase();
                if !existing_ids.contains(&tag_lower) {
                    let content = format!(
                        "---\ntitle: \"{}\"\ntype: \"Tag\"\nextends: \"Root Tag\"\ntag_properties: []\ntags: []\n---\n- Tag properties are inherited by all nodes using the tag.\n",
                        tag
                    );
                    match store.create(tag, &content, &[]).await {
                        Ok(tag_note) => {
                            let _ = index.reindex(&tag_note).await;
                            info!("Auto-created tag page: {}", tag);
                        }
                        Err(e) => warn!("Failed to auto-create tag page '{}': {}", tag, e),
                    }
                }
            }
        }
    }

    // Create built-in pages by writing files directly (store.create() overwrites frontmatter)
    {
        let _ = std::fs::create_dir_all(&notes_dir);

        let builtin_pages: Vec<(&str, &str)> = vec![
            ("root-tag.md", "---\ntitle: \"Root Tag\"\ntype: \"Tag\"\nicon: \"📄\"\ntag_properties: []\ntags: []\n---\n- The base tag that all other tags extend.\n"),
            ("task.md", "---\ntitle: \"Task\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"☑\"\ntag_properties: [\"Status\", \"Priority\", \"Deadline\", \"Scheduled\"]\ntags: []\n---\n- Task tag page.\n"),
            ("project.md", "---\ntitle: \"Project\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"🗂\"\ntag_properties: [\"Status\", \"Deadline\"]\ntags: []\n---\n- Project tag page.\n"),
            ("person.md", "---\ntitle: \"Person\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"👤\"\ntag_properties: [\"Email\", \"Team\"]\ntags: []\n---\n- Person tag page.\n"),
            ("status.md", "---\ntitle: \"Status\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"backlog\", \"todo\", \"doing\", \"in-review\", \"done\", \"canceled\"]\ndefault: \"todo\"\ntags: []\n---\n- Status property.\n"),
            ("priority.md", "---\ntitle: \"Priority\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"critical\", \"high\", \"medium\", \"low\"]\ndefault: \"medium\"\ntags: []\n---\n- Priority property.\n"),
            ("deadline.md", "---\ntitle: \"Deadline\"\ntype: \"Property\"\nvalue_type: \"date\"\ntags: []\n---\n- Deadline property.\n"),
            ("scheduled.md", "---\ntitle: \"Scheduled\"\ntype: \"Property\"\nvalue_type: \"date\"\ntags: []\n---\n- Scheduled property.\n"),

            // Life OS types
            ("domain.md", "---\ntitle: \"Domain\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"globe\"\ntag_properties: [\"Description\"]\ntags: []\n---\n- Top-level life area (Work, Family, Health, Home, etc.).\n"),
            ("lifeproject.md", "---\ntitle: \"LifeProject\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"folder\"\ntag_properties: [\"Status\", \"DomainRef\", \"Deadline\", \"Description\"]\ntags: []\n---\n- Multi-task effort within a domain.\n"),
            ("issue.md", "---\ntitle: \"Issue\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"lightbulb\"\ntag_properties: [\"IssueStatus\", \"DomainRef\", \"Description\"]\ntags: []\n---\n- Deliberation item — needs thought, may become a project or task.\n"),
            ("ritual.md", "---\ntitle: \"Ritual\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"sparkles\"\ntag_properties: [\"Cadence\", \"DomainRef\"]\ntags: []\n---\n- Daily or recurring mental check-in.\n"),
            ("scheduleditem.md", "---\ntitle: \"ScheduledItem\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"calendar\"\ntag_properties: [\"Cadence\", \"DomainRef\", \"LastCompleted\"]\ntags: []\n---\n- Recurring task with a cadence.\n"),

            // Life OS properties
            ("issuestatus.md", "---\ntitle: \"IssueStatus\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"inbox\", \"open\", \"thinking\", \"resolved\", \"became-project\", \"became-task\"]\ndefault: \"inbox\"\ntags: []\n---\n- Lifecycle status for issues.\n"),
            ("cadence.md", "---\ntitle: \"Cadence\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"daily\", \"weekly\", \"biweekly\", \"monthly\", \"quarterly\", \"yearly\"]\ntags: []\n---\n- How often a ritual or scheduled item recurs.\n"),
            ("description.md", "---\ntitle: \"Description\"\ntype: \"Property\"\nvalue_type: \"text\"\ntags: []\n---\n- Text description for any entity.\n"),
            ("lastcompleted.md", "---\ntitle: \"LastCompleted\"\ntype: \"Property\"\nvalue_type: \"date\"\ntags: []\n---\n- When a recurring item was last completed.\n"),
            ("domainref.md", "---\ntitle: \"DomainRef\"\ntype: \"Property\"\nvalue_type: \"node\"\ntags: []\n---\n- Links an item to its parent Domain page.\n"),
        ];

        for (filename, content) in builtin_pages {
            let path = notes_dir.join(filename);
            if !path.exists() {
                if let Err(e) = std::fs::write(&path, content) {
                    warn!("Failed to create built-in page {}: {}", filename, e);
                } else {
                    info!("Auto-created built-in page: {}", filename);
                }
            }
        }
    }

    let indexer_handle = indexer.start().await?;

    // Bridge NoteEvents from the Indexer to WsEvents for WebSocket clients
    let ws_tx_bridge = ws_tx.clone();
    tokio::spawn(async move {
        let mut rx = note_event_tx.subscribe();
        while let Ok(event) = rx.recv().await {
            let ws_event = match event {
                NoteEvent::Created(note) => WsEvent::NoteCreated { note },
                NoteEvent::Updated(note) => WsEvent::NoteUpdated { note },
                NoteEvent::Deleted(id) => WsEvent::NoteDeleted { id: id.to_string() },
            };
            let _ = ws_tx_bridge.send(ws_event);
        }
    });

    let app_state = AppState {
        store,
        index,
        ws_tx,
        type_registry,
    };
    let router = routes::build(app_state);

    let addr = "127.0.0.1:7474";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("tesela-server listening on http://{}", addr);

    axum::serve(listener, router).await?;

    indexer_handle.stop().await;
    Ok(())
}

fn find_mosaic() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(".tesela").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!("No mosaic found. Run 'tesela init' first.")
}

/// Auto-backup on server startup. Creates a daily backup of notes/.
/// Only creates one backup per day. Keeps last 5 daily backups.
fn auto_backup(mosaic: &Path) {
    let notes_dir = mosaic.join("notes");
    if !notes_dir.exists() {
        return;
    }

    let backup_root = mosaic.join(".tesela").join("backups");
    if std::fs::create_dir_all(&backup_root).is_err() {
        warn!("Failed to create backup directory");
        return;
    }

    let today = chrono::Local::now().format("%Y%m%d").to_string();
    let backup_dir = backup_root.join(format!("daily-{}", today));

    // Skip if today's backup already exists
    if backup_dir.exists() {
        info!("Today's backup already exists: {}", backup_dir.display());
        return;
    }

    // Copy notes/ recursively
    if let Err(e) = copy_dir_recursive(&notes_dir, &backup_dir) {
        warn!("Auto-backup failed: {}", e);
        return;
    }

    info!("Auto-backup created: {}", backup_dir.display());

    // Clean old backups (keep last 5)
    if let Ok(entries) = std::fs::read_dir(&backup_root) {
        let mut backups: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("daily-"))
            .collect();
        backups.sort_by_key(|e| e.file_name());
        let total = backups.len();
        if total > 5 {
            for entry in backups.into_iter().take(total - 5) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
