// Web TS consumer of the shared query-DSL conformance fixture
// (`crates/tesela-core/tests/fixtures/query-conformance.json`).
//
// The fixture is the ONE source of truth for DSL matching semantics
// across the three implementations — Rust (query.rs, consumed by
// crates/tesela-core/tests/query_conformance.rs), this file (web TS,
// running the real `parseQuery` → `blockMatches` pipeline from
// src/lib/query-language.ts), and the iOS Swift consumer (mirroring
// LocalQueryEngine.swift). Rust is the source of truth — where
// implementations disagree, fix the implementation, never the fixture.
//
// Adapter contract (mirrors the `_contract` header in the fixture and
// the Rust consumer's `to_parsed_block`): the language-neutral fixture
// block maps onto ParsedBlock as
//   text        → `text` (and `raw_text` = "- {text}")
//   tags        → `tags` (own tags; inherited chain left empty)
//   properties  → `properties`
//   isHeading   → asserted consistent with what the matcher derives
//                 from `text` (see the dedicated test below)
//   onDailyPage → `note_id` = "2026-06-10" (canonical daily id) when
//                 true, "fixture-note" otherwise
//   noteType    → `parent_note_type`
//
// Zero skips: every fixture predicate (tags, properties, has, on:,
// is:, text:, comma-OR, tag-in) is structurally evaluable client-side.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

import {
  parseQuery,
  blockMatches,
  INBOX_VIEW_DSL,
} from "../../src/lib/query-language.ts";

const fixtureUrl = new URL(
  "../../../crates/tesela-core/tests/fixtures/query-conformance.json",
  import.meta.url,
);
const fixture = JSON.parse(readFileSync(fixtureUrl, "utf8"));

function toParsedBlock(b) {
  const noteId = b.onDailyPage ? "2026-06-10" : "fixture-note";
  return {
    id: `${noteId}:1`,
    bid: null,
    text: b.text,
    raw_text: `- ${b.text}`,
    tags: b.tags,
    inline_tags: [],
    trailing_tags: [],
    inherited_tags: [],
    properties: b.properties,
    indent_level: 0,
    note_id: noteId,
    parent_note_type: b.noteType,
  };
}

test("all conformance cases pass through the real parser + matcher", () => {
  const failures = [];
  for (const c of fixture.cases) {
    const q = parseQuery(c.dsl);
    const got = blockMatches(toParsedBlock(c.block), q);
    if (got !== c.expect) {
      failures.push(
        `  ${c.name} — dsl ${JSON.stringify(c.dsl)}: expected ${c.expect}, got ${got}`,
      );
    }
  }
  assert.equal(
    failures.length,
    0,
    `${failures.length} of ${fixture.cases.length} conformance case(s) diverged from the fixture:\n${failures.join("\n")}`,
  );
});

test("isHeading flags are consistent with what the matcher derives from text", () => {
  // TS may read the flag or derive from text — assert both agree so a
  // flag-reading engine can't pass while disagreeing with Rust.
  const q = parseQuery("is:heading");
  for (const c of fixture.cases) {
    assert.equal(
      blockMatches(toParsedBlock(c.block), q),
      c.block.isHeading,
      `case ${c.name}: isHeading flag disagrees with text ${JSON.stringify(c.block.text)}`,
    );
  }
});

test("case names are unique (cross-language assertion ids)", () => {
  const seen = new Set();
  for (const c of fixture.cases) {
    assert.ok(!seen.has(c.name), `duplicate case name: ${c.name}`);
    seen.add(c.name);
  }
});

test("fixture covers the required surface and pins the Inbox DSL verbatim", () => {
  assert.ok(
    fixture.cases.length >= 40,
    `fixture has ${fixture.cases.length} cases; the spec requires 40+`,
  );
  const inboxCases = fixture.cases.filter((c) => c.dsl === INBOX_VIEW_DSL).length;
  assert.ok(
    inboxCases >= 5,
    `expected a full Inbox-default matrix (>=5 cases using INBOX_VIEW_DSL verbatim), found ${inboxCases}`,
  );
});
