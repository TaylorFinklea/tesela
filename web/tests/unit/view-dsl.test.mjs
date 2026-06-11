// Unit tests for the saved-views DSL editor helpers
// (web/src/lib/views/view-dsl.ts): validation parity with the server's
// validate_dsl, chip clause toggling (chips-as-inserters), and the cheap
// key autocomplete.
import { test } from "node:test";
import assert from "node:assert/strict";

import {
  validateViewDsl,
  toggleClausesInDsl,
  clausesActiveInDsl,
  dslKeySuggestions,
  applyDslSuggestion,
  BASE_DSL_KEYS,
} from "../../src/lib/views/view-dsl.ts";
import { INBOX_VIEW_DSL } from "../../src/lib/query-language.ts";

// ── validateViewDsl (mirrors routes/views.rs validate_dsl) ────────────────

test("validate — the seeded Inbox DSL is valid", () => {
  assert.equal(validateViewDsl(INBOX_VIEW_DSL), null);
});

test("validate — empty / whitespace-only is rejected", () => {
  assert.match(validateViewDsl(""), /must not be empty/);
  assert.match(validateViewDsl("   "), /must not be empty/);
});

test("validate — zero recognized predicates is rejected with the inline message", () => {
  // The parser is liberal: stray punctuation yields no predicates at all.
  const err = validateViewDsl("???");
  assert.match(err, /no predicates recognized/);
  assert.match(err, /\?\?\?/); // names the offending DSL
});

test("validate — lone kind: selector and bare ORDER BY are carve-outs", () => {
  assert.equal(validateViewDsl("kind:page"), null);
  assert.equal(validateViewDsl("ORDER BY created DESC"), null);
});

test("validate — ordinary predicates pass", () => {
  assert.equal(validateViewDsl("status:doing"), null);
  assert.equal(validateViewDsl("tag:project -has:scheduled"), null);
  assert.equal(validateViewDsl("status:backlog,todo"), null);
});

// ── toggleClausesInDsl / clausesActiveInDsl (chips as inserters) ──────────

test("toggle — appends an absent single-clause fragment at the end", () => {
  const out = toggleClausesInDsl("status:doing", ["-has:scheduled"]);
  assert.equal(out, "status:doing -has:scheduled");
});

test("toggle — removes a present fragment (round-trips)", () => {
  const dsl = "status:doing -has:scheduled";
  assert.equal(clausesActiveInDsl(dsl, ["-has:scheduled"]), true);
  assert.equal(toggleClausesInDsl(dsl, ["-has:scheduled"]), "status:doing");
});

test("toggle — multi-clause chip: adds only the missing clauses", () => {
  // Chip with two clauses, one already present → end state is all-on,
  // no duplicate of the already-present clause.
  const out = toggleClausesInDsl("a:1 x:y", ["a:1", "b:2"]);
  assert.equal(out, "a:1 x:y b:2");
  assert.equal(clausesActiveInDsl(out, ["a:1", "b:2"]), true);
});

test("toggle — multi-clause chip: removes every clause when all present", () => {
  const out = toggleClausesInDsl("a:1 x:y b:2", ["a:1", "b:2"]);
  assert.equal(out, "x:y");
});

test("toggle — preserves the order of untouched hand-written tokens", () => {
  const out = toggleClausesInDsl("tag:project status:todo ORDER BY created", [
    "-is:heading",
  ]);
  assert.equal(out, "tag:project status:todo ORDER BY created -is:heading");
});

test("clausesActiveInDsl — empty clause list is never active", () => {
  assert.equal(clausesActiveInDsl("status:todo", []), false);
});

test("toggle round-trip with every inbox chip fragment stays valid", () => {
  // The real registry fragments (chips.ts) are what the editor inserts;
  // inserting then removing any of them must leave a saveable DSL.
  const fragments = [["-has:status"], ["-is:heading"], ["-on:daily-page"], ["-on:system-pages"], ["has:scheduled"], ["has:deadline"], ["-has:tag"]];
  for (const clauses of fragments) {
    const on = toggleClausesInDsl(INBOX_VIEW_DSL, clauses);
    assert.equal(validateViewDsl(on), null, `insert ${clauses} stays valid`);
    const off = toggleClausesInDsl(on, clauses);
    assert.equal(validateViewDsl(off), null, `remove ${clauses} stays valid`);
  }
});

// ── dslKeySuggestions / applyDslSuggestion ────────────────────────────────

test("suggest — partial key at the caret offers matching keys", () => {
  const s = dslKeySuggestions("sta", 3);
  assert.ok(s);
  assert.deepEqual(s.items, ["status:"]);
  assert.equal(s.from, 0);
  assert.equal(s.to, 3);
});

test("suggest — after a space, a new partial word suggests", () => {
  const s = dslKeySuggestions("status:todo ta", 14);
  assert.ok(s);
  assert.deepEqual(s.items, ["tag:", "tag-in:"]);
  assert.equal(s.from, 12);
});

test("suggest — leading negation dash is preserved (replaces only the key)", () => {
  const s = dslKeySuggestions("-h", 2);
  assert.ok(s);
  assert.deepEqual(s.items, ["has:"]);
  assert.equal(s.from, 1); // after the '-'
  const applied = applyDslSuggestion("-h", s, s.items[0]);
  assert.equal(applied.dsl, "-has:");
  assert.equal(applied.cursor, 5);
});

test("suggest — nothing for an empty token, a completed key, or mid-word caret", () => {
  assert.equal(dslKeySuggestions("status:todo ", 12), null); // empty token
  assert.equal(dslKeySuggestions("status:to", 9), null); // already has key:
  assert.equal(dslKeySuggestions("status", 3), null); // caret mid-word
});

test("suggest — registry property keys mix in after the base keys, deduped", () => {
  const s = dslKeySuggestions("p", 1, ["Priority", "Points", "page"]);
  assert.ok(s);
  // base 'page' first, then registry keys lowercased, 'page' deduped.
  assert.deepEqual(s.items, ["page:", "priority:", "points:"]);
});

test("suggest — exact key match still offers the colon completion", () => {
  const s = dslKeySuggestions("status", 6);
  assert.ok(s);
  assert.deepEqual(s.items, ["status:"]);
});

test("applyDslSuggestion — replaces the partial and reports the caret", () => {
  const s = dslKeySuggestions("status:todo ta", 14);
  const applied = applyDslSuggestion("status:todo ta", s, "tag:");
  assert.equal(applied.dsl, "status:todo tag:");
  assert.equal(applied.cursor, 16);
});

test("BASE_DSL_KEYS covers the task's required key set", () => {
  for (const k of ["status", "tag", "has", "is", "on", "kind", "text"]) {
    assert.ok(BASE_DSL_KEYS.includes(k), `${k} present`);
  }
});
