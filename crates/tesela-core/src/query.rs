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
    /// `LIKE` — SQL-style pattern match. `%` matches any run of chars,
    /// `_` matches exactly one char. Case-insensitive; regex
    /// metacharacters in the pattern are treated as literals.
    Like,
    /// `NOT LIKE` — negation of `Like`.
    NotLike,
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

/// One filter token in the parsed query. Legacy flat-AND view; populated
/// only when the parsed expression is a flat conjunction of simple
/// `key OP value` predicates. Queries with `OR` / parens / `IN (…)` /
/// `NOT IN (…)` produce an empty `filters` and live entirely in `expr`.
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

/// A single leaf predicate in the boolean expression tree. Carries either
/// a comparison (`key OP value`) or set membership (`key IN (…)` /
/// `key NOT IN (…)`). All other DSL constructs (negation, conjunction,
/// disjunction, grouping) live at the [`BoolExpr`] level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Predicate {
    /// `key op value` — Eq / Ne / Lt / Lte / Gt / Gte against a single value.
    Cmp { key: String, op: QueryOp, value: String },
    /// `key IN (a, b, c)` or `key NOT IN (a, b, c)`. Drives the chip-bar
    /// Types group and any future multi-value chip clusters.
    In {
        key: String,
        values: Vec<String>,
        /// `true` for `NOT IN`; flips the membership check.
        negated: bool,
    },
}

/// Boolean expression tree built by `parse_query`. The DSL is now a real
/// algebra over predicates — `AND` / `OR` / `NOT` / parens — so a single
/// flat predicate list (the legacy `filters` field) can't represent
/// every parseable query. The matcher walks this tree; the legacy
/// `filters` field is populated only when the tree is a flat AND of
/// simple `Cmp` atoms, for SQL-prefilter compatibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum BoolExpr {
    /// Matches when every child matches. Empty `And` is the identity
    /// (matches everything) — that's what `parse_query("")` returns.
    And { args: Vec<BoolExpr> },
    /// Matches when any child matches. Empty `Or` matches nothing,
    /// but the parser never produces an empty `Or`.
    Or { args: Vec<BoolExpr> },
    /// Matches when the child does NOT match.
    Not { arg: Box<BoolExpr> },
    /// Leaf predicate.
    Atom { pred: Predicate },
}

impl Default for BoolExpr {
    fn default() -> Self {
        BoolExpr::And { args: Vec::new() }
    }
}

/// A parsed query: a `Kind`, the canonical expression tree (`expr`), and
/// a legacy flat-AND view (`filters`) populated only for queries that
/// happen to be expressible as one. Code that needs to filter blocks
/// reads `expr`; code that wants to do SQL-level pre-filtering can
/// inspect `filters` and degrade to "no prefilter" when it's empty.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(TS))]
#[cfg_attr(test, ts(export, export_to = "../../../web/src/lib/types/"))]
pub struct ParsedQuery {
    pub kind: Kind,
    /// Canonical boolean expression. Always set; defaults to an empty
    /// `And` (matches everything) for the empty query string.
    pub expr: BoolExpr,
    /// Legacy flat-AND view. Populated only when `expr` is a flat
    /// conjunction of `Cmp` atoms (no `OR` / `NOT IN` / parens). Empty
    /// otherwise — readers must handle that gracefully (the SQL
    /// prefilter in `db/sqlite.rs::execute_block_query` does).
    pub filters: Vec<QueryFilter>,
    /// `ORDER BY` clause from the DSL — pre-composed into the comma-
    /// separated `"key1 desc, key2 asc, key3"` shape that
    /// `db::sqlite::apply_sort` already accepts. `None` when the query
    /// has no `ORDER BY`, in which case the caller's external `sort`
    /// param (e.g. the HTTP body's `sort` field) is the fallback.
    #[serde(default)]
    #[cfg_attr(test, ts(optional))]
    pub sort: Option<String>,
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
/// Wrap a leaf predicate as a `BoolExpr::Atom`. Tiny shorthand used
/// throughout the parser now that `parse_predicate` returns BoolExpr
/// (so range-style sugars like `BETWEEN` can desugar into composite
/// trees without inventing new `Predicate` variants).
fn atom(pred: Predicate) -> BoolExpr {
    BoolExpr::Atom { pred }
}

pub fn parse_query(input: &str) -> ParsedQuery {
    let tokens = tokenize(input);
    let mut parser = Parser { input, tokens, pos: 0, kind: Kind::Block };
    let expr = parser.parse_or().unwrap_or_default();
    let sort = parser.parse_order_by();
    let filters = flatten_to_legacy_filters(&expr);
    ParsedQuery { kind: parser.kind, expr, filters, sort }
}

// ────────────────────────────────────────────────────────────────────
// Tokenizer
// ────────────────────────────────────────────────────────────────────

/// Stream of tokens fed to the recursive-descent parser. The tokenizer
/// is liberal — unrecognized punctuation becomes a `Word` so legacy
/// quirky DSL strings (e.g. `tag:`) still survive parsing.
///
/// Each token in the tokenizer's output is paired with its byte offsets
/// in the source string (see [`Spanned`]) so the parser can detect
/// adjacency. That matters for values that legitimately contain `:`
/// (e.g. block ids in `block:python:5` — the colon is part of the
/// value, not a structural separator).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    /// A bare identifier or value. Hyphens inside the word are kept
    /// (so `has-link` / `tag-in` parse as a single token), matching
    /// the legacy parser's `is_key_char` rule.
    Word(String),
    /// `"..."` literal — value carries the unwrapped contents.
    Quoted(String),
    LParen,
    RParen,
    Comma,
    /// `:` — legacy "field follows" delimiter.
    Colon,
    /// `=`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Lte,
    /// `>`
    Gt,
    /// `>=`
    Gte,
    /// Standalone `-` (before whitespace boundary). Used by the parser
    /// as a unary `NOT` shorthand on the next atom.
    Minus,
}

/// Token paired with its source span. `end` is exclusive (one past the
/// last byte). Adjacency is `prev.end == next.start` — the parser uses
/// this to slurp colons / digits / dashes that belong to a single
/// value (e.g. `block:python:5` where `python:5` is the value).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Spanned {
    tok: Token,
    start: usize,
    end: usize,
}

fn tokenize(input: &str) -> Vec<Spanned> {
    let bytes = input.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if (b as char).is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let (tok, new_i) = match b {
            b'(' => (Token::LParen, i + 1),
            b')' => (Token::RParen, i + 1),
            b',' => (Token::Comma, i + 1),
            b':' => (Token::Colon, i + 1),
            b'=' => (Token::Eq, i + 1),
            b'!' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => (Token::Ne, i + 2),
            b'<' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => (Token::Lte, i + 2),
            b'>' if i + 1 < bytes.len() && bytes[i + 1] == b'=' => (Token::Gte, i + 2),
            b'<' => (Token::Lt, i + 1),
            b'>' => (Token::Gt, i + 1),
            b'"' => {
                let val_start = i + 1;
                let mut j = val_start;
                while j < bytes.len() && bytes[j] != b'"' { j += 1; }
                let val = input[val_start..j].to_string();
                let end = if j < bytes.len() && bytes[j] == b'"' { j + 1 } else { j };
                (Token::Quoted(val), end)
            }
            b'-' => (Token::Minus, i + 1),
            b if is_word_char(b) => {
                let mut j = i;
                while j < bytes.len() && is_word_char(bytes[j]) { j += 1; }
                (Token::Word(input[i..j].to_string()), j)
            }
            _ => {
                // Unknown byte — skip silently so malformed input doesn't
                // panic the parser.
                i += 1;
                continue;
            }
        };
        tokens.push(Spanned { tok, start, end: new_i });
        i = new_i;
    }
    tokens
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

// ────────────────────────────────────────────────────────────────────
// Recursive-descent parser
// ────────────────────────────────────────────────────────────────────

struct Parser<'a> {
    /// Source string; needed by `parse_value_slurp` to re-extract the
    /// raw byte range when a value spans multiple adjacent tokens
    /// (e.g. `block:python:5` where the value `python:5` contains a
    /// colon that the tokenizer split off).
    input: &'a str,
    tokens: Vec<Spanned>,
    pos: usize,
    /// `kind:block` / `kind:page` is plucked out of the predicate stream
    /// and stored here, not in the expression tree — it controls which
    /// candidate set the matcher walks, not how individual rows are
    /// filtered. Defaults to Block.
    kind: Kind,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|s| &s.tok)
    }

    fn bump(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].tok.clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }


    /// Is the upcoming token a boolean keyword (case-insensitive)?
    fn peek_keyword(&self, kw: &str) -> bool {
        match self.peek() {
            Some(Token::Word(w)) => w.eq_ignore_ascii_case(kw),
            _ => false,
        }
    }

    /// Is the upcoming two-token sequence `ORDER BY`? Used by
    /// `parse_unary` to stop expression parsing so the trailing sort
    /// clause can be picked up by `parse_order_by` at the top level.
    fn peek_order_by(&self) -> bool {
        let peek_at = |off: usize| self.tokens.get(self.pos + off).map(|s| &s.tok);
        matches!(peek_at(0), Some(Token::Word(w)) if w.eq_ignore_ascii_case("order"))
            && matches!(peek_at(1), Some(Token::Word(w)) if w.eq_ignore_ascii_case("by"))
    }

    /// Parse a trailing `ORDER BY field1 [ASC|DESC] [, field2 [ASC|DESC]] …`
    /// clause and pre-compose it into the comma-separated string shape
    /// `db::sqlite::apply_sort` already accepts. Direction defaults to
    /// ascending (omitted) when not specified, matching SQL convention.
    /// Returns `None` if no `ORDER BY` is present at the cursor.
    fn parse_order_by(&mut self) -> Option<String> {
        if !self.peek_order_by() {
            return None;
        }
        self.bump(); // consume "ORDER"
        self.bump(); // consume "BY"
        let mut parts: Vec<String> = Vec::new();
        loop {
            let key = match self.bump() {
                Some(Token::Word(k)) => k.to_ascii_lowercase(),
                _ => break,
            };
            let suffix = if self.peek_keyword("desc") {
                self.bump();
                " desc"
            } else if self.peek_keyword("asc") {
                self.bump();
                " asc"
            } else {
                ""
            };
            parts.push(format!("{key}{suffix}"));
            if !matches!(self.peek(), Some(Token::Comma)) {
                break;
            }
            self.bump(); // consume comma
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }

    /// Is the upcoming token something that starts a new unary expression?
    /// (Used to detect implicit AND between space-separated atoms.)
    fn peek_starts_unary(&self) -> bool {
        match self.peek() {
            Some(Token::LParen) | Some(Token::Word(_)) | Some(Token::Minus) => {
                // `OR` / `AND` / `NOT` keywords don't start a unary — they're
                // part of a higher-level rule.
                !self.peek_keyword("or") && !self.peek_keyword("and")
            }
            _ => false,
        }
    }

    fn parse_or(&mut self) -> Option<BoolExpr> {
        let mut left = self.parse_and()?;
        let mut alts = Vec::new();
        while self.peek_keyword("or") {
            self.bump();
            if let Some(rhs) = self.parse_and() {
                if alts.is_empty() {
                    alts.push(left.clone());
                }
                alts.push(rhs);
            }
        }
        if !alts.is_empty() {
            left = BoolExpr::Or { args: alts };
        }
        Some(left)
    }

    fn parse_and(&mut self) -> Option<BoolExpr> {
        let mut left = self.parse_unary()?;
        let mut args = Vec::new();
        loop {
            if self.peek_keyword("and") {
                self.bump();
            } else if !self.peek_starts_unary() {
                break;
            }
            if let Some(rhs) = self.parse_unary() {
                if args.is_empty() {
                    args.push(left.clone());
                }
                args.push(rhs);
            } else {
                break;
            }
        }
        if !args.is_empty() {
            left = BoolExpr::And { args };
        }
        Some(left)
    }

    fn parse_unary(&mut self) -> Option<BoolExpr> {
        // Loop so we can keep trying after `kind:value` predicates that
        // get consumed for their side-effect (mutating `self.kind`) but
        // produce no expression. Without the loop, `kind:block tag:Task`
        // would lose the `tag:Task` clause because the first parse_unary
        // returns None.
        loop {
            // Stop here when we see a trailing `ORDER BY` clause —
            // the sort spec is parsed separately at the parse_query
            // level. Without this, `ORDER` / `BY` / the field names
            // would be consumed by the predicate loop as malformed
            // bareword predicates and silently dropped, and `sort`
            // would never be populated.
            if self.peek_order_by() {
                return None;
            }
            if self.peek_keyword("not") {
                self.bump();
                let inner = self.parse_unary()?;
                return Some(BoolExpr::Not { arg: Box::new(inner) });
            }
            if matches!(self.peek(), Some(Token::Minus)) {
                self.bump();
                let inner = self.parse_unary()?;
                return Some(BoolExpr::Not { arg: Box::new(inner) });
            }
            if matches!(self.peek(), Some(Token::LParen)) {
                self.bump();
                let inner = self.parse_or().unwrap_or_default();
                if matches!(self.peek(), Some(Token::RParen)) {
                    self.bump();
                }
                return Some(inner);
            }
            let start_pos = self.pos;
            match self.parse_predicate() {
                Some(e) => return Some(e),
                None => {
                    if self.pos == start_pos {
                        // No progress — give up to avoid infinite loop.
                        return None;
                    }
                    // Predicate consumed (likely `kind:foo`); try the
                    // next unary at the new cursor. An explicit `AND`
                    // keyword between the consumed-for-side-effect
                    // predicate and the next real one needs to be
                    // eaten so we don't stall — e.g. `kind:block AND
                    // status:todo` would otherwise stop at AND.
                    if self.peek_keyword("and") {
                        self.bump();
                    }
                    if !self.peek_starts_unary() {
                        return None;
                    }
                    continue;
                }
            }
        }
    }

    /// Parse one predicate. Backward-compat: every legacy form
    /// (`key:value`, `key:>=N`, `tag-in:a,b,c`, `has:foo`) must still
    /// produce the same predicate the old flat parser produced.
    fn parse_predicate(&mut self) -> Option<BoolExpr> {
        let key_token = self.bump()?;
        let raw_key = match key_token {
            Token::Word(w) => w,
            // A standalone quoted string or punctuation at predicate
            // position is malformed — drop and let the next pass
            // re-synchronize at the next whitespace.
            _ => return None,
        };
        let key = raw_key.to_ascii_lowercase();

        // `kind:` is meta — consume the value, set self.kind, return None
        // so this token doesn't end up in the expression tree.
        if key == "kind" {
            if matches!(self.peek(), Some(Token::Colon)) {
                self.bump();
            }
            if let Some(v) = self.parse_value() {
                self.kind = if matches!(v.to_ascii_lowercase().as_str(), "page" | "pages") {
                    Kind::Page
                } else {
                    Kind::Block
                };
            }
            return None;
        }

        // Legacy `tag-in:a,b,c` shape: key ends with `-in`, next token
        // is `:`, value is comma-separated bareword list. Equivalent to
        // `tag IN (a, b, c)` in the new grammar.
        if key.ends_with("-in") && matches!(self.peek(), Some(Token::Colon)) {
            self.bump(); // consume ':'
            let real_key = key[..key.len() - "-in".len()].to_string();
            let values = self.parse_comma_list_until_whitespace();
            return Some(atom(Predicate::In { key: real_key, values, negated: false }));
        }

        // New-style infix `key IN (…)` / `key NOT IN (…)`
        if self.peek_keyword("in") {
            self.bump();
            let values = self.parse_paren_value_list();
            return Some(atom(Predicate::In { key, values, negated: false }));
        }
        if self.peek_keyword("not") {
            // Tentatively consume NOT; commit only if followed by IN or LIKE.
            let save = self.pos;
            self.bump();
            if self.peek_keyword("in") {
                self.bump();
                let values = self.parse_paren_value_list();
                return Some(atom(Predicate::In { key, values, negated: true }));
            }
            if self.peek_keyword("like") {
                self.bump();
                let value = self.parse_value().unwrap_or_default();
                return Some(atom(Predicate::Cmp { key, op: QueryOp::NotLike, value }));
            }
            self.pos = save;
        }

        // `key LIKE "pattern"` — SQL-style wildcard match. Distinct
        // path from the generic infix-op handling because LIKE is a
        // keyword (Word token), not punctuation; without this branch
        // it would slip past `consume_infix_op` and fall through.
        if self.peek_keyword("like") {
            self.bump();
            let value = self.parse_value().unwrap_or_default();
            return Some(atom(Predicate::Cmp { key, op: QueryOp::Like, value }));
        }

        // `key IS NULL` / `key IS NOT NULL` — sugar for `-has:key` /
        // `has:key`. Lives between NOT IN and the infix-op check because
        // `IS` is a Word token (not punctuation); without this branch it
        // would slip past `consume_infix_op` and the predicate would be
        // dropped silently. Desugars to a plain `Cmp` over the `has`
        // pseudo-key so the matcher, SQL prefilter, and BoolExpr walker
        // all keep working with no new variants.
        if self.peek_keyword("is") {
            let save = self.pos;
            self.bump(); // consume "is"
            let negated = if self.peek_keyword("not") {
                self.bump();
                true
            } else {
                false
            };
            // Accept `NULL` or `EMPTY` — JQL spells the absence test
            // both ways; the desugar is identical.
            if self.peek_keyword("null") || self.peek_keyword("empty") {
                self.bump();
                return Some(atom(Predicate::Cmp {
                    key: "has".to_string(),
                    // `IS NOT NULL`/`IS NOT EMPTY` → present → has:key → Eq
                    // `IS NULL`    /`IS EMPTY`     → absent  → -has:key → Ne
                    op: if negated { QueryOp::Eq } else { QueryOp::Ne },
                    value: key,
                }));
            }
            // Wasn't a NULL test (`key IS something`) — rewind so the
            // next clause has a chance, even though no current grammar
            // shape uses bareword `IS` for anything else.
            self.pos = save;
        }

        // `key BETWEEN a AND b` — sugar for `key >= a AND key <= b`.
        // Inclusive on both ends (JQL + SQL convention). Desugars into a
        // BoolExpr::And of two ordinary Cmp atoms so the matcher + SQL
        // prefilter stay variant-free. If parsing fails midway (no `AND`
        // separator, no high bound), we rewind so the rest of the
        // expression still has a chance.
        if self.peek_keyword("between") {
            let save = self.pos;
            self.bump(); // consume "between"
            if let Some(low) = self.parse_value() {
                if self.peek_keyword("and") {
                    self.bump();
                    if let Some(high) = self.parse_value() {
                        return Some(BoolExpr::And {
                            args: vec![
                                atom(Predicate::Cmp {
                                    key: key.clone(),
                                    op: QueryOp::Gte,
                                    value: low,
                                }),
                                atom(Predicate::Cmp {
                                    key,
                                    op: QueryOp::Lte,
                                    value: high,
                                }),
                            ],
                        });
                    }
                }
            }
            self.pos = save;
        }

        // Infix comparison operator: `key = value`, `key != value`, etc.
        if let Some(op) = self.consume_infix_op() {
            let value = self.parse_value().unwrap_or_default();
            return Some(atom(Predicate::Cmp { key, op, value }));
        }

        // Legacy colon syntax: `key:value`, `key:>=N`, etc. `has:foo` is
        // the one legitimate "no value" form — `value` is the property
        // name. For everything else, an empty value drops the predicate.
        if matches!(self.peek(), Some(Token::Colon)) {
            self.bump();
            let op = self.consume_legacy_colon_op().unwrap_or(QueryOp::Eq);
            let value = self.parse_value().unwrap_or_default();
            if key != "has" && value.is_empty() {
                return None;
            }
            return Some(atom(Predicate::Cmp { key, op, value }));
        }

        // A bareword with no operator at all isn't a valid predicate;
        // the legacy parser dropped these silently.
        None
    }

    fn consume_infix_op(&mut self) -> Option<QueryOp> {
        let op = match self.peek()? {
            Token::Eq => QueryOp::Eq,
            Token::Ne => QueryOp::Ne,
            Token::Lt => QueryOp::Lt,
            Token::Lte => QueryOp::Lte,
            Token::Gt => QueryOp::Gt,
            Token::Gte => QueryOp::Gte,
            _ => return None,
        };
        self.bump();
        Some(op)
    }

    fn consume_legacy_colon_op(&mut self) -> Option<QueryOp> {
        let op = match self.peek()? {
            Token::Ne => QueryOp::Ne,
            Token::Lte => QueryOp::Lte,
            Token::Gte => QueryOp::Gte,
            Token::Lt => QueryOp::Lt,
            Token::Gt => QueryOp::Gt,
            _ => return None,
        };
        self.bump();
        Some(op)
    }

    fn parse_value(&mut self) -> Option<String> {
        // Quoted strings are always self-contained — never slurp past
        // them; the user opted into explicit quoting.
        if matches!(self.peek(), Some(Token::Quoted(_))) {
            return self.bump().and_then(|t| match t {
                Token::Quoted(s) => Some(s),
                _ => None,
            });
        }
        // Slurp every token contiguous with the first Word — preserves
        // legacy values that contain `:` (block ids: `python:5`),
        // periods, etc. Stop at the first whitespace gap or at a
        // non-value-like token (`(` / `)` / `,`).
        let first_idx = self.pos;
        let first = self.bump()?;
        let mut buf = match first {
            Token::Word(w) => w,
            _ => return None,
        };
        let mut end_offset = self.tokens[first_idx].end;
        while self.pos < self.tokens.len() {
            let span = &self.tokens[self.pos];
            if span.start != end_offset {
                break; // whitespace gap → value ends
            }
            match &span.tok {
                Token::Word(_) | Token::Colon | Token::Eq | Token::Ne
                | Token::Lt | Token::Lte | Token::Gt | Token::Gte
                | Token::Minus => {
                    // Append the raw source bytes — preserves the exact
                    // characters the tokenizer split off.
                    buf.push_str(&self.input[span.start..span.end]);
                    end_offset = span.end;
                    self.pos += 1;
                }
                // Quoted / paren / comma terminate the value.
                _ => break,
            }
        }
        Some(buf)
    }

    /// Parse `(a, b, c)` — used for `IN (…)` / `NOT IN (…)`. Tolerates
    /// missing parens (returns an empty list) so malformed input never
    /// panics.
    fn parse_paren_value_list(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if !matches!(self.peek(), Some(Token::LParen)) {
            return out;
        }
        self.bump();
        loop {
            match self.peek() {
                Some(Token::RParen) => { self.bump(); break; }
                Some(Token::Comma) => { self.bump(); }
                Some(Token::Word(_)) | Some(Token::Quoted(_)) => {
                    if let Some(v) = self.parse_value() {
                        out.push(v);
                    }
                }
                _ => break,
            }
        }
        out
    }

    /// Parse `a,b,c` (legacy `tag-in:a,b,c` shape — no parens). Stops
    /// at the next token that can't be part of a comma list. The
    /// legacy parser treated whitespace as the boundary; here, any
    /// non-Word / non-Comma token ends the list.
    fn parse_comma_list_until_whitespace(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        loop {
            match self.peek() {
                Some(Token::Word(_)) | Some(Token::Quoted(_)) => {
                    if let Some(v) = self.parse_value() {
                        out.push(v);
                    }
                }
                Some(Token::Comma) => { self.bump(); }
                _ => break,
            }
        }
        // Strip empty entries (e.g. trailing comma) and lowercase nothing
        // — the matcher already case-folds.
        out.into_iter().filter(|s| !s.is_empty()).collect()
    }
}

/// Flatten a `BoolExpr` into a legacy `Vec<QueryFilter>` view, ONLY
/// when the expression is a flat conjunction of simple `Cmp` atoms
/// (or `Not(Cmp)` which becomes a flipped op). Returns an empty
/// vector for any expression that can't be expressed in the legacy
/// shape — readers that depend on `filters` (e.g. the SQL prefilter)
/// must handle the empty case as "no usable prefilter."
fn flatten_to_legacy_filters(expr: &BoolExpr) -> Vec<QueryFilter> {
    let atoms: Vec<&BoolExpr> = match expr {
        BoolExpr::And { args } => args.iter().collect(),
        // A single non-AND expression — wrap in a one-element view.
        BoolExpr::Atom { .. } | BoolExpr::Not { .. } => vec![expr],
        BoolExpr::Or { .. } => return Vec::new(),
    };
    let mut out = Vec::with_capacity(atoms.len());
    for a in atoms {
        match a {
            BoolExpr::Atom { pred: Predicate::Cmp { key, op, value } } => {
                out.push(QueryFilter {
                    key: key.clone(),
                    op: *op,
                    value: value.clone(),
                });
            }
            BoolExpr::Not { arg } => match arg.as_ref() {
                BoolExpr::Atom { pred: Predicate::Cmp { key, op, value } } => {
                    out.push(QueryFilter {
                        key: key.clone(),
                        op: invert(*op),
                        value: value.clone(),
                    });
                }
                _ => return Vec::new(),
            },
            // `IN` / nested `And` / `Or` predicates don't fit the
            // legacy flat view; bail with an empty vector and let the
            // caller fall back to "no prefilter."
            _ => return Vec::new(),
        }
    }
    out
}

fn invert(op: QueryOp) -> QueryOp {
    match op {
        QueryOp::Eq => QueryOp::Ne,
        QueryOp::Ne => QueryOp::Eq,
        QueryOp::Gt => QueryOp::Lte,
        QueryOp::Lt => QueryOp::Gte,
        QueryOp::Gte => QueryOp::Lt,
        QueryOp::Lte => QueryOp::Gt,
        QueryOp::Like => QueryOp::NotLike,
        QueryOp::NotLike => QueryOp::Like,
    }
}

/// Translate a SQL `LIKE` pattern into an anchored, case-insensitive
/// regex. `%` becomes `.*`, `_` becomes `.`; every other char that's
/// special to the regex engine is escaped so the pattern reads as
/// literal text. Anchored so the WHOLE value must match (substring
/// semantics require the user to write `%foo%` explicitly — matches
/// SQL's behavior).
fn like_to_regex(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() * 2 + 8);
    out.push_str("(?i)^");
    for ch in pattern.chars() {
        match ch {
            '%' => out.push_str(".*"),
            '_' => out.push('.'),
            // Regex meta-characters that must be escaped to be treated
            // as literals. Kept in sync with `regex::escape`'s set;
            // hand-rolled here to avoid a per-call allocation when the
            // pattern is short.
            '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
            | '\\' | '#' | '&' | '-' | '~' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.push('$');
    out
}

/// Compile a LIKE pattern and test it against `actual`. Compilation
/// failures fall back to "no match" — a malformed pattern shouldn't
/// blow up the matcher.
fn like_matches(actual: &str, pattern: &str) -> bool {
    let re = match regex::Regex::new(&like_to_regex(pattern)) {
        Ok(r) => r,
        Err(_) => return false,
    };
    re.is_match(actual)
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
        QueryOp::Like => like_matches(actual, expected),
        QueryOp::NotLike => !like_matches(actual, expected),
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
    eval_expr(block, &q.expr)
}

/// Walk the `BoolExpr` tree, short-circuiting AND/OR. Empty `And` matches
/// everything (the identity); empty `Or` matches nothing.
fn eval_expr(block: &ParsedBlock, expr: &BoolExpr) -> bool {
    match expr {
        BoolExpr::And { args } => args.iter().all(|a| eval_expr(block, a)),
        BoolExpr::Or { args } => args.iter().any(|a| eval_expr(block, a)),
        BoolExpr::Not { arg } => !eval_expr(block, arg),
        BoolExpr::Atom { pred } => pred_matches(block, pred),
    }
}

/// Evaluate a leaf predicate. Routes `Cmp` to the existing per-key
/// filter logic (preserved verbatim from the legacy parser so behavior
/// stays identical); `In` walks the value list and short-circuits.
fn pred_matches(block: &ParsedBlock, pred: &Predicate) -> bool {
    match pred {
        Predicate::Cmp { key, op, value } => {
            // Build a transient QueryFilter so we can reuse the existing
            // per-key matchers without duplicating their logic. This is
            // the cheap-and-correct path; if the matchers ever get
            // expensive enough that the allocation hurts, inline them.
            let f = QueryFilter {
                key: key.clone(),
                op: *op,
                value: value.clone(),
            };
            filter_matches(block, &f)
        }
        Predicate::In { key, values, negated } => {
            // `key in (a, b, c)` is OR over `key = v` for each v.
            // `key not in (a, b, c)` is the negation. Uses the same
            // per-key matcher path so semantics line up with `tag:foo`
            // / property lookups exactly.
            let any_match = values.iter().any(|v| {
                let f = QueryFilter {
                    key: key.clone(),
                    op: QueryOp::Eq,
                    value: v.clone(),
                };
                filter_matches(block, &f)
            });
            if *negated { !any_match } else { any_match }
        }
    }
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
        "tag" | "type" | "pagetag" | "blocktag" => {
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
        "text" => {
            // `text:foo` and `text LIKE "%foo%"` search the block's
            // display text (first line, tags stripped). Eq/Ne do
            // case-insensitive equality, Like/NotLike do SQL-style
            // pattern match, comparison ops fall back to string
            // ordering. The cleaned `text` field (not `raw_text`) is
            // what every other surface displays, so users searching
            // for "wood" find what they see.
            apply_op(&block.text, f.op, &f.value)
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
            bid: None,
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
            bid: None,
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
    fn block_matches_tag_in_empty_values_matches_nothing() {
        // `tag-in:` (with no values) is an empty membership set, which
        // semantically matches nothing — same as `tag in ()`. Old
        // parser dropped this clause entirely (vacuous AND → matched
        // every block); the new BoolExpr-based parser preserves it as
        // an empty `In` predicate so the semantics are honest. No
        // realistic user authors a `tag-in:` with no values — the chip
        // system always emits a populated list or no clause at all.
        let q = parse_query("tag-in:");
        // Legacy flat-filters view stays empty (In doesn't fit the
        // simple Cmp shape that flat-filters captures).
        assert_eq!(q.filters.len(), 0);
        // But block_matches now correctly excludes everything — the
        // In predicate is in the expression tree.
        assert!(!block_matches(&block_with(vec!["Task"], &[]), &q));
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

    // ── JQL-style grammar: OR / AND / NOT / IN / parens ─────────────

    #[test]
    fn parses_or_keyword_case_insensitive() {
        // `a OR b` and `a or b` both build an Or with two args.
        for input in ["tag:Task OR tag:Note", "tag:Task or tag:Note"] {
            let q = parse_query(input);
            assert!(matches!(q.expr, BoolExpr::Or { ref args } if args.len() == 2),
                "input {input:?} parsed as {:?}", q.expr);
        }
    }

    #[test]
    fn block_matches_or_disjunction() {
        // (status:todo OR status:doing) — either matches the row.
        let q = parse_query("status:todo OR status:doing");
        assert!(block_matches(&block_with(vec![], &[("status", "todo")]), &q));
        assert!(block_matches(&block_with(vec![], &[("status", "doing")]), &q));
        assert!(!block_matches(&block_with(vec![], &[("status", "done")]), &q));
    }

    #[test]
    fn block_matches_paren_grouping() {
        // (status:todo OR status:doing) AND has:deadline — needs the
        // OR inside parens to bind tighter than the surrounding AND.
        let q = parse_query("(status:todo OR status:doing) AND has:deadline");
        assert!(block_matches(
            &block_with(vec![], &[("status", "todo"), ("deadline", "2026-01-01")]),
            &q,
        ));
        // todo but no deadline → falls out
        assert!(!block_matches(&block_with(vec![], &[("status", "todo")]), &q));
        // has deadline but wrong status → falls out
        assert!(!block_matches(
            &block_with(vec![], &[("status", "done"), ("deadline", "2026-01-01")]),
            &q,
        ));
    }

    #[test]
    fn parses_not_keyword_as_unary() {
        // `NOT tag:Task` is a Not wrapping the atom.
        let q = parse_query("NOT tag:Task");
        assert!(matches!(q.expr, BoolExpr::Not { .. }));
        assert!(!block_matches(&block_with(vec!["Task"], &[]), &q));
        assert!(block_matches(&block_with(vec!["Note"], &[]), &q));
    }

    #[test]
    fn parses_infix_in_with_parens() {
        // `tag in (Task, Domain, Issue)` is the JQL-style form;
        // `tag-in:Task,Domain,Issue` is the legacy shorthand. Both
        // produce the same Predicate::In.
        let qa = parse_query("tag in (Task, Domain, Issue)");
        let qb = parse_query("tag-in:Task,Domain,Issue");
        // Both should match the same blocks.
        for q in [&qa, &qb] {
            assert!(block_matches(&block_with(vec!["Task"], &[]), q));
            assert!(block_matches(&block_with(vec!["Domain"], &[]), q));
            assert!(block_matches(&block_with(vec!["Issue"], &[]), q));
            assert!(!block_matches(&block_with(vec!["Person"], &[]), q));
        }
    }

    #[test]
    fn parses_not_in() {
        // `tag NOT IN (Done, Cancelled)` — set-membership exclusion.
        let q = parse_query("tag NOT IN (Done, Cancelled)");
        assert!(!block_matches(&block_with(vec!["Done"], &[]), &q));
        assert!(!block_matches(&block_with(vec!["Cancelled"], &[]), &q));
        assert!(block_matches(&block_with(vec!["Task"], &[]), &q));
    }

    #[test]
    fn parses_infix_comparison_ops() {
        // `priority >= 3` parses identically to the legacy `priority:>=3`.
        let qa = parse_query("priority >= 3");
        let qb = parse_query("priority:>=3");
        let target = block_with(vec![], &[("priority", "5")]);
        assert!(block_matches(&target, &qa));
        assert!(block_matches(&target, &qb));
        let low = block_with(vec![], &[("priority", "1")]);
        assert!(!block_matches(&low, &qa));
        assert!(!block_matches(&low, &qb));
    }

    #[test]
    fn complex_mixed_query() {
        // The shape the user gave as the motivating example, plus a
        // tighter AND clause around it:
        //   (status:todo OR status:doing) AND tag in (Task, Issue)
        let q = parse_query("(status:todo OR status:doing) AND tag in (Task, Issue)");
        // todo + Task → matches
        assert!(block_matches(&block_with(vec!["Task"], &[("status", "todo")]), &q));
        // doing + Issue → matches
        assert!(block_matches(&block_with(vec!["Issue"], &[("status", "doing")]), &q));
        // done + Task → fails (status check)
        assert!(!block_matches(&block_with(vec!["Task"], &[("status", "done")]), &q));
        // todo + Person → fails (tag-in check)
        assert!(!block_matches(&block_with(vec!["Person"], &[("status", "todo")]), &q));
    }

    #[test]
    fn flat_and_query_populates_legacy_filters_field() {
        // Backward-compat: flat-AND queries still expose `filters` for
        // the SQL prefilter in db/sqlite.rs. Mixed-boolean queries
        // leave `filters` empty so callers degrade to "no prefilter."
        let flat = parse_query("kind:block tag:Task -status:done");
        assert_eq!(flat.filters.len(), 2);
        // Mixed-boolean: filters should be empty.
        let mixed = parse_query("tag:Task OR tag:Note");
        assert_eq!(mixed.filters.len(), 0);
        let with_in = parse_query("tag in (Task, Note)");
        assert_eq!(with_in.filters.len(), 0);
    }

    #[test]
    fn kind_clause_with_explicit_and_doesnt_swallow_rest() {
        // Regression: `kind:block AND tag:Task` — `kind:` is consumed
        // for side-effect (sets `self.kind`); the parser must skip the
        // following `AND` keyword and parse the next predicate, or it
        // stalls and the query degrades to "match everything."
        let q = parse_query("kind:block AND tag:Task");
        // The expression should be just the tag clause (kind:block
        // doesn't appear in the AST; it lives on ParsedQuery.kind).
        assert_eq!(q.kind, Kind::Block);
        match &q.expr {
            BoolExpr::Atom { pred: Predicate::Cmp { key, value, .. } } => {
                assert_eq!(key, "tag");
                assert_eq!(value, "Task");
            }
            other => panic!("expected single tag:Task atom, got {:?}", other),
        }
    }

    #[test]
    fn legacy_block_id_with_embedded_colon_still_parses() {
        // Regression: `block:python:5` is a single predicate where the
        // value contains a `:`. The new tokenizer splits on `:` so the
        // parser has to slurp adjacent tokens to reconstruct the value.
        let q = parse_query("block:python:5");
        // Legacy flat-filters view captures it as a single Cmp.
        assert_eq!(q.filters.len(), 1);
        assert_eq!(q.filters[0].key, "block");
        assert_eq!(q.filters[0].value, "python:5");
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

    // ──────────────────────────────────────────────────────────────────
    // IS NULL / IS NOT NULL desugaring + `type` as a `tag` alias
    // ──────────────────────────────────────────────────────────────────

    /// `deadline IS NOT NULL` is sugar for `has:deadline` — matches any
    /// block whose `deadline::` property is present, regardless of
    /// value.
    #[test]
    fn is_not_null_matches_present_property() {
        let q = parse_query("deadline IS NOT NULL");
        assert!(block_matches(
            &block_with(vec![], &[("deadline", "2026-05-25")]),
            &q
        ));
        assert!(!block_matches(&block_with(vec![], &[]), &q));
    }

    /// `deadline IS NULL` is sugar for `-has:deadline` — matches any
    /// block whose `deadline::` property is absent.
    #[test]
    fn is_null_matches_missing_property() {
        let q = parse_query("deadline IS NULL");
        assert!(block_matches(&block_with(vec![], &[]), &q));
        assert!(!block_matches(
            &block_with(vec![], &[("deadline", "2026-05-25")]),
            &q
        ));
    }

    /// `IS NOT NULL` is case-insensitive, matching the rest of the
    /// keyword grammar (`AND`/`OR`/`NOT`/`IN`).
    #[test]
    fn is_null_is_case_insensitive() {
        let q = parse_query("deadline is not null");
        assert!(block_matches(
            &block_with(vec![], &[("deadline", "2026-05-25")]),
            &q
        ));
    }

    /// `type:Task` should match the same blocks as `tag:Task` — the
    /// TypeRegistry surfaces user-defined types (Task / Domain / Issue /
    /// Person / …) as tag pages, so `type` is a semantic alias the
    /// query layer can resolve without forcing the user to know the
    /// underlying storage shape.
    #[test]
    fn type_is_alias_for_tag() {
        let q = parse_query("type:Task");
        assert!(block_matches(&block_with(vec!["Task"], &[]), &q));
        assert!(!block_matches(&block_with(vec!["Person"], &[]), &q));
    }

    /// `type IN (task, domain, issue)` should compose the same way
    /// `tag IN (…)` does (matches if any tag in the block's chain
    /// matches any value, case-insensitive).
    #[test]
    fn type_in_list_matches_any_tag() {
        let q = parse_query("type IN (task, domain, issue)");
        assert!(block_matches(&block_with(vec!["Task"], &[]), &q));
        assert!(block_matches(&block_with(vec!["Domain"], &[]), &q));
        assert!(block_matches(&block_with(vec!["Issue"], &[]), &q));
        assert!(!block_matches(&block_with(vec!["Person"], &[]), &q));
    }

    /// `ORDER BY deadline` populates `sort` with the field name and
    /// nothing else (ASC is implicit).
    #[test]
    fn order_by_single_field_default_ascending() {
        let q = parse_query("status = todo ORDER BY deadline");
        assert_eq!(q.sort.as_deref(), Some("deadline"));
    }

    /// `ORDER BY deadline DESC` appends the direction so `apply_sort`
    /// flips the comparison.
    #[test]
    fn order_by_with_desc() {
        let q = parse_query("status = todo ORDER BY deadline DESC");
        assert_eq!(q.sort.as_deref(), Some("deadline desc"));
    }

    /// Multi-key sort uses comma separation (the same shape `apply_sort`
    /// already accepts via the HTTP `sort` param).
    #[test]
    fn order_by_multi_key() {
        let q = parse_query("type = task ORDER BY status, deadline DESC");
        assert_eq!(q.sort.as_deref(), Some("status, deadline desc"));
    }

    /// `ORDER BY` is case-insensitive (matches the rest of the keyword
    /// grammar).
    #[test]
    fn order_by_case_insensitive() {
        let q = parse_query("status = todo order by deadline asc");
        assert_eq!(q.sort.as_deref(), Some("deadline asc"));
    }

    /// Without an `ORDER BY` clause, `sort` is `None` — the HTTP `sort`
    /// param remains the fallback for callers that set it externally.
    #[test]
    fn no_order_by_leaves_sort_none() {
        let q = parse_query("status = todo");
        assert_eq!(q.sort, None);
    }

    /// `text LIKE "wood%"` does a case-insensitive prefix match on the
    /// block's display text. `%` is the SQL "any run" wildcard.
    #[test]
    fn like_prefix_match_on_block_text() {
        let q = parse_query(r#"text LIKE "wood%""#);
        // block_with seeds text="x"; we synthesize a real block here.
        let mut b = block_with(vec![], &[]);
        b.text = "wood chips".into();
        assert!(block_matches(&b, &q));
        b.text = "Wood Chips".into(); // case-insensitive
        assert!(block_matches(&b, &q));
        b.text = "do wood chips".into(); // not a prefix
        assert!(!block_matches(&b, &q));
    }

    /// Substring match — `%foo%` on both sides means "contains foo".
    #[test]
    fn like_substring_match() {
        let q = parse_query(r#"text LIKE "%chair%""#);
        let mut b = block_with(vec![], &[]);
        b.text = "Research massage chairs".into();
        assert!(block_matches(&b, &q));
        b.text = "Schedule a meeting".into();
        assert!(!block_matches(&b, &q));
    }

    /// `_` matches exactly one character.
    #[test]
    fn like_single_char_wildcard() {
        let q = parse_query(r#"text LIKE "h_t""#);
        let mut b = block_with(vec![], &[]);
        b.text = "hat".into();
        assert!(block_matches(&b, &q));
        b.text = "hot".into();
        assert!(block_matches(&b, &q));
        b.text = "heat".into(); // _ matches one, not two
        assert!(!block_matches(&b, &q));
    }

    /// `NOT LIKE` negates the match.
    #[test]
    fn not_like_negates() {
        let q = parse_query(r#"text NOT LIKE "wood%""#);
        let mut b = block_with(vec![], &[]);
        b.text = "wood chips".into();
        assert!(!block_matches(&b, &q));
        b.text = "research chairs".into();
        assert!(block_matches(&b, &q));
    }

    /// LIKE works on arbitrary property values too — not just the
    /// pseudo-key `text`.
    #[test]
    fn like_on_property() {
        let q = parse_query(r#"status LIKE "in-%""#);
        assert!(block_matches(
            &block_with(vec![], &[("status", "in-review")]),
            &q
        ));
        assert!(block_matches(
            &block_with(vec![], &[("status", "in-progress")]),
            &q
        ));
        assert!(!block_matches(
            &block_with(vec![], &[("status", "todo")]),
            &q
        ));
        // Missing property does NOT match a positive LIKE.
        assert!(!block_matches(&block_with(vec![], &[]), &q));
    }

    /// Regex metacharacters in the pattern are treated as literals so
    /// users don't get surprised by accidental special meaning.
    #[test]
    fn like_escapes_regex_metacharacters() {
        let q = parse_query(r#"text LIKE "a.b""#);
        let mut b = block_with(vec![], &[]);
        b.text = "a.b".into();
        assert!(block_matches(&b, &q));
        b.text = "axb".into(); // `.` is literal, not "any char"
        assert!(!block_matches(&b, &q));
    }

    /// `key BETWEEN a AND b` is sugar for `key >= a AND key <= b`.
    /// Inclusive on both ends, matching JQL + SQL convention.
    #[test]
    fn between_matches_inclusive_range_on_iso_dates() {
        let q = parse_query("deadline BETWEEN 2026-05-01 AND 2026-05-31");
        // Inside range
        assert!(block_matches(
            &block_with(vec![], &[("deadline", "2026-05-15")]),
            &q
        ));
        // Exact low bound (inclusive)
        assert!(block_matches(
            &block_with(vec![], &[("deadline", "2026-05-01")]),
            &q
        ));
        // Exact high bound (inclusive)
        assert!(block_matches(
            &block_with(vec![], &[("deadline", "2026-05-31")]),
            &q
        ));
        // Just below low
        assert!(!block_matches(
            &block_with(vec![], &[("deadline", "2026-04-30")]),
            &q
        ));
        // Just above high
        assert!(!block_matches(
            &block_with(vec![], &[("deadline", "2026-06-01")]),
            &q
        ));
        // Missing property
        assert!(!block_matches(&block_with(vec![], &[]), &q));
    }

    /// BETWEEN also works for numeric ranges.
    #[test]
    fn between_matches_numeric_range() {
        let q = parse_query("priority BETWEEN 2 AND 5");
        assert!(block_matches(&block_with(vec![], &[("priority", "3")]), &q));
        assert!(block_matches(&block_with(vec![], &[("priority", "2")]), &q));
        assert!(block_matches(&block_with(vec![], &[("priority", "5")]), &q));
        assert!(!block_matches(
            &block_with(vec![], &[("priority", "1")]),
            &q
        ));
        assert!(!block_matches(
            &block_with(vec![], &[("priority", "6")]),
            &q
        ));
    }

    /// BETWEEN composes with the rest of the grammar — must work inside
    /// `OR`, parens, and alongside other clauses.
    #[test]
    fn between_composes_with_and_or_parens() {
        let q = parse_query(
            "status != done AND (deadline BETWEEN 2026-05-01 AND 2026-05-31 OR scheduled BETWEEN 2026-05-01 AND 2026-05-31)",
        );
        // Has scheduled in range
        assert!(block_matches(
            &block_with(
                vec![],
                &[("status", "todo"), ("scheduled", "2026-05-10")],
            ),
            &q
        ));
        // Has deadline in range
        assert!(block_matches(
            &block_with(
                vec![],
                &[("status", "doing"), ("deadline", "2026-05-20")],
            ),
            &q
        ));
        // Status=done → first clause fails
        assert!(!block_matches(
            &block_with(
                vec![],
                &[("status", "done"), ("scheduled", "2026-05-10")],
            ),
            &q
        ));
        // Both dates outside range → second clause fails
        assert!(!block_matches(
            &block_with(
                vec![],
                &[("status", "todo"), ("scheduled", "2026-06-15")],
            ),
            &q
        ));
    }

    /// `IS EMPTY` / `IS NOT EMPTY` are aliases for `IS NULL` / `IS NOT
    /// NULL` (JQL spells it both ways).
    #[test]
    fn is_empty_aliases_is_null() {
        let q = parse_query("deadline IS EMPTY");
        assert!(block_matches(&block_with(vec![], &[]), &q));
        assert!(!block_matches(
            &block_with(vec![], &[("deadline", "2026-05-25")]),
            &q
        ));
    }

    #[test]
    fn is_not_empty_aliases_is_not_null() {
        let q = parse_query("deadline IS NOT EMPTY");
        assert!(block_matches(
            &block_with(vec![], &[("deadline", "2026-05-25")]),
            &q
        ));
        assert!(!block_matches(&block_with(vec![], &[]), &q));
    }

    /// End-to-end: the user's example query parses + matches the
    /// expected blocks. Exercises every new piece (`!=` on status,
    /// `type IN`, `IS NOT NULL`, `OR` inside parens, nested `AND`).
    #[test]
    fn parses_and_matches_full_jql_example() {
        let q = parse_query(
            "status != done AND type IN (task, domain, issue) \
             AND (deadline IS NOT NULL OR scheduled IS NOT NULL)",
        );
        // A task with status=todo + scheduled set, no deadline → match.
        assert!(block_matches(
            &block_with(
                vec!["Task"],
                &[("status", "todo"), ("scheduled", "2026-05-25")],
            ),
            &q
        ));
        // A task with deadline set + status=doing, no scheduled → match.
        assert!(block_matches(
            &block_with(
                vec!["Task"],
                &[("status", "doing"), ("deadline", "2026-05-30")],
            ),
            &q
        ));
        // A task with status=done → fails the first clause.
        assert!(!block_matches(
            &block_with(
                vec!["Task"],
                &[("status", "done"), ("scheduled", "2026-05-25")],
            ),
            &q
        ));
        // A Person tag → fails the type clause.
        assert!(!block_matches(
            &block_with(
                vec!["Person"],
                &[("status", "todo"), ("scheduled", "2026-05-25")],
            ),
            &q
        ));
        // A task with neither deadline nor scheduled → fails the date clause.
        assert!(!block_matches(
            &block_with(vec!["Task"], &[("status", "todo")]),
            &q
        ));
    }
}
