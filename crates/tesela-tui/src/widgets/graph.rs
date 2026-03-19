use tesela_core::link::Link;

/// Build display lines for the graph view (backlinks + forward links).
pub fn render_lines(note_title: &str, backlinks: &[Link], forward_links: &[Link]) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(format!("╔══ {} ══╗", note_title));
    lines.push(String::new());

    if backlinks.is_empty() {
        lines.push("  ← (no backlinks)".to_string());
    } else {
        lines.push(format!("  ← Backlinks ({})", backlinks.len()));
        for link in backlinks {
            // For backlinks, the `text` field is the anchor text from the source note
            lines.push(format!("     • [[{}]]", link.text));
        }
    }

    lines.push(String::new());

    if forward_links.is_empty() {
        lines.push("  → (no forward links)".to_string());
    } else {
        lines.push(format!("  → Forward links ({})", forward_links.len()));
        for link in forward_links {
            lines.push(format!("     • {}", link.target));
        }
    }

    lines
}
