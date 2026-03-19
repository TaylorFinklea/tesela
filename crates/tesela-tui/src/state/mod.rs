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
    /// Set by process_action; consumed by run() to spawn an external editor
    pub pending_editor: Option<(PathBuf, NoteId)>,
    /// Whether the NoteView is showing the graph (backlinks) instead of content
    pub graph_view_active: bool,
    /// Backlinks for the current note (loaded when entering GraphView mode)
    pub graph_backlinks: Vec<Link>,
    /// Forward links for the current note (loaded when entering GraphView mode)
    pub graph_forward_links: Vec<Link>,
}
