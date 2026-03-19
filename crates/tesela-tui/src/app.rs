use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::EventStream;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use tokio_stream::StreamExt;

use tesela_core::db::SqliteIndex;
use tesela_core::storage::filesystem::FsNoteStore;
use tesela_core::traits::note_store::NoteStore;
use tesela_core::traits::search_index::SearchIndex;

use crate::{
    action::Action,
    event::{self, Event},
    handler::handle,
    state::{mode::Mode, AppState},
};

pub struct App {
    store: Arc<FsNoteStore>,
    index: Arc<SqliteIndex>,
    state: AppState,
}

impl App {
    pub fn new(store: Arc<FsNoteStore>, index: Arc<SqliteIndex>) -> Self {
        Self {
            store,
            index,
            state: AppState::default(),
        }
    }

    pub async fn run<B: ratatui::backend::Backend>(
        mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        let mut event_stream = EventStream::new();

        loop {
            // Draw
            terminal.draw(|f| self.draw(f))?;

            // Get next event (with timeout for tick)
            let evt = tokio::select! {
                Some(Ok(e)) = event_stream.next() => {
                    match event::from_crossterm(e) {
                        Some(ev) => ev,
                        None => continue,
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(250)) => Event::Tick,
            };

            // Handle -> actions
            let actions = handle(&self.state, &evt);

            // Process actions
            let mut should_quit = false;
            for action in actions {
                if self.process_action(action).await? {
                    should_quit = true;
                }
            }

            if should_quit {
                break;
            }
        }

        Ok(())
    }

    fn draw(&self, f: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(f.area());

        // Main content
        match &self.state.mode {
            Mode::MainMenu => crate::view::main_menu::render(f, chunks[0]),
            Mode::Listing => crate::view::listing::render(f, chunks[0], &self.state),
            Mode::Search => crate::view::search::render(f, chunks[0], &self.state),
            Mode::NoteView => crate::view::note_preview::render(f, chunks[0], &self.state),
            Mode::Help => crate::view::help::render(f, chunks[0]),
        }

        // Status bar
        crate::view::status_bar::render(f, chunks[1], &self.state);
    }

    /// Process an action. Returns true if the app should quit.
    async fn process_action(&mut self, action: Action) -> Result<bool> {
        match action {
            Action::Quit => return Ok(true),
            Action::EnterMode(mode) => {
                self.state.mode = mode;
                self.state.error_message = None;
                self.state.status_message = None;
            }
            Action::RefreshList => {
                match self
                    .store
                    .list(self.state.listing.filter_tag.as_deref(), 100, 0)
                    .await
                {
                    Ok(notes) => {
                        self.state.listing.notes = notes;
                        self.state.listing.selected = 0;
                    }
                    Err(e) => self.state.error_message = Some(e.to_string()),
                }
            }
            Action::SelectNext => match self.state.mode {
                Mode::Listing => self.state.listing.select_next(),
                Mode::Search => {
                    let max = self.state.search.results.len().saturating_sub(1);
                    self.state.search.selected = (self.state.search.selected + 1).min(max);
                }
                _ => {}
            },
            Action::SelectPrev => match self.state.mode {
                Mode::Listing => self.state.listing.select_prev(),
                Mode::Search => {
                    self.state.search.selected = self.state.search.selected.saturating_sub(1);
                }
                _ => {}
            },
            Action::OpenNote(id) => match self.store.get(&id).await {
                Ok(Some(note)) => self.state.current_note = Some(note),
                Ok(None) => {
                    self.state.error_message = Some(format!("Note not found: {}", id))
                }
                Err(e) => self.state.error_message = Some(e.to_string()),
            },
            Action::UpdateSearchQuery(q) => {
                self.state.search.query = q;
                self.state.search.selected = 0;
            }
            Action::ExecuteSearch(q) => {
                self.state.search.push_history(q.clone());
                match self.index.search(&q, 20, 0).await {
                    Ok(hits) => {
                        self.state.search.results = hits;
                        self.state.search.selected = 0;
                        self.state.search.is_searching = false;
                    }
                    Err(e) => self.state.error_message = Some(e.to_string()),
                }
            }
            Action::ClearSearch => self.state.search.clear(),
            Action::ScrollDown => {
                self.state.listing.scroll_offset =
                    self.state.listing.scroll_offset.saturating_add(1);
            }
            Action::ScrollUp => {
                self.state.listing.scroll_offset =
                    self.state.listing.scroll_offset.saturating_sub(1);
            }
            Action::CreateNote { title } => match self.store.create(&title, "", &[]).await {
                Ok(note) => {
                    let _ = self.index.upsert_note(&note).await;
                    self.state.status_message = Some(format!("Created: {}", note.title));
                    self.state.listing.notes.push(note);
                }
                Err(e) => self.state.error_message = Some(e.to_string()),
            },
            Action::SelectItem(idx) => self.state.listing.selected = idx,
            Action::ShowMessage(msg) => self.state.status_message = Some(msg),
            Action::ShowError(err) => self.state.error_message = Some(err),
        }
        Ok(false)
    }
}
