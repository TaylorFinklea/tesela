use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Tesela ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  c  ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Create new note"),
        ]),
        Line::from(vec![
            Span::styled(
                "  n  ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Browse notes"),
        ]),
        Line::from(vec![
            Span::styled(
                "  d  ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Open daily note"),
        ]),
        Line::from(vec![
            Span::styled(
                "  /  ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Search"),
        ]),
        Line::from(vec![
            Span::styled(
                "  ^P ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Quick switcher"),
        ]),
        Line::from(vec![
            Span::styled(
                "  ?  ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Help"),
        ]),
        Line::from(vec![
            Span::styled(
                "  q  ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Quit"),
        ]),
    ];

    let para = Paragraph::new(lines).block(block);
    f.render_widget(para, area);
}
