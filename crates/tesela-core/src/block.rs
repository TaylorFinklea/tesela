//! Block-level parsing for Tesela notes.
//!
//! Parses markdown body text into blocks, extracting tags and properties.
//! Mirrors the Swift `BlockParser` but runs server-side during indexing.

use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::LazyLock;

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#([A-Za-z0-9_/-]+)").unwrap());

static PROPERTY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([A-Za-z_][A-Za-z0-9_]*):: (.+)").unwrap());

/// A parsed block from a note body.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedBlock {
    /// Deterministic ID: `{note_id}:{line_number}`
    pub id: String,
    /// The block's display text (first line without tags)
    pub text: String,
    /// Full raw text including continuation lines
    pub raw_text: String,
    /// Tags found in the block (e.g., ["Task", "urgent"])
    pub tags: Vec<String>,
    /// Properties extracted from the block (e.g., {"status": "todo"})
    pub properties: HashMap<String, String>,
    /// Indentation level (0 = root)
    pub indent_level: usize,
    /// The note this block belongs to
    pub note_id: String,
}

/// Parse a note body into blocks.
///
/// Each `- ` prefixed line starts a new block. Non-`- ` lines that follow
/// are continuation lines (properties or multi-line text) belonging to the
/// previous block.
pub fn parse_blocks(note_id: &str, body: &str) -> Vec<ParsedBlock> {
    let lines: Vec<&str> = body.lines().collect();
    let mut blocks: Vec<ParsedBlock> = Vec::new();
    let mut current_block: Option<(usize, usize, String)> = None; // (line_num, indent, text)

    for (line_num, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Count leading spaces for indent level
        let spaces = line.len() - line.trim_start().len();
        let indent = spaces / 2;

        if trimmed.starts_with("- ") {
            // Finalize previous block
            if let Some((start_line, block_indent, text)) = current_block.take() {
                blocks.push(make_block(note_id, start_line, block_indent, &text));
            }
            // Start new block
            let block_text = trimmed.strip_prefix("- ").unwrap_or(trimmed).to_string();
            current_block = Some((line_num, indent, block_text));
        } else if let Some((_, _, ref mut text)) = current_block {
            // Continuation line — append to current block
            text.push('\n');
            text.push_str(trimmed);
        }
    }

    // Finalize last block
    if let Some((start_line, indent, text)) = current_block {
        blocks.push(make_block(note_id, start_line, indent, &text));
    }

    blocks
}

fn make_block(note_id: &str, line_num: usize, indent_level: usize, raw_text: &str) -> ParsedBlock {
    let tags = extract_tags(raw_text);
    let properties = extract_properties(raw_text);

    // Display text = first line with tags stripped
    let first_line = raw_text.lines().next().unwrap_or(raw_text);
    let display_text = TAG_RE.replace_all(first_line, "").trim().to_string();

    ParsedBlock {
        id: format!("{}:{}", note_id, line_num),
        text: display_text,
        raw_text: raw_text.to_string(),
        tags,
        properties,
        indent_level,
        note_id: note_id.to_string(),
    }
}

fn extract_tags(text: &str) -> Vec<String> {
    TAG_RE
        .captures_iter(text)
        .map(|cap| cap[1].to_string())
        .collect()
}

fn extract_properties(text: &str) -> HashMap<String, String> {
    let mut props = HashMap::new();
    for cap in PROPERTY_RE.captures_iter(text) {
        let key = cap[1].to_string();
        let value = cap[2].to_string();
        props.insert(key, value);
    }
    props
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_block() {
        let blocks = parse_blocks("test", "- Hello world");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "Hello world");
        assert_eq!(blocks[0].id, "test:0");
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let body = "- First\n- Second\n- Third";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].text, "First");
        assert_eq!(blocks[2].text, "Third");
    }

    #[test]
    fn test_parse_block_with_tags() {
        let body = "- Buy groceries #Task #urgent";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks[0].tags, vec!["Task", "urgent"]);
        assert_eq!(blocks[0].text, "Buy groceries");
    }

    #[test]
    fn test_parse_block_with_tag_at_end_of_line() {
        let body = "- Finish report #work";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks[0].tags, vec!["work"]);
    }

    #[test]
    fn test_parse_block_with_special_character_tags() {
        let body = "- Ship release #v2 #projects/tesela #follow-up";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks[0].tags, vec!["v2", "projects/tesela", "follow-up"]);
    }

    #[test]
    fn test_parse_block_with_properties() {
        let body = "- My task #Task\n  status:: todo\n  priority:: high";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].properties.get("status"),
            Some(&"todo".to_string())
        );
        assert_eq!(
            blocks[0].properties.get("priority"),
            Some(&"high".to_string())
        );
        assert_eq!(blocks[0].tags, vec!["Task"]);
    }

    #[test]
    fn test_parse_empty_body() {
        let blocks = parse_blocks("test", "");
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_parse_heading_only() {
        let blocks = parse_blocks("test", "# My Heading\nSome prose");
        assert!(blocks.is_empty()); // No bullet blocks
    }

    #[test]
    fn test_block_ids_are_deterministic() {
        let body = "- First\n- Second";
        let blocks1 = parse_blocks("note1", body);
        let blocks2 = parse_blocks("note1", body);
        assert_eq!(blocks1[0].id, blocks2[0].id);
        assert_eq!(blocks1[1].id, blocks2[1].id);
    }
}
