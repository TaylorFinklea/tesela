// Web TS consumer of the shared property-override-resolution conformance
// fixture (`crates/tesela-core/tests/fixtures/property-override-conformance.json`).
//
// The fixture is the ONE source of truth for per-type property-override
// merge semantics across the three implementations — Rust (db/sqlite.rs
// `build_overrides`/`apply_override`, consumed by
// crates/tesela-core/tests/property_override_conformance.rs, the source of
// truth), this file (web TS, running the REAL `buildOverrides`/
// `applyOverride` from src/lib/property-registry.ts), and the iOS Swift
// consumer (mirroring `PropertyRegistry.buildOverrides`/`applyOverride`).
//
// Adapter contract (mirrors the `_contract` header in the fixture and the
// Rust consumer): `rows` feed `buildOverrides` directly (its
// `{overrides, hidden}` shape matches the fixture verbatim); `property` is
// looked up lowercased against the built override map; `definedInRegistry`/
// `base` construct the starting `PropertyDefinition` exactly as
// `getTagPropertyDefs`'s `def`/`stub` branches do; `expect` is compared
// against the def `applyOverride` returns.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

import { buildOverrides, applyOverride } from "../../src/lib/property-registry.ts";

const fixtureUrl = new URL(
  "../../../crates/tesela-core/tests/fixtures/property-override-conformance.json",
  import.meta.url,
);
const fixture = JSON.parse(readFileSync(fixtureUrl, "utf8"));

// Mirror of `getTagPropertyDefs`'s per-property `def`/`stub` branches: a
// defined registry property starts from its global config; an undefined one
// starts from the §3.5c text stub. Only the fields `applyOverride` reads
// (`choices`, `default`, and passthrough fields it doesn't touch) matter.
function startingDef(c) {
  if (c.definedInRegistry && c.base) {
    return {
      name: c.property,
      value_type: c.base.valueType,
      choices: c.base.choices,
      default: c.base.default,
      show: null,
      hide_by_default: c.base.hideByDefault,
      hide_empty: true,
      chip_icon: null,
      chip_label_mode: null,
      chip_short_label: null,
      chip_value_format: null,
      chord_key: null,
      value_chord_keys: {},
      choice_colors: {},
      nl_triggers: [],
    };
  }
  return {
    name: c.property,
    value_type: "text",
    choices: [],
    default: null,
    show: null,
    hide_by_default: false,
    hide_empty: true,
    chip_icon: null,
    chip_label_mode: null,
    chip_short_label: null,
    chip_value_format: null,
    chord_key: null,
    value_chord_keys: {},
    choice_colors: {},
    nl_triggers: [],
  };
}

test("all property-override conformance cases resolve through the real merge", () => {
  const failures = [];
  for (const c of fixture.cases) {
    const overrides = buildOverrides(c.rows);
    const over = overrides.get(c.property.toLowerCase());
    const def = startingDef(c);
    const hideByDefault = c.base?.hideByDefault ?? false;
    const resolved = applyOverride(def, over, hideByDefault);
    const got = { choices: resolved.choices, default: resolved.default, show: resolved.show };
    const expect = c.expect;
    if (
      JSON.stringify(got.choices) !== JSON.stringify(expect.choices) ||
      got.default !== expect.default ||
      got.show !== expect.show
    ) {
      failures.push(
        `  ${c.name} — expected ${JSON.stringify(expect)}, got ${JSON.stringify(got)}`,
      );
    }
  }
  assert.equal(
    failures.length,
    0,
    `${failures.length} of ${fixture.cases.length} conformance case(s) diverged from the fixture:\n${failures.join("\n")}`,
  );
});

test("case names are unique (cross-language assertion ids)", () => {
  const seen = new Set();
  for (const c of fixture.cases) {
    assert.ok(!seen.has(c.name), `duplicate case name: ${c.name}`);
    seen.add(c.name);
  }
});

test("fixture covers the required surface", () => {
  assert.ok(
    fixture.cases.length >= 10,
    `fixture has ${fixture.cases.length} cases; expected at least 10`,
  );
  const multiRowCases = fixture.cases.filter((c) => c.rows.length > 1).length;
  assert.ok(
    multiRowCases >= 2,
    `expected at least 2 cases exercising a multi-row (extends-chain) walk, found ${multiRowCases}`,
  );
  const stubCases = fixture.cases.filter((c) => !c.definedInRegistry).length;
  assert.ok(stubCases >= 1, "expected at least 1 case exercising the §3.5c text-stub branch");
});
