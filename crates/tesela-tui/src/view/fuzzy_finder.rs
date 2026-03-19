use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::state::AppState;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let dialog = centered_rect(60, 20, area);
    f.render_widget(Clear, dialog);

    let outer = Block::default()
        .title(" Find Note (Ctrl+P) ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(outer, dialog);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(dialog);

    // Query input line
    let input_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Yellow)),
        Span::styled(
            state.fuzzy.query.as_str(),
            Style::default().fg(Color::White),
        ),
        Span::styled("█", Style::default().fg(Color::Yellow)),
    ]);
    let input = Paragraph::new(input_line);
    f.render_widget(input, inner_chunks[0]);

    // Results list
    let items: Vec<ListItem> = state
        .fuzzy
        .matches
        .iter()
        .map(|n| ListItem::new(n.title.as_str()))
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !state.fuzzy.matches.is_empty() {
        list_state.select(Some(state.fuzzy.selected));
    }

    f.render_stateful_widget(list, inner_chunks[1], &mut list_state);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = (area.width * percent_x / 100).max(30).min(area.width);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height.min(area.height))
}
