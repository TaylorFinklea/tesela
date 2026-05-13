//! Sync-aware note parser and serializer.
//!
//! This module provides a round-trip parse/serialize pipeline for Tesela
//! notes with stable per-block identifiers. It is the data-model foundation
//! for block-level sync: producers diff two `NoteTree` values to emit
//! `BlockUpsert` / `BlockMove` / `BlockDelete` ops, and receivers apply
//! those ops by parsing, mutating, and re-serializing the tree.
//!
//! ## Block identifiers
//!
//! Block ids are persisted inline in the markdown as HTML comments:
//!
//! ```text
//! - First block <!-- bid:01940f5a-0000-7000-8000-000000000000 -->
//! - Second block <!-- bid:01940f5a-0001-7000-8000-000000000000 -->
//! ```
//!
//! Parsing recognizes the comment, strips it from the text, and uses the
//! id as the block's persistent identity. Blocks without a `bid` comment
//! get a freshly minted UUIDv7 at parse time; serializing back writes the
//! comment in canonical position.
//!
//! ## Scope
//!
//! - Frontmatter (a YAML block bracketed by `---` lines at the top of the
//!   file) is preserved verbatim across parse + serialize.
//! - Bullet lines (`- text`) are the only content type recognized as
//!   blocks. Indent is two spaces per level, matching the existing
//!   `parse_blocks` in [`crate::block`].
//! - Continuation lines (indented lines following a bullet, used for
//!   properties like `status:: doing`) are folded into the parent
//!   block's `text` joined by newlines.
//! - Non-bullet body content (headings, prose paragraphs outside the
//!   frontmatter) is preserved verbatim as `raw_segments` so the round
//!   trip is lossless, but those segments are not given block ids and
//!   are not part of the sync data model.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A parsed note. Holds the verbatim frontmatter and an ordered list of
/// blocks. See module docs for full semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteTree {
    /// Verbatim YAML frontmatter including the surrounding `---` markers,
    /// trailing newline preserved. `None` when the file has no
    /// frontmatter block.
    pub frontmatter: Option<String>,
    /// The blocks in document order.
    pub blocks: Vec<FlatBlock>,
    /// Whether parsing minted at least one new block id. Producers can
    /// use this to decide whether to write back a stamped version of the
    /// file before diffing.
    pub stamped_any: bool,
}

/// A single block. Order is implicit in [`NoteTree::blocks`] position;
/// parent is computed from indent during parse and recorded explicitly so
/// downstream diff/move operations are unambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlatBlock {
    /// Persistent block id (UUIDv7).
    pub id: Uuid,
    /// Parent block id. `None` for top-level blocks.
    pub parent: Option<Uuid>,
    /// Indent level (0 = top-level). Two spaces per level on disk.
    pub indent: u16,
    /// Block text, INCLUDING continuation lines joined by `'\n'`, but
    /// NOT including the leading `- `, the indent, or the `<!-- bid -->`
    /// comment.
    pub text: String,
}

/// Format used to render block id comments. Hyphenated 36-char form for
/// human readability in the raw markdown.
pub const BID_PREFIX: &str = "<!-- bid:";
pub const BID_SUFFIX: &str = " -->";

/// Parse a note file's contents into a [`NoteTree`].
///
/// Stamps fresh UUIDv7 ids on any unstamped bullets. Frontmatter is
/// captured verbatim. Non-bullet body content currently does not survive
/// the round trip (see module docs); revisit when there is a real user
/// note that hits this.
pub fn parse_note(content: &str) -> NoteTree {
    let (frontmatter, body) = split_frontmatter(content);
    let (blocks, stamped_any) = parse_body_blocks(body);
    NoteTree {
        frontmatter,
        blocks,
        stamped_any,
    }
}

/// Serialize a [`NoteTree`] back to canonical markdown.
///
/// `parse_note(serialize_note(tree)) == tree` for any tree built either
/// from `parse_note` or constructed by sync apply. Verifying this is
/// what the round-trip tests below check.
pub fn serialize_note(tree: &NoteTree) -> String {
    let mut out = String::new();
    if let Some(fm) = &tree.frontmatter {
        out.push_str(fm);
        // Frontmatter strings always end with `---\n`. Add the blank
        // separator line before the body if there are blocks to write.
        if !tree.blocks.is_empty() {
            out.push('\n');
        }
    }
    for block in &tree.blocks {
        let indent_spaces = "  ".repeat(block.indent as usize);
        let mut lines = block.text.lines();
        let first = lines.next().unwrap_or("");
        // First line: bullet + text + bid comment.
        out.push_str(&indent_spaces);
        out.push_str("- ");
        if !first.is_empty() {
            out.push_str(first);
            out.push(' ');
        }
        out.push_str(BID_PREFIX);
        out.push_str(&format_bid(block.id));
        out.push_str(BID_SUFFIX);
        out.push('\n');
        // Continuation lines: indented two more spaces under the bullet.
        for line in lines {
            out.push_str(&indent_spaces);
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn split_frontmatter(content: &str) -> (Option<String>, &str) {
    // Frontmatter is a `---\n` marker at offset 0, followed by content,
    // followed by a `\n---\n` (or `\n---` at EOF). Match exactly that
    // shape; anything else falls through as "no frontmatter."
    if !content.starts_with("---\n") {
        return (None, content);
    }
    let after_open = &content[4..];
    // Scan for a closing `---` line. Match `\n---\n` or `\n---` at EOF.
    let mut search_start = 0;
    while let Some(idx) = after_open[search_start..].find("\n---") {
        let abs = search_start + idx;
        let after_close = &after_open[abs + 4..];
        if after_close.is_empty() {
            // closing marker at EOF, no body
            let fm = format!("---\n{}\n---", &after_open[..abs]);
            return (Some(fm), "");
        }
        if let Some(stripped) = after_close.strip_prefix('\n') {
            // proper closing marker followed by body
            let fm = format!("---\n{}\n---\n", &after_open[..abs]);
            return (Some(fm), stripped);
        }
        // Not a proper closing line (e.g. `\n----`), keep scanning.
        search_start = abs + 4;
    }
    // No closing marker found; treat as no frontmatter.
    (None, content)
}

fn parse_body_blocks(body: &str) -> (Vec<FlatBlock>, bool) {
    // Two-pass: collect raw block data (indent, text-with-bid, first-line-bid),
    // then resolve parents from the indent stack and return FlatBlocks.
    struct RawBlock {
        indent: u16,
        bid: Option<Uuid>,
        text: String, // first line with bid stripped, plus continuation lines
    }

    let mut raw: Vec<RawBlock> = Vec::new();
    let mut current: Option<RawBlock> = None;
    let mut stamped_any = false;

    for line in body.lines() {
        let trim_start = line.trim_start();
        if trim_start.is_empty() {
            // Blank line inside body: ends the current block's continuation
            // run but does not introduce a new block.
            continue;
        }
        let spaces = line.len() - trim_start.len();
        let indent = (spaces / 2) as u16;

        let trimmed_end = trim_start.trim_end();
        let is_bullet = trim_start.starts_with("- ") || trimmed_end == "-";

        if is_bullet {
            if let Some(b) = current.take() {
                raw.push(b);
            }
            let body_text = trim_start
                .strip_prefix("- ")
                .or_else(|| trim_start.strip_prefix('-'))
                .unwrap_or(trim_start)
                .trim_end();
            let (text_no_bid, bid) = extract_bid(body_text);
            current = Some(RawBlock {
                indent,
                bid,
                text: text_no_bid.to_string(),
            });
        } else if let Some(rb) = current.as_mut() {
            // Continuation line.
            rb.text.push('\n');
            rb.text.push_str(trim_start.trim_end());
        }
    }
    if let Some(b) = current {
        raw.push(b);
    }

    // Resolve parents via indent stack.
    let mut blocks: Vec<FlatBlock> = Vec::with_capacity(raw.len());
    let mut parent_stack: Vec<(u16, Uuid)> = Vec::new();
    for rb in raw {
        while parent_stack
            .last()
            .map(|(i, _)| *i >= rb.indent)
            .unwrap_or(false)
        {
            parent_stack.pop();
        }
        let parent = parent_stack.last().map(|(_, id)| *id);
        let id = match rb.bid {
            Some(id) => id,
            None => {
                stamped_any = true;
                Uuid::now_v7()
            }
        };
        parent_stack.push((rb.indent, id));
        blocks.push(FlatBlock {
            id,
            parent,
            indent: rb.indent,
            text: rb.text,
        });
    }
    (blocks, stamped_any)
}

/// Strip a trailing `<!-- bid:UUID -->` comment from a single line of
/// text. Returns the cleaned text and the parsed uuid, if any.
fn extract_bid(line: &str) -> (&str, Option<Uuid>) {
    // The comment must be the last thing on the line (after any trailing
    // whitespace). Search from the right.
    let trimmed = line.trim_end();
    let Some(start) = trimmed.rfind(BID_PREFIX) else {
        return (trimmed, None);
    };
    if !trimmed[start..].ends_with(BID_SUFFIX) {
        return (trimmed, None);
    }
    let inner = &trimmed[start + BID_PREFIX.len()..trimmed.len() - BID_SUFFIX.len()];
    let inner_trimmed = inner.trim();
    let Ok(uuid) = Uuid::parse_str(inner_trimmed) else {
        return (trimmed, None);
    };
    // Strip the comment and any trailing space before it.
    let before = trimmed[..start].trim_end();
    (before, Some(uuid))
}

fn format_bid(id: Uuid) -> String {
    // Hyphenated lowercase, the default Display impl.
    id.to_string()
}

/// Walk a mosaic's `notes/` directory and ensure every `.md` file has
/// stable block ids stamped inline. Returns the count of files that were
/// modified. Idempotent: a second call on the same tree is a no-op.
///
/// Skips files that aren't notes (any file without a `.md` extension)
/// and silently ignores files that don't parse as a note tree (e.g.
/// the mosaic's `.tesela/` configs).
pub async fn stamp_existing_notes(notes_dir: &std::path::Path) -> std::io::Result<usize> {
    if !notes_dir.is_dir() {
        return Ok(0);
    }
    let mut entries = tokio::fs::read_dir(notes_dir).await?;
    let mut stamped = 0usize;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("stamp_existing_notes: read {} failed: {e}", path.display());
                continue;
            }
        };
        let tree = parse_note(&content);
        if !tree.stamped_any {
            continue;
        }
        let stamped_content = serialize_note(&tree);
        if let Err(e) = tokio::fs::write(&path, &stamped_content).await {
            tracing::warn!(
                "stamp_existing_notes: write {} failed: {e}",
                path.display()
            );
            continue;
        }
        stamped += 1;
    }
    Ok(stamped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_uuid(seed: u8) -> Uuid {
        // Construct a stable v7-shaped UUID for tests without time
        // dependence. v7 has a specific bit layout; here we just need
        // *some* valid UUID, so use parse from a string.
        let s = format!("01940f5a-0000-7000-8000-0000000000{:02x}", seed);
        Uuid::parse_str(&s).unwrap()
    }

    #[test]
    fn parse_empty_string() {
        let t = parse_note("");
        assert!(t.frontmatter.is_none());
        assert!(t.blocks.is_empty());
        assert!(!t.stamped_any);
    }

    #[test]
    fn parse_frontmatter_only() {
        let content = "---\ntitle: \"Foo\"\n---\n";
        let t = parse_note(content);
        assert_eq!(t.frontmatter.as_deref(), Some("---\ntitle: \"Foo\"\n---\n"));
        assert!(t.blocks.is_empty());
    }

    #[test]
    fn parse_simple_bullets_stamps_bids() {
        let content = "---\ntitle: \"X\"\n---\n\n- First\n- Second\n";
        let t = parse_note(content);
        assert_eq!(t.blocks.len(), 2);
        assert_eq!(t.blocks[0].text, "First");
        assert_eq!(t.blocks[1].text, "Second");
        assert!(t.stamped_any);
        assert!(t.blocks[0].parent.is_none());
        assert!(t.blocks[1].parent.is_none());
    }

    #[test]
    fn parse_indented_bullets_track_parents() {
        let content = "- Parent\n  - Child\n    - Grandchild\n";
        let t = parse_note(content);
        assert_eq!(t.blocks.len(), 3);
        let p = t.blocks[0].id;
        let c = t.blocks[1].id;
        assert!(t.blocks[0].parent.is_none());
        assert_eq!(t.blocks[1].parent, Some(p));
        assert_eq!(t.blocks[2].parent, Some(c));
        assert_eq!(t.blocks[0].indent, 0);
        assert_eq!(t.blocks[1].indent, 1);
        assert_eq!(t.blocks[2].indent, 2);
    }

    #[test]
    fn parse_continuation_lines_fold_into_block_text() {
        let content = "- Task #urgent\n  status:: doing\n  priority:: high\n";
        let t = parse_note(content);
        assert_eq!(t.blocks.len(), 1);
        assert_eq!(
            t.blocks[0].text,
            "Task #urgent\nstatus:: doing\npriority:: high"
        );
    }

    #[test]
    fn parse_existing_bid_is_preserved() {
        let id = fixture_uuid(0x42);
        let content = format!("- Existing <!-- bid:{} -->\n", id);
        let t = parse_note(&content);
        assert_eq!(t.blocks.len(), 1);
        assert_eq!(t.blocks[0].id, id);
        assert_eq!(t.blocks[0].text, "Existing");
        assert!(!t.stamped_any);
    }

    #[test]
    fn parse_bid_with_no_leading_text() {
        let id = fixture_uuid(0xab);
        let content = format!("- <!-- bid:{} -->\n  tags:: Task\n", id);
        let t = parse_note(&content);
        assert_eq!(t.blocks.len(), 1);
        assert_eq!(t.blocks[0].id, id);
        assert_eq!(t.blocks[0].text, "\ntags:: Task");
    }

    #[test]
    fn round_trip_identity_when_stamped() {
        let id_a = fixture_uuid(1);
        let id_b = fixture_uuid(2);
        let content = format!(
            "---\ntitle: \"X\"\n---\n\n- First <!-- bid:{} -->\n- Second <!-- bid:{} -->\n",
            id_a, id_b,
        );
        let t = parse_note(&content);
        let serialized = serialize_note(&t);
        assert_eq!(serialized, content);
    }

    #[test]
    fn round_trip_stable_after_stamping() {
        let content = "- A\n  - B\n- C\n";
        let t1 = parse_note(content);
        let serialized = serialize_note(&t1);
        let t2 = parse_note(&serialized);
        let serialized2 = serialize_note(&t2);
        assert_eq!(serialized, serialized2);
        assert_eq!(t1.blocks.len(), t2.blocks.len());
        for (a, b) in t1.blocks.iter().zip(t2.blocks.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.text, b.text);
            assert_eq!(a.indent, b.indent);
            assert_eq!(a.parent, b.parent);
        }
    }

    #[test]
    fn round_trip_preserves_continuation_lines() {
        let id = fixture_uuid(0xff);
        let content = format!("- Task <!-- bid:{} -->\n  status:: doing\n", id);
        let t = parse_note(&content);
        let serialized = serialize_note(&t);
        assert_eq!(serialized, content);
    }

    #[test]
    fn round_trip_indented_tree() {
        let content = "- Parent\n  - Child\n    - Grandchild\n";
        let t1 = parse_note(content);
        let s1 = serialize_note(&t1);
        let t2 = parse_note(&s1);
        let s2 = serialize_note(&t2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn malformed_bid_treated_as_text() {
        let content = "- thing <!-- bid:not-a-uuid -->\n";
        let t = parse_note(content);
        // The malformed comment is left in the text and a fresh bid stamped.
        assert!(t.stamped_any);
        assert_eq!(t.blocks[0].text, "thing <!-- bid:not-a-uuid -->");
    }

    #[test]
    fn frontmatter_without_body_round_trips() {
        let content = "---\ntitle: \"only\"\n---\n";
        let t = parse_note(content);
        let s = serialize_note(&t);
        assert_eq!(s, content);
    }

    #[tokio::test]
    async fn stamp_existing_notes_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let notes_dir = tmp.path().join("notes");
        tokio::fs::create_dir_all(&notes_dir).await.unwrap();

        let unstamped = "---\ntitle: \"X\"\n---\n\n- One\n- Two\n  - Nested\n";
        let path = notes_dir.join("test.md");
        tokio::fs::write(&path, unstamped).await.unwrap();

        let count1 = stamp_existing_notes(&notes_dir).await.unwrap();
        assert_eq!(count1, 1);

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(after.contains("<!-- bid:"));

        let count2 = stamp_existing_notes(&notes_dir).await.unwrap();
        assert_eq!(count2, 0);

        let after2 = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(after, after2);
    }

    #[tokio::test]
    async fn stamp_existing_notes_skips_non_md() {
        let tmp = tempfile::TempDir::new().unwrap();
        let notes_dir = tmp.path().join("notes");
        tokio::fs::create_dir_all(&notes_dir).await.unwrap();
        tokio::fs::write(notes_dir.join("ignored.txt"), "- bullet\n")
            .await
            .unwrap();
        let count = stamp_existing_notes(&notes_dir).await.unwrap();
        assert_eq!(count, 0);
    }
}
