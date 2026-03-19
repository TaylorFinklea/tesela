use ratatui::style::Color;

pub struct Theme {
    pub accent: Color,
    pub accent_alt: Color,
    pub text: Color,
    pub text_dim: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub error: Color,
    pub status_bg: Color,
    pub tag: Color,
    pub tree: Color,
}

pub const DEFAULT: Theme = Theme {
    accent: Color::Cyan,
    accent_alt: Color::Green,
    text: Color::White,
    text_dim: Color::DarkGray,
    selection_bg: Color::DarkGray,
    selection_fg: Color::White,
    error: Color::Red,
    status_bg: Color::Black,
    tag: Color::Yellow,
    tree: Color::DarkGray,
};
