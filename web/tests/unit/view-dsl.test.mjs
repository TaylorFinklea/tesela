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
import { CHIP_REGISTRY } from "../../src/lib/ambients/inbox/chips.ts";

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

test("toggle round-trip with legacy colon-DSL fragments stays valid (backcompat)", () => {
  // Colon-DSL still parses (product lock: JQL is the documented default,
  // colon fragments keep working) — inserting then removing any of these
  // must leave a saveable DSL.
  const fragments = [["-has:status"], ["-is:heading"], ["-on:daily-page"], ["-on:system-pages"], ["has:scheduled"], ["has:deadline"], ["-has:tag"]];
  for (const clauses of fragments) {
    const on = toggleClausesInDsl(INBOX_VIEW_DSL, clauses);
    assert.equal(validateViewDsl(on), null, `insert ${clauses} stays valid`);
    const off = toggleClausesInDsl(on, clauses);
    assert.equal(validateViewDsl(off), null, `remove ${clauses} stays valid`);
  }
});

test("toggle round-trip with every REAL CHIP_REGISTRY (JQL) clause stays valid", () => {
  // The actual registry fragments (chips.ts, post tesela-vp9.3 migration)
  // are what the editor inserts today; inserting then removing any of them
  // must leave a saveable DSL, and the off-state must equal the original.
  for (const chip of CHIP_REGISTRY) {
    const on = toggleClausesInDsl(INBOX_VIEW_DSL, chip.clauses);
    assert.equal(validateViewDsl(on), null, `insert ${chip.id} stays valid`);
    assert.equal(clausesActiveInDsl(on, chip.clauses), true, `${chip.id} reads active after insert`);
    const off = toggleClausesInDsl(on, chip.clauses);
    assert.equal(validateViewDsl(off), null, `remove ${chip.id} stays valid`);
    assert.equal(clausesActiveInDsl(off, chip.clauses), false, `${chip.id} reads inactive after removal`);
  }
});

// ── mixed hand-typed JQL + chip toggling (tesela-vp9.3 DO item 4) ──────────

test("toggle — chip predicate round-trips around hand-typed JQL with an explicit AND", () => {
  const handTyped = "points > 5 AND status IS NULL";
  const untriaged = CHIP_REGISTRY.find((c) => c.id === "untriaged");
  assert.equal(clausesActiveInDsl(handTyped, untriaged.clauses), true);
  const off = toggleClausesInDsl(handTyped, untriaged.clauses);
  assert.equal(off, "points > 5");
  const on = toggleClausesInDsl(off, untriaged.clauses);
  assert.equal(on, "points > 5 status IS NULL");
  assert.equal(clausesActiveInDsl(on, untriaged.clauses), true);
});

test("toggle — turning a second chip on/off leaves an unrelated hand-typed predicate alone", () => {
  const handTyped = "points > 5 AND status IS NULL";
  const hasDeadline = CHIP_REGISTRY.find((c) => c.id === "hasDeadline");
  // Appending doesn't touch the existing text at all (only removal
  // reconstructs spans) — the hand-typed "AND" survives verbatim.
  const on = toggleClausesInDsl(handTyped, hasDeadline.clauses);
  assert.equal(on, "points > 5 AND status IS NULL deadline IS NOT NULL");
  // Removing hasDeadline's own predicate DOES reconstruct from spans,
  // rejoining the two survivors with a plain space.
  const off = toggleClausesInDsl(on, hasDeadline.clauses);
  assert.equal(off, "points > 5 status IS NULL");
});

// ── span-removal byte-exactness ─────────────────────────────────────────

test("toggle — removal preserves unrelated predicate text byte-exactly (odd spacing/casing)", () => {
  const dsl = "Points>5   AND   status IS NULL   AND  Tag:Project";
  const untriaged = CHIP_REGISTRY.find((c) => c.id === "untriaged");
  const out = toggleClausesInDsl(dsl, untriaged.clauses);
  // "Points>5" and "Tag:Project" keep their EXACT original characters
  // (spacing/casing) — only the join between survivors is normalized to a
  // single space, and doubled whitespace collapses.
  assert.equal(out, "Points>5 Tag:Project");
});

test("toggle — removal drops a dangling explicit AND alongside the removed predicate", () => {
  const dsl = "status IS NULL AND priority:>=3";
  const untriaged = CHIP_REGISTRY.find((c) => c.id === "untriaged");
  assert.equal(toggleClausesInDsl(dsl, untriaged.clauses), "priority:>=3");
});

// ── active-state detection: top-level AND atoms only ────────────────────

test("clausesActiveInDsl — a predicate nested inside an OR group is NOT active", () => {
  const untriaged = CHIP_REGISTRY.find((c) => c.id === "untriaged");
  // status IS NULL only fires if `points > 5`, so the chip's predicate
  // isn't unconditionally in effect — must not read as "active".
  assert.equal(clausesActiveInDsl("points > 5 OR status IS NULL", untriaged.clauses), false);
  // Parenthesized OR group ANDed with something else at the top level:
  // still nested, still not active.
  assert.equal(
    clausesActiveInDsl("(points > 5 OR status IS NULL) AND tag:project", untriaged.clauses),
    false,
  );
});

test("clausesActiveInDsl — the same predicate IS active at the top level alongside an unrelated OR group", () => {
  const untriaged = CHIP_REGISTRY.find((c) => c.id === "untriaged");
  assert.equal(
    clausesActiveInDsl("status IS NULL AND (tag:project OR tag:work)", untriaged.clauses),
    true,
  );
});

test("toggle — never removes a predicate that only appears inside an OR group", () => {
  const untriaged = CHIP_REGISTRY.find((c) => c.id === "untriaged");
  const dsl = "points > 5 OR status IS NULL";
  // Not active → toggle APPENDS rather than trying (and failing) to remove.
  const out = toggleClausesInDsl(dsl, untriaged.clauses);
  assert.equal(out, "points > 5 OR status IS NULL status IS NULL");
});
