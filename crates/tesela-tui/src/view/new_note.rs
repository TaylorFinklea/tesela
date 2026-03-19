use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::state::AppState;
use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    // Dim background by rendering a blank
    let block = Block::default()
        .title(" Tesela ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().fg(T.text_dim));
    f.render_widget(block, area);

    // Center the input dialog
    let dialog = centered_rect(50, 7, area);
    f.render_widget(Clear, dialog);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(dialog);

    let outer = Block::default()
        .title(" New Note ")
        .title_style(Style::default().fg(T.accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(T.accent));
    f.render_widget(outer, dialog);

    let prompt = Paragraph::new("Title:").style(Style::default().fg(T.text_dim));
    f.render_widget(prompt, chunks[1]);

    let input_line = Line::from(vec![
        Span::styled(
            state.new_note_input.as_str(),
            Style::default().fg(T.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(T.accent)),
    ]);
    let input = Paragraph::new(input_line);
    f.render_widget(input, chunks[2]);

    let hint = Paragraph::new("Enter: create  Esc: cancel")
        .style(Style::default().fg(T.text_dim))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[3]);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let width = (area.width * percent_x / 100).max(30).min(area.width);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height.min(area.height))
}
