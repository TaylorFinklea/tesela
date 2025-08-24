//! Terminal User Interface module for Tesela
//!
//! Provides an interactive TUI with real-time feedback, keyboard navigation,
//! and seamless integration with existing Tesela functionality.

use anyhow::Result;

mod app;
mod handlers;
mod ui;
mod widgets;

pub use app::App;

/// Run the TUI application
pub fn run() -> Result<()> {
    // Terminal setup
    let mut terminal = ui::setup_terminal()?;

    // Create and run app
    let app = App::new()?;
    let result = app.run(&mut terminal);

    // Terminal cleanup (always runs, even on error)
    ui::restore_terminal(&mut terminal)?;

    result
}
