// Unit tests for QueryInput's completion-list assembly
// (web/src/lib/query-input/completion.ts) — tesela-vp9.2. Pure: given a
// CaretContext (tier + key) and a fake property/type source, returns the
// tier's full candidate list. Prefix filtering is deliberately NOT this
// module's job (AutocompleteMenu's own fuzzy `filter` prop does that).
import { test } from "node:test";
import assert from "node:assert/strict";

import {
  buildCompletions,
  META_KEYS,
  OPERATOR_ITEMS,
} from "../../src/lib/query-input/completion.ts";

const FAKE_SOURCES = {
  properties: [
    { name: "Status", value_type: "select", values: ["todo", "doing", "done"] },
    { name: "Priority", value_type: "multiselect", values: ["p1", "p2", "p3"] },
    { name: "Points", value_type: "number", values: null },
    { name: "Notes", value_type: "text", values: null },
  ],
  types: [{ name: "Task" }, { name: "Project" }, { name: "Person" }],
};

function ctx(tier, key = null, prefix = "") {
  return { tier, from: 0, to: prefix.length, prefix, key };
}

// ── key tier ─────────────────────────────────────────────────────────────

test("key tier: every registered property plus every meta key, no duplicates", () => {
  const items = buildCompletions(ctx("key"), FAKE_SOURCES);
  const ids = items.map((i) => i.id);
  const lowerIds = new Set(ids.map((s) => s.toLowerCase()));
  for (const p of FAKE_SOURCES.properties) assert.ok(ids.includes(p.name), p.name);
  // A meta key present ONLY as itself unless a same-named property (case-
  // insensitively) already occupies that slot — see the dedup test below.
  for (const k of META_KEYS) assert.ok(lowerIds.has(k), k);
  assert.equal(lowerIds.size, ids.length); // no case-insensitive duplicates
});

test("key tier: a property whose name collides with a meta key wins (property shown, no dup)", () => {
  const sources = {
    properties: [{ name: "status", value_type: "select", values: ["a"] }],
    types: [],
  };
  const items = buildCompletions(ctx("key"), sources);
  const statusItems = items.filter((i) => i.id.toLowerCase() === "status");
  assert.equal(statusItems.length, 1);
  assert.equal(statusItems[0].secondary, "select"); // the property's, not "meta"
});

test("key tier: property items carry their value_type as `secondary`", () => {
  const items = buildCompletions(ctx("key"), FAKE_SOURCES);
  const status = items.find((i) => i.id === "Status");
  assert.equal(status.secondary, "select");
});

// ── operator tier ───────────────────────────────────────────────────────

test("operator tier: the full fixed menu, independent of key/sources", () => {
  const items = buildCompletions(ctx("operator"), FAKE_SOURCES);
  assert.deepEqual(items.map((i) => i.label), [...OPERATOR_ITEMS]);
});

test("operator tier includes the legacy colon plus AND/OR/ORDER BY per spec decision 4", () => {
  const labels = buildCompletions(ctx("operator"), FAKE_SOURCES).map((i) => i.label);
  for (const expected of [":", "IN", "NOT IN", "LIKE", "BETWEEN", "AND", "OR", "ORDER BY", "ASC", "DESC"]) {
    assert.ok(labels.includes(expected), expected);
  }
});

// ── value tier ───────────────────────────────────────────────────────────

test("value tier: a select property offers its declared choices", () => {
  const items = buildCompletions(ctx("value", "status"), FAKE_SOURCES);
  assert.deepEqual(items.map((i) => i.label), ["todo", "doing", "done"]);
});

test("value tier: a multiselect ('multiselect', Rust spelling) property also offers choices", () => {
  const items = buildCompletions(ctx("value", "priority"), FAKE_SOURCES);
  assert.deepEqual(items.map((i) => i.label), ["p1", "p2", "p3"]);
});

test("value tier: a non-select property (number/text) offers nothing", () => {
  assert.deepEqual(buildCompletions(ctx("value", "points"), FAKE_SOURCES), []);
  assert.deepEqual(buildCompletions(ctx("value", "notes"), FAKE_SOURCES), []);
});

test("value tier: an unknown key offers nothing", () => {
  assert.deepEqual(buildCompletions(ctx("value", "nonexistent"), FAKE_SOURCES), []);
});

test("value tier: no key at all offers nothing", () => {
  assert.deepEqual(buildCompletions(ctx("value", null), FAKE_SOURCES), []);
});

test("value tier: 'type'/'kind' offer type names instead of property values", () => {
  const byType = buildCompletions(ctx("value", "type"), FAKE_SOURCES);
  const byKind = buildCompletions(ctx("value", "kind"), FAKE_SOURCES);
  assert.deepEqual(byType.map((i) => i.label), ["Task", "Project", "Person"]);
  assert.deepEqual(byKind.map((i) => i.label), ["Task", "Project", "Person"]);
});

test("value tier: key match is case-insensitive", () => {
  const items = buildCompletions(ctx("value", "STATUS"), FAKE_SOURCES);
  assert.deepEqual(items.map((i) => i.label), ["todo", "doing", "done"]);
});

// ── none tier ────────────────────────────────────────────────────────────

test("none tier: nothing", () => {
  assert.deepEqual(buildCompletions(ctx("none"), FAKE_SOURCES), []);
});

// ── style with the 'multi-select' (web) spelling too ────────────────────

test("value tier: web spelling 'multi-select' (hyphenated) also offers choices", () => {
  const sources = {
    properties: [{ name: "tags", value_type: "multi-select", values: ["a", "b"] }],
    types: [],
  };
  const items = buildCompletions(ctx("value", "tags"), sources);
  assert.deepEqual(items.map((i) => i.label), ["a", "b"]);
});
