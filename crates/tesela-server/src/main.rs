mod error;
mod routes;
mod state;

use anyhow::Result;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::broadcast;
use tracing::{info, warn};

use tesela_core::{
    config::Config,
    db::SqliteIndex,
    indexer::{Indexer, NoteEvent},
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    types::TypeRegistry,
    NoteId,
};

use state::{AppState, WsEvent};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mosaic = find_mosaic()?;
    let config = Config::default();
    let db_path = mosaic.join(".tesela").join("tesela.db");
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

    let indexer = Indexer::new(store_dyn, index_dyn, graph_dyn)
        .with_notify_tx(note_event_tx.clone());
    indexer.initial_index().await?;

    // Auto-create tag pages for any tags that don't have a corresponding page
    {
        let all_notes = store.list(None, usize::MAX, 0).await?;
        let existing_ids: std::collections::HashSet<String> =
            all_notes.iter().map(|n| n.id.as_str().to_lowercase()).collect();
        for note in &all_notes {
            for tag in &note.metadata.tags {
                if tag == "daily" { continue; }
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

    // Create built-in property pages if they don't exist
    {
        let builtin_properties = vec![
            ("Status", "select", r#"["backlog", "todo", "doing", "in-review", "done", "canceled"]"#, "todo"),
            ("Priority", "select", r#"["critical", "high", "medium", "low"]"#, "medium"),
            ("Deadline", "date", "[]", ""),
            ("Scheduled", "date", "[]", ""),
        ];
        for (name, vtype, choices, default_val) in builtin_properties {
            let prop_id = NoteId::new(name.to_lowercase());
            if store.get(&prop_id).await?.is_none() {
                let content = format!(
                    "---\ntitle: \"{name}\"\ntype: \"Property\"\nvalue_type: \"{vtype}\"\nchoices: {choices}\ndefault: \"{default_val}\"\ntags: []\n---\n- {name} property.\n"
                );
                match store.create(name, &content, &[]).await {
                    Ok(prop_note) => {
                        let _ = index.reindex(&prop_note).await;
                        info!("Auto-created property page: {}", name);
                    }
                    Err(e) => warn!("Failed to auto-create property page '{}': {}", name, e),
                }
            }
        }
        // Create Root Tag if it doesn't exist
        let root_tag_id = NoteId::new("root-tag");
        if store.get(&root_tag_id).await?.is_none() {
            let content = "---\ntitle: \"Root Tag\"\ntype: \"Tag\"\ntag_properties: []\ntags: []\n---\n- The base tag that all other tags extend.\n";
            match store.create("Root Tag", content, &[]).await {
                Ok(note) => {
                    let _ = index.reindex(&note).await;
                    info!("Auto-created Root Tag page");
                }
                Err(e) => warn!("Failed to auto-create Root Tag: {}", e),
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
