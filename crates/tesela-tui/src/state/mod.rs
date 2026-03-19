pub mod listing;
pub mod mode;
pub mod search;

use tesela_core::note::Note;

#[derive(Debug, Clone, Default)]
pub struct AppState {
    pub mode: mode::Mode,
    pub listing: listing::ListingState,
    pub search: search::SearchState,
    pub current_note: Option<Note>,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
}
