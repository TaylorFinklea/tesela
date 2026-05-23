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

/// Drop "bare leaf" bullets from a note's body. A block is bare if its
/// `text` (which includes any continuation/property sub-lines, since
/// those fold into `FlatBlock.text` during parse) is whitespace-only.
/// A block is a leaf if no later block sits at a deeper indent —
/// i.e. it owns no children.
///
/// Walks blocks back-to-front so dropping a bare child re-exposes its
/// parent as a leaf within the same pass: an empty parent whose only
/// child was also empty will collapse alongside it. Empty parents that
/// retain at least one non-bare descendant are preserved so the kept
/// child doesn't get orphaned at the wrong indent on the next parse.
///
/// Frontmatter survives unchanged. This is the canonical cleanup step
/// for any path that writes a note body — it mirrors the iOS client's
/// `droppingBareLeafBlocks` so writes from every client converge on the
/// same on-disk form.
pub fn prune_bare_leaf_blocks(content: &str) -> String {
    let mut tree = parse_note(content);
    // Walk back-to-front, tracking the indent of every block we've
    // decided to keep. A block has a deeper successor iff the most
    // recent kept block (which, in reverse order, is the block
    // immediately *after* it in the file) sits deeper than it.
    let mut keep = vec![true; tree.blocks.len()];
    let mut kept_indents: Vec<u16> = Vec::with_capacity(tree.blocks.len());
    for (idx, block) in tree.blocks.iter().enumerate().rev() {
        let has_deeper_successor = kept_indents
            .last()
            .map(|next_indent| *next_indent > block.indent)
            .unwrap_or(false);
        let bare = block.text.trim().is_empty();
        if bare && !has_deeper_successor {
            keep[idx] = false;
        } else {
            kept_indents.push(block.indent);
        }
    }
    tree.blocks = tree
        .blocks
        .into_iter()
        .zip(keep)
        .filter_map(|(b, k)| if k { Some(b) } else { None })
        .collect();
    serialize_note(&tree)
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
                text: text_no_bid,
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

/// Public wrapper around [`extract_bid`] for callers that only need the
/// cleaned text (the presentational form, with `<!-- bid:UUID -->`
/// stripped). `parse_blocks` uses it to build `ParsedBlock.text` so
/// rendered surfaces — agenda rows, inbox previews, search hits —
/// never leak the on-disk identifier.
pub fn strip_bid_comment(line: &str) -> String {
    extract_bid(line).0
}

/// Strip `<!-- bid:UUID -->` comments from a line of text. Returns the
/// cleaned text and the first parsed UUID, if any. Tolerates the comment
/// appearing anywhere on the line (not only as a trailing token) so users
/// who type past a hidden bid in the editor don't drop block identity on
/// save. A single leading whitespace char is consumed along with each bid
/// to keep the join clean. Malformed bid comments are left in the text.
fn extract_bid(line: &str) -> (String, Option<Uuid>) {
    let input = line.trim_end();
    let mut out = String::with_capacity(input.len());
    let mut id: Option<Uuid> = None;
    let mut idx = 0;
    while idx < input.len() {
        let Some(rel_start) = input[idx..].find(BID_PREFIX) else {
            out.push_str(&input[idx..]);
            break;
        };
        let start = idx + rel_start;
        let after_prefix = start + BID_PREFIX.len();
        let Some(rel_end) = input[after_prefix..].find(BID_SUFFIX) else {
            out.push_str(&input[idx..]);
            break;
        };
        let inner_end = after_prefix + rel_end;
        let end = inner_end + BID_SUFFIX.len();
        match Uuid::parse_str(input[after_prefix..inner_end].trim()) {
            Ok(uuid) => {
                let preceding_end = if start > idx
                    && matches!(input.as_bytes()[start - 1], b' ' | b'\t')
                {
                    start - 1
                } else {
                    start
                };
                out.push_str(&input[idx..preceding_end]);
                if id.is_none() {
                    id = Some(uuid);
                }
                idx = end;
            }
            Err(_) => {
                out.push_str(&input[idx..after_prefix]);
                idx = after_prefix;
            }
        }
    }
    (out.trim_end().to_string(), id)
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
    fn text_typed_past_hidden_bid_rejoins_cleanly() {
        // Editor hides the bid as atomic, but End-of-line + typing can still
        // land characters in the doc *after* the bid comment. On save the
        // parser must reconstruct the visible text and preserve the id.
        let id = fixture_uuid(0x42);
        let content = format!("- foo <!-- bid:{} -->bar\n", id);
        let t = parse_note(&content);
        assert!(!t.stamped_any, "existing bid should be reused, not re-stamped");
        assert_eq!(t.blocks.len(), 1);
        assert_eq!(t.blocks[0].id, id);
        assert_eq!(t.blocks[0].text, "foobar");
        // And the serializer puts the bid back at the canonical end position.
        let serialized = serialize_note(&t);
        assert_eq!(serialized, format!("- foobar <!-- bid:{} -->\n", id));
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

    // ── prune_bare_leaf_blocks ───────────────────────────────────────────

    #[test]
    fn prune_drops_trailing_empty_bullets() {
        let id = fixture_uuid(0x01);
        let content = format!(
            "- Real content <!-- bid:{} -->\n- <!-- bid:{} -->\n- <!-- bid:{} -->\n",
            id,
            fixture_uuid(0x02),
            fixture_uuid(0x03),
        );
        let pruned = prune_bare_leaf_blocks(&content);
        assert_eq!(pruned, format!("- Real content <!-- bid:{} -->\n", id));
    }

    #[test]
    fn prune_is_no_op_when_all_blocks_have_content() {
        let content = format!(
            "- First <!-- bid:{} -->\n- Second <!-- bid:{} -->\n",
            fixture_uuid(0x10),
            fixture_uuid(0x11),
        );
        let pruned = prune_bare_leaf_blocks(&content);
        assert_eq!(pruned, content);
    }

    #[test]
    fn prune_keeps_empty_parent_with_kept_child() {
        // The parent has no text, but it owns a child with content — we
        // must keep both so the child doesn't get orphaned at the wrong
        // indent on the next parse.
        let parent_id = fixture_uuid(0x20);
        let child_id = fixture_uuid(0x21);
        let content = format!(
            "- <!-- bid:{} -->\n  - Child text <!-- bid:{} -->\n",
            parent_id, child_id,
        );
        let pruned = prune_bare_leaf_blocks(&content);
        assert_eq!(pruned, content);
    }

    #[test]
    fn prune_recursively_drops_empty_parent_and_empty_child() {
        // Both parent and child are bare. Walking back-to-front, the
        // child is dropped first (it's a leaf), which re-exposes the
        // parent as a leaf, so it's dropped too.
        let parent_id = fixture_uuid(0x30);
        let child_id = fixture_uuid(0x31);
        let kept_id = fixture_uuid(0x32);
        let content = format!(
            "- <!-- bid:{} -->\n  - <!-- bid:{} -->\n- Kept <!-- bid:{} -->\n",
            parent_id, child_id, kept_id,
        );
        let pruned = prune_bare_leaf_blocks(&content);
        assert_eq!(pruned, format!("- Kept <!-- bid:{} -->\n", kept_id));
    }

    #[test]
    fn prune_preserves_frontmatter() {
        let kept_id = fixture_uuid(0x40);
        let drop_id = fixture_uuid(0x41);
        let content = format!(
            "---\ntitle: \"Daily\"\ntags: [daily]\n---\n\n- Kept <!-- bid:{} -->\n- <!-- bid:{} -->\n",
            kept_id, drop_id,
        );
        let pruned = prune_bare_leaf_blocks(&content);
        assert_eq!(
            pruned,
            format!(
                "---\ntitle: \"Daily\"\ntags: [daily]\n---\n\n- Kept <!-- bid:{} -->\n",
                kept_id,
            ),
        );
    }
}
