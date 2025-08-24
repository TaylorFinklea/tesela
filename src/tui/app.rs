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
use std::fs;

/// Application modes
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    MainMenu,
    Input(InputMode),
    Listing(ListingMode),
    Search(SearchMode),
    Message(String, Instant),
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

/// Search mode - combines input with live results
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMode {
    pub query: String,
    pub cursor_position: usize,
    pub results: Vec<ListItem>,
    pub selected_result: usize,
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
}

/// Types of lists we can display
#[derive(Debug, Clone, PartialEq)]
pub enum ListType {
    Notes,
    SearchResults,
}

/// A single item in a list
#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub title: String,
    pub subtitle: String,
    pub metadata: String,
    pub context: Option<String>, // For search results, shows the matching line
    pub match_indices: Vec<(usize, usize)>, // Start and end positions of matches in context
}

/// Main application state
pub struct App {
    pub mode: Mode,
    pub should_quit: bool,
    pub last_error: Option<String>,
    pub needs_terminal_restore: bool,
}

impl App {
    /// Create a new App instance
    pub fn new() -> Result<Self> {
        // Check if we're in a valid mosaic
        if !Path::new("tesela.toml").exists() {
            eprintln!("⚠️  No mosaic found. Initialize with: tesela init");
        }

        Ok(Self {
            mode: Mode::MainMenu,
            should_quit: false,
            last_error: None,
            needs_terminal_restore: false,
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
            Mode::Search(search_mode) => self.handle_search(key, search_mode)?,
            Mode::Message(_, _) => {
                // Any key returns to main menu from message
                if key != KeyCode::Null {
                    self.mode = Mode::MainMenu;
                }
            }
        }
        Ok(())
    }

    /// Handle main menu navigation
    fn handle_main_menu(&mut self, key: KeyCode) -> Result<()> {
        match key {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('n') => self.start_input(InputType::NewNote),
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
                self.execute_list_selection(&listing_mode)?;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if listing_mode.selected > 0 {
                    listing_mode.selected -= 1;
                    // Load preview for newly selected item
                    if listing_mode.list_type == ListType::Notes && !listing_mode.items.is_empty() {
                        listing_mode.preview_content = Self::load_note_preview(
                            &listing_mode.items[listing_mode.selected].subtitle,
                        );
                        listing_mode.preview_scroll = 0; // Reset scroll when changing selection
                    }
                }
                self.mode = Mode::Listing(listing_mode);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if listing_mode.selected < listing_mode.items.len().saturating_sub(1) {
                    listing_mode.selected += 1;
                    // Load preview for newly selected item
                    if listing_mode.list_type == ListType::Notes && !listing_mode.items.is_empty() {
                        listing_mode.preview_content = Self::load_note_preview(
                            &listing_mode.items[listing_mode.selected].subtitle,
                        );
                        listing_mode.preview_scroll = 0; // Reset scroll when changing selection
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
            _ => {
                self.mode = Mode::Listing(listing_mode);
            }
        }
        Ok(())
    }

    /// Start search mode
    fn start_search(&mut self) {
        self.mode = Mode::Search(SearchMode {
            query: String::new(),
            cursor_position: 0,
            results: Vec::new(),
            selected_result: 0,
        });
    }

    /// Handle search mode (combined input and results)
    fn handle_search(&mut self, key: KeyCode, mut search_mode: SearchMode) -> Result<()> {
        match key {
            KeyCode::Esc => {
                self.mode = Mode::MainMenu;
            }
            KeyCode::Enter => {
                // Open the selected result if there are any
                if !search_mode.results.is_empty() {
                    let item = &search_mode.results[search_mode.selected_result];
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
            KeyCode::Backspace => {
                if search_mode.cursor_position > 0 {
                    search_mode.query.remove(search_mode.cursor_position - 1);
                    search_mode.cursor_position -= 1;
                    // Update search results
                    search_mode.results = self.search_notes(&search_mode.query)?;
                    search_mode.selected_result = 0;
                }
                self.mode = Mode::Search(search_mode);
            }
            KeyCode::Left => {
                if search_mode.cursor_position > 0 {
                    search_mode.cursor_position -= 1;
                }
                self.mode = Mode::Search(search_mode);
            }
            KeyCode::Right => {
                if search_mode.cursor_position < search_mode.query.len() {
                    search_mode.cursor_position += 1;
                }
                self.mode = Mode::Search(search_mode);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if search_mode.selected_result > 0 {
                    search_mode.selected_result -= 1;
                }
                self.mode = Mode::Search(search_mode);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if search_mode.selected_result < search_mode.results.len().saturating_sub(1) {
                    search_mode.selected_result += 1;
                }
                self.mode = Mode::Search(search_mode);
            }
            KeyCode::Char(c) => {
                search_mode.query.insert(search_mode.cursor_position, c);
                search_mode.cursor_position += 1;
                // Update search results
                search_mode.results = self.search_notes(&search_mode.query)?;
                search_mode.selected_result = 0;
                self.mode = Mode::Search(search_mode);
            }
            _ => {
                self.mode = Mode::Search(search_mode);
            }
        }
        Ok(())
    }

    /// Start input mode with appropriate prompt
    fn start_input(&mut self, input_type: InputType) {
        let prompt = match input_type {
            InputType::NewNote => "📝 New note title:",
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
            InputType::NewNote => match commands::create_note(&input_mode.input) {
                Ok(_) => {
                    self.show_message(format!("✅ Created note: {}", input_mode.input));
                }
                Err(e) => {
                    self.show_error(format!("Failed to create note: {}", e));
                }
            },
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
                    }];

                    self.mode = Mode::Listing(ListingMode {
                        title: "📚 All Notes".to_string(),
                        items,
                        selected: 0,
                        list_type: ListType::Notes,
                        preview_content: None,
                        preview_scroll: 0,
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
                        });
                    }

                    // Notes are already sorted by timestamp (newest first)

                    // Load preview for the first note if available
                    let preview = if !items.is_empty() {
                        Self::load_note_preview(&items[0].subtitle)
                    } else {
                        None
                    };

                    self.mode = Mode::Listing(ListingMode {
                        title: format!("📚 All Notes ({})", items.len()),
                        items,
                        selected: 0,
                        list_type: ListType::Notes,
                        preview_content: preview,
                        preview_scroll: 0,
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

    /// Search notes and return results
    fn search_notes(&mut self, query: &str) -> Result<Vec<ListItem>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut search_results = Vec::new();

        // Search through all notes and filter by content
        match commands::get_notes_with_paths() {
            Ok(notes) => {
                let query_lower = query.to_lowercase();

                for (path, title, timestamp) in notes {
                    // Try to read the note content
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let content_lower = content.to_lowercase();

                        if content_lower.contains(&query_lower) {
                            // Find all matching lines with their context
                            let lines: Vec<&str> = content.lines().collect();
                            let mut found_match = false;
                            let mut match_context = String::new();
                            let mut match_indices = Vec::new();
                            let mut match_count = 0;

                            for (line_num, line) in lines.iter().enumerate() {
                                if line.to_lowercase().contains(&query_lower) {
                                    found_match = true;
                                    match_count += 1;

                                    // Get the matching line as context with match positions
                                    if match_context.is_empty() {
                                        let (context, indices) =
                                            Self::extract_context_with_matches(line, query, 100);
                                        match_context = context;
                                        match_indices = indices;
                                    }
                                }
                            }

                            if found_match {
                                // Format time ago
                                let time_ago = commands::format_time_ago(timestamp);

                                // Determine note type
                                let note_type = if path.starts_with("dailies/") {
                                    "📅 Daily"
                                } else {
                                    "📝 Note"
                                };

                                search_results.push(ListItem {
                                    title: title.clone(),
                                    subtitle: path,
                                    metadata: format!(
                                        "{} • {} • {} match{}",
                                        note_type,
                                        time_ago,
                                        match_count,
                                        if match_count == 1 { "" } else { "es" }
                                    ),
                                    context: Some(match_context),
                                    match_indices,
                                });
                            }
                        }
                    }
                }

                Ok(search_results)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    /// Perform real-time search with context display (legacy, keeping for compatibility)
    fn perform_realtime_search(&mut self, query: &str) -> Result<()> {
        if query.trim().is_empty() {
            return Ok(());
        }

        Ok(())
    }

    /// Perform search
    fn perform_search(&mut self, query: &str) -> Result<()> {
        // Search through all notes and filter by content
        match commands::get_notes_with_paths() {
            Ok(notes) => {
                let mut search_results = Vec::new();

                // Search through each note's content
                for (path, title, timestamp) in notes {
                    // Try to read the note content
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let query_lower = query.to_lowercase();
                        let content_lower = content.to_lowercase();

                        if content_lower.contains(&query_lower) {
                            // Count occurrences
                            let occurrences = content_lower.matches(&query_lower).count();

                            // Format time ago
                            let time_ago = commands::format_time_ago(timestamp);

                            // Determine note type
                            let note_type = if path.starts_with("dailies/") {
                                "📅 Daily"
                            } else {
                                "📝 Note"
                            };

                            // Find the first matching line for context
                            let lines: Vec<&str> = content.lines().collect();
                            let mut match_context = String::new();
                            let mut match_indices = Vec::new();

                            for line in lines {
                                if line.to_lowercase().contains(&query_lower) {
                                    let (context, indices) =
                                        Self::extract_context_with_matches(line, query, 100);
                                    match_context = context;
                                    match_indices = indices;
                                    break;
                                }
                            }

                            search_results.push(ListItem {
                                title: title.clone(),
                                subtitle: path,
                                metadata: format!(
                                    "{} • {} • {} match{}",
                                    note_type,
                                    time_ago,
                                    occurrences,
                                    if occurrences == 1 { "" } else { "es" }
                                ),
                                context: Some(match_context),
                                match_indices,
                            });
                        }
                    }
                }

                if search_results.is_empty() {
                    self.show_message(format!("No results found for: {}", query));
                } else {
                    self.mode = Mode::Listing(ListingMode {
                        title: format!(
                            "🔍 Search Results for '{}' ({} found)",
                            query,
                            search_results.len()
                        ),
                        items: search_results,
                        selected: 0,
                        list_type: ListType::SearchResults,
                        preview_content: None,
                        preview_scroll: 0,
                    });
                }
            }
            Err(e) => {
                self.show_error(format!("Search failed: {}", e));
            }
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

    /// Extract context line with match positions for highlighting
    fn extract_context_with_matches(
        line: &str,
        query: &str,
        max_len: usize,
    ) -> (String, Vec<(usize, usize)>) {
        let line_lower = line.to_lowercase();
        let query_lower = query.to_lowercase();
        let mut match_positions = Vec::new();

        // Find all match positions in the original line
        let mut start = 0;
        while let Some(pos) = line_lower[start..].find(&query_lower) {
            let absolute_pos = start + pos;
            match_positions.push((absolute_pos, absolute_pos + query.len()));
            start = absolute_pos + 1;
        }

        // If line is too long, try to center around first match
        let context = if line.len() > max_len && !match_positions.is_empty() {
            let first_match = match_positions[0].0;
            let context_start = first_match.saturating_sub(max_len / 3);
            let context_end = (context_start + max_len).min(line.len());

            // Adjust match positions for the truncated context
            let adjusted_positions: Vec<(usize, usize)> = match_positions
                .iter()
                .filter_map(|(start, end)| {
                    if *start >= context_start && *end <= context_end {
                        Some((*start - context_start, *end - context_start))
                    } else {
                        None
                    }
                })
                .collect();

            let mut context_str = String::new();
            if context_start > 0 {
                context_str.push_str("...");
            }
            context_str.push_str(&line[context_start..context_end]);
            if context_end < line.len() {
                context_str.push_str("...");
            }

            (context_str, adjusted_positions)
        } else {
            (line.to_string(), match_positions)
        };

        context
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
}
