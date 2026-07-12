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
//! - Bullet lines (`- text`) retain their outliner structure. Indent is
//!   two spaces per level, matching the existing `parse_blocks` in
//!   [`crate::block`].
//! - Continuation lines (indented lines following a bullet, used for
//!   properties like `status:: doing`) are folded into the parent
//!   block's `text` joined by newlines.
//! - Non-bullet headings, prose regions, and fenced regions are lifted
//!   into ordinary top-level blocks. This keeps one authoritative block
//!   model while making the parse structurally full-coverage. Writers
//!   that run automatically over arbitrary external files must still
//!   gate canonicalization — see [`stamp_existing_notes`].

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
    /// Page-level properties — `key:: value` lines at the top of the
    /// body, before any bullet (Logseq page properties). Ordered for
    /// deterministic serialization. These are the `query::` / `type::` /
    /// `sort::` lines that the bullet-only parser previously DROPPED
    /// (silent data loss on any block op to such a note). Block-level
    /// properties (indented `key:: value` under a bullet) are NOT here —
    /// those still fold into the owning block's `text`.
    pub page_properties: Vec<(String, String)>,
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
    /// Ordered container properties materialized as `key:: value`
    /// continuation lines AFTER the block's prose. Values are already
    /// canonical-stringified (the materializer formats scalars / joins
    /// multi-value before populating this). These are SEPARATE from any
    /// legacy `key:: value` lines folded into `text` — the engine
    /// populates this from the block's typed `props` container, while
    /// `parse_note` leaves it empty (the parser still folds in-text
    /// property continuations into `text`). Empty for ordinary blocks.
    #[serde(default)]
    pub properties: Vec<(String, String)>,
}

/// Format used to render block id comments. Hyphenated 36-char form for
/// human readability in the raw markdown.
pub const BID_PREFIX: &str = "<!-- bid:";
pub const BID_SUFFIX: &str = " -->";

/// Parse a note file's contents into a [`NoteTree`].
///
/// Stamps fresh UUIDv7 ids on any unstamped or lifted blocks. Frontmatter
/// is captured verbatim. Non-bullet body regions are represented as
/// ordinary top-level blocks (see module docs).
pub fn parse_note(content: &str) -> NoteTree {
    let (frontmatter, body) = split_frontmatter(content);
    let (page_properties, rest) = split_page_properties(body);
    let (blocks, stamped_any) = parse_body_blocks(rest);
    NoteTree {
        frontmatter,
        page_properties,
        blocks,
        stamped_any,
    }
}

/// Consume leading page-property lines (`key:: value`) from the top of
/// the body, before any bullet. Returns the parsed properties (ordered)
/// and the remaining body for block parsing.
///
/// Rules (Logseq page-property semantics):
/// - Skip leading blank lines (the frontmatter separator).
/// - A line at indent 0 matching `key:: value` (key = word chars / `-`)
///   that is NOT a bullet is a page property.
/// - The first bullet or non-property line ends collection; everything
///   from there is the block body.
/// - If no properties are found, the body is returned UNCHANGED (so the
///   block parser sees the original, including leading blanks).
fn split_page_properties(body: &str) -> (Vec<(String, String)>, &str) {
    let mut props: Vec<(String, String)> = Vec::new();
    let mut consumed_end = 0usize; // byte offset up to which we've consumed
    for (line, line_range) in line_spans(body) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            // Blank line: part of the leading separator only while we
            // haven't hit content yet. Tentatively consume it; if no
            // props ultimately follow we return the original body.
            consumed_end = line_range.end;
            continue;
        }
        // A bullet ends page-property collection.
        if trimmed.starts_with("- ") || trimmed == "-" {
            break;
        }
        // Page properties are unindented.
        let indented = line.len() != trimmed.len();
        if indented {
            break;
        }
        match parse_property_line(trimmed) {
            Some((k, v)) => {
                props.push((k, v));
                consumed_end = line_range.end;
            }
            None => break,
        }
    }
    if props.is_empty() {
        (props, body)
    } else {
        (props, &body[consumed_end..])
    }
}

/// Iterate lines with their byte ranges (so we can slice the remainder).
/// Each yielded range covers the line plus its trailing `\n` if present.
fn line_spans(s: &str) -> impl Iterator<Item = (&str, std::ops::Range<usize>)> {
    let mut start = 0usize;
    std::iter::from_fn(move || {
        if start >= s.len() {
            return None;
        }
        let rest = &s[start..];
        let (line, next) = match rest.find('\n') {
            Some(nl) => (&rest[..nl], start + nl + 1),
            None => (rest, s.len()),
        };
        let range = start..next;
        start = next;
        Some((line, range))
    })
}

/// Parse a single `key:: value` property line. Key is one or more word
/// characters or `-`; the separator is `::` followed by an optional
/// space; value is the rest (may be empty, may contain `::`). Returns
/// `None` if the line isn't a property.
fn parse_property_line(line: &str) -> Option<(String, String)> {
    let idx = line.find("::")?;
    let key = &line[..idx];
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let after = &line[idx + 2..];
    // Require the `::` to be followed by a space or end-of-line, so a
    // bare `http://x` style isn't mistaken for a property.
    let value = match after.strip_prefix(' ') {
        Some(v) => v,
        None if after.is_empty() => "",
        None => return None,
    };
    Some((key.to_string(), value.to_string()))
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
/// A block is "bare" when it carries neither prose nor any materialized
/// container property. The property check matters once props leave `text`
/// and live in the block's typed container (the engine populates
/// `FlatBlock.properties` from `prop_keys`; `parse_note` leaves it empty).
/// An eager-seeded but empty props map materializes to an empty Vec, so a
/// blank bullet stays bare — mirrors iOS `droppingBareLeafBlocks`, which
/// also checks `properties.isEmpty`.
fn block_is_bare(block: &FlatBlock) -> bool {
    block.text.trim().is_empty() && block.properties.is_empty()
}

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
        let bare = block_is_bare(block);
        if bare && !has_deeper_successor {
            keep[idx] = false;
        } else {
            kept_indents.push(block.indent);
        }
    }
    // Bail when nothing needs pruning. Critical for non-outliner
    // notes (Query / Tag / Property / Template) whose bodies carry
    // `key:: value` lines rather than bullets — `parse_note`'s
    // round-trip only retains bullet blocks, so a no-op re-serialize
    // would silently strip the user's `query::` definition. Returning
    // the original content keeps every byte intact when we have
    // nothing to do anyway.
    let any_dropped = keep.iter().any(|k| !k);
    if !any_dropped {
        return content.to_string();
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
        // separator line before the body if there's any body to write.
        if !tree.blocks.is_empty() || !tree.page_properties.is_empty() {
            out.push('\n');
        }
    }
    // Page properties, in order, immediately after the separator and
    // before the blocks. `key:: value` per line (canonical form).
    for (key, value) in &tree.page_properties {
        out.push_str(key);
        out.push_str(":: ");
        out.push_str(value);
        out.push('\n');
    }
    for block in &tree.blocks {
        let indent_spaces = "  ".repeat(block.indent as usize);
        let first = block.text.lines().next().unwrap_or("");
        let fence_first = fence_marker(first).is_some();
        // A leading fence cannot share its opener with a trailing bid:
        // that would change the info string. Emit a bid-only bullet and
        // put the complete fence in continuation lines instead.
        out.push_str(&indent_spaces);
        out.push_str("- ");
        if !first.is_empty() && !fence_first {
            out.push_str(first);
            out.push(' ');
        }
        out.push_str(BID_PREFIX);
        out.push_str(&format_bid(block.id));
        out.push_str(BID_SUFFIX);
        out.push('\n');
        if fence_first {
            // `split` (unlike `lines`) retains a terminal empty logical
            // line. That matters for an unclosed fence ending in a blank
            // payload line, which the scanner stores in `text` as a
            // trailing newline.
            for line in block.text.split('\n') {
                out.push_str(&indent_spaces);
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
        } else {
            // Ordinary block continuation lines keep the historical
            // normalization of a terminal line ending.
            for line in block.text.lines().skip(1) {
                out.push_str(&indent_spaces);
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
        }
        // Container property lines: rendered AFTER the prose, in the given
        // order, one `key:: value` continuation line each (Logseq reflow).
        // Values are already canonical-stringified by the materializer.
        for (key, value) in &block.properties {
            out.push_str(&indent_spaces);
            out.push_str("  ");
            out.push_str(key);
            out.push_str(":: ");
            out.push_str(value);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FenceMarker {
    byte: u8,
    width: usize,
}

/// Recognize a Markdown fence opener after up to three preserved spaces.
/// Both backtick and tilde fences are supported. The caller retains the
/// original line; this helper is only scanner state.
fn fence_marker(line: &str) -> Option<FenceMarker> {
    let bytes = line.as_bytes();
    let leading = bytes.iter().take_while(|byte| **byte == b' ').count();
    if leading > 3 || leading >= bytes.len() {
        return None;
    }
    let byte = bytes[leading];
    if byte != b'`' && byte != b'~' {
        return None;
    }
    let width = bytes[leading..]
        .iter()
        .take_while(|candidate| **candidate == byte)
        .count();
    (width >= 3).then_some(FenceMarker { byte, width })
}

fn closes_fence(line: &str, marker: FenceMarker) -> bool {
    let bytes = line.as_bytes();
    let leading = bytes.iter().take_while(|byte| **byte == b' ').count();
    if leading > 3 || leading >= bytes.len() || bytes[leading] != marker.byte {
        return false;
    }
    let width = bytes[leading..]
        .iter()
        .take_while(|candidate| **candidate == marker.byte)
        .count();
    width >= marker.width
        && bytes[leading + width..]
            .iter()
            .all(|byte| byte.is_ascii_whitespace())
}

fn continuation_line(line: &str, block_indent: u16) -> Option<&str> {
    let expected = (usize::from(block_indent) + 1) * 2;
    let prefix = line.as_bytes().get(..expected)?;
    prefix
        .iter()
        .all(|byte| *byte == b' ')
        .then(|| &line[expected..])
}

fn bullet_line(line: &str) -> Option<(u16, String, Option<Uuid>)> {
    let spaces = line
        .as_bytes()
        .iter()
        .take_while(|byte| **byte == b' ')
        .count();
    let rest = &line[spaces..];
    let body = if let Some(body) = rest.strip_prefix("- ") {
        body
    } else if rest.trim_end() == "-" {
        ""
    } else {
        return None;
    };
    let (text, bid) = extract_bid(body.trim_end());
    Some(((spaces / 2) as u16, text, bid))
}

fn parse_body_blocks(body: &str) -> (Vec<FlatBlock>, bool) {
    // Two-pass: first scan every nonblank body line into either an
    // existing bullet or a lifted top-level region, then resolve parents
    // from the indent stack.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SourceKind {
        Bullet,
        Lifted,
    }

    struct RawBlock {
        indent: u16,
        bid: Option<Uuid>,
        text: String,
        source: SourceKind,
        fence: Option<FenceMarker>,
    }

    let mut raw: Vec<RawBlock> = Vec::new();
    let mut current: Option<RawBlock> = None;
    let mut stamped_any = false;

    for (line, _) in line_spans(body) {
        // Once a fence opens it owns every line through a matching close,
        // including blank and bullet-looking lines. For a fence nested in
        // a bullet, strip exactly the list continuation prefix and retain
        // every additional space.
        if let Some((source, indent, marker)) = current.as_ref().and_then(|block| {
            block
                .fence
                .map(|marker| (block.source, block.indent, marker))
        }) {
            let content_line = match source {
                SourceKind::Bullet => continuation_line(line, indent).unwrap_or(line),
                SourceKind::Lifted => line,
            };
            let closes = closes_fence(content_line, marker);
            let block = current.as_mut().expect("fence state has a current block");
            block.text.push('\n');
            block.text.push_str(content_line);
            if closes {
                block.fence = None;
            }
            if closes && source == SourceKind::Lifted {
                raw.push(current.take().expect("lifted fence block exists"));
            }
            continue;
        }

        if line.trim().is_empty() {
            if let Some(b) = current.take() {
                raw.push(b);
            }
            continue;
        }

        if let Some((indent, text, bid)) = bullet_line(line) {
            if let Some(b) = current.take() {
                raw.push(b);
            }
            let fence = fence_marker(&text);
            current = Some(RawBlock {
                indent,
                bid,
                text,
                source: SourceKind::Bullet,
                fence,
            });
            continue;
        }

        if current
            .as_ref()
            .is_some_and(|block| block.source == SourceKind::Bullet)
        {
            let (indent, text_is_empty) = {
                let block = current.as_ref().expect("checked current bullet");
                (block.indent, block.text.is_empty())
            };
            if let Some(continuation) = continuation_line(line, indent) {
                let opening_fence = fence_marker(continuation);
                let block = current.as_mut().expect("checked current bullet");
                // Preserve the legacy leading newline for an empty bullet's
                // ordinary continuation/property line. A fence-first block
                // is the one exception: its bid-only canonical bullet is
                // scaffolding and must not enter the visible block text.
                if !text_is_empty || opening_fence.is_none() {
                    block.text.push('\n');
                }
                block.text.push_str(continuation);
                block.fence = opening_fence;
                continue;
            }
            raw.push(current.take().expect("checked current bullet"));
        }

        if current
            .as_ref()
            .is_some_and(|block| block.source == SourceKind::Lifted)
        {
            if let Some(opening_fence) = fence_marker(line) {
                raw.push(current.take().expect("checked lifted block"));
                current = Some(RawBlock {
                    indent: 0,
                    bid: None,
                    text: line.to_string(),
                    source: SourceKind::Lifted,
                    fence: Some(opening_fence),
                });
            } else {
                let block = current.as_mut().expect("checked lifted block");
                block.text.push('\n');
                block.text.push_str(line);
            }
            continue;
        }

        current = Some(RawBlock {
            indent: 0,
            bid: None,
            text: line.to_string(),
            source: SourceKind::Lifted,
            fence: fence_marker(line),
        });
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
            properties: Vec::new(),
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

/// Public wrapper around [`extract_bid`] for callers that need the
/// canonical block id without the surrounding `<!-- bid:UUID -->`
/// scaffolding. Returns the parsed UUID if the line carries one;
/// `None` for bid-less local blocks the server hasn't stamped yet.
/// Phase 2.2 (2026-05-27): used by `parse_blocks` so `ParsedBlock`
/// surfaces the bid to clients, which then re-emit it on save —
/// without this, every save round emits a bid-less line, the server
/// stamps a fresh UUID, and `apply_block_upsert` appends a new file
/// row instead of updating the existing one (visible as duplicate
/// blocks across sync round-trips).
pub fn parse_bid(line: &str) -> Option<Uuid> {
    extract_bid(line).1
}

/// Strip the rightmost valid `<!-- bid:UUID -->` comment from a line of
/// text and return its UUID. The serializer owns that rightmost position;
/// any earlier valid-looking comment may be literal lifted content and is
/// preserved. The owned comment may still appear before typed text, so
/// users who type past a hidden bid do not drop block identity on save. A
/// single leading whitespace char is consumed with it to keep the join
/// clean. Malformed bid comments are left in the text.
fn extract_bid(line: &str) -> (String, Option<Uuid>) {
    let input = line.trim_end();
    let mut idx = 0;
    let mut owned: Option<(usize, usize, Uuid)> = None;
    while idx < input.len() {
        let Some(rel_start) = input[idx..].find(BID_PREFIX) else {
            break;
        };
        let start = idx + rel_start;
        let after_prefix = start + BID_PREFIX.len();
        let Some(rel_end) = input[after_prefix..].find(BID_SUFFIX) else {
            break;
        };
        let inner_end = after_prefix + rel_end;
        let end = inner_end + BID_SUFFIX.len();
        if let Ok(uuid) = Uuid::parse_str(input[after_prefix..inner_end].trim()) {
            owned = Some((start, end, uuid));
        }
        idx = end;
    }

    let Some((start, end, id)) = owned else {
        return (input.to_string(), None);
    };
    let preceding_end = if start > 0 && matches!(input.as_bytes()[start - 1], b' ' | b'\t') {
        start - 1
    } else {
        start
    };
    let mut out = String::with_capacity(input.len() - (end - preceding_end));
    out.push_str(&input[..preceding_end]);
    out.push_str(&input[end..]);
    (out.trim_end().to_string(), Some(id))
}

fn format_bid(id: Uuid) -> String {
    // Hyphenated lowercase, the default Display impl.
    id.to_string()
}

/// Remove every valid `<!-- bid:UUID -->` comment (plus the single space
/// or tab immediately preceding it, mirroring [`extract_bid`]'s join rule)
/// from `content`, leaving every other byte untouched — including
/// whitespace, blank lines, and malformed bid comments.
///
/// This is the comparison key for the stamp gate below: two contents that
/// are equal after this strip differ ONLY by bid comments, which is
/// exactly the change stamping is allowed to make.
fn strip_valid_bid_comments(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut idx = 0;
    while idx < content.len() {
        let Some(rel_start) = content[idx..].find(BID_PREFIX) else {
            out.push_str(&content[idx..]);
            break;
        };
        let start = idx + rel_start;
        let after_prefix = start + BID_PREFIX.len();
        let Some(rel_end) = content[after_prefix..].find(BID_SUFFIX) else {
            out.push_str(&content[idx..]);
            break;
        };
        let inner_end = after_prefix + rel_end;
        let end = inner_end + BID_SUFFIX.len();
        match Uuid::parse_str(content[after_prefix..inner_end].trim()) {
            Ok(_) => {
                let preceding_end =
                    if start > idx && matches!(content.as_bytes()[start - 1], b' ' | b'\t') {
                        start - 1
                    } else {
                        start
                    };
                out.push_str(&content[idx..preceding_end]);
                idx = end;
            }
            Err(_) => {
                // Malformed bid: keep it verbatim (the parser leaves it in
                // block text, so the serializer re-emits it too).
                out.push_str(&content[idx..after_prefix]);
                idx = after_prefix;
            }
        }
    }
    out
}

/// Whether rewriting `original` as `stamped` changes ONLY bid comments —
/// i.e. the parse→serialize round trip preserved every other byte. False
/// whenever the round trip would reshape source syntax: lifted non-bullet
/// regions become canonical bullets, separators/indentation may normalize,
/// or a missing frontmatter separator line is inserted.
pub fn stamp_is_content_preserving(original: &str, stamped: &str) -> bool {
    strip_valid_bid_comments(original) == strip_valid_bid_comments(stamped)
}

/// Whether `canonical` has the same modeled note structure as `original`
/// after parsing, ignoring generated block ids and their derived parent ids.
///
/// This is intentionally broader than [`stamp_is_content_preserving`]: it
/// permits the explicit canonical lift from raw Markdown regions into
/// ordinary bullet blocks, while still rejecting lost/reordered text,
/// changed indentation, frontmatter, or page properties. Automatic startup
/// stamping continues to use the byte-conservative predicate above.
pub fn canonicalization_preserves_structure(original: &str, canonical: &str) -> bool {
    let original = parse_note(original);
    let canonical = parse_note(canonical);

    original.frontmatter == canonical.frontmatter
        && original.page_properties == canonical.page_properties
        && original.blocks.len() == canonical.blocks.len()
        && original
            .blocks
            .iter()
            .zip(&canonical.blocks)
            .all(|(left, right)| {
                left.indent == right.indent
                    && left.text == right.text
                    && left.properties == right.properties
            })
}

/// Walk a mosaic's `notes/` directory and ensure every `.md` file has
/// stable block ids stamped inline. Returns the count of files that were
/// modified. Idempotent: a second call on the same tree is a no-op.
///
/// Skips files that aren't notes (any file without a `.md` extension)
/// and silently ignores files that don't parse as a note tree (e.g.
/// the mosaic's `.tesela/` configs).
///
/// ## Conservative startup rewrite gate (audit A9b, 2026-06-09)
/// This runs at every server startup over files Tesela did not necessarily
/// write (hand-authored notes, external drops, migration artifacts). Even
/// though parsing now preserves non-bullet content structurally, automatic
/// startup must not silently canonicalize its source syntax. Before writing,
/// the stamped output is compared against the original with bid comments
/// stripped from both: if anything else would change, the file is left
/// byte-for-byte intact and a warning names it. Explicit engine hydration may
/// use [`canonicalization_preserves_structure`] before accepting that reshape.
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
        // Lossy-round-trip gate: only write when the rewrite changes
        // nothing but bid comments. See the function docs.
        if !stamp_is_content_preserving(&content, &stamped_content) {
            tracing::warn!(
                "stamp_existing_notes: skipping {} — stamping would \
                 canonicalize non-bullet or non-canonical source syntax; \
                 leaving it byte-identical and unstamped",
                path.display()
            );
            continue;
        }
        if let Err(e) = tokio::fs::write(&path, &stamped_content).await {
            tracing::warn!("stamp_existing_notes: write {} failed: {e}", path.display());
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
    fn parse_page_properties_only_note() {
        // A query/page-property page — NO bullets. Previously parsed to
        // zero blocks and serialized empty (silent data loss).
        let content =
            "---\ntitle: Saved\n---\n\nquery:: kind:page\nsort:: modified desc\nicon:: clock\n";
        let t = parse_note(content);
        assert_eq!(
            t.page_properties,
            vec![
                ("query".to_string(), "kind:page".to_string()),
                ("sort".to_string(), "modified desc".to_string()),
                ("icon".to_string(), "clock".to_string()),
            ]
        );
        assert!(t.blocks.is_empty());
    }

    #[test]
    fn round_trip_page_properties_only() {
        let content = "---\ntitle: Saved\n---\n\nquery:: kind:page\nsort:: modified desc\n";
        let t = parse_note(content);
        assert_eq!(
            serialize_note(&t),
            content,
            "byte round-trip for clean input"
        );
    }

    #[test]
    fn round_trip_page_property_value_with_colons() {
        // Values may contain `::` and operators — only the first `:: `
        // splits key from value.
        let content = "query:: status != done AND type IN (task, issue)\n";
        let t = parse_note(content);
        assert_eq!(t.page_properties.len(), 1);
        assert_eq!(t.page_properties[0].0, "query");
        assert_eq!(
            t.page_properties[0].1,
            "status != done AND type IN (task, issue)"
        );
        assert_eq!(serialize_note(&t), content);
    }

    #[test]
    fn page_properties_then_bullets() {
        let content = "type:: ChatGPT\n- a bullet\n";
        let t = parse_note(content);
        assert_eq!(
            t.page_properties,
            vec![("type".to_string(), "ChatGPT".to_string())]
        );
        assert_eq!(t.blocks.len(), 1);
        assert_eq!(t.blocks[0].text, "a bullet");
        // Re-parse the serialized form is stable.
        let s = serialize_note(&t);
        let t2 = parse_note(&s);
        assert_eq!(t2.page_properties, t.page_properties);
        assert_eq!(t2.blocks.len(), 1);
        assert_eq!(serialize_note(&t2), s);
    }

    #[test]
    fn bullet_starting_note_has_no_page_properties() {
        // Regression: a normal bullet note must not absorb anything as
        // page properties, and block-level `key:: value` continuations
        // still fold into block text.
        let content = "- Task\n  status:: doing\n";
        let t = parse_note(content);
        assert!(t.page_properties.is_empty());
        assert_eq!(t.blocks.len(), 1);
        assert!(t.blocks[0].text.contains("status:: doing"));
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

    fn structural_blocks(tree: &NoteTree) -> Vec<(u16, &str)> {
        tree.blocks
            .iter()
            .map(|block| (block.indent, block.text.as_str()))
            .collect()
    }

    fn assert_structural_round_trip(content: &str) -> String {
        let parsed = parse_note(content);
        let canonical = serialize_note(&parsed);
        let reparsed = parse_note(&canonical);

        assert_eq!(reparsed.frontmatter, parsed.frontmatter);
        assert_eq!(reparsed.page_properties, parsed.page_properties);
        assert_eq!(structural_blocks(&reparsed), structural_blocks(&parsed));
        assert_eq!(
            reparsed
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            parsed
                .blocks
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>()
        );
        assert!(!reparsed.stamped_any);
        assert_eq!(serialize_note(&reparsed), canonical);
        assert!(canonicalization_preserves_structure(content, &canonical));

        canonical
    }

    #[test]
    fn lifts_heading_and_prose_around_existing_bullets() {
        let bullet_id = fixture_uuid(0x90);
        let content = format!(
            "# Heading\n\nFirst prose line\nsecond prose line\n\n- Existing <!-- bid:{} -->\n\nTrailing prose\n",
            bullet_id,
        );

        let tree = parse_note(&content);
        assert_eq!(
            structural_blocks(&tree),
            vec![
                (0, "# Heading"),
                (0, "First prose line\nsecond prose line"),
                (0, "Existing"),
                (0, "Trailing prose"),
            ]
        );
        assert_structural_round_trip(&content);
    }

    #[test]
    fn lifts_page_properties_then_raw_body_and_all_raw_pages() {
        let with_properties = "type:: Reference\nsort:: title\n\n# Catalog\n\nAlpha\nBeta";
        let tree = parse_note(with_properties);
        assert_eq!(
            tree.page_properties,
            vec![
                ("type".to_string(), "Reference".to_string()),
                ("sort".to_string(), "title".to_string()),
            ]
        );
        assert_eq!(
            structural_blocks(&tree),
            vec![(0, "# Catalog"), (0, "Alpha\nBeta")]
        );
        assert_structural_round_trip(with_properties);

        let all_raw = "One paragraph\ncontinues here\n\nSecond paragraph";
        assert_eq!(
            structural_blocks(&parse_note(all_raw)),
            vec![
                (0, "One paragraph\ncontinues here"),
                (0, "Second paragraph"),
            ]
        );
        assert_structural_round_trip(all_raw);
    }

    #[test]
    fn canonicalizes_query_fence_as_bid_only_bullet_without_leading_newline() {
        let content = "```query\n{:find [?b]\n :where\n [?b :block/content \"- literal\"]}\n```";
        let tree = parse_note(content);
        assert_eq!(structural_blocks(&tree), vec![(0, content)]);

        let canonical = assert_structural_round_trip(content);
        assert!(canonical.starts_with("- <!-- bid:"));
        assert!(canonical.contains(" -->\n  ```query\n"));
        assert_eq!(parse_note(&canonical).blocks[0].text, content);
    }

    #[test]
    fn fence_payload_preserves_blank_lines_extra_indent_and_internal_bullets() {
        let content = "```text\n+---+\n\n    - payload, not a block  \n  | x |\n```\n- Real block";
        let tree = parse_note(content);
        assert_eq!(tree.blocks.len(), 2);
        assert_eq!(
            tree.blocks[0].text,
            "```text\n+---+\n\n    - payload, not a block  \n  | x |\n```"
        );
        assert_eq!(tree.blocks[1].text, "Real block");
        assert_structural_round_trip(content);
    }

    #[test]
    fn fence_inside_bullet_owns_bullet_like_lines_until_close() {
        let content = "- Example\n  before\n  ```query\n  - payload, not a child\n    extra indent\n  ```\n  after\n- Sibling";
        let tree = parse_note(content);
        assert_eq!(tree.blocks.len(), 2);
        assert_eq!(
            tree.blocks[0].text,
            "Example\nbefore\n```query\n- payload, not a child\n  extra indent\n```\nafter"
        );
        assert_eq!(tree.blocks[1].text, "Sibling");
        assert_structural_round_trip(content);
    }

    #[test]
    fn unclosed_fence_owns_the_rest_of_the_body() {
        let content = "```query\n- payload, not a block\n\n  still payload";
        let tree = parse_note(content);
        assert_eq!(tree.blocks.len(), 1);
        assert_eq!(tree.blocks[0].text, content);
        assert_structural_round_trip(content);
    }

    #[test]
    fn unclosed_fence_preserves_a_terminal_blank_payload_line() {
        let content = "```text\npayload\n\n";
        let tree = parse_note(content);
        assert_eq!(tree.blocks[0].text, "```text\npayload\n");
        assert_structural_round_trip(content);
    }

    #[test]
    fn longer_tilde_fence_ignores_shorter_marker_and_splits_adjacent_prose() {
        let content = "before\n~~~~query\n~~~\n- payload, not a block\n~~~~\nafter";
        let tree = parse_note(content);
        assert_eq!(
            structural_blocks(&tree),
            vec![
                (0, "before"),
                (0, "~~~~query\n~~~\n- payload, not a block\n~~~~"),
                (0, "after"),
            ]
        );
        assert_structural_round_trip(content);
    }

    #[test]
    fn valid_bid_comment_in_lifted_prose_remains_content() {
        let literal_bid = fixture_uuid(0xa1);
        let content = format!("literal metadata <!-- bid:{} --> remains", literal_bid);
        let tree = parse_note(&content);
        assert_eq!(tree.blocks[0].text, content);
        assert_ne!(tree.blocks[0].id, literal_bid);

        let canonical = assert_structural_round_trip(&content);
        assert!(canonical.contains(&content));
        assert_eq!(parse_note(&canonical).blocks[0].text, content);
    }

    #[test]
    fn continuation_removes_only_expected_indent_and_short_indent_lifts_raw() {
        let content = "- Diagram\n    +---+\n      | x |\n one-space raw\n    deeper raw";
        let tree = parse_note(content);
        assert_eq!(tree.blocks.len(), 2);
        assert_eq!(tree.blocks[0].text, "Diagram\n  +---+\n    | x |");
        assert_eq!(tree.blocks[1].text, " one-space raw\n    deeper raw");
        assert_structural_round_trip(content);
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
        assert!(
            !t.stamped_any,
            "existing bid should be reused, not re-stamped"
        );
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

    /// Audit A9b (2026-06-09): startup stamping once deleted non-bullet
    /// content. The parser now lifts that content, but the stamper remains
    /// deliberately byte-conservative: it must not silently change an
    /// external heading/prose page into canonical bullets on first launch.
    #[tokio::test]
    async fn stamp_existing_notes_preserves_non_bullet_heading() {
        let tmp = tempfile::TempDir::new().unwrap();
        let notes_dir = tmp.path().join("notes");
        tokio::fs::create_dir_all(&notes_dir).await.unwrap();

        let original = "# Heading\n\nSome prose paragraph.\n\n- a bullet\n- another\n";
        let path = notes_dir.join("external-drop.md");
        tokio::fs::write(&path, original).await.unwrap();

        let count = stamp_existing_notes(&notes_dir).await.unwrap();

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(
            after.contains("# Heading"),
            "startup stamping must NOT destroy the heading; file now:\n{after}"
        );
        assert!(
            after.contains("Some prose paragraph."),
            "startup stamping must NOT destroy prose; file now:\n{after}"
        );
        assert_eq!(
            after, original,
            "a lossy-round-trip note must be skipped byte-for-byte"
        );
        assert_eq!(count, 0, "the skipped note must not count as stamped");
    }

    /// The gate must still ALLOW stamping of canonical notes (frontmatter
    /// + blank separator + bullets, two-space indents), including
    /// partially-stamped ones — only the bid comments may differ.
    #[test]
    fn stamp_gate_allows_canonical_and_partially_stamped_notes() {
        let canonical = "---\ntitle: \"X\"\n---\n\n- One\n- Two\n  - Nested\n";
        let tree = parse_note(canonical);
        assert!(stamp_is_content_preserving(
            canonical,
            &serialize_note(&tree)
        ));

        let partly = format!(
            "---\ntitle: \"X\"\n---\n\n- Done <!-- bid:{} -->\n- New bullet\n",
            fixture_uuid(0x77),
        );
        let tree = parse_note(&partly);
        assert!(stamp_is_content_preserving(&partly, &serialize_note(&tree)));

        // And it must REFUSE when a heading would be structurally preserved
        // but source-canonicalized into a bullet.
        let heading = "# H\n\n- bullet\n";
        let tree = parse_note(heading);
        assert!(!stamp_is_content_preserving(
            heading,
            &serialize_note(&tree)
        ));
    }

    /// Same gate, another canonicalizing shape: a standalone paragraph after
    /// a bullet is now lifted safely, but startup still leaves the external
    /// source byte-identical rather than rewriting it as a new bullet.
    #[tokio::test]
    async fn stamp_existing_notes_skips_prose_after_bullet() {
        let tmp = tempfile::TempDir::new().unwrap();
        let notes_dir = tmp.path().join("notes");
        tokio::fs::create_dir_all(&notes_dir).await.unwrap();

        let original = "- a bullet\n\nstandalone paragraph\n";
        let path = notes_dir.join("folded.md");
        tokio::fs::write(&path, original).await.unwrap();

        let count = stamp_existing_notes(&notes_dir).await.unwrap();

        let after = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(
            after, original,
            "a note whose round-trip canonicalizes prose must be skipped"
        );
        assert_eq!(count, 0);
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
    fn prune_is_byte_identical_noop_on_non_outliner_body() {
        // Regression: `parse_note → serialize_note` only retains bullet
        // blocks, so a no-op re-serialize would strip `key:: value`
        // body content from non-outliner notes (the `inbox` Query note,
        // Tag pages, Property pages, Templates). The pruner must bail
        // when nothing changes so those bodies survive every PUT.
        let content = "---\ntitle: \"Views\"\ntype: \"Query\"\ntags: []\n---\n\nquery:: kind:block -has:status tag-in:Task\n";
        assert_eq!(prune_bare_leaf_blocks(content), content);
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

    #[test]
    fn prune_keeps_property_only_block() {
        // A block whose prose is empty but that carries a materialized
        // container property is NOT bare — it must survive the prune so
        // its property isn't silently dropped. The pruner operates on
        // engine-built trees where `FlatBlock.properties` is populated
        // (the parser never sets it), so exercise the bareness predicate
        // directly.
        let block = FlatBlock {
            id: fixture_uuid(0x60),
            parent: None,
            indent: 0,
            text: "   ".to_string(),
            properties: vec![("status".to_string(), "doing".to_string())],
        };
        assert!(!block_is_bare(&block));
    }

    #[test]
    fn prune_drops_block_with_empty_props() {
        // The empty-seeded-map guard: eager-seeding mints an EMPTY props
        // map per block, which materializes to an empty `properties` Vec.
        // Empty text + empty properties stays bare so a blank bullet is
        // still pruned.
        let block = FlatBlock {
            id: fixture_uuid(0x61),
            parent: None,
            indent: 0,
            text: "   ".to_string(),
            properties: Vec::new(),
        };
        assert!(block_is_bare(&block));
    }

    // ── container property materialization (P1.5) ────────────────────────

    #[test]
    fn serialize_emits_block_properties_after_prose() {
        // A block's container properties render as `key:: value`
        // continuation lines, in the given order, AFTER the prose line.
        let id = fixture_uuid(0x50);
        let block = FlatBlock {
            id,
            parent: None,
            indent: 0,
            text: "Task".to_string(),
            properties: vec![
                ("status".to_string(), "doing".to_string()),
                ("priority".to_string(), "3".to_string()),
            ],
        };
        let tree = NoteTree {
            frontmatter: None,
            page_properties: Vec::new(),
            blocks: vec![block],
            stamped_any: false,
        };
        assert_eq!(
            serialize_note(&tree),
            format!(
                "- Task <!-- bid:{} -->\n  status:: doing\n  priority:: 3\n",
                id,
            ),
        );
    }

    #[test]
    fn serialize_block_properties_respect_indent() {
        // Continuation property lines indent two spaces deeper than the
        // owning bullet, matching the prose-continuation convention.
        let parent = fixture_uuid(0x60);
        let child = fixture_uuid(0x61);
        let tree = NoteTree {
            frontmatter: None,
            page_properties: Vec::new(),
            blocks: vec![
                FlatBlock {
                    id: parent,
                    parent: None,
                    indent: 0,
                    text: "Parent".to_string(),
                    properties: Vec::new(),
                },
                FlatBlock {
                    id: child,
                    parent: Some(parent),
                    indent: 1,
                    text: "Child".to_string(),
                    properties: vec![("status".to_string(), "done".to_string())],
                },
            ],
            stamped_any: false,
        };
        assert_eq!(
            serialize_note(&tree),
            format!(
                "- Parent <!-- bid:{} -->\n  - Child <!-- bid:{} -->\n    status:: done\n",
                parent, child,
            ),
        );
    }

    #[test]
    fn serialize_block_properties_follow_multiline_prose() {
        // The property lines come AFTER all prose lines (block text with an
        // embedded newline keeps its continuation prose first).
        let id = fixture_uuid(0x70);
        let tree = NoteTree {
            frontmatter: None,
            page_properties: Vec::new(),
            blocks: vec![FlatBlock {
                id,
                parent: None,
                indent: 0,
                text: "First line\nsecond line".to_string(),
                properties: vec![("k".to_string(), "v".to_string())],
            }],
            stamped_any: false,
        };
        assert_eq!(
            serialize_note(&tree),
            format!("- First line <!-- bid:{} -->\n  second line\n  k:: v\n", id,),
        );
    }

    #[test]
    fn flatblock_properties_defaults_empty_via_serde() {
        // The new field is `#[serde(default)]` so older serialized
        // FlatBlocks (no `properties` key) deserialize cleanly.
        let id = fixture_uuid(0x80);
        let json = format!(r#"{{"id":"{}","parent":null,"indent":0,"text":"hi"}}"#, id,);
        let block: FlatBlock = serde_json::from_str(&json).unwrap();
        assert!(block.properties.is_empty());
        assert_eq!(block.text, "hi");
    }
}
