// Unit tests for the saved-views DSL editor helpers
// (web/src/lib/views/view-dsl.ts): validation parity with the server's
// validate_dsl and chip clause toggling (chips-as-inserters). The cheap
// key-only autocomplete this file used to own moved to the shared
// QueryInput widget's completion module (tesela-vp9.2) — see
// web/tests/unit/query-input-completion.test.mjs.
import { test } from "node:test";
import assert from "node:assert/strict";

import {
  validateViewDsl,
  toggleClausesInDsl,
  clausesActiveInDsl,
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
