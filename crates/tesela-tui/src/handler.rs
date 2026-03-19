use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::Action;
use crate::event::Event;
use crate::state::mode::Mode;
use crate::state::AppState;

/// Pure function: given current state and an event, return actions to take.
/// No side effects. Testable without terminal.
pub fn handle(state: &AppState, event: &Event) -> Vec<Action> {
    match event {
        Event::Key(key) => handle_key(state, key),
        Event::Tick => handle_tick(state),
        Event::Resize(_, _) => vec![],
    }
}

fn handle_key(state: &AppState, key: &KeyEvent) -> Vec<Action> {
    // Help overlay captures all input when active
    if state.help_active {
        return handle_help(key);
    }

    // Fuzzy finder captures all input when active
    if state.fuzzy.active {
        return handle_fuzzy(state, key);
    }

    // Global shortcuts (work in any mode)
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            return vec![Action::Quit];
        }
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
            return vec![Action::ToggleFuzzy];
        }
        (_, KeyCode::Char('?')) if state.mode != Mode::Search && state.mode != Mode::NewNote => {
            return vec![Action::ToggleHelp];
        }
        _ => {}
    }

    // Mode-specific handling
    match &state.mode {
        Mode::MainMenu => handle_main_menu(key),
        Mode::Listing => handle_listing(state, key),
        Mode::Search => handle_search(state, key),
        Mode::NoteView | Mode::GraphView => handle_note_view(state, key),
        Mode::NewNote => handle_new_note(state, key),
    }
}

fn handle_main_menu(key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Char('n') => vec![Action::EnterMode(Mode::Listing), Action::RefreshList],
        KeyCode::Char('c') => vec![Action::EnterMode(Mode::NewNote)],
        KeyCode::Char('d') => vec![Action::OpenDailyNote],
        KeyCode::Char('/') | KeyCode::Char('s') => vec![Action::EnterMode(Mode::Search)],
        KeyCode::Char('q') | KeyCode::Esc => vec![Action::Quit],
        _ => vec![],
    }
}

fn handle_listing(state: &AppState, key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => vec![Action::SelectNext],
        KeyCode::Char('k') | KeyCode::Up => vec![Action::SelectPrev],
        KeyCode::Enter => {
            if let Some(note) = state.listing.selected_note() {
                vec![
                    Action::OpenNote(note.id.clone()),
                    Action::EnterMode(Mode::NoteView),
                ]
            } else {
                vec![]
            }
        }
        KeyCode::Char('c') => vec![Action::EnterMode(Mode::NewNote)],
        KeyCode::Char('/') => vec![Action::EnterMode(Mode::Search)],
        KeyCode::Esc | KeyCode::Char('q') => vec![Action::EnterMode(Mode::MainMenu)],
        _ => vec![],
    }
}

fn handle_search(state: &AppState, key: &KeyEvent) -> Vec<Action> {
    let has_results = !state.search.results.is_empty();
    match key.code {
        // j/k navigate results when populated; otherwise fall through to type into query
        KeyCode::Char('j') | KeyCode::Down if has_results => vec![Action::SelectNext],
        KeyCode::Char('k') | KeyCode::Up if has_results => vec![Action::SelectPrev],
        KeyCode::Char(c) => {
            vec![Action::UpdateSearchQuery(format!(
                "{}{}",
                state.search.query, c
            ))]
        }
        KeyCode::Backspace => {
            let mut q = state.search.query.clone();
            q.pop();
            vec![Action::UpdateSearchQuery(q)]
        }
        KeyCode::Enter => {
            if has_results {
                // Open the selected search result
                let hit = &state.search.results[state.search.selected];
                vec![
                    Action::OpenNote(hit.note_id.clone()),
                    Action::EnterMode(Mode::NoteView),
                ]
            } else if !state.search.query.is_empty() {
                vec![Action::ExecuteSearch(state.search.query.clone())]
            } else {
                vec![]
            }
        }
        KeyCode::Esc => vec![Action::ClearSearch, Action::EnterMode(Mode::MainMenu)],
        KeyCode::Down => vec![Action::SelectNext],
        KeyCode::Up => vec![Action::SelectPrev],
        _ => vec![],
    }
}

fn handle_note_view(state: &AppState, key: &KeyEvent) -> Vec<Action> {
    // Cancel pending delete on any key except D
    if state.confirm_delete.is_some() && key.code != KeyCode::Char('D') {
        let mut actions = vec![Action::CancelDelete];
        actions.extend(handle_note_view_keys(state, key));
        return actions;
    }
    handle_note_view_keys(state, key)
}

fn handle_note_view_keys(state: &AppState, key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            vec![Action::EnterMode(Mode::Listing), Action::RefreshList]
        }
        KeyCode::Char('j') | KeyCode::Down => vec![Action::ScrollDown],
        KeyCode::Char('k') | KeyCode::Up => vec![Action::ScrollUp],
        KeyCode::Char('e') => {
            if let Some(note) = &state.current_note {
                vec![Action::EditNote(note.id.clone())]
            } else {
                vec![]
            }
        }
        KeyCode::Char('g') => vec![Action::ToggleGraphView],
        KeyCode::Char(']') => vec![Action::OpenNextNote],
        KeyCode::Char('[') => vec![Action::OpenPrevNote],
        KeyCode::Char('D') => {
            if let Some(pending_id) = &state.confirm_delete {
                // Second press: execute delete
                vec![Action::DeleteNote(pending_id.clone())]
            } else if let Some(note) = &state.current_note {
                // First press: arm confirmation
                vec![Action::ConfirmDelete(note.id.clone())]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

fn handle_new_note(state: &AppState, key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Enter => {
            let title = state.new_note_input.trim().to_string();
            if title.is_empty() {
                vec![Action::EnterMode(Mode::MainMenu)]
            } else {
                vec![Action::CreateNote { title }]
            }
        }
        KeyCode::Esc => vec![
            Action::NewNoteInput(String::new()),
            Action::EnterMode(Mode::MainMenu),
        ],
        KeyCode::Backspace => {
            let mut s = state.new_note_input.clone();
            s.pop();
            vec![Action::NewNoteInput(s)]
        }
        KeyCode::Char(c) => {
            vec![Action::NewNoteInput(format!(
                "{}{}",
                state.new_note_input, c
            ))]
        }
        _ => vec![],
    }
}

fn handle_fuzzy(state: &AppState, key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Esc => vec![Action::ToggleFuzzy],
        KeyCode::Enter => vec![Action::FuzzySelect],
        KeyCode::Down => vec![Action::FuzzySelectNext],
        KeyCode::Up => vec![Action::FuzzySelectPrev],
        KeyCode::Backspace => {
            let mut q = state.fuzzy.query.clone();
            q.pop();
            vec![Action::FuzzyQuery(q)]
        }
        KeyCode::Char(c) => {
            vec![Action::FuzzyQuery(format!("{}{}", state.fuzzy.query, c))]
        }
        _ => vec![],
    }
}

fn handle_help(key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
            vec![Action::ToggleHelp]
        }
        _ => vec![],
    }
}

fn handle_tick(_state: &AppState) -> Vec<Action> {
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tesela_core::note::NoteId;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ctrl(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL))
    }

    #[test]
    fn test_quit_from_main_menu() {
        let state = AppState::default();
        let actions = handle(&state, &key(KeyCode::Char('q')));
        assert!(actions.iter().any(|a| matches!(a, Action::Quit)));
    }

    #[test]
    fn test_enter_listing_from_main_menu() {
        let state = AppState::default();
        let actions = handle(&state, &key(KeyCode::Char('n')));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EnterMode(Mode::Listing))));
    }

    #[test]
    fn test_enter_search_from_main_menu() {
        let state = AppState::default();
        let actions = handle(&state, &key(KeyCode::Char('/')));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EnterMode(Mode::Search))));
    }

    #[test]
    fn test_enter_new_note_from_main_menu() {
        let state = AppState::default();
        let actions = handle(&state, &key(KeyCode::Char('c')));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EnterMode(Mode::NewNote))));
    }

    #[test]
    fn test_open_daily_from_main_menu() {
        let state = AppState::default();
        let actions = handle(&state, &key(KeyCode::Char('d')));
        assert!(actions.iter().any(|a| matches!(a, Action::OpenDailyNote)));
    }

    #[test]
    fn test_search_updates_query() {
        let state = AppState {
            mode: Mode::Search,
            ..AppState::default()
        };

        let actions = handle(&state, &key(KeyCode::Char('r')));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::UpdateSearchQuery(q) if q.contains('r'))));
    }

    #[test]
    fn test_esc_returns_to_main_menu_from_search() {
        let state = AppState {
            mode: Mode::Search,
            ..AppState::default()
        };

        let actions = handle(&state, &key(KeyCode::Esc));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EnterMode(Mode::MainMenu))));
    }

    #[test]
    fn test_nav_in_listing() {
        let state = AppState {
            mode: Mode::Listing,
            ..AppState::default()
        };

        let down = handle(&state, &key(KeyCode::Char('j')));
        assert!(down.iter().any(|a| matches!(a, Action::SelectNext)));

        let up = handle(&state, &key(KeyCode::Char('k')));
        assert!(up.iter().any(|a| matches!(a, Action::SelectPrev)));
    }

    #[test]
    fn test_help_toggle() {
        let state = AppState::default();
        let actions = handle(&state, &key(KeyCode::Char('?')));
        assert!(actions.iter().any(|a| matches!(a, Action::ToggleHelp)));
    }

    #[test]
    fn test_help_overlay_captures_input() {
        let mut state = AppState::default();
        state.help_active = true;
        // Normal mode shortcuts should not fire; help captures all input
        let actions = handle(&state, &key(KeyCode::Char('q')));
        assert!(actions.iter().any(|a| matches!(a, Action::ToggleHelp)));
        assert!(!actions.iter().any(|a| matches!(a, Action::Quit)));
    }

    #[test]
    fn test_ctrl_p_opens_fuzzy() {
        let state = AppState::default();
        let actions = handle(&state, &ctrl(KeyCode::Char('p')));
        assert!(actions.iter().any(|a| matches!(a, Action::ToggleFuzzy)));
    }

    #[test]
    fn test_fuzzy_captures_input_when_active() {
        let mut state = AppState::default();
        state.fuzzy.active = true;
        // Normal mode shortcuts should not fire; fuzzy handler takes over
        let actions = handle(&state, &key(KeyCode::Char('q')));
        // 'q' in fuzzy mode appends to query, not quit
        assert!(!actions.iter().any(|a| matches!(a, Action::Quit)));
    }

    #[test]
    fn test_note_view_edit() {
        use chrono::Utc;
        use std::path::PathBuf;
        use tesela_core::note::{Note, NoteId, NoteMetadata};
        let mut state = AppState {
            mode: Mode::NoteView,
            ..AppState::default()
        };
        let id = NoteId::new("test-note");
        let note = Note {
            id: id.clone(),
            title: "Test".to_string(),
            content: String::new(),
            body: String::new(),
            metadata: NoteMetadata::default(),
            path: PathBuf::from("notes/test-note.md"),
            checksum: String::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: vec![],
        };
        state.current_note = Some(note);
        let actions = handle(&state, &key(KeyCode::Char('e')));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EditNote(action_id) if action_id == &id)));
    }

    #[test]
    fn test_toggle_graph_view() {
        let state = AppState {
            mode: Mode::NoteView,
            ..AppState::default()
        };
        let actions = handle(&state, &key(KeyCode::Char('g')));
        assert!(actions.iter().any(|a| matches!(a, Action::ToggleGraphView)));
    }

    #[test]
    fn test_delete_first_press_arms_confirmation() {
        let state = make_note_view_state();
        let actions = handle(&state, &key(KeyCode::Char('D')));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::ConfirmDelete(_))));
        assert!(!actions.iter().any(|a| matches!(a, Action::DeleteNote(_))));
    }

    #[test]
    fn test_delete_second_press_executes() {
        let mut state = make_note_view_state();
        state.confirm_delete = Some(NoteId::new("test-note"));
        let actions = handle(&state, &key(KeyCode::Char('D')));
        assert!(actions.iter().any(|a| matches!(a, Action::DeleteNote(_))));
    }

    #[test]
    fn test_delete_cancel_on_other_key() {
        let mut state = make_note_view_state();
        state.confirm_delete = Some(NoteId::new("test-note"));
        let actions = handle(&state, &key(KeyCode::Char('j')));
        assert!(actions.iter().any(|a| matches!(a, Action::CancelDelete)));
        // Should also process the key normally (scroll down)
        assert!(actions.iter().any(|a| matches!(a, Action::ScrollDown)));
    }

    #[test]
    fn test_search_enter_opens_result() {
        use std::path::PathBuf;
        use tesela_core::note::{NoteId, SearchHit};
        let mut state = AppState {
            mode: Mode::Search,
            ..AppState::default()
        };
        state.search.query = "test".to_string();
        state.search.results = vec![SearchHit {
            note_id: NoteId::new("hit-1"),
            title: "Hit 1".to_string(),
            snippet: String::new(),
            rank: 1.0,
            tags: vec![],
            path: PathBuf::from("notes/hit-1.md"),
        }];
        let actions = handle(&state, &key(KeyCode::Enter));
        assert!(actions.iter().any(|a| matches!(a, Action::OpenNote(_))));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EnterMode(Mode::NoteView))));
    }

    #[test]
    fn test_search_enter_executes_when_no_results() {
        let mut state = AppState {
            mode: Mode::Search,
            ..AppState::default()
        };
        state.search.query = "test".to_string();
        let actions = handle(&state, &key(KeyCode::Enter));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::ExecuteSearch(_))));
    }

    #[test]
    fn test_esc_from_note_view_refreshes_list() {
        let state = AppState {
            mode: Mode::NoteView,
            ..AppState::default()
        };
        let actions = handle(&state, &key(KeyCode::Esc));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EnterMode(Mode::Listing))));
        assert!(actions.iter().any(|a| matches!(a, Action::RefreshList)));
    }

    #[test]
    fn test_next_prev_note_from_note_view() {
        let state = AppState {
            mode: Mode::NoteView,
            ..AppState::default()
        };
        let actions = handle(&state, &key(KeyCode::Char(']')));
        assert!(actions.iter().any(|a| matches!(a, Action::OpenNextNote)));
        let actions = handle(&state, &key(KeyCode::Char('[')));
        assert!(actions.iter().any(|a| matches!(a, Action::OpenPrevNote)));
    }

    fn make_note_view_state() -> AppState {
        use chrono::Utc;
        use std::path::PathBuf;
        use tesela_core::note::{Note, NoteMetadata};
        let mut state = AppState {
            mode: Mode::NoteView,
            ..AppState::default()
        };
        state.current_note = Some(Note {
            id: NoteId::new("test-note"),
            title: "Test".to_string(),
            content: String::new(),
            body: String::new(),
            metadata: NoteMetadata::default(),
            path: PathBuf::from("notes/test-note.md"),
            checksum: String::new(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: vec![],
        });
        state
    }
}
