//! Token-style query language for filtering blocks and pages.
//!
//! Mirrors `web/src/lib/query-language.ts` so the same DSL parses identically
//! on the client (for stub previews) and on the server (for execution).
//!
//! # Grammar
//!
//! ```text
//! query   := token (whitespace token)*
//! token   := negation? key ':' op? value
//! negation:= '-'
//! key     := identifier ("kind", "tag", "status", "has", or any property name)
//! op      := '>=' | '<=' | '>' | '<' | '!='   (optional, default '=')
//! value   := bareword (stops at whitespace) | quoted string ("...")
//! ```
//!
//! Examples:
//!   - `kind:block tag:Task -status:done` — block-kind query, tag filter, negated status
//!   - `kind:page note_type:Project` — page-kind query
//!   - `tag:Task priority:>=3 deadline:<=2026-05-01` — comparison ops
//!   - `has:deadline -has:status` — has/lacks-property predicates
//!
//! # Special pseudo-keys
//!
//! - `kind:block | kind:page` — narrows the result set. Default if absent: `block`.
//! - `has:foo` (op `=`) — block has property `foo` regardless of value.
//! - `has:foo` (op `!=`) — block lacks property `foo`. Equivalently `-has:foo`.
//! - `tag:foo` — block's resolved tag chain (direct + inherited) includes `foo`.

use crate::block::ParsedBlock;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use ts_rs::TS;

/// Comparison operator on a filter value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub enum QueryOp {
    /// `=` — case-insensitive equality.
    Eq,
    /// `!=` — case-insensitive inequality (and "missing property" matches `!=`).
    Ne,
    /// `>` — numeric or ISO-date strictly greater.
    Gt,
    /// `<` — numeric or ISO-date strictly less.
    Lt,
    /// `>=`
    Gte,
    /// `<=`
    Lte,
}

impl QueryOp {
    fn parse(s: &str, idx: usize) -> (QueryOp, usize) {
        let bytes = s.as_bytes();
        if idx + 1 < bytes.len() {
            match (bytes[idx], bytes[idx + 1]) {
                (b'>', b'=') => return (QueryOp::Gte, idx + 2),
                (b'<', b'=') => return (QueryOp::Lte, idx + 2),
                (b'!', b'=') => return (QueryOp::Ne, idx + 2),
                _ => {}
            }
        }
        if idx < bytes.len() {
            match bytes[idx] {
                b'>' => return (QueryOp::Gt, idx + 1),
                b'<' => return (QueryOp::Lt, idx + 1),
                _ => {}
            }
        }
        (QueryOp::Eq, idx)
    }
}

/// Which entity the query targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Match individual blocks across the corpus. Default.
    #[default]
    Block,
    /// Match notes (pages).
    Page,
}

/// One filter token in the parsed query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct QueryFilter {
    /// Lowercased filter key (e.g. `tag`, `status`, `has`, or a custom property).
    pub key: String,
    /// Comparison op. Negation flips `Eq` to `Ne` etc., applied during parse.
    pub op: QueryOp,
    /// The filter value. Empty string for `has:` predicates.
    pub value: String,
}

/// A parsed query: a `Kind` plus a flat list of filters that all must match.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct ParsedQuery {
    pub kind: Kind,
    pub filters: Vec<QueryFilter>,
}

/// One row in a [`QueryResult`] — either a block or a whole page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct QueryItem {
    /// Present when the row is a block; `None` for page-kind rows.
    pub block_id: Option<String>,
    /// The containing page (or, for page-kind rows, the page itself).
    pub page_id: String,
    /// Page title.
    pub title: String,
    /// Display text — block's first line, or the page title for page-kind rows.
    pub text: String,
    /// Ancestor chain in the block's containing page (page title, then root
    /// block text, …). Empty for page-kind rows.
    pub parent_breadcrumb: Vec<String>,
    /// Mirrors the query's [`Kind`].
    pub kind: Kind,
    /// First tag in the block's resolved chain (used for kind glyphs).
    pub primary_tag: Option<String>,
    /// Block-level properties (or page metadata for page rows).
    pub properties: std::collections::HashMap<String, String>,
    /// `note_type` of the containing page — used by the inbox post-filter to
    /// exclude blocks from system pages (Tag, Property, Query, Template).
    /// `None` for plain pages.
    pub page_note_type: Option<String>,
}

/// Grouped query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct QueryGroup {
    /// Group label (e.g. `"DOING"`, `"TODAY"`, or empty for "ungrouped").
    pub key: String,
    /// Number of items in this group.
    pub count: u32,
    pub items: Vec<QueryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct QueryResult {
    pub groups: Vec<QueryGroup>,
}

/// Per-day marker counts surfaced in the rail's mini calendar (Phase 9.2).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct DayMarkers {
    /// Blocks with `deadline:: <date>` due that day.
    pub tasks: u32,
    /// Blocks with `scheduled:: <date>` on that day.
    pub events: u32,
    /// Whether a daily note exists for this date.
    pub notes: bool,
}

/// Calendar marks payload — keys are `YYYY-MM-DD` strings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct CalendarMarks {
    pub days: std::collections::HashMap<String, DayMarkers>,
}

/// Whether an agenda row represents a task or a calendar event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
#[serde(rename_all = "lowercase")]
pub enum AgendaRowKind {
    Task,
    Event,
}

/// Which dated property the row's anchor came from. Surfaced so clients
/// can distinguish ⚑ deadline (a date the work *must* be done by — a
/// commitment to others / a hard cutoff) from 🕒 scheduled (a date the
/// user picked for *doing* the work — a self-commitment). Drives the
/// Todoist-style split of the Overdue bucket into two sub-buckets
/// because rescheduling a missed deadline is semantically different
/// from rescheduling a missed planned-do date.
///
/// When a block carries both `deadline::` and `scheduled::`, the agenda
/// query anchors on `scheduled` (the "when am I doing it" answer), so
/// `field` is `Scheduled` in that case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
#[serde(rename_all = "lowercase")]
pub enum AgendaField {
    Deadline,
    Scheduled,
}

/// One row in an agenda view — either a task or an event within a date window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct AgendaRow {
    pub block_id: String,
    pub source_note_id: String,
    /// YYYY-MM-DD of the occurrence (anchor for non-recurring; a projected
    /// future date for recurring projections).
    pub occurrence_date: String,
    /// Optional HH:MM if the source date carries a time.
    pub occurrence_time: Option<String>,
    pub kind: AgendaRowKind,
    /// `true` if `occurrence_date` is before today at query time.
    pub overdue: bool,
    /// The block's `recurring::` value (canonical string) when projecting; `None` otherwise.
    pub recurrence: Option<String>,
    /// `true` for the block's current anchor; `false` for projected future occurrences.
    pub is_anchor: bool,
    /// The block's text (sans `status::`/`deadline::`/etc. property lines).
    pub text: String,
    /// `status::` value (`"todo"`, `"done"`, ...) for task rows; `None` for events.
    pub status: Option<String>,
    /// Which dated property the row's anchor came from. See [`AgendaField`].
    pub field: AgendaField,
}

/// Extract the first ISO date (`YYYY-MM-DD`) anywhere in a property value.
/// Handles bare dates AND wiki-link wrapped (`[[2026-04-15]]`) forms.
pub fn extract_iso_date(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let n = bytes.len();
    let mut i = 0usize;
    while i + 10 <= n {
        let slice = &bytes[i..i + 10];
        if slice[4] == b'-'
            && slice[7] == b'-'
            && slice[..4].iter().all(|c| c.is_ascii_digit())
            && slice[5..7].iter().all(|c| c.is_ascii_digit())
            && slice[8..10].iter().all(|c| c.is_ascii_digit())
        {
            return Some(value[i..i + 10].to_string());
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod agenda_row_tests {
    use super::*;

    #[test]
    fn agenda_row_round_trips_via_serde() {
        let r = AgendaRow {
            block_id: "b1".to_string(),
            source_note_id: "2026-05-22".to_string(),
            occurrence_date: "2026-05-22".to_string(),
            occurrence_time: Some("14:00".to_string()),
            kind: AgendaRowKind::Task,
            overdue: false,
            recurrence: Some("weekly".to_string()),
            is_anchor: true,
            text: "do this thing".to_string(),
            status: Some("todo".to_string()),
            field: AgendaField::Scheduled,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: AgendaRow = serde_json::from_str(&json).unwrap();
        assert_eq!(back.block_id, "b1");
        assert_eq!(back.kind, AgendaRowKind::Task);
        assert_eq!(back.field, AgendaField::Scheduled);
    }
}

#[cfg(test)]
mod date_tests {
    use super::extract_iso_date;
    #[test]
    fn extracts_bare_date() {
        assert_eq!(extract_iso_date("2026-04-15"), Some("2026-04-15".into()));
    }
    #[test]
    fn extracts_wiki_wrapped() {
        assert_eq!(
            extract_iso_date("[[2026-04-15]]Write doc"),
            Some("2026-04-15".into())
        );
    }
    #[test]
    fn no_date_returns_none() {
        assert_eq!(extract_iso_date("low"), None);
    }
}

/// Parse a DSL string into a [`ParsedQuery`]. Unrecognized syntax is dropped
/// silently — matches the TS parser at `web/src/lib/query-language.ts:32`.
pub fn parse_query(input: &str) -> ParsedQuery {
    let bytes = input.as_bytes();
    let mut filters = Vec::new();
    let mut kind = Kind::Block;
    let mut explicit_kind = false;

    let mut i = 0usize;
    while i < bytes.len() {
        // Skip whitespace
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        // Optional negation prefix
        let mut negated = false;
        if bytes[i] == b'-' {
            negated = true;
            i += 1;
        }

        // Read key — alphanumeric + underscore + hyphen (so `has-link` parses as one key)
        let key_start = i;
        while i < bytes.len() && is_key_char(bytes[i]) {
            i += 1;
        }
        if i == key_start {
            // No key after '-' or unrecognized character — skip one byte to avoid loop
            i += 1;
            continue;
        }
        let key = input[key_start..i].to_ascii_lowercase();

        // Expect ':'. If missing, skip the token entirely.
        if i >= bytes.len() || bytes[i] != b':' {
            continue;
        }
        i += 1;

        // Optional comparison op
        let (op_raw, next) = QueryOp::parse(input, i);
        i = next;

        // Value: quoted or bareword
        let mut value = String::new();
        if i < bytes.len() && bytes[i] == b'"' {
            i += 1;
            let val_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            value.push_str(&input[val_start..i]);
            if i < bytes.len() && bytes[i] == b'"' {
                i += 1;
            }
        } else {
            let val_start = i;
            while i < bytes.len() && !(bytes[i] as char).is_whitespace() {
                i += 1;
            }
            value.push_str(&input[val_start..i]);
        }

        // `has:foo` may legitimately have no value when written as `-has:foo` —
        // but `has:foo` always carries a key-name as value. Empty values for
        // non-`has` filters are dropped.
        if key != "has" && value.is_empty() {
            continue;
        }

        // Apply negation by flipping the op.
        let op = if negated { invert(op_raw) } else { op_raw };

        // `kind:` is consumed into ParsedQuery.kind, not a filter.
        if key == "kind" {
            // `-kind:foo` is meaningless; ignore the negation.
            if matches!(value.to_ascii_lowercase().as_str(), "page" | "pages") {
                kind = Kind::Page;
            } else {
                kind = Kind::Block;
            }
            explicit_kind = true;
            continue;
        }

        filters.push(QueryFilter { key, op, value });
    }

    let _ = explicit_kind; // reserved for future "implicit kind warnings"
    ParsedQuery { kind, filters }
}

fn is_key_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn invert(op: QueryOp) -> QueryOp {
    match op {
        QueryOp::Eq => QueryOp::Ne,
        QueryOp::Ne => QueryOp::Eq,
        QueryOp::Gt => QueryOp::Lte,
        QueryOp::Lt => QueryOp::Gte,
        QueryOp::Gte => QueryOp::Lt,
        QueryOp::Lte => QueryOp::Gt,
    }
}

/// Comparison helper that tries number → ISO-date → case-insensitive string.
/// Mirrors `query-language.ts:compare`.
fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    if let (Ok(an), Ok(bn)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
        return an.partial_cmp(&bn).unwrap_or(Ordering::Equal);
    }
    if is_iso_date(a) && is_iso_date(b) {
        return a.cmp(b); // ISO dates are lexicographically sortable
    }
    a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())
}

/// `true` when `note_id` is the canonical `YYYY-MM-DD` daily-note id.
/// Cheap byte-by-byte check — no regex compile. Matches the same shape
/// the iOS InboxView uses in `DATE_ID_RE` and the web `isInboxableRow`.
fn is_daily_note_id(note_id: &str) -> bool {
    let b = note_id.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[0..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}

/// `true` when `note_type` is one of Tesela's system page types — the
/// pages that hold tag definitions, property metadata, saved queries,
/// or templates rather than authored content. Drives `on:system-pages`.
fn is_system_note_type(note_type: &str) -> bool {
    matches!(note_type, "Tag" | "Property" | "Query" | "Template")
}

/// `true` when `text`'s first non-whitespace run is a markdown heading
/// marker (1–6 `#`s followed by whitespace). Drives `is:heading`.
/// Seven-or-more `#`s in a row aren't a heading in CommonMark, so we
/// cap at six. A bare `#urgent` (no whitespace after the `#`s) is a
/// hashtag, not a heading — also rejected.
fn is_heading_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    let mut hashes = 0usize;
    for ch in trimmed.chars() {
        if ch == '#' {
            hashes += 1;
            if hashes > 6 {
                return false;
            }
        } else {
            return hashes >= 1 && ch.is_whitespace();
        }
    }
    // String was all `#`s — not a heading (no body).
    false
}

fn is_iso_date(s: &str) -> bool {
    s.len() >= 10
        && s.as_bytes()[4] == b'-'
        && s.as_bytes()[7] == b'-'
        && s[..4].chars().all(|c| c.is_ascii_digit())
        && s[5..7].chars().all(|c| c.is_ascii_digit())
        && s[8..10].chars().all(|c| c.is_ascii_digit())
}

fn apply_op(actual: &str, op: QueryOp, expected: &str) -> bool {
    use std::cmp::Ordering;
    match op {
        QueryOp::Eq => actual.eq_ignore_ascii_case(expected),
        QueryOp::Ne => !actual.eq_ignore_ascii_case(expected),
        op => {
            let cmp = compare(actual, expected);
            match op {
                QueryOp::Gt => cmp == Ordering::Greater,
                QueryOp::Lt => cmp == Ordering::Less,
                QueryOp::Gte => cmp != Ordering::Less,
                QueryOp::Lte => cmp != Ordering::Greater,
                _ => unreachable!(),
            }
        }
    }
}

/// Check whether a parsed block matches every filter in the query. Used by
/// callers that already have blocks in memory (e.g. the indexer's broad-filter
/// → in-memory-refine pattern in `SqliteIndex::get_typed_blocks`).
pub fn block_matches(block: &ParsedBlock, q: &ParsedQuery) -> bool {
    q.filters.iter().all(|f| filter_matches(block, f))
}

fn filter_matches(block: &ParsedBlock, f: &QueryFilter) -> bool {
    match f.key.as_str() {
        // Tag-system Phase 16 DSL extensions:
        //   `tag:foo`      — either page-level or block-level (current default)
        //   `pagetag:foo`  — page-level (frontmatter) only
        //   `blocktag:foo` — block-level only (in content or via `tags::`)
        //
        // The synthetic page-block constructed in `SqliteIndex` for page-kind
        // queries fills `tags` from the page's frontmatter, while real block-
        // kind queries fill it from the block parser. The current behavior is
        // already kind-dependent; `pagetag:` and `blocktag:` are aliases that
        // make the intent explicit at the query level.
        "tag" | "pagetag" | "blocktag" => {
            let needle = f.value.to_ascii_lowercase();
            // For `blocktag:` we deliberately skip inherited_tags — block-level
            // means "this block carries the tag," not "inherited from an
            // ancestor." For `tag:` and `pagetag:` we keep the inherited chain
            // so a child of a tagged parent still matches.
            let include_inherited = f.key != "blocktag";
            let has_tag = if include_inherited {
                block
                    .tags
                    .iter()
                    .chain(block.inherited_tags.iter())
                    .any(|t| t.eq_ignore_ascii_case(&needle))
            } else {
                block.tags.iter().any(|t| t.eq_ignore_ascii_case(&needle))
            };
            match f.op {
                QueryOp::Eq => has_tag,
                QueryOp::Ne => !has_tag,
                _ => false, // comparison ops not meaningful for tags
            }
        }
        "has-link" => {
            // Block contains `[[<value>]]` (case-insensitive) anywhere in raw_text.
            let needle = format!("[[{}]]", f.value);
            let raw = block.raw_text.to_ascii_lowercase();
            let present = raw.contains(&needle.to_ascii_lowercase());
            match f.op {
                QueryOp::Eq => present,
                QueryOp::Ne => !present,
                _ => false,
            }
        }
        "has" => {
            // `has:foo` checks property presence regardless of value.
            let needle = f.value.to_ascii_lowercase();
            let present = block
                .properties
                .keys()
                .any(|k| k.eq_ignore_ascii_case(&needle));
            match f.op {
                QueryOp::Eq => present,
                QueryOp::Ne => !present,
                _ => false,
            }
        }
        "page" => {
            // `page:<note_id>` matches blocks whose containing note id
            // equals the value (case-insensitive, mirroring the rest of
            // the DSL). Drives the Inbox "Hide all from this page"
            // action — the negated form (`-page:foo`) is the common
            // case, written into the saved query whenever the user
            // hides a noisy page.
            let matched = block.note_id.eq_ignore_ascii_case(&f.value);
            match f.op {
                QueryOp::Eq => matched,
                QueryOp::Ne => !matched,
                _ => false,
            }
        }
        "block" => {
            // `block:<bid>` matches by the block's deterministic id
            // (`<note_id>:<line_number>`). Drives "Hide this block" —
            // a per-row escape hatch for individual noisy rows that
            // page / type filtering can't catch.
            let matched = block.id.eq_ignore_ascii_case(&f.value);
            match f.op {
                QueryOp::Eq => matched,
                QueryOp::Ne => !matched,
                _ => false,
            }
        }
        "tag-in" => {
            // `tag-in:A,B,C` matches blocks tagged with ANY of the
            // comma-separated values. The only OR primitive in the
            // DSL. Drives the Inbox Types chip-group: multi-selecting
            // types composes a single `tag-in:` clause.
            //
            // Empty value list (`tag-in:`) degrades to "matches
            // nothing" for `Eq` / "matches everything" for `Ne` so a
            // chip-group with no active chips doesn't accidentally
            // exclude every row.
            let needles: Vec<String> = f
                .value
                .split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            let matched = if needles.is_empty() {
                false
            } else {
                block
                    .tags
                    .iter()
                    .chain(block.inherited_tags.iter())
                    .any(|t| needles.iter().any(|n| t.eq_ignore_ascii_case(n)))
            };
            match f.op {
                QueryOp::Eq => matched,
                QueryOp::Ne => !matched,
                _ => false,
            }
        }
        "on" => {
            // `on:daily-page` / `on:system-pages` filter blocks by their
            // *containing page's* identity — daily journal entries vs
            // system pages (Tag/Property/Query/Template). Drives the
            // Inbox chips that keep these classes of blocks out of
            // triage. `parent_note_type` is populated by the SQL
            // candidate path (`execute_block_query`); the standalone
            // `parse_blocks` form leaves it None, in which case
            // `system-pages` falls through to `false`.
            //
            // Unknown `on:*` value → false-on-Eq, true-on-Ne so a
            // misspelled chip degrades to "lets everything through"
            // rather than silently excluding every row.
            let matched = match f.value.to_ascii_lowercase().as_str() {
                "daily-page" => is_daily_note_id(&block.note_id),
                "system-pages" => block
                    .parent_note_type
                    .as_deref()
                    .map(is_system_note_type)
                    .unwrap_or(false),
                _ => false,
            };
            match f.op {
                QueryOp::Eq => matched,
                QueryOp::Ne => !matched,
                _ => false,
            }
        }
        "is" => {
            // `is:heading` matches blocks whose first non-whitespace run
            // is a markdown heading marker (`#` through `######` followed
            // by whitespace). Drives the Inbox chip that filters out
            // section-divider bullets from reference notes. Other `is:`
            // predicates can be added later; an unknown one returns
            // `false` for `Eq` and `true` for `Ne` so a `-is:unknown`
            // chip degrades gracefully (lets everything through) rather
            // than silently excluding every row.
            let matched = match f.value.to_ascii_lowercase().as_str() {
                "heading" => is_heading_text(&block.text),
                _ => false,
            };
            match f.op {
                QueryOp::Eq => matched,
                QueryOp::Ne => !matched,
                _ => false,
            }
        }
        key => {
            // Property lookup — case-insensitive key match.
            let actual = block
                .properties
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.as_str());
            match (actual, f.op) {
                (None, QueryOp::Ne) => true, // missing != value matches
                (None, _) => false,
                (Some(a), op) => apply_op(a, op, &f.value),
            }
        }
    }
}

// ----------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::ParsedBlock;
    use std::collections::HashMap;

    fn block_with(tags: Vec<&str>, props: &[(&str, &str)]) -> ParsedBlock {
        let mut p = HashMap::new();
        for (k, v) in props {
            p.insert((*k).to_string(), (*v).to_string());
        }
        ParsedBlock {
            id: "n:1".into(),
            text: "x".into(),
            raw_text: "- x".into(),
            tags: tags.iter().map(|s| (*s).into()).collect(),
            inline_tags: vec![],
            trailing_tags: vec![],
            inherited_tags: vec![],
            properties: p,
            indent_level: 0,
            note_id: "n".into(),
            parent_note_type: None,
        }
    }

    #[test]
    fn parses_kind_default_to_block() {
        let q = parse_query("tag:Task");
        assert_eq!(q.kind, Kind::Block);
        assert_eq!(q.filters.len(), 1);
        assert_eq!(q.filters[0].key, "tag");
        assert_eq!(q.filters[0].value, "Task");
    }

    #[test]
    fn parses_kind_page() {
        let q = parse_query("kind:page note_type:Project");
        assert_eq!(q.kind, Kind::Page);
        assert_eq!(q.filters.len(), 1);
        assert_eq!(q.filters[0].key, "note_type");
    }

    #[test]
    fn parses_negation() {
        let q = parse_query("-status:done");
        assert_eq!(q.filters[0].op, QueryOp::Ne);
        assert_eq!(q.filters[0].value, "done");
    }

    #[test]
    fn parses_comparison_ops() {
        let q = parse_query("priority:>=3 deadline:<=2026-05-01");
        assert_eq!(q.filters[0].op, QueryOp::Gte);
        assert_eq!(q.filters[1].op, QueryOp::Lte);
    }

    #[test]
    fn parses_quoted_value() {
        let q = parse_query(r#"tag:"To Read""#);
        assert_eq!(q.filters[0].value, "To Read");
    }

    #[test]
    fn parses_has_predicate() {
        let q = parse_query("has:deadline");
        assert_eq!(q.filters[0].key, "has");
        assert_eq!(q.filters[0].value, "deadline");
        assert_eq!(q.filters[0].op, QueryOp::Eq);
    }

    #[test]
    fn parses_negated_has_as_ne() {
        let q = parse_query("-has:status");
        assert_eq!(q.filters[0].key, "has");
        assert_eq!(q.filters[0].op, QueryOp::Ne);
    }

    /// Helper to build a block with arbitrary text — needed for the
    /// `is:heading` tests since the basic `block_with` helper hardcodes
    /// `text = "x"`.
    fn block_with_text(text: &str) -> ParsedBlock {
        ParsedBlock {
            id: "n:1".into(),
            text: text.into(),
            raw_text: format!("- {}", text),
            tags: vec![],
            inline_tags: vec![],
            trailing_tags: vec![],
            inherited_tags: vec![],
            properties: std::collections::HashMap::new(),
            indent_level: 0,
            note_id: "n".into(),
            parent_note_type: None,
        }
    }

    #[test]
    fn block_matches_is_heading_positive() {
        // `is:heading` matches blocks whose text starts with a markdown
        // heading marker (`#` … `######` followed by whitespace) — these
        // are section dividers inside reference pages, not actionable
        // outliner blocks. Drives the Inbox filter chip that lets users
        // keep heading-style bullets out of triage queues.
        let q = parse_query("is:heading");
        assert!(block_matches(&block_with_text("### Raw Strings"), &q));
        assert!(block_matches(&block_with_text("# Top"), &q));
        assert!(block_matches(&block_with_text("###### Six"), &q));
        // Indented heading-like text still counts.
        assert!(block_matches(&block_with_text("   ## Indented"), &q));
    }

    #[test]
    fn block_matches_is_heading_negative() {
        let q = parse_query("is:heading");
        assert!(!block_matches(&block_with_text("Buy milk"), &q));
        // Hashtag at the start isn't a heading (no space after #s).
        assert!(!block_matches(&block_with_text("#urgent thing"), &q));
        // Seven `#`s aren't a markdown heading either.
        assert!(!block_matches(&block_with_text("####### Too many"), &q));
    }

    /// Build a block whose containing note id and parent note_type can
    /// be set. Drives `on:daily-page` / `on:system-pages` tests.
    fn block_on(note_id: &str, parent_note_type: Option<&str>) -> ParsedBlock {
        let mut b = block_with(vec![], &[]);
        b.note_id = note_id.into();
        b.parent_note_type = parent_note_type.map(String::from);
        b
    }

    #[test]
    fn block_matches_on_daily_page() {
        // `on:daily-page` matches when the block's note_id is the
        // canonical YYYY-MM-DD daily-note id. Drives the Inbox chip
        // that hides "untriaged" daily-page bullets — they're journal
        // captures, not triage items.
        let q = parse_query("on:daily-page");
        assert!(block_matches(&block_on("2026-05-23", None), &q));
        assert!(!block_matches(&block_on("python", None), &q));
        // Negated form excludes daily-page rows.
        let q_neg = parse_query("-on:daily-page");
        assert!(!block_matches(&block_on("2026-05-23", None), &q_neg));
        assert!(block_matches(&block_on("python", None), &q_neg));
    }

    #[test]
    fn block_matches_on_system_pages() {
        // `on:system-pages` is sugar for "parent note_type ∈
        // {Tag, Property, Query, Template}" — the Tesela page-type
        // taxonomy for non-content pages. Drives the Inbox chip that
        // keeps system-page bullets out of triage.
        let q = parse_query("on:system-pages");
        assert!(block_matches(&block_on("tag-page", Some("Tag")), &q));
        assert!(block_matches(&block_on("query-page", Some("Query")), &q));
        assert!(block_matches(&block_on("template-page", Some("Template")), &q));
        assert!(block_matches(&block_on("prop-page", Some("Property")), &q));
        assert!(!block_matches(&block_on("project", Some("Project")), &q));
        // Missing parent_note_type degrades to "doesn't match" rather
        // than panicking — the standalone parse_blocks path leaves it
        // None and should still be queryable safely.
        assert!(!block_matches(&block_on("note", None), &q));
        // Negated form is the common Inbox default.
        let q_neg = parse_query("-on:system-pages");
        assert!(!block_matches(&block_on("tag-page", Some("Tag")), &q_neg));
        assert!(block_matches(&block_on("project", Some("Project")), &q_neg));
    }

    #[test]
    fn block_matches_page_exact() {
        // `page:foo` matches blocks whose containing note_id is exactly
        // `foo`. Drives the Inbox "Hide all from this page" action and
        // the per-page-exclusion chips. Case-insensitive comparison
        // matches the rest of the DSL's behavior.
        let q = parse_query("page:python");
        assert!(block_matches(&block_on("python", None), &q));
        assert!(!block_matches(&block_on("javascript", None), &q));
        // Negated form is the common one (the hide action writes it).
        let qn = parse_query("-page:python");
        assert!(!block_matches(&block_on("python", None), &qn));
        assert!(block_matches(&block_on("project", None), &qn));
    }

    #[test]
    fn block_matches_block_exact() {
        // `block:<bid>` matches blocks whose id is exactly that bid.
        // Drives "Hide this block" — surgical exclusion of one specific
        // row when filtering by page or type is too coarse.
        let q = parse_query("block:python:5");
        let target = {
            let mut b = block_with(vec![], &[]);
            b.id = "python:5".into();
            b
        };
        let other = {
            let mut b = block_with(vec![], &[]);
            b.id = "python:6".into();
            b
        };
        assert!(block_matches(&target, &q));
        assert!(!block_matches(&other, &q));
    }

    #[test]
    fn block_matches_tag_in_or_over_values() {
        // `tag-in:Foo,Bar,Baz` matches blocks tagged with ANY of the
        // listed values (OR over values — the only OR primitive in the
        // DSL). Drives the Inbox Types chip-group: multi-select types
        // compose a single `tag-in:` clause.
        let q = parse_query("tag-in:Task,Domain,Issue");
        assert!(block_matches(&block_with(vec!["Task"], &[]), &q));
        assert!(block_matches(&block_with(vec!["Domain"], &[]), &q));
        assert!(block_matches(&block_with(vec!["Issue"], &[]), &q));
        // A block with a tag not in the set is excluded.
        assert!(!block_matches(&block_with(vec!["Person"], &[]), &q));
        // A block with no tags at all is excluded.
        assert!(!block_matches(&block_with(vec![], &[]), &q));
        // Case-insensitive match against the values.
        assert!(block_matches(&block_with(vec!["task"], &[]), &q));
    }

    #[test]
    fn block_matches_negated_tag_in() {
        // `-tag-in:Foo,Bar` excludes blocks tagged with ANY of the
        // listed values — set-membership exclusion. Useful for "show
        // me everything except these noisy categories."
        let q = parse_query("-tag-in:Task,Done");
        assert!(!block_matches(&block_with(vec!["Task"], &[]), &q));
        assert!(!block_matches(&block_with(vec!["Done"], &[]), &q));
        assert!(block_matches(&block_with(vec!["Note"], &[]), &q));
        assert!(block_matches(&block_with(vec![], &[]), &q));
    }

    #[test]
    fn block_matches_tag_in_empty_values_dropped_by_parser() {
        // `tag-in:` with no value gets dropped during parsing — same as
        // every other non-`has` clause with an empty value. The
        // resulting query has zero filters and matches every block
        // (vacuous AND). Documenting the behavior so future "make this
        // smarter" tweaks are deliberate.
        let q = parse_query("tag-in:");
        assert_eq!(q.filters.len(), 0);
        assert!(block_matches(&block_with(vec!["Task"], &[]), &q));
    }

    #[test]
    fn block_matches_negated_is_heading() {
        // `-is:heading` excludes heading-style blocks; common Inbox
        // default-on chip.
        let q = parse_query("-is:heading");
        assert!(block_matches(&block_with_text("Buy milk"), &q));
        assert!(!block_matches(&block_with_text("### Raw Strings"), &q));
    }

    #[test]
    fn drops_unrecognized_syntax() {
        let q = parse_query("not a query @ all tag:Task");
        // Tokens without ':' get dropped; only `tag:Task` survives.
        assert_eq!(q.filters.len(), 1);
        assert_eq!(q.filters[0].key, "tag");
    }

    #[test]
    fn block_matches_tag() {
        let q = parse_query("tag:Task");
        assert!(block_matches(&block_with(vec!["Task"], &[]), &q));
        assert!(!block_matches(&block_with(vec!["Note"], &[]), &q));
    }

    #[test]
    fn block_matches_negated_tag() {
        let q = parse_query("-tag:Done");
        assert!(block_matches(&block_with(vec!["Task"], &[]), &q));
        assert!(!block_matches(&block_with(vec!["Done"], &[]), &q));
    }

    // Tag-system Phase 16 — pagetag / blocktag DSL keys.

    fn block_with_inherited(
        tags: Vec<&str>,
        inherited: Vec<&str>,
    ) -> ParsedBlock {
        let mut b = block_with(tags, &[]);
        b.inherited_tags = inherited.iter().map(|s| (*s).into()).collect();
        b
    }

    #[test]
    fn pagetag_filter_aliases_tag_on_block_tags() {
        // `pagetag:` resolves through the same `tags` field as `tag:`. The
        // semantic distinction lives in the caller (page-kind query fills
        // `tags` from frontmatter); the filter itself behaves like `tag:`.
        let q = parse_query("pagetag:Task");
        assert!(block_matches(&block_with(vec!["Task"], &[]), &q));
        assert!(!block_matches(&block_with(vec!["Other"], &[]), &q));
    }

    #[test]
    fn pagetag_filter_matches_via_inherited_chain() {
        // `pagetag:` keeps the inherited chain (frontmatter-style "tag is on
        // the page" semantics).
        let q = parse_query("pagetag:Project");
        let b = block_with_inherited(vec![], vec!["Project"]);
        assert!(block_matches(&b, &q));
    }

    #[test]
    fn blocktag_filter_matches_only_direct_block_tags() {
        // `blocktag:` excludes inherited tags — must be literally on this block.
        let q = parse_query("blocktag:Task");
        let direct = block_with(vec!["Task"], &[]);
        assert!(block_matches(&direct, &q));

        let inherited_only = block_with_inherited(vec![], vec!["Task"]);
        assert!(!block_matches(&inherited_only, &q));
    }

    #[test]
    fn blocktag_filter_negation_works() {
        let q = parse_query("-blocktag:Done");
        let direct = block_with(vec!["Task"], &[]);
        assert!(block_matches(&direct, &q));

        let done_direct = block_with(vec!["Done"], &[]);
        assert!(!block_matches(&done_direct, &q));

        // Done in inherited chain only — `-blocktag:Done` still matches
        // because the literal-block check returns false.
        let done_inherited = block_with_inherited(vec!["Task"], vec!["Done"]);
        assert!(block_matches(&done_inherited, &q));
    }

    #[test]
    fn pagetag_filter_negation_works() {
        let q = parse_query("-pagetag:Done");
        let direct = block_with(vec!["Task"], &[]);
        assert!(block_matches(&direct, &q));

        let done_inherited = block_with_inherited(vec!["Task"], vec!["Done"]);
        // pagetag includes inherited chain — Done is present → -pagetag:Done false
        assert!(!block_matches(&done_inherited, &q));
    }

    #[test]
    fn block_matches_property_eq() {
        let q = parse_query("status:doing");
        assert!(block_matches(
            &block_with(vec![], &[("status", "doing")]),
            &q
        ));
        assert!(!block_matches(
            &block_with(vec![], &[("status", "done")]),
            &q
        ));
    }

    #[test]
    fn missing_property_matches_ne() {
        let q = parse_query("-status:done");
        assert!(block_matches(&block_with(vec![], &[]), &q));
    }

    #[test]
    fn has_predicate_checks_presence() {
        let q = parse_query("has:deadline");
        assert!(block_matches(
            &block_with(vec![], &[("deadline", "2026-05-01")]),
            &q
        ));
        assert!(!block_matches(&block_with(vec![], &[]), &q));
    }

    #[test]
    fn comparison_on_numeric_property() {
        let q = parse_query("priority:>=3");
        assert!(block_matches(&block_with(vec![], &[("priority", "5")]), &q));
        assert!(!block_matches(
            &block_with(vec![], &[("priority", "1")]),
            &q
        ));
    }

    #[test]
    fn comparison_on_iso_date() {
        let q = parse_query("deadline:<=2026-05-01");
        assert!(block_matches(
            &block_with(vec![], &[("deadline", "2026-04-15")]),
            &q
        ));
        assert!(!block_matches(
            &block_with(vec![], &[("deadline", "2026-06-01")]),
            &q
        ));
    }

    #[test]
    fn invert_round_trips() {
        for op in [
            QueryOp::Eq,
            QueryOp::Ne,
            QueryOp::Gt,
            QueryOp::Lt,
            QueryOp::Gte,
            QueryOp::Lte,
        ] {
            assert_eq!(invert(invert(op)), op);
        }
    }
}
