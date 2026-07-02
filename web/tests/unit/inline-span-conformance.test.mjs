// Web TS consumer of the shared inline-span rendering conformance fixture
// (`crates/tesela-core/tests/fixtures/inline-span-conformance.json`,
// tesela-pfix.6).
//
// The fixture pins the flat, ordered inline-span list that a block's
// single-line prose renders into. There is no Rust consumer — Rust never
// renders UI — so this fixture has exactly two engines: this file (the real
// `parseInlineSpans` from src/lib/block-parser.ts) and the iOS mirror
// (BlockText.parseInlineSpans, InlineSpanConformanceTests.swift). See the
// fixture's `_contract` header for the full scope/precedence rules.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

import { parseInlineSpans } from "../../src/lib/block-parser.ts";

const fixtureUrl = new URL(
  "../../../crates/tesela-core/tests/fixtures/inline-span-conformance.json",
  import.meta.url,
);
const fixture = JSON.parse(readFileSync(fixtureUrl, "utf8"));

test("all conformance cases pass through the real parseInlineSpans", () => {
  const failures = [];
  for (const c of fixture.cases) {
    const got = parseInlineSpans(c.text);
    const gotStr = JSON.stringify(got);
    const wantStr = JSON.stringify(c.expected);
    if (gotStr !== wantStr) {
      failures.push(`  ${c.name} — text ${JSON.stringify(c.text)}:\n    expected ${wantStr}\n    got      ${gotStr}`);
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

test("fixture covers every required span kind", () => {
  assert.ok(fixture.cases.length >= 20, `fixture has ${fixture.cases.length} cases; expected 20+`);
  const kinds = new Set();
  for (const c of fixture.cases) {
    for (const span of c.expected) kinds.add(span.kind);
  }
  for (const required of ["plain", "bold", "italic", "code", "strike", "link", "wikilink"]) {
    assert.ok(kinds.has(required), `fixture must cover the "${required}" span kind`);
  }
});
