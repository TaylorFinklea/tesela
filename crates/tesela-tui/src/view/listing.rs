use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::state::AppState;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Left: note list
    let items: Vec<ListItem> = state
        .listing
        .notes
        .iter()
        .map(|n| ListItem::new(n.title.as_str()))
        .collect();

    let mut list_state = ListState::default();
    if !state.listing.notes.is_empty() {
        list_state.select(Some(state.listing.selected));
    }

    let list = List::new(items)
        .block(Block::default().title("Notes").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Right: preview
    let preview_text = state
        .listing
        .selected_note()
        .map(|n| n.body.as_str())
        .unwrap_or("");

    let preview = Paragraph::new(preview_text)
        .block(Block::default().title("Preview").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    f.render_widget(preview, chunks[1]);
}
