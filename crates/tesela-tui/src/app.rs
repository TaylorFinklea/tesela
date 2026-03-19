use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::EventStream,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use tokio_stream::StreamExt;

use tesela_core::daily::DailyNoteConfig;
use tesela_core::db::SqliteIndex;
use tesela_core::note::NoteId;
use tesela_core::storage::filesystem::FsNoteStore;
use tesela_core::traits::link_graph::LinkGraph;
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
    fuzzy_matcher: SkimMatcherV2,
}

impl App {
    pub fn new(store: Arc<FsNoteStore>, index: Arc<SqliteIndex>) -> Self {
        Self {
            store,
            index,
            state: AppState::default(),
            fuzzy_matcher: SkimMatcherV2::default(),
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

            // Spawn external editor if requested (needs terminal access)
            if let Some((path, note_id)) = self.state.pending_editor.take() {
                self.spawn_editor(terminal, &path, &note_id).await?;
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
            Mode::GraphView => crate::view::note_preview::render_graph(f, chunks[0], &self.state),
            Mode::NewNote => crate::view::new_note::render(f, chunks[0], &self.state),
            Mode::Help => crate::view::help::render(f, chunks[0]),
        }

        // Status bar
        crate::view::status_bar::render(f, chunks[1], &self.state);

        // Fuzzy finder overlay (drawn on top of everything)
        if self.state.fuzzy.active {
            crate::view::fuzzy_finder::render(f, f.area(), &self.state);
        }
    }

    /// Spawn an external editor, suspending the TUI while it runs.
    /// Uses stdout directly to avoid generic Backend: Write constraint.
    async fn spawn_editor<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        path: &std::path::Path,
        note_id: &NoteId,
    ) -> Result<()> {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

        // Suspend TUI — restore normal terminal state
        disable_raw_mode()?;
        execute!(std::io::stdout(), LeaveAlternateScreen)?;

        // Run editor synchronously
        std::process::Command::new(&editor).arg(path).status().ok();

        // Restore TUI
        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen)?;
        terminal.clear()?;

        // Reload note from disk and update index
        if let Ok(Some(note)) = self.store.get(note_id).await {
            let _ = self.index.upsert_note(&note).await;
            self.state.current_note = Some(note);
            if let Ok(notes) = self
                .store
                .list(self.state.listing.filter_tag.as_deref(), 100, 0)
                .await
            {
                self.state.listing.notes = notes;
            }
        }

        Ok(())
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
                Ok(Some(note)) => {
                    self.state.current_note = Some(note);
                    self.state.graph_view_active = false;
                }
                Ok(None) => self.state.error_message = Some(format!("Note not found: {}", id)),
                Err(e) => self.state.error_message = Some(e.to_string()),
            },

            Action::EditNote(id) => {
                if let Ok(Some(note)) = self.store.get(&id).await {
                    let mosaic = self.store.mosaic_root().await.to_path_buf();
                    let path = mosaic.join(&note.path);
                    self.state.pending_editor = Some((path, id));
                    self.state.mode = Mode::NoteView;
                }
            }

            Action::OpenDailyNote => {
                let today = chrono::Utc::now().date_naive();
                let config = DailyNoteConfig::default();
                match self.store.daily_note(Some(today), &config).await {
                    Ok(note) => {
                        let mosaic = self.store.mosaic_root().await.to_path_buf();
                        let path = mosaic.join(&note.path);
                        let id = note.id.clone();
                        self.state.current_note = Some(note);
                        self.state.pending_editor = Some((path, id));
                        self.state.mode = Mode::NoteView;
                    }
                    Err(e) => self.state.error_message = Some(e.to_string()),
                }
            }

            Action::DeleteNote(id) => match self.store.delete(&id).await {
                Ok(()) => {
                    let _ = self.index.remove_note(&id).await;
                    self.state.current_note = None;
                    self.state.status_message = Some("Note deleted".to_string());
                    if let Ok(notes) = self
                        .store
                        .list(self.state.listing.filter_tag.as_deref(), 100, 0)
                        .await
                    {
                        self.state.listing.notes = notes;
                        self.state.listing.selected = 0;
                    }
                    self.state.mode = Mode::Listing;
                }
                Err(e) => self.state.error_message = Some(e.to_string()),
            },

            Action::CreateNote { title } => match self.store.create(&title, "- ", &[]).await {
                Ok(note) => {
                    let _ = self.index.upsert_note(&note).await;
                    let mosaic = self.store.mosaic_root().await.to_path_buf();
                    let path = mosaic.join(&note.path);
                    let id = note.id.clone();
                    self.state.listing.notes.insert(0, note.clone());
                    self.state.current_note = Some(note);
                    self.state.new_note_input = String::new();
                    self.state.pending_editor = Some((path, id));
                    self.state.mode = Mode::NoteView;
                }
                Err(e) => self.state.error_message = Some(e.to_string()),
            },

            Action::NewNoteInput(s) => {
                self.state.new_note_input = s;
            }

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

            Action::SelectItem(idx) => self.state.listing.selected = idx,

            Action::ToggleGraphView => {
                self.state.graph_view_active = !self.state.graph_view_active;
                if self.state.graph_view_active {
                    self.state.mode = Mode::GraphView;
                    // Load link data for the current note
                    if let Some(note) = &self.state.current_note {
                        let id = note.id.clone();
                        self.state.graph_backlinks =
                            self.index.get_backlinks(&id).await.unwrap_or_default();
                        self.state.graph_forward_links =
                            self.index.get_forward_links(&id).await.unwrap_or_default();
                    }
                } else {
                    self.state.mode = Mode::NoteView;
                    self.state.graph_backlinks = Vec::new();
                    self.state.graph_forward_links = Vec::new();
                }
            }

            Action::ToggleFuzzy => {
                if self.state.fuzzy.active {
                    self.state.fuzzy.deactivate();
                } else {
                    let notes = self.store.list(None, 1000, 0).await.unwrap_or_default();
                    self.state.fuzzy.activate(notes);
                }
            }

            Action::FuzzyQuery(q) => {
                let all_notes = self.store.list(None, 1000, 0).await.unwrap_or_default();
                self.state.fuzzy.query = q.clone();
                self.state.fuzzy.selected = 0;
                if q.is_empty() {
                    self.state.fuzzy.matches = all_notes;
                } else {
                    self.state.fuzzy.matches = all_notes
                        .into_iter()
                        .filter(|n| self.fuzzy_matcher.fuzzy_match(&n.title, &q).is_some())
                        .collect();
                }
            }

            Action::FuzzySelectNext => {
                let max = self.state.fuzzy.matches.len().saturating_sub(1);
                self.state.fuzzy.selected = (self.state.fuzzy.selected + 1).min(max);
            }

            Action::FuzzySelectPrev => {
                self.state.fuzzy.selected = self.state.fuzzy.selected.saturating_sub(1);
            }

            Action::FuzzySelect => {
                if let Some(note) = self.state.fuzzy.selected_note().cloned() {
                    let id = note.id.clone();
                    self.state.fuzzy.deactivate();
                    self.state.graph_view_active = false;
                    if let Ok(Some(full)) = self.store.get(&id).await {
                        self.state.current_note = Some(full);
                    }
                    self.state.mode = Mode::NoteView;
                }
            }

            Action::ShowMessage(msg) => self.state.status_message = Some(msg),
            Action::ShowError(err) => self.state.error_message = Some(err),
        }
        Ok(false)
    }
}
