//! Rust consumer of the shared property-override-resolution conformance
//! fixture (`tests/fixtures/property-override-conformance.json`).
//!
//! The fixture is the ONE source of truth for per-type property-override
//! merge semantics across the three implementations — this file (Rust,
//! running the REAL `db::sqlite::build_overrides` / `apply_override`, the
//! same functions `get_resolved_tag_def`/`get_all_tag_defs` use in
//! production), the web TS consumer (mirroring `buildOverrides`/
//! `applyOverride` in `web/src/lib/property-registry.ts`), and the iOS
//! Swift consumer (mirroring `PropertyRegistry.buildOverrides`/
//! `applyOverride`). Rust is the source of truth — where implementations
//! disagree, fix the implementation, never the fixture.
//!
//! Adapter contract (mirrors the `_contract` header in the fixture):
//!   rows        → fed to `build_overrides` as `(overrides_json, hidden_pairs)`
//!                 tuples; Rust re-serializes each row's `overrides` object
//!                 to a JSON string (the real DB stores it that way).
//!   property    → looked up case-insensitively in the built override map.
//!   definedInRegistry/base → constructs the starting `PropertyDef` exactly
//!                 as `get_resolved_tag_def`'s `Some(row)`/`None` branches do.
//!   expect      → compared against the `PropertyDef` after `apply_override`.

use std::collections::HashMap;

use serde::Deserialize;
use tesela_core::db::sqlite::{apply_override, build_overrides};
use tesela_core::types::{PropertyDef, Visibility};

#[derive(Deserialize)]
struct Fixture {
    #[serde(rename = "_contract")]
    _contract: Vec<String>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Row {
    overrides: serde_json::Value,
    hidden: HashMap<String, Vec<String>>,
}

#[derive(Deserialize)]
struct Base {
    #[serde(rename = "valueType")]
    value_type: String,
    choices: Vec<String>,
    default: Option<String>,
    #[serde(rename = "hideByDefault")]
    hide_by_default: bool,
}

#[derive(Deserialize)]
struct Expect {
    choices: Vec<String>,
    default: Option<String>,
    show: Visibility,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    rows: Vec<Row>,
    property: String,
    #[serde(rename = "definedInRegistry")]
    defined_in_registry: bool,
    #[serde(default)]
    base: Option<Base>,
    expect: Expect,
}

fn load_fixture() -> Fixture {
    let raw = include_str!("fixtures/property-override-conformance.json");
    serde_json::from_str(raw).expect("property-override-conformance.json parses")
}

/// Mirror of `get_resolved_tag_def`'s `Some(row)` / `None` branches: a
/// defined registry property starts from its global config; an undefined
/// one starts from the §3.5c text stub.
fn starting_def(case: &Case) -> PropertyDef {
    match (&case.base, case.defined_in_registry) {
        (Some(b), true) => PropertyDef {
            name: case.property.clone(),
            value_type: b.value_type.clone(),
            values: Some(b.choices.clone()),
            default: b.default.clone(),
            required: false,
            ..Default::default()
        },
        _ => PropertyDef {
            name: case.property.clone(),
            value_type: "text".to_string(),
            values: None,
            default: None,
            required: false,
            ..Default::default()
        },
    }
}

/// Every fixture case must resolve through the real `build_overrides` +
/// `apply_override` production pipeline.
#[test]
fn all_conformance_cases_resolve_through_the_real_merge() {
    let fixture = load_fixture();
    let mut failures = Vec::new();
    for case in &fixture.cases {
        let override_rows: Vec<(String, Vec<(String, Vec<String>)>)> = case
            .rows
            .iter()
            .map(|r| {
                let overrides_json =
                    serde_json::to_string(&r.overrides).expect("row overrides serialize");
                let hidden_pairs: Vec<(String, Vec<String>)> = r
                    .hidden
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                (overrides_json, hidden_pairs)
            })
            .collect();
        let overrides = build_overrides(&override_rows);
        let over = overrides.get(&case.property.to_ascii_lowercase());

        let mut def = starting_def(case);
        let hide_by_default = case
            .base
            .as_ref()
            .map(|b| b.hide_by_default)
            .unwrap_or(false);
        apply_override(&mut def, over, hide_by_default);

        let got_choices = def.values.clone().unwrap_or_default();
        let got_show = def.show;
        if got_choices != case.expect.choices
            || def.default != case.expect.default
            || got_show != Some(case.expect.show)
        {
            failures.push(format!(
                "  {} — expected choices={:?} default={:?} show={:?}, got choices={:?} default={:?} show={:?}",
                case.name,
                case.expect.choices,
                case.expect.default,
                case.expect.show,
                got_choices,
                def.default,
                got_show
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

/// The fixture meets a minimum breadth bar covering the locked merge rules
/// (choices REPLACE, hide_choices SUBTRACT, first-insert-wins, additive
/// legacy hidden_ fold, §3.5c text-stub) plus the extends-chain walk.
#[test]
fn fixture_covers_required_surface() {
    let fixture = load_fixture();
    assert!(
        fixture.cases.len() >= 10,
        "fixture has {} cases; expected at least 10",
        fixture.cases.len()
    );
    let multi_row_cases = fixture.cases.iter().filter(|c| c.rows.len() > 1).count();
    assert!(
        multi_row_cases >= 2,
        "expected at least 2 cases exercising a multi-row (extends-chain) walk, found {multi_row_cases}"
    );
    let stub_cases = fixture
        .cases
        .iter()
        .filter(|c| !c.defined_in_registry)
        .count();
    assert!(
        stub_cases >= 1,
        "expected at least 1 case exercising the §3.5c text-stub branch"
    );
}
