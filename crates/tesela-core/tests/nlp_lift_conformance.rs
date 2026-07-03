//! Shared inline-NLP-lift conformance fixture
//! (`tests/fixtures/nlp-lift-conformance.json`).
//!
//! Every case's `expected` is a PINNED LITERAL (computed by running the REAL
//! web `detectTaskTokens`, `web/src/lib/task-tokens.ts`, against `text` +
//! `registry` + `anchor_date`, then copied in) — the fixture predates the
//! Rust hoist (`tesela-pfix.3` landed first per the module's original
//! docs) and stays pinned rather than becoming Rust-generated, so this file
//! doubles as an independent conformance check on the Rust port
//! (`tesela_core::nlp_lift`, `tesela-ug7`) rather than the port grading its
//! own homework. `rust_detect_task_tokens_matches_fixture` (below) asserts
//! the REAL `nlp_lift::detect_task_tokens` against every case; the web
//! (`web/tests/unit/nlp-lift-conformance.test.mjs`) and iOS
//! (`app/Tesela-iOS/Tests/NLPLiftConformanceTests.swift`, which now asserts
//! through the FFI — `detectNlpLifts` — rather than a native Swift
//! reimplementation) conformance runners assert their own consumer against
//! the same pinned values, so drift between all three is caught
//! immediately.
//!
//! `registry` is ONE shared `DetectSpec`-shaped spec (select `priority` +
//! date `deadline`, deliberately excluding `number`-typed properties —
//! iOS's `InlineNLP` has no `<number> <trigger>` lift path yet, so a
//! shared fixture can't exercise it without scope-creeping this bead into
//! adding that feature) reused by every case, mirroring how
//! `web/tests/unit/task-tokens.test.mjs`'s hand-authored `SPEC` constant
//! already covers this shape. Keeping exactly one date-typed property also
//! sidesteps a SEPARATE, larger, out-of-scope divergence: iOS's bare-date
//! lift always targets the block's FIRST date-typed property in
//! `tag_properties` order, while web's targets the tag's
//! `default_date_property` frontmatter (default `"scheduled"`) — with only
//! one date property those two selection rules trivially agree.
//!
//! `anchor_date` (`2026-05-22`, a Friday) is the SAME frozen "now" the
//! recurrence fixture and `date-parser.test.mjs` already use, so relative
//! phrases ("tomorrow", "next tuesday") resolve identically everywhere.
//!
//! Case coverage (per the `tesela-pfix.3` bead):
//!   - baseline select/date-trigger lifts (regression parity)
//!   - `today noon` / `<relative-day> noon` bare-trailing lifts — the fix
//!     for the web `extractTime` gap (iOS already handled trailing
//!     "noon"/"midnight"; web only matched the bare exact-string case)
//!   - the trailing-position rule: a bare (untriggered) date lifts at
//!     line-start / trailing position / right after a date-intent word,
//!     but NOT mid-prose with no intent word — "Taylor's locked decision"
//!     (`EditorAutocomplete.swift`'s `InlineNLP.detect`), now also true of
//!     web's `detectTokens` step 4 (previously ungated — this fixture's
//!     `bare_midprose_not_lifted` case is what "confirm web honors it"
//!     caught: it didn't, until this bead fixed it)
//!   - literal-range guards: a trigger/date word embedded in a bare URL,
//!     `[[wiki link]]`, markdown link, or inline `` `code` `` span lifts on
//!     NEITHER client (web already had `literalRanges`; iOS's `InlineNLP`
//!     had no equivalent guard, closed by this bead)

use serde::Serialize;

#[derive(Serialize)]
struct PropertySpec {
    key: String,
    value_type: String,
    choices: Vec<String>,
    triggers: Vec<String>,
}

#[derive(Serialize)]
struct Registry {
    default_date_property: String,
    properties: Vec<PropertySpec>,
}

#[derive(Serialize)]
struct ExpectedProp {
    key: String,
    value: String,
}

#[derive(Serialize)]
struct Expected {
    stripped: String,
    props: Vec<ExpectedProp>,
}

#[derive(Serialize)]
struct Case {
    name: String,
    text: String,
    expected: Expected,
}

#[derive(Serialize)]
struct Fixture {
    _contract: Vec<String>,
    registry: Registry,
    anchor_date: String,
    cases: Vec<Case>,
}

fn fixture_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/nlp-lift-conformance.json")
}

fn build_registry() -> Registry {
    Registry {
        default_date_property: "deadline".to_string(),
        properties: vec![
            PropertySpec {
                key: "priority".to_string(),
                value_type: "select".to_string(),
                choices: vec!["p1", "p2", "p3", "p4"].into_iter().map(str::to_string).collect(),
                triggers: vec!["p1", "p2", "p3", "p4"].into_iter().map(str::to_string).collect(),
            },
            PropertySpec {
                key: "deadline".to_string(),
                value_type: "date".to_string(),
                choices: vec![],
                triggers: vec!["due", "deadline"].into_iter().map(str::to_string).collect(),
            },
        ],
    }
}

/// (name, text, stripped, props). `props` pairs are `(key, value)`, in the
/// order the real `detectTaskTokens` emits them (claim order: select props
/// first, then triggered dates, then bare/default dates). Values were
/// computed against the REAL web `detectTaskTokens(text, SPEC, anchor)` —
/// see the module docs — not hand-derived.
#[allow(clippy::type_complexity)]
fn raw_cases() -> Vec<(&'static str, &'static str, &'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "select_lift_basic",
            "fix the bug p1",
            "fix the bug",
            vec![("priority", "p1")],
        ),
        (
            "select_lift_p3",
            "clean up p3",
            "clean up",
            vec![("priority", "p3")],
        ),
        (
            "date_trigger_lift",
            "email report due tomorrow",
            "email report",
            vec![("deadline", "2026-05-23")],
        ),
        (
            "date_trigger_with_time",
            "call vet due thu at 8",
            "call vet",
            vec![("deadline", "2026-05-28 08:00")],
        ),
        (
            "bare_trailing_lift",
            "buy milk tomorrow",
            "buy milk",
            vec![("deadline", "2026-05-23")],
        ),
        (
            "bare_trailing_today_noon",
            "call mom today noon",
            "call mom",
            vec![("deadline", "2026-05-22 12:00")],
        ),
        (
            "bare_trailing_tomorrow_noon",
            "ping team tomorrow noon",
            "ping team",
            vec![("deadline", "2026-05-23 12:00")],
        ),
        (
            "bare_midprose_not_lifted",
            "call her tomorrow about the launch",
            "call her tomorrow about the launch",
            vec![],
        ),
        (
            "intent_word_lift",
            "ping team on tuesday afternoon",
            "ping team on afternoon",
            vec![("deadline", "2026-05-26")],
        ),
        (
            "url_embedded_priority_no_lift",
            "check https://x.com/p1/doc",
            "check https://x.com/p1/doc",
            vec![],
        ),
        (
            "wikilink_embedded_priority_no_lift",
            "see [[p1 notes]]",
            "see [[p1 notes]]",
            vec![],
        ),
        (
            "wikilink_embedded_date_no_lift",
            "plan [[meeting tomorrow]]",
            "plan [[meeting tomorrow]]",
            vec![],
        ),
        (
            "markdown_link_embedded_priority_no_lift",
            "read [spec](https://x.com/p1/doc) today-ish",
            "read [spec](https://x.com/p1/doc) today-ish",
            vec![],
        ),
        (
            "inline_code_embedded_priority_no_lift",
            "run `p1` now",
            "run `p1` now",
            vec![],
        ),
        (
            "combined_priority_and_trailing_date",
            "fix parser p1 due tomorrow",
            "fix parser",
            vec![("priority", "p1"), ("deadline", "2026-05-23")],
        ),
        (
            "combined_priority_and_bare_trailing_date",
            "buy milk p2 tomorrow",
            "buy milk",
            vec![("priority", "p2"), ("deadline", "2026-05-23")],
        ),
        (
            // tesela-j7g regression: iOS's block editor keeps a block's
            // `#tag` cluster on the SAME line as its prose (unlike web's
            // separate `tags::` line) — a trailing bare date must still
            // lift with a hashtag trailing it; the tag is boundary noise,
            // not prose that defeats the trailing-position gate.
            "bare_trailing_lift_before_trailing_hashtag",
            "Call dentist tomorrow #Task",
            "Call dentist #Task",
            vec![("deadline", "2026-05-23")],
        ),
        (
            // A hashtag mid-prose must not itself grant trailing status to
            // a bare date that follows it — the mid-prose intent-word gate
            // still applies.
            "midprose_hashtag_does_not_grant_trailing",
            "call her #urgent tomorrow about the launch",
            "call her #urgent tomorrow about the launch",
            vec![],
        ),
    ]
}

fn build_fixture() -> Fixture {
    let cases = raw_cases()
        .into_iter()
        .map(|(name, text, stripped, props)| Case {
            name: name.to_string(),
            text: text.to_string(),
            expected: Expected {
                stripped: stripped.to_string(),
                props: props
                    .into_iter()
                    .map(|(key, value)| ExpectedProp { key: key.to_string(), value: value.to_string() })
                    .collect(),
            },
        })
        .collect();
    Fixture {
        _contract: vec![
            "Shared inline-NLP-lift conformance fixture (tesela-pfix.3) — GENERATED by".to_string(),
            "crates/tesela-core/tests/nlp_lift_conformance.rs. No Rust NLP-lift parser exists".to_string(),
            "yet (blocked on tesela-ug7), so every case's `expected` is a PINNED LITERAL".to_string(),
            "(computed from the real web detectTaskTokens, then copied in) rather than".to_string(),
            "Rust-derived — see the module docs in nlp_lift_conformance.rs. Consumed by the".to_string(),
            "same test (round-trip) plus the web (nlp-lift-conformance.test.mjs) and iOS".to_string(),
            "(NLPLiftConformanceTests.swift) mirrors, which assert their REAL lift".to_string(),
            "implementation (detectTaskTokens / InlineNLP.detectLifts) against these values.".to_string(),
            "".to_string(),
            "`registry` is ONE shared DetectSpec-shaped spec reused by every case: select".to_string(),
            "`priority` (p1..p4) + date `deadline` (triggers due/deadline, also the sole".to_string(),
            "date-typed property so it doubles as the bare-date default). `anchor_date` is".to_string(),
            "the frozen \"now\" (2026-05-22, a Friday) both clients pass so relative phrases".to_string(),
            "resolve identically.".to_string(),
            "".to_string(),
            "Case shape: { name, text, expected: { stripped, props: [{key, value}] } }".to_string(),
        ],
        registry: build_registry(),
        anchor_date: "2026-05-22".to_string(),
        cases,
    }
}

/// Regenerates the on-disk fixture from the case table and asserts the
/// checked-in file is up to date. Run with `UPDATE_FIXTURES=1` to
/// regenerate after editing `raw_cases()`.
#[test]
fn fixture_is_generated_and_up_to_date() {
    let fixture = build_fixture();
    let json = serde_json::to_string_pretty(&fixture).expect("fixture serializes") + "\n";
    let path = fixture_path();

    if std::env::var("UPDATE_FIXTURES").is_ok() || !path.exists() {
        std::fs::write(&path, &json).expect("write nlp-lift-conformance.json");
    }

    let on_disk = std::fs::read_to_string(&path).expect("read nlp-lift-conformance.json");
    assert_eq!(
        on_disk, json,
        "nlp-lift-conformance.json is stale — rerun with UPDATE_FIXTURES=1 to regenerate"
    );
}

/// Case names are unique (they're the cross-language assertion ids).
#[test]
fn case_names_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for (name, _, _, _) in raw_cases() {
        assert!(seen.insert(name), "duplicate case name: {name}");
    }
}

/// The fixture covers the bead's required surface: today-noon, the
/// URL-embedded no-lift guard, and the trailing-position rule (a positive
/// AND a negative case).
#[test]
fn fixture_covers_required_surface() {
    let cases = raw_cases();
    assert!(cases.len() >= 10, "fixture has {} cases; expected 10+", cases.len());

    let by_name = |n: &str| cases.iter().find(|(name, ..)| *name == n).unwrap_or_else(|| panic!("missing case: {n}"));

    // today noon parses on both — a real time is lifted, not left unparsed.
    let (_, _, _, today_noon_props) = by_name("bare_trailing_today_noon");
    assert_eq!(today_noon_props, &vec![("deadline", "2026-05-22 12:00")]);

    // URL-embedded p1 lifts on NEITHER client.
    let (_, _, url_stripped, url_props) = by_name("url_embedded_priority_no_lift");
    assert!(url_props.is_empty(), "URL-embedded trigger must not lift");
    assert_eq!(*url_stripped, "check https://x.com/p1/doc", "URL must survive untouched");

    // trailing-date rule: positive (trailing lifts) and negative (mid-prose
    // without an intent word does not) cases both present.
    let (_, _, _, trailing_props) = by_name("bare_trailing_lift");
    assert!(!trailing_props.is_empty(), "trailing bare date must lift");
    let (_, midprose_text, midprose_stripped, midprose_props) = by_name("bare_midprose_not_lifted");
    assert!(midprose_props.is_empty(), "mid-prose bare date without an intent word must not lift");
    assert_eq!(midprose_stripped, midprose_text, "unchanged when nothing lifts");
}

/// The Rust hoist (`tesela_core::nlp_lift::detect_task_tokens`,
/// `tesela-ug7`) must reproduce every pinned case — this is the
/// conformance check that proves the Rust port is faithful to the web
/// implementation the fixture was generated from, independent of the
/// generator above.
#[test]
fn rust_detect_task_tokens_matches_fixture() {
    use std::collections::HashSet;
    use tesela_core::nlp_lift;

    // Round-trip this file's `Registry` (identical JSON shape) into the
    // real `nlp_lift::Registry` rather than re-parsing the on-disk file.
    let registry_json = serde_json::to_string(&build_registry()).expect("registry serializes");
    let registry: nlp_lift::Registry =
        serde_json::from_str(&registry_json).expect("registry round-trips");
    let today = chrono::NaiveDate::parse_from_str("2026-05-22", "%Y-%m-%d").expect("anchor date");

    let mut failures = Vec::new();
    for (name, text, expected_stripped, expected_props) in raw_cases() {
        let result = nlp_lift::detect_task_tokens(text, &registry, today);
        let want: HashSet<String> = expected_props.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let got: HashSet<String> = result.props.iter().map(|p| format!("{}={}", p.key, p.value)).collect();
        if result.stripped != expected_stripped || got != want {
            failures.push(format!(
                "  {name} — text {text:?}: expected stripped={expected_stripped:?} props={want:?}, got stripped={:?} props={got:?}",
                result.stripped
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} conformance case(s) diverged:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
