use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, area: Rect) {
    let text = "\
  Global:\n\
    q / Ctrl+C  Quit\n\
    ?           Toggle help\n\
\n\
  Navigation:\n\
    j / Down    Next item\n\
    k / Up      Previous item\n\
    Enter       Select / Open\n\
    Esc         Back\n\
\n\
  Modes:\n\
    n           Browse notes\n\
    /           Search\n\
";
    let para = Paragraph::new(text)
        .block(Block::default().title("Help").borders(Borders::ALL));
    f.render_widget(para, area);
}
