use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::theme::DEFAULT as T;

pub fn render(f: &mut Frame, area: Rect) {
    let key_style = Style::default().fg(T.accent).add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(T.text);
    let section_style = Style::default().fg(T.text).add_modifier(Modifier::BOLD);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Global", section_style)),
        key_desc("    q / Ctrl+C  ", "Quit", key_style, desc_style),
        key_desc("    ?           ", "Toggle help", key_style, desc_style),
        key_desc("    ^P          ", "Quick switcher", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("  Navigation", section_style)),
        key_desc("    j / ↓       ", "Next item", key_style, desc_style),
        key_desc("    k / ↑       ", "Previous item", key_style, desc_style),
        key_desc("    Enter       ", "Select / Open", key_style, desc_style),
        key_desc("    Esc         ", "Back", key_style, desc_style),
        Line::from(""),
        Line::from(Span::styled("  Notes", section_style)),
        key_desc("    c           ", "Create note", key_style, desc_style),
        key_desc("    n           ", "Browse notes", key_style, desc_style),
        key_desc("    d           ", "Daily note", key_style, desc_style),
        key_desc("    e           ", "Edit in $EDITOR", key_style, desc_style),
        key_desc(
            "    g           ",
            "Toggle graph view",
            key_style,
            desc_style,
        ),
        key_desc("    D           ", "Delete note", key_style, desc_style),
        key_desc("    /           ", "Search", key_style, desc_style),
    ];

    let block = Block::default()
        .title(" Help ")
        .title_style(Style::default().fg(T.accent).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(T.accent));

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}

fn key_desc<'a>(key: &'a str, desc: &'a str, ks: Style, ds: Style) -> Line<'a> {
    Line::from(vec![Span::styled(key, ks), Span::styled(desc, ds)])
}
