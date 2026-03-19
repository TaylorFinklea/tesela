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

/// Nerd Font icons (requires a Nerd Font in the terminal)
#[allow(dead_code)]
pub mod icons {
    pub const NOTE: &str = "\u{f0219}"; // 󰈙 nf-md-file_document
    pub const SEARCH: &str = "\u{f002}"; //  nf-fa-search
    pub const CALENDAR: &str = "\u{f073}"; //  nf-fa-calendar
    pub const GRAPH: &str = "\u{f0e8}"; //  nf-fa-sitemap
    pub const PLUS: &str = "\u{f067}"; //  nf-fa-plus
    pub const PENCIL: &str = "\u{f040}"; //  nf-fa-pencil
    pub const TRASH: &str = "\u{f014}"; //  nf-fa-trash
    pub const HELP: &str = "\u{f059}"; //  nf-fa-question_circle
    pub const FOLDER: &str = "\u{f07b}"; //  nf-fa-folder
    pub const LINK: &str = "\u{f0c1}"; //  nf-fa-link
    pub const TAG: &str = "\u{f02b}"; //  nf-fa-tag
    pub const QUIT: &str = "\u{f011}"; //  nf-fa-power_off
    pub const ARROW_LEFT: &str = "\u{f060}"; //  nf-fa-arrow_left
    pub const ARROW_RIGHT: &str = "\u{f061}"; //  nf-fa-arrow_right
    pub const KEYBOARD: &str = "\u{f11c}"; //  nf-fa-keyboard_o
}
