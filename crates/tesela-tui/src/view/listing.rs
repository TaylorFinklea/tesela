use chrono::{DateTime, Utc};
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
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let has_selection = !state.listing.notes.is_empty();
    let now = Utc::now();

    // Left: note list with relative timestamps
    let items: Vec<ListItem> = state
        .listing
        .notes
        .iter()
        .map(|n| {
            let age = format_relative_time(now, n.modified_at);
            let line = Line::from(vec![
                Span::raw(n.title.clone()),
                Span::raw("  "),
                Span::styled(age, Style::default().fg(T.text_dim)),
            ]);
            ListItem::new(line)
        })
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

fn format_relative_time(now: DateTime<Utc>, then: DateTime<Utc>) -> String {
    let duration = now.signed_duration_since(then);
    let secs = duration.num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = duration.num_minutes();
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = duration.num_hours();
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = duration.num_days();
    if days < 7 {
        return format!("{days}d ago");
    }
    then.format("%b %d").to_string()
}
