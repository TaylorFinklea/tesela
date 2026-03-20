use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders},
    Frame,
};
use tui_textarea::TextArea;

use crate::state::AppState;
use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect, state: &AppState, textarea: &Option<TextArea<'static>>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(area);

    let title = state
        .current_note
        .as_ref()
        .map(|n| n.title.as_str())
        .unwrap_or("Untitled");

    if let Some(ta) = textarea {
        let mut ta = ta.clone();
        ta.set_block(
            Block::default()
                .title(format!(
                    " {} Editing: {} ",
                    crate::theme::icons::PENCIL,
                    title
                ))
                .title_style(Style::default().fg(T.accent).add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(T.accent)),
        );
        f.render_widget(&ta, chunks[0]);
    }
}
