use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Search input
    let input = Paragraph::new(state.search.query.as_str())
        .block(
            Block::default()
                .title(" Search ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(T.accent)),
        )
        .style(Style::default().fg(T.text));
    f.render_widget(input, chunks[0]);

    // Results
    let items: Vec<ListItem> = state
        .search
        .results
        .iter()
        .map(|hit| {
            let text = if hit.snippet.is_empty() {
                hit.title.clone()
            } else {
                format!("{} — {}", hit.title, hit.snippet)
            };
            ListItem::new(text)
        })
        .collect();

    let mut list_state = ListState::default();
    if !state.search.results.is_empty() {
        list_state.select(Some(state.search.selected));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Results ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(T.text_dim)),
        )
        .highlight_style(
            Style::default()
                .fg(T.selection_fg)
                .bg(T.selection_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, chunks[1], &mut list_state);
}
