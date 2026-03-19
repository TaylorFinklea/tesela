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
    EditNote(NoteId),
    DeleteNote(NoteId),
    ConfirmDelete(NoteId),
    CancelDelete,
    OpenDailyNote,
    OpenNextNote,
    OpenPrevNote,
    RefreshList,

    // New note input
    NewNoteInput(String),

    // Search
    UpdateSearchQuery(String),
    ExecuteSearch(String),
    ClearSearch,

    // Fuzzy finder
    ToggleFuzzy,
    FuzzyQuery(String),
    FuzzySelect,
    FuzzySelectNext,
    FuzzySelectPrev,

    // UI
    ScrollUp,
    ScrollDown,
    SelectNext,
    SelectPrev,
    SelectItem(usize),
    ToggleGraphView,
    ToggleHelp,

    // Status
    ShowMessage(String),
    ShowError(String),
}
