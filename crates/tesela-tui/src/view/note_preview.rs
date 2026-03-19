use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::state::AppState;
use crate::widgets::outliner;

/// Render note content using the outliner widget.
pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let (title, body) = state
        .current_note
        .as_ref()
        .map(|n| (n.title.as_str(), n.body.as_str()))
        .unwrap_or(("No note selected", ""));

    let blocks = outliner::parse_blocks(body);
    let display_lines = outliner::render_lines(&blocks);

    let lines: Vec<Line> = display_lines
        .into_iter()
        .map(|l| {
            // Highlight tags (#word) in a different color
            if l.contains('#') {
                colorize_tags(l)
            } else {
                Line::from(l)
            }
        })
        .collect();

    let block_widget = Block::default()
        .title(format!(" {} ", title))
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

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

    let display_lines = crate::widgets::graph::render_lines(
        title,
        &state.graph_backlinks,
        &state.graph_forward_links,
    );

    let lines: Vec<Line> = display_lines.into_iter().map(Line::from).collect();

    let block_widget = Block::default()
        .title(format!(" {} — Graph ", title))
        .title_style(Style::default().add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let para = Paragraph::new(lines)
        .block(block_widget)
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);
}

/// Simple tag colorizer: splits on `#` and applies cyan color to tag words.
fn colorize_tags(line: String) -> Line<'static> {
    let mut spans = Vec::new();
    let mut remaining = line.as_str();
    // Work with the owned string by splitting around '#'
    let parts: Vec<&str> = remaining.split('#').collect();
    if parts.len() == 1 {
        return Line::from(line);
    }
    // First part is pre-tag text
    if !parts[0].is_empty() {
        spans.push(Span::raw(parts[0].to_string()));
    }
    for part in &parts[1..] {
        // Split on the first whitespace to separate the tag word from the rest
        let (tag_word, rest) = if let Some(pos) = part.find(|c: char| c.is_whitespace()) {
            (&part[..pos], &part[pos..])
        } else {
            (*part, "")
        };
        spans.push(Span::styled(
            format!("#{}", tag_word),
            Style::default().fg(Color::Cyan),
        ));
        if !rest.is_empty() {
            spans.push(Span::raw(rest.to_string()));
        }
        remaining = rest;
    }
    let _ = remaining;
    Line::from(spans)
}
