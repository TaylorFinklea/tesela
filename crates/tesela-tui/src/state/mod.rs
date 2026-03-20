pub mod listing;
pub mod mode;
pub mod search;

use std::path::PathBuf;
use tesela_core::link::Link;
use tesela_core::note::{Note, NoteId};

#[derive(Debug, Clone, Default)]
pub struct FuzzyState {
    pub active: bool,
    pub query: String,
    pub matches: Vec<Note>,
    /// Per-match character indices that were matched by the fuzzy query
    pub match_indices: Vec<Vec<usize>>,
    pub selected: usize,
}

impl FuzzyState {
    pub fn activate(&mut self, all_notes: Vec<Note>) {
        self.active = true;
        self.query = String::new();
        self.match_indices = Vec::new();
        self.matches = all_notes;
        self.selected = 0;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.query = String::new();
        self.matches = Vec::new();
        self.match_indices = Vec::new();
        self.selected = 0;
    }

    pub fn selected_note(&self) -> Option<&Note> {
        self.matches.get(self.selected)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TagPickerState {
    pub active: bool,
    pub query: String,
    pub all_tags: Vec<String>,
    pub filtered: Vec<String>,
    pub selected: usize,
}

impl TagPickerState {
    pub fn activate(&mut self, tags: Vec<String>) {
        self.active = true;
        self.query = String::new();
        self.all_tags = tags;
        self.filtered = Vec::new(); // populated by filter()
        self.selected = 0;
        self.filter();
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.query = String::new();
        self.all_tags = Vec::new();
        self.filtered = Vec::new();
        self.selected = 0;
    }

    pub fn filter(&mut self) {
        // First item is always "(all)" to clear filter
        let mut result = vec!["(all)".to_string()];
        if self.query.is_empty() {
            result.extend(self.all_tags.iter().cloned());
        } else {
            let q = self.query.to_lowercase();
            result.extend(
                self.all_tags
                    .iter()
                    .filter(|t| t.to_lowercase().contains(&q))
                    .cloned(),
            );
        }
        self.filtered = result;
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub fn selected_tag(&self) -> Option<&str> {
        self.filtered.get(self.selected).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub mode: mode::Mode,
    pub listing: listing::ListingState,
    pub search: search::SearchState,
    pub current_note: Option<Note>,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    /// Text buffer for the NewNote input mode
    pub new_note_input: String,
    /// Fuzzy finder overlay state
    pub fuzzy: FuzzyState,
    /// Tag picker overlay state
    pub tag_picker: TagPickerState,
    /// Set by process_action; consumed by run() to spawn an external editor
    pub pending_editor: Option<(PathBuf, NoteId)>,
    /// Whether the NoteView is showing the graph (backlinks) instead of content
    pub graph_view_active: bool,
    /// Backlinks for the current note (loaded when entering GraphView mode)
    pub graph_backlinks: Vec<Link>,
    /// Forward links for the current note (loaded when entering GraphView mode)
    pub graph_forward_links: Vec<Link>,
    /// Help overlay is visible (drawn on top of current mode)
    pub help_active: bool,
    /// Armed delete confirmation — first D press sets this, second D executes
    pub confirm_delete: Option<NoteId>,
}
