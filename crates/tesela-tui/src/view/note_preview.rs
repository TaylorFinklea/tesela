use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::state::AppState;
use crate::theme::DEFAULT as T;
use crate::widgets::outliner;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let (title, body) = state
        .current_note
        .as_ref()
        .map(|n| (n.title.as_str(), n.body.as_str()))
        .unwrap_or(("No note selected", ""));

    let blocks = outliner::parse_blocks(body);
    let lines = outliner::render_lines(&blocks);

    let block_widget = Block::default()
        .title(format!(" {} ", title))
        .title_style(Style::default().fg(T.text).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(T.accent));

    let para = Paragraph::new(lines)
        .block(block_widget)
        .wrap(Wrap { trim: false })
        .scroll((state.listing.scroll_offset as u16, 0));

    f.render_widget(para, area);
}

/// Render the graph (backlinks + forward links) view for the current note.
pub fn render_graph(f: &mut Frame, area: Rect, state: &AppState) {
    let title = state
        .current_note
        .as_ref()
        .map(|n| n.title.as_str())
        .unwrap_or("No note selected");

    let lines = crate::widgets::graph::render_lines(
        title,
        &state.graph_backlinks,
        &state.graph_forward_links,
    );

    let block_widget = Block::default()
        .title(format!(" {} — Graph ", title))
        .title_style(Style::default().fg(T.text).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(T.accent_alt));

    let para = Paragraph::new(lines)
        .block(block_widget)
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);
}
