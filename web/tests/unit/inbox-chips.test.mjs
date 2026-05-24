import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CHIP_REGISTRY,
  chipsFromDsl,
  dslFromChips,
  defaultInboxDsl,
} from "../../src/lib/ambients/inbox/chips.ts";

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

test("dslFromChips — round-trips default state through tokenize", () => {
  const dsl = defaultInboxDsl();
  const back = dslFromChips(chipsFromDsl(dsl));
  assert.equal(back, dsl);
});

test("dslFromChips — keeps preserved unknown clauses on the way out", () => {
  const original = "kind:block -has:status priority:>=3";
  const back = dslFromChips(chipsFromDsl(original));
  // tokens may reorder (chips first, unknowns after) but every clause survives
  const tokens = (s) => s.split(/\s+/).sort();
  assert.deepEqual(tokens(back), tokens(original));
});

test("dslFromChips — toggling a chip off removes its clauses", () => {
  const state = chipsFromDsl(defaultInboxDsl());
  state.active.notHeading = false;
  const dsl = dslFromChips(state);
  assert.ok(!dsl.includes("-is:heading"), `expected -is:heading gone, got: ${dsl}`);
  assert.ok(dsl.includes("-has:status"), `untriaged should still be there: ${dsl}`);
});

test("dslFromChips — toggling a default-off chip on adds its clauses", () => {
  const state = chipsFromDsl(defaultInboxDsl());
  state.active.hasScheduled = true;
  const dsl = dslFromChips(state);
  assert.ok(dsl.includes("has:scheduled"), `expected has:scheduled present: ${dsl}`);
});

test("CHIP_REGISTRY — every id is unique and stable", () => {
  const ids = CHIP_REGISTRY.map((c) => c.id);
  const unique = new Set(ids);
  assert.equal(ids.length, unique.size);
});
