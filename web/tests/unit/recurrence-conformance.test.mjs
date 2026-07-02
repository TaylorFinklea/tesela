// Web TS consumer of the shared recurrence-DSL conformance fixture
// (`crates/tesela-core/tests/fixtures/recurrence-conformance.json`).
//
// The fixture is GENERATED from the Rust side
// (crates/tesela-core/tests/recurrence_conformance.rs) — Rust is the
// source of truth for `valid`. `canonical_display` is sourced from THIS
// file's `formatRecurrence` (no Rust formatter exists), so this consumer
// pins the display behavior other consumers (iOS) must mirror.
//
// Every case runs through the REAL parser (`parseRecurrenceInput`) and
// the REAL formatter (`formatRecurrence`) from src/lib/date-parser.ts and
// src/lib/recurrence-format.ts — not a reimplementation. The client-only
// `client_extraction_cases` section runs through the REAL
// `parseDateAndRecurrenceInput` extraction path instead.

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

import { parseRecurrenceInput, parseDateAndRecurrenceInput } from "../../src/lib/date-parser.ts";
import { formatRecurrence } from "../../src/lib/recurrence-format.ts";

const fixtureUrl = new URL(
  "../../../crates/tesela-core/tests/fixtures/recurrence-conformance.json",
  import.meta.url,
);
const fixture = JSON.parse(readFileSync(fixtureUrl, "utf8"));

test("parseRecurrenceInput validity matches the Rust fixture", () => {
  const failures = [];
  for (const c of fixture.cases) {
    const got = parseRecurrenceInput(c.input) !== null;
    if (got !== c.valid) {
      failures.push(
        `  ${c.name} — input ${JSON.stringify(c.input)}: expected valid=${c.valid}, got ${got}`,
      );
    }
  }
  assert.equal(
    failures.length,
    0,
    `${failures.length} of ${fixture.cases.length} conformance case(s) diverged on validity:\n${failures.join("\n")}`,
  );
});

test("formatRecurrence output matches the fixture's canonical_display", () => {
  const failures = [];
  for (const c of fixture.cases) {
    const got = formatRecurrence(c.input);
    if (got !== c.canonical_display) {
      failures.push(
        `  ${c.name} — input ${JSON.stringify(c.input)}: expected ${JSON.stringify(c.canonical_display)}, got ${JSON.stringify(got)}`,
      );
    }
  }
  assert.equal(
    failures.length,
    0,
    `${failures.length} of ${fixture.cases.length} conformance case(s) diverged on display:\n${failures.join("\n")}`,
  );
});

test("case names are unique (cross-language assertion ids)", () => {
  const seen = new Set();
  for (const c of fixture.cases) {
    assert.ok(!seen.has(c.name), `duplicate case name: ${c.name}`);
    seen.add(c.name);
  }
});

test("fixture covers the 2026-06-20 grammar (biweekly-class cadences)", () => {
  assert.ok(
    fixture.cases.length >= 30,
    `fixture has ${fixture.cases.length} cases; expected 30+ for full-grammar coverage`,
  );
  for (const name of ["biweekly", "fortnightly", "quarterly"]) {
    const c = fixture.cases.find((c) => c.name === name);
    assert.ok(c, `fixture must include a "${name}" case`);
    assert.equal(c.valid, true, `"${name}" must be valid`);
  }
});

// Client-only section: mixed "<date phrase> <recurrence phrase>" EXTRACTION
// via the REAL parseDateAndRecurrenceInput — not standalone
// parseRecurrenceInput. Rust has no extraction concept (see
// recurrence_conformance.rs module docs), so these are pinned literals
// rather than parser-derived, but they still run through this file's
// real extraction path, catching regressions in TRAILING_RECUR_RE.
test("parseDateAndRecurrenceInput extraction matches the fixture's client_extraction_cases", () => {
  const failures = [];
  for (const c of fixture.client_extraction_cases) {
    const [y, m, d] = c.anchor_date.split("-").map(Number);
    const anchor = new Date(y, m - 1, d);
    const got = parseDateAndRecurrenceInput(c.input, anchor);
    if (c.expected === null) {
      if (got !== null) {
        failures.push(`  ${c.name} — input ${JSON.stringify(c.input)}: expected null, got ${JSON.stringify(got)}`);
      }
      continue;
    }
    const want = c.expected;
    const gotShape = got && {
      date: got.date,
      time: got.time,
      recurrence: got.recurrence,
      field: got.field,
    };
    if (
      !got ||
      got.date !== want.date ||
      got.time !== want.time ||
      got.recurrence !== want.recurrence ||
      got.field !== want.field
    ) {
      failures.push(
        `  ${c.name} — input ${JSON.stringify(c.input)}: expected ${JSON.stringify(want)}, got ${JSON.stringify(gotShape)}`,
      );
    }
  }
  assert.equal(
    failures.length,
    0,
    `${failures.length} of ${fixture.client_extraction_cases.length} client extraction case(s) diverged:\n${failures.join("\n")}`,
  );
});

test("client_extraction_cases covers trailing extraction for the new grammar", () => {
  const inputs = fixture.client_extraction_cases.map((c) => c.input);
  for (const needle of ["biweekly", "fortnightly", "quarterly", "every other", "every weekday"]) {
    assert.ok(
      inputs.some((i) => i.includes(needle)),
      `client_extraction_cases must exercise trailing "${needle}" extraction`,
    );
  }
});
