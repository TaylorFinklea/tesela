//! Keyboard shortcuts and help system for the TUI

use crossterm::event::KeyCode;
use std::collections::HashMap;

/// Keyboard shortcut definition
#[derive(Debug, Clone)]
pub struct Shortcut {
    pub key: KeyCode,
    pub modifiers: Vec<KeyModifier>,
    pub description: String,
    pub context: ShortcutContext,
}

/// Modifier keys
#[derive(Debug, Clone, PartialEq)]
pub enum KeyModifier {
    Ctrl,
    Alt,
    Shift,
}

/// Context where the shortcut is active
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShortcutContext {
    Global,
    MainMenu,
    Search,
    Listing,
    Input,
    Editor,
}

/// Shortcut manager
pub struct ShortcutManager {
    shortcuts: Vec<Shortcut>,
    context_map: HashMap<ShortcutContext, Vec<usize>>,
}

impl ShortcutManager {
    /// Create default shortcut manager with all shortcuts
    pub fn new() -> Self {
        let mut manager = Self {
            shortcuts: Vec::new(),
            context_map: HashMap::new(),
        };

        // Global shortcuts
        manager.add(Shortcut {
            key: KeyCode::Char('q'),
            modifiers: vec![],
            description: "Quit application".to_string(),
            context: ShortcutContext::Global,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('?'),
            modifiers: vec![],
            description: "Show help".to_string(),
            context: ShortcutContext::Global,
        });

        manager.add(Shortcut {
            key: KeyCode::Esc,
            modifiers: vec![],
            description: "Go back / Cancel".to_string(),
            context: ShortcutContext::Global,
        });

        // Main menu shortcuts
        manager.add(Shortcut {
            key: KeyCode::Char('n'),
            modifiers: vec![],
            description: "Create new note".to_string(),
            context: ShortcutContext::MainMenu,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('d'),
            modifiers: vec![],
            description: "Open daily note".to_string(),
            context: ShortcutContext::MainMenu,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('l'),
            modifiers: vec![],
            description: "List all notes".to_string(),
            context: ShortcutContext::MainMenu,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('/'),
            modifiers: vec![],
            description: "Search notes".to_string(),
            context: ShortcutContext::MainMenu,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('s'),
            modifiers: vec![KeyModifier::Ctrl],
            description: "Quick search (fuzzy)".to_string(),
            context: ShortcutContext::MainMenu,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('r'),
            modifiers: vec![],
            description: "Refresh index".to_string(),
            context: ShortcutContext::MainMenu,
        });

        // Search mode shortcuts
        manager.add(Shortcut {
            key: KeyCode::Enter,
            modifiers: vec![],
            description: "Open selected result".to_string(),
            context: ShortcutContext::Search,
        });

        manager.add(Shortcut {
            key: KeyCode::Tab,
            modifiers: vec![],
            description: "Accept suggestion".to_string(),
            context: ShortcutContext::Search,
        });

        manager.add(Shortcut {
            key: KeyCode::Up,
            modifiers: vec![],
            description: "Previous result".to_string(),
            context: ShortcutContext::Search,
        });

        manager.add(Shortcut {
            key: KeyCode::Down,
            modifiers: vec![],
            description: "Next result".to_string(),
            context: ShortcutContext::Search,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('f'),
            modifiers: vec![KeyModifier::Ctrl],
            description: "Toggle search filters".to_string(),
            context: ShortcutContext::Search,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('r'),
            modifiers: vec![KeyModifier::Ctrl],
            description: "Toggle regex search".to_string(),
            context: ShortcutContext::Search,
        });

        // Listing mode shortcuts
        manager.add(Shortcut {
            key: KeyCode::Enter,
            modifiers: vec![],
            description: "Open note".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('j'),
            modifiers: vec![],
            description: "Move down".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('k'),
            modifiers: vec![],
            description: "Move up".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('g'),
            modifiers: vec![],
            description: "Go to top".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('G'),
            modifiers: vec![KeyModifier::Shift],
            description: "Go to bottom".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::Char(' '),
            modifiers: vec![],
            description: "Toggle preview".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('v'),
            modifiers: vec![],
            description: "Toggle view mode (preview/graph)".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('e'),
            modifiers: vec![],
            description: "Edit note".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::Char('d'),
            modifiers: vec![KeyModifier::Ctrl],
            description: "Delete note".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::PageUp,
            modifiers: vec![],
            description: "Page up".to_string(),
            context: ShortcutContext::Listing,
        });

        manager.add(Shortcut {
            key: KeyCode::PageDown,
            modifiers: vec![],
            description: "Page down".to_string(),
            context: ShortcutContext::Listing,
        });

        // Input mode shortcuts
        manager.add(Shortcut {
            key: KeyCode::Enter,
            modifiers: vec![],
            description: "Submit input".to_string(),
            context: ShortcutContext::Input,
        });

        manager.add(Shortcut {
            key: KeyCode::Tab,
            modifiers: vec![],
            description: "Next suggestion".to_string(),
            context: ShortcutContext::Input,
        });

        manager.add(Shortcut {
            key: KeyCode::Backspace,
            modifiers: vec![KeyModifier::Ctrl],
            description: "Clear input".to_string(),
            context: ShortcutContext::Input,
        });

        manager.add(Shortcut {
            key: KeyCode::Left,
            modifiers: vec![KeyModifier::Ctrl],
            description: "Move to beginning of line".to_string(),
            context: ShortcutContext::Input,
        });

        manager.add(Shortcut {
            key: KeyCode::Right,
            modifiers: vec![KeyModifier::Ctrl],
            description: "Move to end of line".to_string(),
            context: ShortcutContext::Input,
        });

        manager
    }

    /// Add a shortcut
    fn add(&mut self, shortcut: Shortcut) {
        let index = self.shortcuts.len();
        let context = shortcut.context.clone();
        self.shortcuts.push(shortcut);

        self.context_map
            .entry(context)
            .or_insert_with(Vec::new)
            .push(index);
    }

    /// Get shortcuts for a specific context
    pub fn get_shortcuts(&self, context: &ShortcutContext) -> Vec<&Shortcut> {
        let mut shortcuts = Vec::new();

        // Add global shortcuts
        if let Some(indices) = self.context_map.get(&ShortcutContext::Global) {
            for &idx in indices {
                shortcuts.push(&self.shortcuts[idx]);
            }
        }

        // Add context-specific shortcuts
        if context != &ShortcutContext::Global {
            if let Some(indices) = self.context_map.get(context) {
                for &idx in indices {
                    shortcuts.push(&self.shortcuts[idx]);
                }
            }
        }

        shortcuts
    }

    /// Format shortcuts for display
    pub fn format_shortcuts(&self, context: &ShortcutContext) -> Vec<String> {
        self.get_shortcuts(context)
            .into_iter()
            .map(|s| self.format_shortcut(s))
            .collect()
    }

    /// Format a single shortcut
    fn format_shortcut(&self, shortcut: &Shortcut) -> String {
        let key_str = self.format_key(&shortcut.key, &shortcut.modifiers);
        format!("{:<15} {}", key_str, shortcut.description)
    }

    /// Format key with modifiers
    fn format_key(&self, key: &KeyCode, modifiers: &[KeyModifier]) -> String {
        let mut parts = Vec::new();

        for modifier in modifiers {
            parts.push(match modifier {
                KeyModifier::Ctrl => "Ctrl",
                KeyModifier::Alt => "Alt",
                KeyModifier::Shift => "Shift",
            });
        }

        let key_str = match key {
            KeyCode::Enter => "Enter",
            KeyCode::Esc => "Esc",
            KeyCode::Tab => "Tab",
            KeyCode::Backspace => "Backspace",
            KeyCode::Left => "←",
            KeyCode::Right => "→",
            KeyCode::Up => "↑",
            KeyCode::Down => "↓",
            KeyCode::PageUp => "PgUp",
            KeyCode::PageDown => "PgDn",
            KeyCode::Char(' ') => "Space",
            KeyCode::Char(c) => {
                return if parts.is_empty() {
                    c.to_string()
                } else {
                    format!("{}-{}", parts.join("-"), c)
                }
            }
            _ => "?",
        };

        if parts.is_empty() {
            key_str.to_string()
        } else {
            format!("{}-{}", parts.join("-"), key_str)
        }
    }
}

impl Default for ShortcutManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple shortcut info for UI display
#[derive(Debug, Clone)]
pub struct ShortcutInfo {
    pub keys: String,
    pub description: String,
}

/// Get shortcuts for a specific context (for UI display)
pub fn get_shortcuts_for_context(context: ShortcutContext) -> Vec<ShortcutInfo> {
    let manager = ShortcutManager::new();
    let shortcuts = manager.get_shortcuts(&context);

    shortcuts
        .into_iter()
        .map(|s| {
            let key_str = manager.format_key(&s.key, &s.modifiers);
            ShortcutInfo {
                keys: key_str,
                description: s.description.clone(),
            }
        })
        .collect()
}

/// Generate help text for a given context
pub fn generate_help_text(context: &ShortcutContext) -> String {
    let manager = ShortcutManager::new();
    let shortcuts = manager.format_shortcuts(context);

    let mut help = String::from("Keyboard Shortcuts\n");
    help.push_str("═══════════════════════════════\n\n");

    for shortcut in shortcuts {
        help.push_str(&format!("  {}\n", shortcut));
    }

    help
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_manager() {
        let manager = ShortcutManager::new();

        // Test global shortcuts
        let global_shortcuts = manager.get_shortcuts(&ShortcutContext::Global);
        assert!(!global_shortcuts.is_empty());

        // Test context-specific shortcuts
        let search_shortcuts = manager.get_shortcuts(&ShortcutContext::Search);
        assert!(!search_shortcuts.is_empty());

        // Check that global shortcuts are included
        let has_quit = search_shortcuts
            .iter()
            .any(|s| matches!(s.key, KeyCode::Char('q')) && s.modifiers.is_empty());
        assert!(has_quit);
    }

    #[test]
    fn test_help_text() {
        let help = generate_help_text(&ShortcutContext::MainMenu);
        assert!(help.contains("Keyboard Shortcuts"));
        assert!(help.contains("Create new note"));
        assert!(help.contains("Search notes"));
    }
}
