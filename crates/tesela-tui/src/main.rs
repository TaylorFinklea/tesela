mod action;
mod app;
mod event;
mod handler;
mod state;
mod view;
mod widgets;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, path::PathBuf, sync::Arc};

use tesela_core::{
    config::Config,
    db::SqliteIndex,
    indexer::Indexer,
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Find mosaic (same logic as CLI)
    let mosaic = find_mosaic()?;

    let config = Config::default();
    let db_path = mosaic.join(".tesela").join("tesela.db");

    let store = Arc::new(FsNoteStore::new(mosaic, config.storage));
    let index = Arc::new(SqliteIndex::open(&db_path).await?);

    // Wire up the Indexer so file changes appear in search results immediately
    let store_dyn: Arc<dyn NoteStore> = Arc::clone(&store) as Arc<dyn NoteStore>;
    let index_dyn: Arc<dyn SearchIndex> = Arc::clone(&index) as Arc<dyn SearchIndex>;
    let graph_dyn: Arc<dyn LinkGraph> = Arc::clone(&index) as Arc<dyn LinkGraph>;
    let indexer = Indexer::new(store_dyn, index_dyn, graph_dyn);
    indexer.initial_index().await?;
    let indexer_handle = indexer.start().await?;

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let app = app::App::new(store, index);
    let result = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    indexer_handle.stop().await;
    result
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
