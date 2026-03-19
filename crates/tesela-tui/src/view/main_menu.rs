use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(f: &mut Frame, area: Rect) {
    let block = Block::default().title(" Tesela ").borders(Borders::ALL);

    let text = "\n  n  Browse notes\n  /  Search\n  q  Quit\n  ?  Help";
    let para = Paragraph::new(text).block(block);
    f.render_widget(para, area);
}
