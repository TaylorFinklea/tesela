// Web TS consumer of the shared inline-NLP-lift conformance fixture
// (`crates/tesela-core/tests/fixtures/nlp-lift-conformance.json`).
//
// No Rust NLP-lift parser exists yet (blocked on tesela-ug7), so the
// fixture's `expected` values are PINNED LITERALS generated from THIS
// file's real `detectTaskTokens` (see the module docs in
// `nlp_lift_conformance.rs`) — this test still asserts through the REAL
// implementation (`web/src/lib/task-tokens.ts`), not a reimplementation,
// so any future regression here is caught immediately, same as if Rust had
// derived the fixture.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

import { detectTaskTokens } from "../../src/lib/task-tokens.ts";

const fixtureUrl = new URL(
  "../../../crates/tesela-core/tests/fixtures/nlp-lift-conformance.json",
  import.meta.url,
);
const fixture = JSON.parse(readFileSync(fixtureUrl, "utf8"));

/** The fixture's shared `registry` is already DetectSpec-shaped. */
const SPEC = {
  defaultDateProperty: fixture.registry.default_date_property,
  properties: fixture.registry.properties.map((p) => ({
    key: p.key,
    valueType: p.value_type,
    choices: p.choices,
    triggers: p.triggers,
  })),
};

const [y, m, d] = fixture.anchor_date.split("-").map(Number);
const anchor = new Date(y, m - 1, d);

function propsKey(props) {
  return props
    .map((p) => `${p.key}=${p.value}`)
    .sort()
    .join(",");
}

test("detectTaskTokens matches the shared nlp-lift-conformance fixture", () => {
  const failures = [];
  for (const c of fixture.cases) {
    const got = detectTaskTokens(c.text, SPEC, anchor);
    const wantProps = c.expected.props;
    const stripOk = got.stripped === c.expected.stripped;
    const propsOk = propsKey(got.props) === propsKey(wantProps);
    if (!stripOk || !propsOk) {
      failures.push(
        `  ${c.name} — text ${JSON.stringify(c.text)}:\n` +
          `    expected stripped=${JSON.stringify(c.expected.stripped)} props=${JSON.stringify(wantProps)}\n` +
          `    got      stripped=${JSON.stringify(got.stripped)} props=${JSON.stringify(got.props)}`,
      );
    }
  }
  assert.equal(
    failures.length,
    0,
    `${failures.length} of ${fixture.cases.length} conformance case(s) diverged:\n${failures.join("\n")}`,
  );
});

test("case names are unique (cross-language assertion ids)", () => {
  const seen = new Set();
  for (const c of fixture.cases) {
    assert.ok(!seen.has(c.name), `duplicate case name: ${c.name}`);
    seen.add(c.name);
  }
});

test("fixture covers today-noon, the URL-embedded no-lift guard, and the trailing-position rule", () => {
  const byName = (n) => {
    const c = fixture.cases.find((c) => c.name === n);
    assert.ok(c, `fixture must include a "${n}" case`);
    return c;
  };

  // "today noon" — the web extractTime gap this bead fixes.
  const todayNoon = byName("bare_trailing_today_noon");
  assert.deepEqual(todayNoon.expected.props, [{ key: "deadline", value: "2026-05-22 12:00" }]);

  // URL-embedded p1 lifts on neither client.
  const url = byName("url_embedded_priority_no_lift");
  assert.deepEqual(url.expected.props, []);
  assert.equal(url.expected.stripped, url.text, "URL survives the (non-)strip intact");

  // Trailing-position rule: a positive case (trailing lifts)...
  const trailing = byName("bare_trailing_lift");
  assert.ok(trailing.expected.props.length > 0, "trailing bare date must lift");
  // ...and a negative case (mid-prose without an intent word does not).
  const midProse = byName("bare_midprose_not_lifted");
  assert.deepEqual(midProse.expected.props, []);
  assert.equal(midProse.expected.stripped, midProse.text);
});
