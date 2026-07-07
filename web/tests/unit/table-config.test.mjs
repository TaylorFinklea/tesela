import { test } from "node:test";
import assert from "node:assert/strict";
import {
  EMPTY_TABLE_CONFIG,
  applyTableConfig,
  toggleColumnHidden,
  moveColumnInConfig,
  toggleSortInConfig,
} from "../../src/lib/table/table-config.ts";

// tesela-ya4.4 — table column display config (hide/reorder/sort
// persistence, spec gap G5). Pure projection/mutation logic, extracted so
// it's unit-testable without mounting QueryTable.svelte.

const STATUS = { name: "status", value_type: "select", values: ["todo", "doing", "done"] };
const PRIORITY = { name: "priority", value_type: "select", values: ["low", "high"] };
const DEADLINE = { name: "deadline", value_type: "date", values: null };
const NOTES = { name: "notes", value_type: "text", values: null };
const COLUMNS = [STATUS, PRIORITY, DEADLINE, NOTES];

test("applyTableConfig: EMPTY_TABLE_CONFIG is a no-op projection", () => {
  const result = applyTableConfig(COLUMNS, EMPTY_TABLE_CONFIG);
  assert.deepEqual(result, COLUMNS);
});

test("applyTableConfig: hidden columns are dropped, order preserved", () => {
  const result = applyTableConfig(COLUMNS, { ...EMPTY_TABLE_CONFIG, hidden: ["priority"] });
  assert.deepEqual(result, [STATUS, DEADLINE, NOTES]);
});

test("applyTableConfig: explicit order override wins, un-mentioned columns append after", () => {
  const result = applyTableConfig(COLUMNS, {
    ...EMPTY_TABLE_CONFIG,
    order: ["notes", "status"],
  });
  assert.deepEqual(result, [NOTES, STATUS, PRIORITY, DEADLINE], "priority/deadline weren't in `order` — they append in their originally-resolved order");
});

test("applyTableConfig: order + hidden compose (hidden wins even if named in order)", () => {
  const result = applyTableConfig(COLUMNS, {
    hidden: ["priority"],
    order: ["priority", "notes", "status"],
    sort_by: null,
    sort_dir: null,
  });
  assert.deepEqual(result, [NOTES, STATUS, DEADLINE], "a hidden column named in `order` still doesn't render");
});

test("applyTableConfig: an order entry naming an unknown/absent column is silently skipped", () => {
  const result = applyTableConfig([STATUS, PRIORITY], {
    ...EMPTY_TABLE_CONFIG,
    order: ["ghost", "priority"],
  });
  assert.deepEqual(result, [PRIORITY, STATUS]);
});

test("toggleColumnHidden: hides an unhidden column", () => {
  const next = toggleColumnHidden(EMPTY_TABLE_CONFIG, "status");
  assert.deepEqual(next.hidden, ["status"]);
  assert.notEqual(next, EMPTY_TABLE_CONFIG, "returns a new object");
});

test("toggleColumnHidden: unhides an already-hidden column", () => {
  const cfg = { ...EMPTY_TABLE_CONFIG, hidden: ["status", "priority"] };
  const next = toggleColumnHidden(cfg, "status");
  assert.deepEqual(next.hidden, ["priority"]);
});

test("moveColumnInConfig: moves a column one slot left", () => {
  const next = moveColumnInConfig(["status", "priority", "deadline"], "priority", "left");
  assert.deepEqual(next, ["priority", "status", "deadline"]);
});

test("moveColumnInConfig: moves a column one slot right", () => {
  const next = moveColumnInConfig(["status", "priority", "deadline"], "priority", "right");
  assert.deepEqual(next, ["status", "deadline", "priority"]);
});

test("moveColumnInConfig: no-op at the left boundary", () => {
  const names = ["status", "priority", "deadline"];
  const next = moveColumnInConfig(names, "status", "left");
  assert.deepEqual(next, names);
});

test("moveColumnInConfig: no-op at the right boundary", () => {
  const names = ["status", "priority", "deadline"];
  const next = moveColumnInConfig(names, "deadline", "right");
  assert.deepEqual(next, names);
});

test("moveColumnInConfig: no-op when the column isn't present", () => {
  const names = ["status", "priority"];
  const next = moveColumnInConfig(names, "ghost", "left");
  assert.deepEqual(next, names);
});

test("moveColumnInConfig round-trips through applyTableConfig: moving priority left actually reorders the rendered columns", () => {
  const visible = applyTableConfig(COLUMNS, EMPTY_TABLE_CONFIG).map((c) => c.name);
  const nextOrder = moveColumnInConfig(visible, "priority", "left");
  const result = applyTableConfig(COLUMNS, { ...EMPTY_TABLE_CONFIG, order: nextOrder });
  assert.deepEqual(result.map((c) => c.name), ["priority", "status", "deadline", "notes"]);
});

test("toggleSortInConfig: sorting a new column defaults to ascending", () => {
  const next = toggleSortInConfig(EMPTY_TABLE_CONFIG, "status");
  assert.equal(next.sort_by, "status");
  assert.equal(next.sort_dir, "asc");
});

test("toggleSortInConfig: re-toggling the same column flips direction", () => {
  const asc = toggleSortInConfig(EMPTY_TABLE_CONFIG, "status");
  const desc = toggleSortInConfig(asc, "status");
  assert.equal(desc.sort_by, "status");
  assert.equal(desc.sort_dir, "desc");
  const backToAsc = toggleSortInConfig(desc, "status");
  assert.equal(backToAsc.sort_dir, "asc");
});

test("toggleSortInConfig: switching to a different column resets to ascending", () => {
  const desc = { ...EMPTY_TABLE_CONFIG, sort_by: "status", sort_dir: "desc" };
  const next = toggleSortInConfig(desc, "priority");
  assert.equal(next.sort_by, "priority");
  assert.equal(next.sort_dir, "asc");
});

test("serialization default-compat: a stored JSON blob missing newer fields still parses to sane values", () => {
  // Simulates an OLDER persisted config predating a hypothetical future
  // field — JSON.parse never fails on a missing key, so the config just
  // carries `undefined` for it. Spreading over EMPTY_TABLE_CONFIG is the
  // documented recovery shape callers (tag-view-prefs' getTableConfig) use.
  const legacyJson = JSON.stringify({ hidden: ["notes"], order: [], sort_by: null, sort_dir: null });
  const parsed = JSON.parse(legacyJson);
  const result = applyTableConfig(COLUMNS, parsed);
  assert.deepEqual(result, [STATUS, PRIORITY, DEADLINE]);
});

test("serialization default-compat: an empty object round-trips as an all-default config (server's serde default mirrors this)", () => {
  const parsed = JSON.parse("{}");
  const merged = { ...EMPTY_TABLE_CONFIG, ...parsed };
  assert.deepEqual(applyTableConfig(COLUMNS, merged), COLUMNS);
});
