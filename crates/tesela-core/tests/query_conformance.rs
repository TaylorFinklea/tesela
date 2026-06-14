//! Rust consumer of the shared query-DSL conformance fixture
//! (`tests/fixtures/query-conformance.json`).
//!
//! The fixture is the ONE source of truth for DSL matching semantics
//! across the three implementations — this file (Rust), the web TS
//! consumer (mirroring `web/src/lib/query-language.ts`), and the iOS
//! Swift consumer (mirroring `LocalQueryEngine.swift`). Every case
//! runs through the REAL parser + matcher (`parse_query` →
//! `block_matches`), not a reimplementation.
//!
//! Adapter contract (mirrors the `_contract` header in the fixture):
//! the language-neutral fixture block maps onto `ParsedBlock` as
//!   text        → `text` (and `raw_text` = "- {text}")
//!   tags        → `tags` (own tags; inherited chain left empty)
//!   properties  → `properties`
//!   isHeading   → derived from `text` in Rust; the flag is asserted
//!                 consistent so TS/Swift consumers can trust it
//!   onDailyPage → `note_id` = "2026-06-10" (canonical daily id) when
//!                 true, "fixture-note" otherwise
//!   noteType    → `parent_note_type`

use std::collections::HashMap;

use serde::Deserialize;
use tesela_core::block::ParsedBlock;
use tesela_core::property::ValueType;
use tesela_core::query::{block_matches, block_matches_typed, parse_query, INBOX_VIEW_DSL};

#[derive(Deserialize)]
struct Fixture {
    #[serde(rename = "_contract")]
    _contract: Vec<String>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    dsl: String,
    block: FixtureBlock,
    expect: bool,
    /// L5 optional registry: lowercased property name → value_type string.
    /// Absent/empty ⇒ the registry-free matcher (heuristic); present ⇒ the
    /// typed matcher (`block_matches_typed`).
    #[serde(default, rename = "propertyTypes")]
    property_types: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureBlock {
    text: String,
    tags: Vec<String>,
    properties: HashMap<String, String>,
    is_heading: bool,
    on_daily_page: bool,
    note_type: Option<String>,
}

fn load_fixture() -> Fixture {
    let raw = include_str!("fixtures/query-conformance.json");
    serde_json::from_str(raw).expect("query-conformance.json parses")
}

fn to_parsed_block(b: &FixtureBlock) -> ParsedBlock {
    let note_id = if b.on_daily_page {
        "2026-06-10".to_string()
    } else {
        "fixture-note".to_string()
    };
    ParsedBlock {
        id: format!("{note_id}:1"),
        bid: None,
        text: b.text.clone(),
        raw_text: format!("- {}", b.text),
        tags: b.tags.clone(),
        inline_tags: Vec::new(),
        trailing_tags: Vec::new(),
        inherited_tags: Vec::new(),
        properties: b.properties.clone(),
        indent_level: 0,
        note_id,
        parent_note_type: b.note_type.clone(),
    }
}

/// Every fixture case must match through the real parser + matcher.
#[test]
fn all_conformance_cases_pass_through_real_matcher() {
    let fixture = load_fixture();
    let mut failures = Vec::new();
    for case in &fixture.cases {
        let q = parse_query(&case.dsl);
        let block = to_parsed_block(&case.block);
        let got = if case.property_types.is_empty() {
            block_matches(&block, &q)
        } else {
            let types: HashMap<String, ValueType> = case
                .property_types
                .iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), ValueType::parse(v)))
                .collect();
            block_matches_typed(&block, &q, &types)
        };
        if got != case.expect {
            failures.push(format!(
                "  {} — dsl {:?}: expected {}, got {}",
                case.name, case.dsl, case.expect, got
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} conformance case(s) diverged from the fixture:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The fixture's `isHeading` flag must agree with what Rust derives
/// from `text` — TS/Swift consumers may read the flag directly, so an
/// inconsistent flag would let their engines pass while disagreeing
/// with Rust.
#[test]
fn is_heading_flags_are_consistent_with_text() {
    let fixture = load_fixture();
    let q = parse_query("is:heading");
    for case in &fixture.cases {
        let block = to_parsed_block(&case.block);
        assert_eq!(
            block_matches(&block, &q),
            case.block.is_heading,
            "case {}: isHeading flag disagrees with text {:?}",
            case.name,
            case.block.text
        );
    }
}

/// Case names are unique (they're the cross-language assertion ids).
#[test]
fn case_names_are_unique() {
    let fixture = load_fixture();
    let mut seen = std::collections::HashSet::new();
    for case in &fixture.cases {
        assert!(
            seen.insert(case.name.clone()),
            "duplicate case name: {}",
            case.name
        );
    }
}

/// The fixture meets the spec's breadth bar and pins the canonical
/// Inbox DSL verbatim (the server seeds `INBOX_VIEW_DSL`; the fixture
/// must gate exactly that string).
#[test]
fn fixture_covers_required_surface() {
    let fixture = load_fixture();
    assert!(
        fixture.cases.len() >= 40,
        "fixture has {} cases; the spec requires 40+",
        fixture.cases.len()
    );
    let inbox_cases = fixture
        .cases
        .iter()
        .filter(|c| c.dsl == INBOX_VIEW_DSL)
        .count();
    assert!(
        inbox_cases >= 5,
        "expected a full Inbox-default matrix (>=5 cases using INBOX_VIEW_DSL verbatim), found {inbox_cases}"
    );
}
