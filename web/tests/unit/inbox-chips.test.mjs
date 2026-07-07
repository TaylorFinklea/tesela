import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CHIP_REGISTRY,
  chipsFromDsl,
  dslFromChips,
  defaultInboxDsl,
} from "../../src/lib/ambients/inbox/chips.ts";
import { parseQuery } from "../../src/lib/query-language.ts";

test("chipsFromDsl — parses default Inbox DSL into chip state", () => {
  const dsl = "kind:block -has:status -is:heading -on:daily-page -on:system-pages";
  const state = chipsFromDsl(dsl);
  assert.equal(state.active.untriaged, true);
  assert.equal(state.active.notHeading, true);
  assert.equal(state.active.notDailyPage, true);
  assert.equal(state.active.notSystemPages, true);
  assert.equal(state.active.hasScheduled, false);
  assert.deepEqual(state.unknownClauses, []);
});

test("chipsFromDsl — kind:block is implicit, not surfaced as unknown", () => {
  const state = chipsFromDsl("kind:block");
  assert.deepEqual(state.unknownClauses, []);
});

test("chipsFromDsl — preserves unknown clauses verbatim", () => {
  const state = chipsFromDsl("kind:block -has:status text:urgent priority:>=3");
  assert.equal(state.active.untriaged, true);
  assert.deepEqual(state.unknownClauses.sort(), ["priority:>=3", "text:urgent"]);
});

test("chipsFromDsl — pulls tag-in: into activeTypes (the only OR primitive)", () => {
  const state = chipsFromDsl("kind:block tag-in:Task,Domain,Issue");
  assert.deepEqual(state.activeTypes, ["Task", "Domain", "Issue"]);
  // tag-in: shouldn't end up in unknownClauses now that it's claimed.
  assert.deepEqual(state.unknownClauses, []);
});

test("chipsFromDsl — pulls -page:/-block: exclusions into hiddenPages/hiddenBlocks", () => {
  const state = chipsFromDsl(
    "kind:block -has:status -page:python -page:javascript -block:foo:42",
  );
  assert.deepEqual(state.hiddenPages.sort(), ["javascript", "python"]);
  assert.deepEqual(state.hiddenBlocks, ["foo:42"]);
  assert.deepEqual(state.unknownClauses, []);
});

test("dslFromChips — composes tag-in: clause when activeTypes non-empty", () => {
  const state = chipsFromDsl(defaultInboxDsl());
  state.activeTypes = ["Task", "Domain"];
  const dsl = dslFromChips(state);
  assert.ok(dsl.includes("tag-in:Task,Domain"), `expected tag-in clause: ${dsl}`);
});

test("dslFromChips — emits -page:/-block: for each exclusion", () => {
  const state = chipsFromDsl(defaultInboxDsl());
  state.hiddenPages = ["python", "javascript"];
  state.hiddenBlocks = ["foo:42"];
  const dsl = dslFromChips(state);
  assert.ok(dsl.includes("-page:python"));
  assert.ok(dsl.includes("-page:javascript"));
  assert.ok(dsl.includes("-block:foo:42"));
});

test("dslFromChips — round-trips dynamic fields through tokenize", () => {
  const original = chipsFromDsl(defaultInboxDsl());
  original.activeTypes = ["Task", "Issue"];
  original.hiddenPages = ["python"];
  original.hiddenBlocks = ["bid:99"];
  const dsl = dslFromChips(original);
  const back = chipsFromDsl(dsl);
  assert.deepEqual(back.activeTypes, ["Task", "Issue"]);
  assert.deepEqual(back.hiddenPages, ["python"]);
  assert.deepEqual(back.hiddenBlocks, ["bid:99"]);
});

test("dslFromChips — round-trips default state through tokenize", () => {
  const dsl = defaultInboxDsl();
  const back = dslFromChips(chipsFromDsl(dsl));
  assert.equal(back, dsl);
});

test("dslFromChips — keeps preserved unknown clauses on the way out", () => {
  const original = "kind:block priority:>=3 text:urgent";
  const back = dslFromChips(chipsFromDsl(original));
  // tokens may reorder (chips first, unknowns after) but every clause survives
  const tokens = (s) => s.split(/\s+/).sort();
  assert.deepEqual(tokens(back), tokens(original));
});

test("dslFromChips — a legacy colon-DSL fragment round-trips to its canonical JQL form", () => {
  // Old-style colon fragments still parse (backcompat) and are recognized
  // as the SAME predicate as the migrated chip clause — chipsFromDsl claims
  // them, and dslFromChips rewrites them to the canonical JQL wording (not
  // preserved verbatim, unlike genuinely unknown clauses).
  const original = "kind:block -has:status priority:>=3";
  const state = chipsFromDsl(original);
  assert.equal(state.active.untriaged, true);
  assert.deepEqual(state.unknownClauses, ["priority:>=3"]);
  const back = dslFromChips(state);
  assert.ok(back.includes("status IS NULL"), `expected canonical form: ${back}`);
  assert.ok(!back.includes("-has:status"), `expected legacy form gone: ${back}`);
});

test("dslFromChips — toggling a chip off removes its clauses", () => {
  const state = chipsFromDsl(defaultInboxDsl());
  state.active.notHeading = false;
  const dsl = dslFromChips(state);
  assert.ok(!dsl.includes("is != heading"), `expected is != heading gone, got: ${dsl}`);
  assert.ok(dsl.includes("status IS NULL"), `untriaged should still be there: ${dsl}`);
});

test("dslFromChips — toggling a default-off chip on adds its clauses", () => {
  const state = chipsFromDsl(defaultInboxDsl());
  state.active.hasScheduled = true;
  const dsl = dslFromChips(state);
  assert.ok(dsl.includes("scheduled IS NOT NULL"), `expected scheduled IS NOT NULL present: ${dsl}`);
});

test("CHIP_REGISTRY — every id is unique and stable", () => {
  const ids = CHIP_REGISTRY.map((c) => c.id);
  const unique = new Set(ids);
  assert.equal(ids.length, unique.size);
});

// ── JQL migration equivalence (tesela-vp9.3, decision 5) ───────────────────
// Every CHIP_REGISTRY clause is now a JQL predicate string. This table pins
// each one against the LEGACY colon-DSL fragment it replaces — parsing both
// must produce the SAME predicate (after unwrapping the unary NOT the
// legacy negated forms produce into the single inverted-op atom the new
// JQL keyword sugar produces directly; mirrors the same NOT/cmp ⇄ Ne
// normalization `flattenToLegacyFilters` in query-language.ts already
// applies). This is an INDEPENDENT check (its own normalization, not
// view-dsl.ts's) so a bug in the shared canonicalization can't hide here.
const CHIP_JQL_MIGRATIONS = [
  { id: "untriaged", legacy: "-has:status" },
  { id: "notHeading", legacy: "-is:heading" },
  { id: "notDailyPage", legacy: "-on:daily-page" },
  { id: "notSystemPages", legacy: "-on:system-pages" },
  { id: "hasScheduled", legacy: "has:scheduled" },
  { id: "hasDeadline", legacy: "has:deadline" },
  { id: "untagged", legacy: "-has:tag" },
];

const INVERT_OP = {
  Eq: "Ne",
  Ne: "Eq",
  Gt: "Lte",
  Lt: "Gte",
  Gte: "Lt",
  Lte: "Gt",
  Like: "NotLike",
  NotLike: "Like",
};

/** Unwrap `NOT (cmp)` → the inverted-op atom directly, so a NOT-wrapped
 *  legacy negation and the new JQL keyword-sugar atom compare equal. */
function normalizeExpr(expr) {
  if (expr.op === "not" && expr.arg.op === "atom" && expr.arg.pred.kind === "cmp") {
    return { op: "atom", pred: { ...expr.arg.pred, op: INVERT_OP[expr.arg.pred.op] } };
  }
  return expr;
}

test("CHIP_REGISTRY — every migration table entry is covered exactly once", () => {
  assert.deepEqual(
    CHIP_JQL_MIGRATIONS.map((m) => m.id).sort(),
    CHIP_REGISTRY.map((c) => c.id).sort(),
  );
});

for (const { id, legacy } of CHIP_JQL_MIGRATIONS) {
  test(`CHIP_REGISTRY — ${id}'s JQL clause parses to the same predicate as legacy "${legacy}"`, () => {
    const chip = CHIP_REGISTRY.find((c) => c.id === id);
    assert.ok(chip, `chip ${id} exists in CHIP_REGISTRY`);
    assert.equal(chip.clauses.length, 1, `${id}: expected a single-predicate clause`);
    const legacyExpr = normalizeExpr(parseQuery(legacy).expr);
    const jqlExpr = normalizeExpr(parseQuery(chip.clauses[0]).expr);
    assert.deepEqual(jqlExpr, legacyExpr, `"${chip.clauses[0]}" !≡ legacy "${legacy}"`);
  });
}
