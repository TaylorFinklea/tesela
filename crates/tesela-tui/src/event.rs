use crossterm::event::{Event as CrosstermEvent, KeyEvent};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Event {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
}

/// Convert a crossterm event into our Event type.
/// Returns `None` for events we don't care about (e.g. mouse, focus).
pub fn from_crossterm(event: CrosstermEvent) -> Option<Event> {
    match event {
        CrosstermEvent::Key(key) => Some(Event::Key(key)),
        CrosstermEvent::Resize(w, h) => Some(Event::Resize(w, h)),
        _ => None,
    }
}
