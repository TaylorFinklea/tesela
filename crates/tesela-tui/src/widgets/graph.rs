use ratatui::{
    style::Style,
    text::{Line, Span},
};
use tesela_core::link::Link;

use crate::theme::DEFAULT as T;

/// Build styled display lines for the graph view (backlinks + forward links).
pub fn render_lines(
    note_title: &str,
    backlinks: &[Link],
    forward_links: &[Link],
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let header_style = Style::default().fg(T.accent_alt);
    let arrow_style = Style::default().fg(T.accent);
    let link_style = Style::default().fg(T.text_dim);
    let label_style = Style::default().fg(T.text);

    lines.push(Line::from(Span::styled(
        format!("╔══ {} ══╗", note_title),
        header_style,
    )));
    lines.push(Line::from(""));

    if backlinks.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  ← ", arrow_style),
            Span::styled("(no backlinks)", link_style),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ← ", arrow_style),
            Span::styled(format!("Backlinks ({})", backlinks.len()), label_style),
        ]));
        for link in backlinks {
            lines.push(Line::from(vec![
                Span::raw("     "),
                Span::styled("• ", link_style),
                Span::styled(format!("[[{}]]", link.text), arrow_style),
            ]));
        }
    }

    lines.push(Line::from(""));

    if forward_links.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("  → ", arrow_style),
            Span::styled("(no forward links)", link_style),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  → ", arrow_style),
            Span::styled(
                format!("Forward links ({})", forward_links.len()),
                label_style,
            ),
        ]));
        for link in forward_links {
            lines.push(Line::from(vec![
                Span::raw("     "),
                Span::styled("• ", link_style),
                Span::styled(link.target.to_string(), arrow_style),
            ]));
        }
    }

    lines
}
