//! Block-level parsing for Tesela notes.
//!
//! Parses markdown body text into blocks, extracting tags and properties.
//! Mirrors the Swift `BlockParser` but runs server-side during indexing.

use crate::regex_cache::{PROPERTY_RE, TAG_RE};
use serde::Serialize;
use std::collections::HashMap;

#[cfg(test)]
use ts_rs::TS;

/// A parsed block from a note body.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct ParsedBlock {
    /// Deterministic ID: `{note_id}:{line_number}`
    pub id: String,
    /// The block's display text (first line without tags)
    pub text: String,
    /// Full raw text including continuation lines
    pub raw_text: String,
    /// Tags found directly on this block (e.g., ["Task", "urgent"]).
    /// Union of `inline_tags`, `trailing_tags`, and any `tags::` property
    /// value. Kept as a single flat list for back-compat with surfaces that
    /// don't care about position.
    pub tags: Vec<String>,
    /// `#tag` tokens that appear inside the block's text content (not at the
    /// trailing cluster). These render inline in the editor.
    pub inline_tags: Vec<String>,
    /// `#tag` tokens that appear in the trailing-cluster (one or more
    /// `#tag` tokens at the very end of the block's raw text, separated
    /// only by whitespace). These render as chips at the end of the block.
    /// Drives the tag-system spec's chip-vs-inline rendering.
    pub trailing_tags: Vec<String>,
    /// Tags inherited from ancestor blocks (parent, grandparent, etc.)
    pub inherited_tags: Vec<String>,
    /// Properties extracted from the block (e.g., {"status": "todo"})
    pub properties: HashMap<String, String>,
    /// Indentation level (0 = root)
    pub indent_level: usize,
    /// The note this block belongs to
    pub note_id: String,
    /// `note_type` of the containing page when known. Populated by the
    /// query candidate path (`SqliteIndex::execute_block_query`) so the
    /// `on:system-pages` DSL clause can filter blocks by their parent
    /// note's type without re-fetching note metadata at filter time.
    /// `None` for the standalone `parse_blocks(note_id, body)` form —
    /// `on:*` predicates that rely on parent metadata gracefully
    /// degrade (they match no block) rather than error.
    #[serde(default)]
    pub parent_note_type: Option<String>,
}

/// Parse a note body into blocks.
///
/// Each `- ` prefixed line starts a new block. Non-`- ` lines that follow
/// are continuation lines (properties or multi-line text) belonging to the
/// previous block. Child blocks inherit the tags of their ancestors.
pub fn parse_blocks(note_id: &str, body: &str) -> Vec<ParsedBlock> {
    // Pass 1: collect raw block data (line_num, indent, text)
    let mut raw_blocks: Vec<(usize, usize, String)> = Vec::new();
    let mut current: Option<(usize, usize, String)> = None;

    for (line_num, line) in body.lines().enumerate() {
        let trim_start = line.trim_start();
        if trim_start.is_empty() {
            continue;
        }
        let spaces = line.len() - trim_start.len();
        let indent = spaces / 2;

        // A bullet starts a block if the line begins with "- " (with content) OR
        // equals "-" / "- " exactly (an empty-content block, used for blocks
        // whose tags/properties live on continuation lines).
        let trimmed_end = trim_start.trim_end();
        let is_bullet = trim_start.starts_with("- ") || trimmed_end == "-";

        if is_bullet {
            if let Some(b) = current.take() {
                raw_blocks.push(b);
            }
            let text = trim_start
                .strip_prefix("- ")
                .or_else(|| trim_start.strip_prefix('-'))
                .unwrap_or(trim_start)
                .trim_end()
                .to_string();
            current = Some((line_num, indent, text));
        } else if let Some((_, _, ref mut text)) = current {
            text.push('\n');
            text.push_str(trim_start.trim_end());
        }
    }
    if let Some(b) = current {
        raw_blocks.push(b);
    }

    // Pass 2: build ParsedBlocks, computing inherited_tags via an ancestor stack
    let mut ancestor_stack: Vec<(usize, Vec<String>)> = Vec::new(); // (indent, tags)
    let mut blocks = Vec::with_capacity(raw_blocks.len());

    for (line_num, indent, raw_text) in raw_blocks {
        // Pop stack entries that are at the same or deeper indent (not true ancestors)
        while ancestor_stack
            .last()
            .map(|(i, _)| *i >= indent)
            .unwrap_or(false)
        {
            ancestor_stack.pop();
        }
        // Collect unique tags from all remaining ancestors (preserving order)
        let mut seen = std::collections::HashSet::new();
        let inherited_tags: Vec<String> = ancestor_stack
            .iter()
            .flat_map(|(_, tags)| tags.iter().cloned())
            .filter(|t| seen.insert(t.clone()))
            .collect();

        let block = make_block(note_id, line_num, indent, &raw_text, inherited_tags);
        ancestor_stack.push((indent, block.tags.clone()));
        blocks.push(block);
    }

    blocks
}

fn make_block(
    note_id: &str,
    line_num: usize,
    indent_level: usize,
    raw_text: &str,
    inherited_tags: Vec<String>,
) -> ParsedBlock {
    let mut properties = extract_properties(raw_text);

    // Position-aware tag classification (tag-system spec):
    //
    //   trailing-cluster = one or more `#tag` tokens at the end of the
    //   block's raw text, separated only by whitespace. These render as
    //   chips. All other `#tag` tokens are inline and render inline.
    //
    // The split runs on raw_text (the full block content). The trailing
    // cluster is consumed left-to-right after the last non-tag/non-
    // whitespace character.
    let (inline_tags, trailing_tags) = split_inline_and_trailing_tags(raw_text);

    // Merge tags from three sources, preserving order, deduplicated:
    //   1. `tags::` property line — legacy back-compat read path
    //   2. inline `#tag` tokens (position rule above)
    //   3. trailing-cluster `#tag` tokens
    let mut seen = std::collections::HashSet::new();
    let mut tags: Vec<String> = Vec::new();
    if let Some(tags_value) = properties.remove("tags") {
        for t in tags_value
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            if seen.insert(t.to_string()) {
                tags.push(t.to_string());
            }
        }
    }
    for t in inline_tags.iter().chain(trailing_tags.iter()) {
        if seen.insert(t.clone()) {
            tags.push(t.clone());
        }
    }

    // Display text = first line with the `<!-- bid:UUID -->` marker
    // and inline `#tags` stripped. `raw_text` retains the marker so
    // sync round-trips back to the canonical on-disk form.
    let first_line = raw_text.lines().next().unwrap_or(raw_text);
    let bid_stripped = crate::note_tree::strip_bid_comment(first_line);
    let display_text = TAG_RE.replace_all(&bid_stripped, "").trim().to_string();

    ParsedBlock {
        id: format!("{}:{}", note_id, line_num),
        text: display_text,
        raw_text: raw_text.to_string(),
        tags,
        inline_tags,
        trailing_tags,
        inherited_tags,
        properties,
        indent_level,
        note_id: note_id.to_string(),
        parent_note_type: None,
    }
}

/// Split `#tag` tokens in `raw_text` into (inline, trailing).
///
/// The trailing cluster is one or more `#tag` tokens at the very end of
/// the text, separated by whitespace only. All other `#tag` tokens are
/// inline. Tag names match `[A-Za-z0-9_/-]+` (same alphabet as TAG_RE).
pub fn split_inline_and_trailing_tags(raw_text: &str) -> (Vec<String>, Vec<String>) {
    // Find the trailing-cluster region by scanning from the end of the
    // trimmed text. A token is `#` followed by tag-name chars; tokens may
    // be separated by horizontal whitespace or newlines; the cluster ends
    // at the first non-tag, non-whitespace character.
    let trimmed = raw_text.trim_end();
    let bytes = trimmed.as_bytes();
    let mut cursor = bytes.len();
    let mut trailing_starts: Vec<usize> = Vec::new();

    loop {
        // Skip whitespace going left.
        while cursor > 0 && (bytes[cursor - 1] as char).is_whitespace() {
            cursor -= 1;
        }
        // We expect a tag-name suffix ending at `cursor` and preceded by `#`.
        let name_end = cursor;
        while cursor > 0 {
            let c = bytes[cursor - 1] as char;
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '/' {
                cursor -= 1;
            } else {
                break;
            }
        }
        let name_start = cursor;
        // Must have at least one name char, and a `#` immediately before.
        if name_end == name_start || cursor == 0 || bytes[cursor - 1] != b'#' {
            break;
        }
        cursor -= 1; // consume `#`
        trailing_starts.push(cursor);
    }

    // Trailing cluster starts at the leftmost matched `#`. Anything to the
    // left is inline.
    let cluster_start = trailing_starts.last().copied().unwrap_or(trimmed.len());
    let inline_text = &raw_text[..cluster_start];

    let inline_tags: Vec<String> = TAG_RE
        .captures_iter(inline_text)
        .map(|cap| cap[1].to_string())
        .collect();
    let trailing_tags: Vec<String> = trailing_starts
        .iter()
        .rev() // back to left-to-right
        .filter_map(|&pos| {
            // Slice from after `#` to the end of the name, using the original
            // text so we recover the exact name characters.
            let after_hash = pos + 1;
            let name = raw_text[after_hash..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '/')
                .collect::<String>();
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        })
        .collect();

    (inline_tags, trailing_tags)
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
    fn test_inherited_tags_from_parent() {
        let body = "- Parent #Task\n  - Child\n    status:: todo";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].tags, vec!["Task"]);
        assert!(blocks[0].inherited_tags.is_empty());
        assert!(blocks[1].tags.is_empty());
        assert_eq!(blocks[1].inherited_tags, vec!["Task"]);
    }

    #[test]
    fn test_parse_block_with_tags_property() {
        let body = "- Plain content\n  tags:: Task, urgent";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].tags, vec!["Task", "urgent"]);
        assert_eq!(blocks[0].text, "Plain content");
        // tags:: should NOT appear in properties — it owns block.tags
        assert!(!blocks[0].properties.contains_key("tags"));
    }

    #[test]
    fn test_parse_block_merges_tags_property_and_inline() {
        let body = "- Hybrid #urgent\n  tags:: Task";
        let blocks = parse_blocks("test", body);
        // tags:: comes first, inline #urgent appended
        assert_eq!(blocks[0].tags, vec!["Task", "urgent"]);
    }

    #[test]
    fn test_parse_block_tags_property_dedupes() {
        let body = "- Same #Task\n  tags:: Task, Task, urgent";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks[0].tags, vec!["Task", "urgent"]);
    }

    #[test]
    fn test_parse_block_tags_property_with_other_properties() {
        let body = "- Item\n  tags:: Task\n  status:: doing";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks[0].tags, vec!["Task"]);
        assert_eq!(
            blocks[0].properties.get("status"),
            Some(&"doing".to_string())
        );
        assert!(!blocks[0].properties.contains_key("tags"));
    }

    #[test]
    fn test_parse_empty_content_block_with_tags_property() {
        // Empty-content block written as "- " followed by indented tags::
        let body = "- \n  tags:: Task";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "");
        assert_eq!(blocks[0].tags, vec!["Task"]);
    }

    #[test]
    fn test_parse_blocks_after_empty_content_block() {
        // The parser must recognize "- " (with trailing space) as a block
        // boundary so it doesn't merge with the previous block.
        let body = "- First\n- \n  tags:: Task\n- Third";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].text, "First");
        assert_eq!(blocks[1].text, "");
        assert_eq!(blocks[1].tags, vec!["Task"]);
        assert_eq!(blocks[2].text, "Third");
    }

    #[test]
    fn test_parse_block_tags_property_inherits_to_children() {
        let body = "- Parent\n  tags:: Task\n  - Child";
        let blocks = parse_blocks("test", body);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].inherited_tags, vec!["Task"]);
    }

    #[test]
    fn test_block_ids_are_deterministic() {
        let body = "- First\n- Second";
        let blocks1 = parse_blocks("note1", body);
        let blocks2 = parse_blocks("note1", body);
        assert_eq!(blocks1[0].id, blocks2[0].id);
        assert_eq!(blocks1[1].id, blocks2[1].id);
    }

    #[test]
    fn parse_blocks_strips_bid_comment_from_display_text() {
        // The on-disk form embeds a `<!-- bid:UUID -->` marker so block
        // identity survives across sync. That marker must NOT leak into
        // the presentational `text` field — clients render `text` in
        // agenda rows, inbox previews, search hits, etc., where the bid
        // would be visual noise. `raw_text` keeps the comment so the
        // round-trip back to disk stays lossless.
        let body = "- Do wood chips <!-- bid:019e549e-3fb3-7a72-acf2-3e5b4aba03f4 -->";
        let blocks = parse_blocks("note", body);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "Do wood chips");
        assert!(
            blocks[0].raw_text.contains("<!-- bid:"),
            "raw_text must keep the bid comment for sync round-trip"
        );
    }

    // ── split_inline_and_trailing_tags ────────────────────────────────────

    #[test]
    fn split_no_tags_returns_empty() {
        let (inline, trailing) = split_inline_and_trailing_tags("just text");
        assert!(inline.is_empty());
        assert!(trailing.is_empty());
    }

    #[test]
    fn split_pure_inline_tag() {
        let (inline, trailing) = split_inline_and_trailing_tags("see #foo here");
        assert_eq!(inline, vec!["foo"]);
        assert!(trailing.is_empty());
    }

    #[test]
    fn split_pure_trailing_tag() {
        let (inline, trailing) = split_inline_and_trailing_tags("task name #important");
        assert!(inline.is_empty());
        assert_eq!(trailing, vec!["important"]);
    }

    #[test]
    fn split_multiple_trailing_tags_one_cluster() {
        let (inline, trailing) = split_inline_and_trailing_tags("task #foo #bar #baz");
        assert!(inline.is_empty());
        assert_eq!(trailing, vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn split_inline_plus_trailing() {
        let (inline, trailing) = split_inline_and_trailing_tags("see #foo here #bar");
        assert_eq!(inline, vec!["foo"]);
        assert_eq!(trailing, vec!["bar"]);
    }

    #[test]
    fn split_trailing_whitespace_doesnt_break_cluster() {
        let (_, trailing) = split_inline_and_trailing_tags("x #a   ");
        assert_eq!(trailing, vec!["a"]);
    }

    #[test]
    fn split_cluster_halts_at_first_non_tag_non_whitespace() {
        let (inline, trailing) = split_inline_and_trailing_tags("x #a y #b");
        assert_eq!(inline, vec!["a"]);
        assert_eq!(trailing, vec!["b"]);
    }

    #[test]
    fn split_bare_hash_is_not_a_tag() {
        let (inline, trailing) = split_inline_and_trailing_tags("value is #");
        assert!(inline.is_empty());
        assert!(trailing.is_empty());
    }

    #[test]
    fn split_path_form_tag_with_slashes() {
        let (_, trailing) = split_inline_and_trailing_tags("task #nature/birds/cardinal");
        assert_eq!(trailing, vec!["nature/birds/cardinal"]);
    }

    #[test]
    fn split_newlines_within_cluster() {
        let (_, trailing) = split_inline_and_trailing_tags("- a\n#tag1\n#tag2");
        assert_eq!(trailing, vec!["tag1", "tag2"]);
    }

    #[test]
    fn split_same_tag_inline_and_trailing_yields_both() {
        let (inline, trailing) = split_inline_and_trailing_tags("#foo bar #foo");
        assert_eq!(inline, vec!["foo"]);
        assert_eq!(trailing, vec!["foo"]);
    }
}
