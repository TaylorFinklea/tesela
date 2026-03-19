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
    let dialog = centered_rect(60, 20, area);
    f.render_widget(Clear, dialog);

    let outer = Block::default()
        .title(format!(
            " {} Find Note (Ctrl+P) ",
            crate::theme::icons::SEARCH
        ))
        .title_style(Style::default().fg(T.accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(T.accent));
    f.render_widget(outer, dialog);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(dialog);

    // Query input line
    let input_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(T.accent)),
        Span::styled(state.fuzzy.query.as_str(), Style::default().fg(T.text)),
        Span::styled("█", Style::default().fg(T.accent)),
    ]);
    let input = Paragraph::new(input_line);
    f.render_widget(input, inner_chunks[0]);

    // Results list with per-character fuzzy match highlighting
    let items: Vec<ListItem> = state
        .fuzzy
        .matches
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let indices = state.fuzzy.match_indices.get(i);
            let line = highlight_matches(&n.title, indices);
            ListItem::new(line)
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
    if !state.fuzzy.matches.is_empty() {
        list_state.select(Some(state.fuzzy.selected));
    }

    f.render_stateful_widget(list, inner_chunks[1], &mut list_state);
}

/// Highlight matched character positions in the title.
fn highlight_matches<'a>(title: &str, indices: Option<&Vec<usize>>) -> Line<'a> {
    let Some(indices) = indices else {
        return Line::from(title.to_string());
    };
    if indices.is_empty() {
        return Line::from(title.to_string());
    }

    let match_style = Style::default().fg(T.accent).add_modifier(Modifier::BOLD);
    let normal_style = Style::default().fg(T.text);

    let mut spans = Vec::new();
    let mut last_end = 0;

    let chars: Vec<char> = title.chars().collect();
    for &idx in indices {
        if idx >= chars.len() {
            continue;
        }
        // Add non-matched chars before this match
        if idx > last_end {
            let segment: String = chars[last_end..idx].iter().collect();
            spans.push(Span::styled(segment, normal_style));
        }
        // Add the matched char
        spans.push(Span::styled(chars[idx].to_string(), match_style));
        last_end = idx + 1;
    }
    // Remaining chars after last match
    if last_end < chars.len() {
        let segment: String = chars[last_end..].iter().collect();
        spans.push(Span::styled(segment, normal_style));
    }

    Line::from(spans)
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = (area.width * percent_x / 100).max(30).min(area.width);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height.min(area.height))
}
