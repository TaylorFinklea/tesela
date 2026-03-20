use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::state::AppState;
use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Search input with cursor
    let input_line = Line::from(vec![
        Span::styled(state.search.query.as_str(), Style::default().fg(T.text)),
        Span::styled("█", Style::default().fg(T.accent)),
    ]);
    let input = Paragraph::new(input_line).block(
        Block::default()
            .title(format!(" {} Search ", crate::theme::icons::SEARCH))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(T.accent)),
    );
    f.render_widget(input, chunks[0]);

    // Bottom: results list (40%) + preview (60%)
    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    // Results list
    let result_count = state.search.results.len();
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

    let results_title = if result_count > 0 {
        format!(" {} Results ({result_count}) ", crate::theme::icons::NOTE)
    } else {
        format!(" {} Results ", crate::theme::icons::NOTE)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(results_title)
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

    f.render_stateful_widget(list, bottom[0], &mut list_state);

    // Preview pane: show selected hit's snippet
    let preview_text = if !state.search.results.is_empty() {
        let hit = &state.search.results[state.search.selected];
        if hit.snippet.is_empty() {
            hit.title.clone()
        } else {
            hit.snippet.clone()
        }
    } else if state.search.query.len() < 2 {
        "Type 2+ characters to search…".to_string()
    } else {
        "No results".to_string()
    };

    let preview = Paragraph::new(preview_text)
        .block(
            Block::default()
                .title(" Preview ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(T.text_dim)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(preview, bottom[1]);
}
