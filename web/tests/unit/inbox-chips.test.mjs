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
