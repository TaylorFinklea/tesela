use tesela_core::note::NoteId;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Action {
    // Navigation
    Quit,
    EnterMode(crate::state::mode::Mode),

    // Note operations
    CreateNote { title: String },
    OpenNote(NoteId),
    RefreshList,

    // Search
    UpdateSearchQuery(String),
    ExecuteSearch(String),
    ClearSearch,

    // UI
    ScrollUp,
    ScrollDown,
    SelectNext,
    SelectPrev,
    SelectItem(usize),

    // Status
    ShowMessage(String),
    ShowError(String),
}
