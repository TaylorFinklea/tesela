import { test } from "node:test";
import assert from "node:assert/strict";
import { resolveKanbanGroupBy, isSelectWithChoices } from "../../src/lib/kanban-group-by.ts";

// tesela-ya4.1 — the group-by resolution order IS the acceptance contract
// (spec decision 3): (a) explicit display_group_by on the active saved
// view > (b) per-surface localStorage pref > (c) first select property
// with ≥1 choice > (d) honest empty state (never a silent list fallback).

const STATUS = { name: "status", value_type: "select", values: ["todo", "doing", "done"] };
const PRIORITY = { name: "priority", value_type: "select", values: ["low", "high"] };
const TEXT_PROP = { name: "notes", value_type: "text", values: null };
const EMPTY_SELECT = { name: "empty-select", value_type: "select", values: [] };

const registry = new Map([
  [STATUS.name, STATUS],
  [PRIORITY.name, PRIORITY],
  [TEXT_PROP.name, TEXT_PROP],
  [EMPTY_SELECT.name, EMPTY_SELECT],
]);
const resolveDef = (name) => {
  const def = registry.get(name);
  return def && isSelectWithChoices(def) ? def : undefined;
};

test("isSelectWithChoices — true only for select-type with ≥1 declared value", () => {
  assert.equal(isSelectWithChoices(STATUS), true);
  assert.equal(isSelectWithChoices(TEXT_PROP), false, "text-type is never groupable");
  assert.equal(isSelectWithChoices(EMPTY_SELECT), false, "select with zero choices has no columns to build");
  assert.equal(isSelectWithChoices({ name: "x", value_type: "select", values: null }), false);
});

test("(a) explicit display_group_by wins over stored pref and candidate order", () => {
  const result = resolveKanbanGroupBy({
    displayGroupBy: "priority",
    storedPref: "status",
    candidates: [STATUS, PRIORITY],
    resolveDef,
  });
  assert.equal(result, "priority");
});

test("(a) explicit display_group_by is honored even outside the candidate list", () => {
  // A saved view can pin a group-by that isn't among this board's (c)
  // candidates (e.g. a non-tag-scoped board whose candidates are derived
  // from data currently present) — decisions 3a/3b outrank "does the data
  // have it".
  const result = resolveKanbanGroupBy({
    displayGroupBy: "priority",
    storedPref: null,
    candidates: [STATUS],
    resolveDef,
  });
  assert.equal(result, "priority");
});

test("(a) an invalid/unresolvable display_group_by falls through to (b)", () => {
  const result = resolveKanbanGroupBy({
    displayGroupBy: "notes", // text-type — not select-with-choices
    storedPref: "status",
    candidates: [STATUS, PRIORITY],
    resolveDef,
  });
  assert.equal(result, "status");
});

test("(b) stored pref wins over (c) candidate order when (a) is absent", () => {
  const result = resolveKanbanGroupBy({
    displayGroupBy: null,
    storedPref: "priority",
    candidates: [STATUS, PRIORITY],
    resolveDef,
  });
  assert.equal(result, "priority");
});

test("(b) an invalid/stale stored pref falls through to (c)", () => {
  const result = resolveKanbanGroupBy({
    displayGroupBy: null,
    storedPref: "deleted-property",
    candidates: [STATUS, PRIORITY],
    resolveDef,
  });
  assert.equal(result, "status");
});

test("(c) first candidate wins when (a) and (b) are both absent", () => {
  const result = resolveKanbanGroupBy({
    displayGroupBy: null,
    storedPref: null,
    candidates: [PRIORITY, STATUS],
    resolveDef,
  });
  assert.equal(result, "priority");
});

test("(d) honest empty state — resolves to \"\" when nothing groupable is found", () => {
  const result = resolveKanbanGroupBy({
    displayGroupBy: null,
    storedPref: null,
    candidates: [],
    resolveDef,
  });
  assert.equal(result, "", "must be the empty string, never a truthy sentinel that could pass as a real property");
});

test("(d) empty state is reached even with a non-null but invalid (a) and (b)", () => {
  const result = resolveKanbanGroupBy({
    displayGroupBy: "notes",
    storedPref: "deleted-property",
    candidates: [],
    resolveDef,
  });
  assert.equal(result, "");
});
