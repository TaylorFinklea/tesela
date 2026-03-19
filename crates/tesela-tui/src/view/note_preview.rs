use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::state::AppState;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let (title, body) = state
        .current_note
        .as_ref()
        .map(|n| (n.title.as_str(), n.body.as_str()))
        .unwrap_or(("No note selected", ""));

    let para = Paragraph::new(body)
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .scroll((state.listing.scroll_offset as u16, 0));

    f.render_widget(para, area);
}
