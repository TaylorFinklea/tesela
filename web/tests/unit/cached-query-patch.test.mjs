import { test } from "node:test";
import assert from "node:assert/strict";
import {
  isQueryResult,
  patchCachedProperty,
} from "../../src/lib/cached-query-patch.ts";

// tesela-ya4.1 fix round — regression guard for the critical finding: with
// `queryKey` pointed at KanbanBoard's `kanban-source` cache, the cached data
// is the RAW `QueryResult` (`{ groups: [{ items }] }`) `api.executeQuery`
// returns, never a flat `ParsedBlock[]`. Before this fix,
// `property-update.ts` assumed every cache was the flat-array shape and
// threw `previousBlocks.map is not a function` for every kanban card move
// (drag-drop, `m` picker, `H`/`L`) — silently caught by the caller's
// try/catch, never reaching the real persistence call. These tests exercise
// `patchCachedProperty` directly against both real cache shapes.

function queryItem(block_id, properties) {
  return {
    block_id,
    page_id: "page-1",
    title: "Page",
    text: "some text",
    parent_breadcrumb: [],
    kind: "block",
    primary_tag: "Task",
    properties,
    page_note_type: null,
  };
}

function parsedBlock(id, properties) {
  return {
    id,
    bid: null,
    text: "some text",
    raw_text: "some text",
    tags: [],
    inline_tags: [],
    trailing_tags: [],
    inherited_tags: [],
    properties,
    indent_level: 0,
    note_id: "page-1",
    parent_note_type: null,
  };
}

test("isQueryResult — true for {groups}, false for a flat array", () => {
  assert.equal(isQueryResult({ groups: [] }), true);
  assert.equal(isQueryResult([]), false);
  assert.equal(isQueryResult([parsedBlock("a", {})]), false);
});

test("QueryResult shape — set patches only the matching item, across groups, without throwing", () => {
  const data = {
    groups: [
      { key: "todo", count: 1, items: [queryItem("blk-1", { status: "todo" })] },
      { key: "doing", count: 1, items: [queryItem("blk-2", { status: "doing" })] },
    ],
  };

  // Regression guard: this must not throw `previousBlocks.map is not a
  // function` — the exact TypeError the reviewer's repro produced.
  const patched = patchCachedProperty(data, "blk-2", "status", "done");

  assert.equal(isQueryResult(patched), true, "shape is preserved (still a QueryResult, never coerced to an array)");
  assert.equal(patched.groups[0].items[0].properties.status, "todo", "non-matching item untouched");
  assert.equal(patched.groups[1].items[0].properties.status, "done", "matching item's property set");
  // Group `key`/`count` metadata survives the patch (only `items` is remapped).
  assert.equal(patched.groups[1].key, "doing");
  assert.equal(patched.groups[1].count, 1);
});

test("QueryResult shape — clear (value=null) removes the property, case-insensitively", () => {
  const data = {
    groups: [{ key: "", count: 1, items: [queryItem("blk-1", { Status: "todo", other: "x" })] }],
  };

  const patched = patchCachedProperty(data, "blk-1", "status", null);

  assert.deepEqual(patched.groups[0].items[0].properties, { other: "x" });
});

test("QueryResult shape — no match leaves every item byte-for-byte unchanged (new objects, same values)", () => {
  const data = {
    groups: [{ key: "", count: 1, items: [queryItem("blk-1", { status: "todo" })] }],
  };

  const patched = patchCachedProperty(data, "blk-does-not-exist", "status", "done");

  assert.deepEqual(patched.groups[0].items[0].properties, { status: "todo" });
});

test("ParsedBlock[] shape — set/clear still works (TagTable's existing cache contract, unaffected)", () => {
  const data = [parsedBlock("blk-1", { status: "todo" }), parsedBlock("blk-2", { status: "doing" })];

  const afterSet = patchCachedProperty(data, "blk-1", "status", "done");
  assert.equal(isQueryResult(afterSet), false);
  assert.equal(afterSet[0].properties.status, "done");
  assert.equal(afterSet[1].properties.status, "doing", "non-matching block untouched");

  const afterClear = patchCachedProperty(afterSet, "blk-1", "status", null);
  assert.deepEqual(afterClear[0].properties, {});
});
