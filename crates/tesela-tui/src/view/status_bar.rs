use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

use crate::state::{mode::Mode, AppState};

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let msg = if let Some(err) = &state.error_message {
        format!("Error: {}", err)
    } else if let Some(msg) = &state.status_message {
        msg.clone()
    } else {
        match state.mode {
            Mode::MainMenu => "n: notes | /: search | q: quit | ?: help".to_string(),
            Mode::Listing => "j/k: navigate | Enter: open | /: search | Esc: back".to_string(),
            Mode::Search => "Type to search | Enter: confirm | Esc: cancel".to_string(),
            Mode::NoteView => "j/k: scroll | Esc: back".to_string(),
            Mode::Help => "?: close help".to_string(),
        }
    };

    let style = if state.error_message.is_some() {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let para = Paragraph::new(msg).style(style);
    f.render_widget(para, area);
}
