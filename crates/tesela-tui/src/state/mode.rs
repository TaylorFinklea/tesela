#[derive(Debug, Clone, PartialEq, Default)]
pub enum Mode {
    #[default]
    MainMenu,
    Listing,
    Search,
    NoteView,
    GraphView,
    NewNote,
}
