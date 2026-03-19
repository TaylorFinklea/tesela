use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::state::AppState;
use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let has_selection = !state.listing.notes.is_empty();

    // Left: note list
    let items: Vec<ListItem> = state
        .listing
        .notes
        .iter()
        .map(|n| ListItem::new(n.title.as_str()))
        .collect();

    let mut list_state = ListState::default();
    if has_selection {
        list_state.select(Some(state.listing.selected));
    }

    let left_border_color = if has_selection { T.accent } else { T.text_dim };
    let note_count = state.listing.notes.len();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" Notes ({note_count}) "))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(left_border_color)),
        )
        .highlight_style(
            Style::default()
                .fg(T.selection_fg)
                .bg(T.selection_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, chunks[0], &mut list_state);

    // Right: preview
    let preview_text = state
        .listing
        .selected_note()
        .map(|n| n.body.as_str())
        .unwrap_or("");

    let preview = Paragraph::new(preview_text)
        .block(
            Block::default()
                .title(" Preview ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(T.text_dim)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(preview, chunks[1]);
}
