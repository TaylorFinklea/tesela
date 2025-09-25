//! Main application state and logic for the TUI

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, Terminal};
use std::{
    path::Path,
    time::{Duration, Instant},
};

use crate::commands;
use crate::core::database::Database;
use crate::tui::async_runtime::AsyncRuntime;
use crate::tui::fuzzy_search::FuzzySearch;
use crate::tui::power_search::{ItemAction, PowerSearchMode};
use crate::tui::search_filters::SearchFilters;
use crate::tui::search_history::SearchHistory;
use std::sync::Arc;

/// Application modes
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    MainMenu,
    Input(InputMode),
    Listing(ListingMode),
    PowerSearch(PowerSearchMode),
    Message(String, Instant),
    Help(HelpMode),
}

/// Help mode for displaying keyboard shortcuts
#[derive(Debug, Clone, PartialEq)]
pub struct HelpMode {
    pub context: crate::tui::shortcuts::ShortcutContext,
    pub scroll_offset: u16,
}

/// Input mode configuration
#[derive(Debug, Clone, PartialEq)]
pub struct InputMode {
    pub prompt: String,
    pub input: String,
    pub cursor_position: usize,
    pub input_type: InputType,
    pub suggestions: Vec<String>,
    pub suggestion_index: Option<usize>,
}

/// Different types of input we can collect
#[derive(Debug, Clone, PartialEq)]
pub enum InputType {
    NewNote,
    EditNote,
    SearchQuery,
}

/// Listing mode for displaying results
#[derive(Debug, Clone, PartialEq)]
pub struct ListingMode {
    pub title: String,
    pub items: Vec<ListItem>,
    pub selected: usize,
    pub list_type: ListType,
    pub preview_content: Option<String>,
    pub preview_scroll: u16,
    pub view_mode: ViewMode,
    pub backlinks: Vec<BacklinkItem>,
    pub selected_backlink: usize, // Track which backlink is selected
}

/// Represents a backlink to the current note
#[derive(Debug, Clone, PartialEq)]
pub struct BacklinkItem {
    pub source_title: String,
    pub source_path: String,
    pub context: String, // The line(s) containing the link
    pub line_number: usize,
}

/// Types of lists we can display
#[derive(Debug, Clone, PartialEq)]
pub enum ListType {
    Notes,
    SearchResults,
}

/// View modes for the right pane
#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Preview, // Show note content preview
    Graph,   // Show backlinks with context
}

/// A single item in a list
#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub title: String,
    pub subtitle: String,
    pub metadata: String,
    pub context: Option<String>, // For search results, shows the matching line
    pub match_indices: Vec<(usize, usize)>, // Start and end positions of matches in context
    pub snippet: Option<String>, // HTML snippet with highlighted matches from FTS5
    pub rank: Option<f32>,       // Search result relevance score
}

/// Main application state
pub struct App {
    pub mode: Mode,
    pub should_quit: bool,
    pub last_error: Option<String>,
    pub needs_terminal_restore: bool,
    pub database: Option<Arc<Database>>,
    pub async_runtime: AsyncRuntime,
    pub search_history: SearchHistory,
    pub fuzzy_search: FuzzySearch,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        // Check if we're in a valid mosaic
        if !Path::new("tesela.toml").exists() {}

        // Try to initialize database
        let database = None; // Database initialization would require async context

        // Initialize async runtime for database operations
        let async_runtime = AsyncRuntime::new()?;

        Ok(Self {
            mode: Mode::MainMenu,
            should_quit: false,
            last_error: None,
            needs_terminal_restore: false,
            database,
            async_runtime,
            search_history: SearchHistory::new(),
            fuzzy_search: FuzzySearch::new(),
        })
    }

    /// Main event loop
    pub fn run<B: Backend>(mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // Check if we need to restore terminal after external editor
            if self.needs_terminal_restore {
                // Re-setup terminal
                enable_raw_mode()?;
                execute!(std::io::stdout(), EnterAlternateScreen)?;
                terminal.clear()?;
                self.needs_terminal_restore = false;
            }

            // Draw UI
            terminal.draw(|f| crate::tui::ui::draw(&mut self, f))?;

            // Process pending searches with debouncing (check BEFORE event handling)
            // This ensures searches trigger even when no keys are pressed
            if let Mode::PowerSearch(ref mut power_search) = self.mode {
                if let Some(ref pending) = power_search.pending_query.clone() {
                    // Wait 250ms after last keystroke before searching
                    if power_search.last_query_time.elapsed() >= Duration::from_millis(250) {
                        power_search.is_searching = true;
                        let query = pending.clone();

                        // Get existing notes
                        let existing_notes: Vec<(String, String)> =
                            commands::get_notes_with_paths()
                                .unwrap_or_default()
                                .into_iter()
                                .map(|(path, title, _)| (path, title))
                                .collect();

                        // Perform content search with filters
                        let content_results = if power_search.filters.is_active {
                            let tags: Vec<String> =
                                power_search.filters.tags.iter().cloned().collect();
                            self.async_runtime
                                .search_with_filters(
                                    Some(&query),
                                    tags,
                                    power_search.filters.from_date,
                                    power_search.filters.to_date,
                                )
                                .unwrap_or_default()
                        } else {
                            let results =
                                self.async_runtime.search_notes(&query).unwrap_or_default();
                            results
                        };

                        // Update results
                        if let Mode::PowerSearch(ref mut ps) = self.mode {
                            ps.update_results(&query, existing_notes, content_results);
                            ps.pending_query = None;
                            ps.is_searching = false;

                            // Save to history if we got results
                            if !ps.sections.is_empty() {
                                self.search_history
                                    .add(query, ps.sections.iter().map(|s| s.items.len()).sum());
                            }
                        }
                    }
                }
            }

            // Handle events with timeout for message clearing
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code)?;
                    }
                }
            }

            // Clear expired messages
            if let Mode::Message(_, timestamp) = &self.mode {
                if timestamp.elapsed() > Duration::from_secs(3) {
                    self.mode = Mode::MainMenu;
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle key events based on current mode
    fn handle_key(&mut self, key: KeyCode) -> Result<()> {
        let mode = self.mode.clone();
        match mode {
            Mode::MainMenu => self.handle_main_menu(key)?,
            Mode::Input(input_mode) => self.handle_input(key, input_mode)?,
            Mode::Listing(listing_mode) => self.handle_listing(key, listing_mode)?,
            Mode::PowerSearch(power_search) => self.handle_power_search(key, power_search)?,
            Mode::Message(_, _) => {
                // Any key returns to main menu from message
                if key != KeyCode::Null {
                    self.mode = Mode::MainMenu;
                }
            }
            Mode::Help(help_mode) => self.handle_help(key, help_mode)?,
        }
        Ok(())
    }

    /// Handle main menu navigation
    fn handle_main_menu(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('?') => {
                self.show_help();
            }
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('n') => {
                // Redirect to PowerSearch for unified create/search experience
                self.mode = Mode::PowerSearch(PowerSearchMode::new());
                self.show_message("💡 Use Power Search to create or find notes. Type a name to create if it doesn't exist.".to_string());
            }
            KeyCode::Char('e') => self.start_input(InputType::EditNote),
            KeyCode::Char('s') => self.start_search(),
            KeyCode::Char('l') => self.list_notes()?,
            KeyCode::Char('d') => self.open_daily_note()?,
            _ => {}
        }
        Ok(())
    }

    /// Handle input mode
    fn handle_input(&mut self, key: KeyCode, mut input_mode: InputMode) -> Result<()> {
        match key {
            KeyCode::Esc => {
                self.mode = Mode::MainMenu;
            }
            KeyCode::Enter => {
                self.execute_input(input_mode)?;
            }
            KeyCode::Backspace => {
                if input_mode.cursor_position > 0 {
                    input_mode.input.remove(input_mode.cursor_position - 1);
                    input_mode.cursor_position -= 1;

                    self.update_suggestions(&mut input_mode)?;
                }
                self.mode = Mode::Input(input_mode);
            }
            KeyCode::Left => {
                if input_mode.cursor_position > 0 {
                    input_mode.cursor_position -= 1;
                }
                self.mode = Mode::Input(input_mode);
            }
            KeyCode::Right => {
                if input_mode.cursor_position < input_mode.input.len() {
                    input_mode.cursor_position += 1;
                }
                self.mode = Mode::Input(input_mode);
            }
            KeyCode::Tab => {
                self.handle_tab_completion(&mut input_mode)?;
                self.mode = Mode::Input(input_mode);
            }
            KeyCode::Char(c) => {
                input_mode.input.insert(input_mode.cursor_position, c);
                input_mode.cursor_position += 1;

                self.update_suggestions(&mut input_mode)?;

                self.mode = Mode::Input(input_mode);
            }
            _ => {
                self.mode = Mode::Input(input_mode);
            }
        }
        Ok(())
    }

    /// Handle listing mode navigation
    fn handle_listing(&mut self, key: KeyCode, mut listing_mode: ListingMode) -> Result<()> {
        match key {
            KeyCode::Esc => {
                self.mode = Mode::MainMenu;
            }
            KeyCode::Enter => {
                // In graph mode, open the selected backlink
                if listing_mode.view_mode == ViewMode::Graph && !listing_mode.backlinks.is_empty() {
                    let backlink = &listing_mode.backlinks[listing_mode.selected_backlink];
                    // Open the source file that contains the backlink
                    self.open_note(&backlink.source_path)?;
                } else {
                    self.execute_list_selection(&listing_mode)?;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                match listing_mode.view_mode {
                    ViewMode::Graph => {
                        // Navigate through backlinks in graph mode
                        if listing_mode.selected_backlink > 0 {
                            listing_mode.selected_backlink -= 1;
                        }
                    }
                    ViewMode::Preview => {
                        // Navigate through main list items in preview mode
                        if listing_mode.selected > 0 {
                            listing_mode.selected -= 1;
                            // Load content for newly selected item
                            if listing_mode.list_type == ListType::Notes
                                && !listing_mode.items.is_empty()
                            {
                                let selected_item = &listing_mode.items[listing_mode.selected];
                                listing_mode.preview_content =
                                    Self::load_note_preview(&selected_item.subtitle);
                                listing_mode.preview_scroll = 0;
                            }
                        }
                    }
                }
                self.mode = Mode::Listing(listing_mode);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match listing_mode.view_mode {
                    ViewMode::Graph => {
                        // Navigate through backlinks in graph mode
                        if listing_mode.selected_backlink
                            < listing_mode.backlinks.len().saturating_sub(1)
                        {
                            listing_mode.selected_backlink += 1;
                        }
                    }
                    ViewMode::Preview => {
                        // Navigate through main list items in preview mode
                        if listing_mode.selected < listing_mode.items.len().saturating_sub(1) {
                            listing_mode.selected += 1;
                            // Load content for newly selected item
                            if listing_mode.list_type == ListType::Notes
                                && !listing_mode.items.is_empty()
                            {
                                let selected_item = &listing_mode.items[listing_mode.selected];
                                listing_mode.preview_content =
                                    Self::load_note_preview(&selected_item.subtitle);
                                listing_mode.preview_scroll = 0;
                            }
                        }
                    }
                }
                self.mode = Mode::Listing(listing_mode);
            }
            KeyCode::PageDown => {
                // Scroll preview down
                if listing_mode.preview_content.is_some() {
                    listing_mode.preview_scroll = listing_mode.preview_scroll.saturating_add(5);
                }
                self.mode = Mode::Listing(listing_mode);
            }
            KeyCode::PageUp => {
                // Scroll preview up
                if listing_mode.preview_content.is_some() {
                    listing_mode.preview_scroll = listing_mode.preview_scroll.saturating_sub(5);
                }
                self.mode = Mode::Listing(listing_mode);
            }
            KeyCode::Char('g') => {
                // Toggle between preview and graph mode
                if !listing_mode.items.is_empty() {
                    match listing_mode.view_mode {
                        ViewMode::Preview => {
                            // Switch to graph mode - load backlinks
                            let selected_item = &listing_mode.items[listing_mode.selected];
                            listing_mode.backlinks = self.find_backlinks_with_context(
                                &selected_item.subtitle,
                                &selected_item.title,
                            );
                            listing_mode.view_mode = ViewMode::Graph;
                            listing_mode.selected_backlink = 0; // Reset backlink selection
                            listing_mode.preview_content = None; // Clear preview to save memory
                        }
                        ViewMode::Graph => {
                            // Switch back to preview mode - load preview
                            listing_mode.preview_content = Self::load_note_preview(
                                &listing_mode.items[listing_mode.selected].subtitle,
                            );
                            listing_mode.view_mode = ViewMode::Preview;
                            listing_mode.backlinks.clear(); // Clear backlinks to save memory
                            listing_mode.preview_scroll = 0;
                        }
                    }
                }
                self.mode = Mode::Listing(listing_mode);
            }
            _ => {
                self.mode = Mode::Listing(listing_mode);
            }
        }
        Ok(())
    }

    /// Start power search mode
    fn start_search(&mut self) {
        self.mode = Mode::PowerSearch(PowerSearchMode::new());
    }

    /// Show help screen
    fn show_help(&mut self) {
        let context = match &self.mode {
            Mode::MainMenu => crate::tui::shortcuts::ShortcutContext::MainMenu,
            Mode::PowerSearch(_) => crate::tui::shortcuts::ShortcutContext::Search,
            Mode::Listing(_) => crate::tui::shortcuts::ShortcutContext::Listing,
            Mode::Input(_) => crate::tui::shortcuts::ShortcutContext::Input,
            _ => crate::tui::shortcuts::ShortcutContext::Global,
        };

        self.mode = Mode::Help(HelpMode {
            context,
            scroll_offset: 0,
        });
    }

    /// Handle help mode
    fn handle_help(&mut self, key: KeyCode, mut help_mode: HelpMode) -> Result<()> {
        match key {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.mode = Mode::MainMenu;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if help_mode.scroll_offset > 0 {
                    help_mode.scroll_offset = help_mode.scroll_offset.saturating_sub(1);
                    self.mode = Mode::Help(help_mode);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                help_mode.scroll_offset = help_mode.scroll_offset.saturating_add(1);
                self.mode = Mode::Help(help_mode);
            }
            KeyCode::PageUp => {
                help_mode.scroll_offset = help_mode.scroll_offset.saturating_sub(10);
                self.mode = Mode::Help(help_mode);
            }
            KeyCode::PageDown => {
                help_mode.scroll_offset = help_mode.scroll_offset.saturating_add(10);
                self.mode = Mode::Help(help_mode);
            }
            _ => {
                self.mode = Mode::Help(help_mode);
            }
        }
        Ok(())
    }

    /// Handle power search mode
    fn handle_power_search(
        &mut self,
        key: KeyCode,
        mut power_search: PowerSearchMode,
    ) -> Result<()> {
        match key {
            KeyCode::Esc => {
                if power_search.filter_mode {
                    power_search.filter_mode = false;
                    self.mode = Mode::PowerSearch(power_search);
                } else {
                    self.mode = Mode::MainMenu;
                }
            }
            KeyCode::Char('/') if !power_search.filter_mode => {
                // Toggle to filter mode
                power_search.filter_mode = true;
                self.mode = Mode::PowerSearch(power_search);
            }
            KeyCode::Char('?') => {
                self.show_help();
            }
            KeyCode::Enter => {
                // Execute the selected action
                if let Some(item) = power_search.get_selected_item() {
                    // Clone the action to avoid borrow issues
                    let action = item.action.clone();

                    // Restore terminal before opening external editor
                    let _ = disable_raw_mode();
                    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

                    match action {
                        ItemAction::CreateNote(title) => {
                            match commands::create_note(&title) {
                                Ok(_) => {
                                    // Open the newly created note
                                    let safe_filename = title
                                        .chars()
                                        .map(|c| {
                                            if c.is_alphanumeric() || c == ' ' || c == '-' {
                                                c
                                            } else {
                                                '_'
                                            }
                                        })
                                        .collect::<String>()
                                        .replace(' ', "-")
                                        .to_lowercase();
                                    let _ = commands::open_note_in_editor(&safe_filename);
                                    self.mode = Mode::MainMenu;
                                    self.needs_terminal_restore = true;
                                }
                                Err(e) => {
                                    self.show_error(format!("Failed to create note: {}", e));
                                    self.needs_terminal_restore = true;
                                }
                            }
                        }
                        ItemAction::OpenNote(path) | ItemAction::JumpToBlock(path, _) => {
                            power_search.add_to_recents(std::path::PathBuf::from(&path));
                            match commands::open_note_by_path(&path) {
                                Ok(_) => {
                                    self.mode = Mode::MainMenu;
                                    self.needs_terminal_restore = true;
                                }
                                Err(e) => {
                                    self.show_error(format!("Failed to open note: {}", e));
                                    self.needs_terminal_restore = true;
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                if power_search.filter_mode {
                    // Handle filter string editing
                    if !power_search.filters.filter_string.is_empty() {
                        power_search.filters.filter_string.pop();
                        // Re-parse the filter string
                        if let Ok(filters) =
                            SearchFilters::parse(&power_search.filters.filter_string)
                        {
                            power_search.filters = filters;
                        }
                        power_search.pending_query = Some(power_search.query.clone());
                        power_search.last_query_time = Instant::now();
                    }
                } else if power_search.cursor_position > 0 {
                    power_search.query.remove(power_search.cursor_position - 1);
                    power_search.cursor_position -= 1;
                    // Mark query as pending for debounced search
                    power_search.pending_query = Some(power_search.query.clone());
                    power_search.last_query_time = Instant::now();
                }
                self.mode = Mode::PowerSearch(power_search);
            }
            KeyCode::Left => {
                if power_search.cursor_position > 0 {
                    power_search.cursor_position -= 1;
                }
                self.mode = Mode::PowerSearch(power_search);
            }
            KeyCode::Right => {
                if power_search.cursor_position < power_search.query.len() {
                    power_search.cursor_position += 1;
                }
                self.mode = Mode::PowerSearch(power_search);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                power_search.prev_item();
                self.mode = Mode::PowerSearch(power_search);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                power_search.next_item();
                self.mode = Mode::PowerSearch(power_search);
            }
            KeyCode::Tab => {
                // Tab to navigate between sections
                power_search.next_section();
                self.mode = Mode::PowerSearch(power_search);
            }
            KeyCode::BackTab => {
                // Shift+Tab to navigate backwards
                power_search.prev_section();
                self.mode = Mode::PowerSearch(power_search);
            }
            KeyCode::Char(c) => {
                if power_search.filter_mode {
                    // Add to filter string
                    power_search.filters.filter_string.push(c);
                    // Re-parse the filter string
                    if let Ok(filters) = SearchFilters::parse(&power_search.filters.filter_string) {
                        power_search.filters = filters;
                    }
                    power_search.pending_query = Some(power_search.query.clone());
                    power_search.last_query_time = Instant::now();
                } else {
                    power_search.query.insert(power_search.cursor_position, c);
                    power_search.cursor_position += 1;
                    // Mark query as pending for debounced search
                    power_search.pending_query = Some(power_search.query.clone());
                    power_search.last_query_time = Instant::now();
                }
                self.mode = Mode::PowerSearch(power_search);
            }
            _ => {
                self.mode = Mode::PowerSearch(power_search);
            }
        }
        Ok(())
    }

    /// Start input mode with appropriate prompt
    fn start_input(&mut self, input_type: InputType) {
        let prompt = match input_type {
            InputType::NewNote => "📝 Note name (will open Power Search):",
            InputType::EditNote => "📝 Edit note (name or partial):",
            InputType::SearchQuery => "🔍 Search query:",
        };

        let mut input_mode = InputMode {
            prompt: prompt.to_string(),
            input: String::new(),
            cursor_position: 0,
            input_type,
            suggestions: Vec::new(),
            suggestion_index: None,
        };

        // Load initial suggestions
        let _ = self.update_suggestions(&mut input_mode);

        self.mode = Mode::Input(input_mode);
    }

    /// Update suggestions based on current input
    fn update_suggestions(&mut self, input_mode: &mut InputMode) -> Result<()> {
        if input_mode.input.is_empty() {
            input_mode.suggestions.clear();
            input_mode.suggestion_index = None;
            return Ok(());
        }

        // Get suggestions based on input type
        let suggestions = match input_mode.input_type {
            InputType::EditNote | InputType::NewNote => {
                // Get note names for autocomplete
                let notes = commands::get_note_names_with_timestamps()?;
                let input_lower = input_mode.input.to_lowercase();

                let mut matches: Vec<String> = notes
                    .into_iter()
                    .filter(|(name, _)| name.to_lowercase().contains(&input_lower))
                    .map(|(name, _)| name)
                    .take(5)
                    .collect();

                matches.sort();
                matches
            }
            InputType::SearchQuery => {
                // Could add search keyword suggestions here
                Vec::new()
            }
        };

        input_mode.suggestions = suggestions;
        if !input_mode.suggestions.is_empty() {
            input_mode.suggestion_index = Some(0);
        }

        Ok(())
    }

    /// Handle tab completion
    fn handle_tab_completion(&mut self, input_mode: &mut InputMode) -> Result<()> {
        if input_mode.suggestions.is_empty() {
            return Ok(());
        }

        // Cycle through suggestions
        if let Some(index) = input_mode.suggestion_index {
            let next_index = (index + 1) % input_mode.suggestions.len();
            input_mode.suggestion_index = Some(next_index);

            // Replace input with suggestion
            if let Some(suggestion) = input_mode.suggestions.get(next_index) {
                input_mode.input = suggestion.clone();
                input_mode.cursor_position = input_mode.input.len();
            }
        }

        Ok(())
    }

    /// Execute the input based on type
    fn execute_input(&mut self, input_mode: InputMode) -> Result<()> {
        if input_mode.input.trim().is_empty() {
            self.mode = Mode::MainMenu;
            return Ok(());
        }

        match input_mode.input_type {
            InputType::NewNote => {
                // Redirect to PowerSearch instead of direct creation
                self.mode = Mode::PowerSearch(PowerSearchMode::new());
                if let Mode::PowerSearch(ref mut ps) = self.mode {
                    ps.query = input_mode.input.clone();
                    ps.cursor_position = ps.query.len();
                    ps.pending_query = Some(ps.query.clone());
                    ps.last_query_time = std::time::Instant::now();
                }
            }
            InputType::EditNote => {
                // Restore terminal before opening external editor
                let _ = disable_raw_mode();
                let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

                match commands::open_note_in_editor(&input_mode.input) {
                    Ok(_) => {
                        self.mode = Mode::MainMenu;
                        self.needs_terminal_restore = true;
                    }
                    Err(e) => {
                        self.show_error(format!("Failed to open note: {}", e));
                        self.needs_terminal_restore = true;
                    }
                }
            }
            InputType::SearchQuery => {
                self.perform_search(&input_mode.input)?;
            }
        }

        Ok(())
    }

    /// Execute selection from list
    fn execute_list_selection(&mut self, listing_mode: &ListingMode) -> Result<()> {
        if let Some(item) = listing_mode.items.get(listing_mode.selected) {
            match listing_mode.list_type {
                ListType::Notes => {
                    // Open the selected note
                    // Restore terminal before opening external editor
                    let _ = disable_raw_mode();
                    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

                    match commands::open_note_by_path(&item.subtitle) {
                        Ok(_) => {
                            self.mode = Mode::MainMenu;
                            self.needs_terminal_restore = true;
                        }
                        Err(e) => {
                            self.show_error(format!("Failed to open note: {}", e));
                            self.needs_terminal_restore = true;
                        }
                    }
                }
                ListType::SearchResults => {
                    // Open the search result
                    // Restore terminal before opening external editor
                    let _ = disable_raw_mode();
                    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

                    match commands::open_note_by_path(&item.subtitle) {
                        Ok(_) => {
                            self.mode = Mode::MainMenu;
                            self.needs_terminal_restore = true;
                        }
                        Err(e) => {
                            self.show_error(format!("Failed to open note: {}", e));
                            self.needs_terminal_restore = true;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// List all notes
    fn list_notes(&mut self) -> Result<()> {
        // Get all notes with paths, titles, and timestamps
        match commands::get_notes_with_paths() {
            Ok(notes) => {
                if notes.is_empty() {
                    let items = vec![ListItem {
                        title: "📄 No notes found".to_string(),
                        subtitle: "Create your first note with 'n'".to_string(),
                        metadata: "".to_string(),
                        context: None,
                        match_indices: vec![],
                        snippet: None,
                        rank: None,
                    }];

                    self.mode = Mode::Listing(ListingMode {
                        title: "📚 All Notes".to_string(),
                        items,
                        selected: 0,
                        list_type: ListType::Notes,
                        preview_content: None,
                        preview_scroll: 0,
                        view_mode: ViewMode::Preview,
                        backlinks: Vec::new(),
                        selected_backlink: 0,
                    });
                } else {
                    // Convert notes to ListItems
                    let mut items = Vec::new();

                    for (path, title, timestamp) in notes {
                        // Format the time ago
                        let time_ago = commands::format_time_ago(timestamp);

                        // Determine if it's a daily note or regular note
                        let note_type = if path.starts_with("dailies/") {
                            "📅 Daily"
                        } else {
                            "📝 Note"
                        };

                        items.push(ListItem {
                            title: title,
                            subtitle: path, // Store full path for opening
                            metadata: format!("{} • {}", note_type, time_ago),
                            context: None,
                            match_indices: vec![],
                            snippet: None,
                            rank: None,
                        });
                    }

                    // Notes are already sorted by timestamp (newest first)

                    // Load preview for the first note if available
                    let preview_content = if !items.is_empty() {
                        Self::load_note_preview(&items[0].subtitle)
                    } else {
                        None
                    };

                    self.mode = Mode::Listing(ListingMode {
                        title: format!("📚 All Notes ({})", items.len()),
                        items,
                        selected: 0,
                        list_type: ListType::Notes,
                        preview_content,
                        preview_scroll: 0,
                        view_mode: ViewMode::Preview,
                        backlinks: Vec::new(),
                        selected_backlink: 0,
                    });
                }
            }
            Err(e) => {
                self.show_error(format!("Failed to list notes: {}", e));
            }
        }

        // This function is now mostly replaced by search_notes
        // but keeping it for backward compatibility
        Ok(())
    }

    /// Extract highlight indices from HTML snippet with <mark> tags
    pub fn extract_highlight_indices(snippet: &str) -> Vec<(usize, usize)> {
        let mut indices = Vec::new();
        let mut clean_text = String::new();
        let mut chars = snippet.chars().peekable();
        let mut current_pos = 0;

        while let Some(ch) = chars.next() {
            if ch == '<' {
                // Check if this is a <mark> tag
                let mut tag = String::new();
                tag.push(ch);

                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    tag.push(next_ch);
                    if next_ch == '>' {
                        break;
                    }
                }

                if tag == "<mark>" {
                    // Start of highlight
                    let start = current_pos;

                    // Read until </mark>
                    while let Some(ch) = chars.next() {
                        if ch == '<' {
                            let mut end_tag = String::new();
                            end_tag.push(ch);

                            while let Some(&next_ch) = chars.peek() {
                                chars.next();
                                end_tag.push(next_ch);
                                if next_ch == '>' {
                                    break;
                                }
                            }

                            if end_tag == "</mark>" {
                                indices.push((start, current_pos));
                                break;
                            } else {
                                // Not the end tag, add to clean text
                                clean_text.push_str(&end_tag);
                                current_pos += end_tag.len();
                            }
                        } else {
                            clean_text.push(ch);
                            current_pos += 1;
                        }
                    }
                } else {
                    // Not a mark tag, add it to clean text
                    clean_text.push_str(&tag);
                    current_pos += tag.len();
                }
            } else {
                clean_text.push(ch);
                current_pos += 1;
            }
        }

        indices
    }

    /// Perform search
    fn perform_search(&mut self, query: &str) -> Result<()> {
        let search_results = self.async_runtime.search_notes(query)?;

        // Save to history
        if !search_results.is_empty() {
            self.search_history
                .add(query.to_string(), search_results.len());
        }

        if search_results.is_empty() {
            self.show_message(format!("No results found for: {}", query));
        } else {
            // Convert AsyncSearchResult to ListItem
            let items: Vec<ListItem> = search_results
                .into_iter()
                .map(|result| ListItem {
                    title: result.title,
                    subtitle: result.path,
                    metadata: format!("Score: {:.0}", result.rank * 100.0),
                    context: result.snippet,
                    match_indices: vec![],
                    snippet: Some(
                        result
                            .content
                            .lines()
                            .take(3)
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    rank: Some(result.rank),
                })
                .collect();

            self.mode = Mode::Listing(ListingMode {
                title: format!("🔍 Search Results for '{}' ({} found)", query, items.len()),
                items,
                selected: 0,
                list_type: ListType::SearchResults,
                preview_content: None,
                preview_scroll: 0,
                view_mode: ViewMode::Preview,
                backlinks: Vec::new(),
                selected_backlink: 0,
            });
        }

        Ok(())
    }

    /// Open daily note
    fn open_daily_note(&mut self) -> Result<()> {
        // Restore terminal before opening external editor
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

        match commands::daily_note_and_edit() {
            Ok(_) => {
                self.mode = Mode::MainMenu;
                self.needs_terminal_restore = true;
            }
            Err(e) => {
                self.show_error(format!("Failed to open daily note: {}", e));
                self.needs_terminal_restore = true;
            }
        }

        Ok(())
    }

    /// Show a success message
    fn show_message(&mut self, message: String) {
        self.mode = Mode::Message(message, Instant::now());
    }

    /// Show an error message
    fn show_error(&mut self, error: String) {
        self.last_error = Some(error.clone());
        self.mode = Mode::Message(format!("❌ {}", error), Instant::now());
    }

    /// Load a preview of a note's content
    fn load_note_preview(path: &str) -> Option<String> {
        use std::fs;

        if let Ok(content) = fs::read_to_string(path) {
            // Return the full content for scrolling support
            Some(content)
        } else {
            None
        }
    }

    /// Open a note in the editor
    fn open_note(&mut self, path: &str) -> Result<()> {
        use std::process::Command;

        // Exit alternate screen temporarily
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

        // Get the editor from environment or use default
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

        // Open the file in the editor
        let status = Command::new(&editor)
            .arg(path)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to open editor: {}", e))?;

        if !status.success() {
            return Err(anyhow::anyhow!("Editor exited with non-zero status"));
        }

        // Mark that we need to restore the terminal
        self.needs_terminal_restore = true;

        Ok(())
    }

    /// Find backlinks to a note with context
    fn find_backlinks_with_context(&self, note_path: &str, note_title: &str) -> Vec<BacklinkItem> {
        let mut backlinks = Vec::new();

        // Search patterns to look for
        let search_patterns = vec![
            format!("[[{}]]", note_title),
            format!("[[{}]]", note_path.trim_end_matches(".md")),
        ];

        // Search through all notes
        let directories = ["notes", "dailies"];
        for dir_name in directories {
            let dir_path = std::path::Path::new(dir_name);
            if dir_path.exists() {
                if let Ok(entries) = std::fs::read_dir(dir_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();

                        // Skip the current note itself
                        if path.to_string_lossy().contains(note_path) {
                            continue;
                        }

                        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md")
                        {
                            if let Ok(content) = std::fs::read_to_string(&path) {
                                // Check if this file contains any of our search patterns
                                for pattern in &search_patterns {
                                    if content.contains(pattern) {
                                        // Extract title from the source file
                                        let source_title = self
                                            .extract_title_from_file(&path)
                                            .unwrap_or_else(|_| {
                                                path.file_stem()
                                                    .and_then(|s| s.to_str())
                                                    .unwrap_or("Unknown")
                                                    .to_string()
                                            });

                                        // Find all occurrences with context
                                        for (line_num, line) in content.lines().enumerate() {
                                            if line.contains(pattern) {
                                                // Get context: the line itself plus surrounding lines if needed
                                                let context = self
                                                    .get_link_context(&content, line_num, pattern);

                                                backlinks.push(BacklinkItem {
                                                    source_title: source_title.clone(),
                                                    source_path: path.to_string_lossy().to_string(),
                                                    context,
                                                    line_number: line_num + 1, // Convert to 1-based
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        backlinks
    }

    /// Get context around a link
    fn get_link_context(&self, content: &str, line_num: usize, _pattern: &str) -> String {
        let lines: Vec<&str> = content.lines().collect();

        // Just return the single line containing the link
        if line_num < lines.len() {
            lines[line_num].trim().to_string()
        } else {
            String::new()
        }
    }

    /// Extract title from a file
    fn extract_title_from_file(&self, path: &std::path::Path) -> Result<String> {
        let content = std::fs::read_to_string(path)?;

        // Try to extract from frontmatter
        if content.starts_with("---") {
            if let Some(end) = content[3..].find("---") {
                let frontmatter = &content[3..end + 3];
                for line in frontmatter.lines() {
                    if line.starts_with("title:") {
                        let title = line[6..].trim();
                        // Remove quotes if present
                        let title = title.trim_matches('"').trim_matches('\'');
                        return Ok(title.to_string());
                    }
                }
            }
        }

        // Fall back to first H1 heading
        for line in content.lines() {
            if line.starts_with("# ") {
                return Ok(line[2..].trim().to_string());
            }
        }

        // Fall back to filename
        Ok(path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string())
    }
}
