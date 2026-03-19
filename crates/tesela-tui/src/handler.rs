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
    // Global shortcuts (work in any mode)
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            return vec![Action::Quit];
        }
        (_, KeyCode::Char('?')) if state.mode != Mode::Search => {
            return vec![Action::EnterMode(Mode::Help)];
        }
        _ => {}
    }

    // Mode-specific handling
    match &state.mode {
        Mode::MainMenu => handle_main_menu(key),
        Mode::Listing => handle_listing(state, key),
        Mode::Search => handle_search(state, key),
        Mode::NoteView => handle_note_view(key),
        Mode::Help => handle_help(key),
    }
}

fn handle_main_menu(key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Char('n') => vec![Action::EnterMode(Mode::Listing), Action::RefreshList],
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
        KeyCode::Char('/') => vec![Action::EnterMode(Mode::Search)],
        KeyCode::Esc | KeyCode::Char('q') => vec![Action::EnterMode(Mode::MainMenu)],
        _ => vec![],
    }
}

fn handle_search(state: &AppState, key: &KeyEvent) -> Vec<Action> {
    match key.code {
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
            if !state.search.query.is_empty() {
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

fn handle_note_view(key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => vec![Action::EnterMode(Mode::Listing)],
        KeyCode::Char('j') | KeyCode::Down => vec![Action::ScrollDown],
        KeyCode::Char('k') | KeyCode::Up => vec![Action::ScrollUp],
        _ => vec![],
    }
}

fn handle_help(key: &KeyEvent) -> Vec<Action> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('?') => {
            vec![Action::EnterMode(Mode::MainMenu)]
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

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn test_quit_from_main_menu() {
        let state = AppState::default(); // default mode is MainMenu
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
    fn test_search_updates_query() {
        let mut state = AppState::default();
        state.mode = Mode::Search;

        let actions = handle(&state, &key(KeyCode::Char('r')));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::UpdateSearchQuery(q) if q.contains('r'))));
    }

    #[test]
    fn test_esc_returns_to_main_menu_from_search() {
        let mut state = AppState::default();
        state.mode = Mode::Search;

        let actions = handle(&state, &key(KeyCode::Esc));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EnterMode(Mode::MainMenu))));
    }

    #[test]
    fn test_nav_in_listing() {
        let mut state = AppState::default();
        state.mode = Mode::Listing;

        let down = handle(&state, &key(KeyCode::Char('j')));
        assert!(down.iter().any(|a| matches!(a, Action::SelectNext)));

        let up = handle(&state, &key(KeyCode::Char('k')));
        assert!(up.iter().any(|a| matches!(a, Action::SelectPrev)));
    }

    #[test]
    fn test_help_toggle() {
        let state = AppState::default();
        let actions = handle(&state, &key(KeyCode::Char('?')));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::EnterMode(Mode::Help))));
    }
}
