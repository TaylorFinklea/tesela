import { test } from "node:test";
import assert from "node:assert/strict";
import { resolveTableColumns } from "../../src/lib/table/table-columns.ts";

// tesela-ya4.3 — column resolution mirrors kanban's group-by candidate
// order (decision 3c parity): a tag-scoped table uses the type's own
// declared property order; a non-tag-scoped query falls back to global
// properties actually present on the returned blocks.

const STATUS = { name: "status", value_type: "select", values: ["todo", "doing", "done"] };
const PRIORITY = { name: "priority", value_type: "select", values: ["low", "high"] };
const DEADLINE = { name: "deadline", value_type: "date", values: null };
const NOTES = { name: "notes", value_type: "text", values: null };

test("tag-scoped: returns the type's own declared property order verbatim", () => {
  const result = resolveTableColumns({
    tagName: "task",
    typeProperties: [STATUS, DEADLINE, PRIORITY],
    globalProperties: [PRIORITY, STATUS, DEADLINE, NOTES],
    presentKeys: new Set(["status", "priority"]),
  });
  assert.deepEqual(result, [STATUS, DEADLINE, PRIORITY], "order + membership come from typeProperties, not presentKeys");
});

test("tag-scoped: ignores presentKeys entirely — the type's declared list wins even for absent data", () => {
  const result = resolveTableColumns({
    tagName: "task",
    typeProperties: [STATUS],
    globalProperties: [STATUS, PRIORITY],
    presentKeys: new Set(), // no block actually carries `status` yet
  });
  assert.deepEqual(result, [STATUS]);
});

test("non-tag-scoped: filters global properties down to those present on ≥1 returned block", () => {
  const result = resolveTableColumns({
    tagName: null,
    typeProperties: [],
    globalProperties: [PRIORITY, STATUS, DEADLINE, NOTES],
    presentKeys: new Set(["status", "notes"]),
  });
  assert.deepEqual(result, [STATUS, NOTES], "preserves globalProperties order, drops absent columns");
});

test("non-tag-scoped: presentKeys lookup is case-insensitive against the lowercased property name", () => {
  const result = resolveTableColumns({
    tagName: null,
    typeProperties: [],
    globalProperties: [STATUS],
    presentKeys: new Set(["status"]), // block property keys are lowercased before insertion
  });
  assert.deepEqual(result, [STATUS]);
});

test("non-tag-scoped: no present properties yields an empty column list (not an error)", () => {
  const result = resolveTableColumns({
    tagName: null,
    typeProperties: [],
    globalProperties: [PRIORITY, STATUS],
    presentKeys: new Set(),
  });
  assert.deepEqual(result, []);
});
