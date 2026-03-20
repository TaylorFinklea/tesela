use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let dialog = centered_rect(50, 18, area);
    f.render_widget(Clear, dialog);

    let outer = Block::default()
        .title(format!(" {} Filter by Tag ", crate::theme::icons::TAG))
        .title_style(Style::default().fg(T.tag).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(T.tag));
    f.render_widget(outer, dialog);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(dialog);

    // Query input line
    let input_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(T.tag)),
        Span::styled(state.tag_picker.query.as_str(), Style::default().fg(T.text)),
        Span::styled("█", Style::default().fg(T.tag)),
    ]);
    let input = Paragraph::new(input_line);
    f.render_widget(input, inner_chunks[0]);

    // Tag list
    let items: Vec<ListItem> = state
        .tag_picker
        .filtered
        .iter()
        .map(|tag| {
            let style = if tag == "(all)" {
                Style::default()
                    .fg(T.text_dim)
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(T.tag)
            };
            ListItem::new(Span::styled(tag.as_str(), style))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .fg(T.selection_fg)
                .bg(T.selection_bg)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default();
    if !state.tag_picker.filtered.is_empty() {
        list_state.select(Some(state.tag_picker.selected));
    }

    f.render_stateful_widget(list, inner_chunks[1], &mut list_state);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = (area.width * percent_x / 100).max(30).min(area.width);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height.min(area.height))
}
