use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::theme::icons;
use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(format!(" {} Tesela ", icons::NOTE))
        .title_style(Style::default().fg(T.accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(T.accent));

    let key_style = Style::default().fg(T.accent).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(T.text_dim);
    let icon_style = Style::default().fg(T.text_dim);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  ╭─ keyboard-first notes ─╮",
            Style::default().fg(T.text_dim),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  {} ", icons::PLUS), icon_style),
            Span::styled("c  ", key_style),
            Span::styled("Create new note", desc_style),
        ]),
        Line::from(vec![
            Span::styled(format!("  {} ", icons::FOLDER), icon_style),
            Span::styled("n  ", key_style),
            Span::styled("Browse notes", desc_style),
        ]),
        Line::from(vec![
            Span::styled(format!("  {} ", icons::CALENDAR), icon_style),
            Span::styled("d  ", key_style),
            Span::styled("Open daily note", desc_style),
        ]),
        Line::from(vec![
            Span::styled(format!("  {} ", icons::SEARCH), icon_style),
            Span::styled("/  ", key_style),
            Span::styled("Search", desc_style),
        ]),
        Line::from(vec![
            Span::styled(format!("  {} ", icons::KEYBOARD), icon_style),
            Span::styled("^P ", key_style),
            Span::styled("Quick switcher", desc_style),
        ]),
        Line::from(vec![
            Span::styled(format!("  {} ", icons::HELP), icon_style),
            Span::styled("?  ", key_style),
            Span::styled("Help", desc_style),
        ]),
        Line::from(vec![
            Span::styled(format!("  {} ", icons::QUIT), icon_style),
            Span::styled("q  ", key_style),
            Span::styled("Quit", desc_style),
        ]),
    ];

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}
