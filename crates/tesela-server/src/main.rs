mod error;
mod routes;
mod state;

use anyhow::Result;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::broadcast;
use tracing::info;

use tesela_core::{
    config::Config,
    db::SqliteIndex,
    indexer::{Indexer, NoteEvent},
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
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
