use ratatui::{layout::Rect, style::Style, widgets::Paragraph, Frame};

use crate::state::{mode::Mode, AppState};
use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let msg = if state.fuzzy.active {
        "↑↓: navigate  Enter: open  Esc: close".to_string()
    } else if let Some(err) = &state.error_message {
        format!("Error: {}", err)
    } else if let Some(msg) = &state.status_message {
        msg.clone()
    } else {
        match state.mode {
            Mode::MainMenu => {
                "c: new  n: notes  d: daily  /: search  ^P: find  q: quit  ?: help".to_string()
            }
            Mode::Listing => {
                "j/k: navigate  Enter: open  c: new  /: search  ^P: find  Esc: back".to_string()
            }
            Mode::Search => "type to search  Enter: confirm  Esc: cancel".to_string(),
            Mode::NoteView => {
                "e: edit  g: graph  D: delete  j/k: scroll  ^P: find  Esc: back".to_string()
            }
            Mode::GraphView => "g: toggle  e: edit  j/k: scroll  Esc: back".to_string(),
            Mode::NewNote => "type title  Enter: create  Esc: cancel".to_string(),
            Mode::Help => "?: close help".to_string(),
        }
    };

    let style = if state.error_message.is_some() && !state.fuzzy.active {
        Style::default().fg(T.error).bg(T.status_bg)
    } else {
        Style::default().fg(T.text_dim).bg(T.status_bg)
    };

    let para = Paragraph::new(msg).style(style);
    f.render_widget(para, area);
}
