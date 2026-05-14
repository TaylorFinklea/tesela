// Phase 0 — Prism v4 pane-tree mutation tests. Plain node:test runner;
// Node 22+ strips TypeScript types natively so importing `.ts` works.
// Run via `node --test web/tests/unit/pane-tree.test.mjs` from the
// repo root, or `pnpm test:unit` from web/.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  STATE_VERSION,
  closePane,
  closeTab,
  deserialize,
  findTile,
  focusPane,
  focusedPane,
  focusedTab,
  hsplit,
  initialState,
  jumpToTile,
  makePane,
  moveFocus,
  moveTab,
  newTab,
  paneById,
  renameTab,
  serialize,
  setPaneWidget,
  stackAdd,
  stackClose,
  stackNext,
  swapKind,
  switchTabByIndex,
  vsplit,
} from "../../src/lib/stores/pane-tree.ts";

// ── helpers ─────────────────────────────────────────────────────────────────

function fresh() {
  return initialState();
}

// ── factories ───────────────────────────────────────────────────────────────

test("initialState has one tab with one editor pane focused at 0,0", () => {
  const s = fresh();
  assert.equal(s.version, STATE_VERSION);
  assert.equal(s.tabs.length, 1);
  const t = s.tabs[0];
  assert.equal(t.id, s.activeTabId);
  assert.deepEqual(t.focus, [0, 0]);
  assert.equal(t.layout.length, 1);
  assert.equal(t.layout[0].length, 1);
  assert.equal(t.layout[0][0].kind, "editor");
});

test("makePane stamps a fresh id per call", () => {
  const a = makePane("editor");
  const b = makePane("editor");
  assert.notEqual(a.id, b.id);
});

// ── focus ───────────────────────────────────────────────────────────────────

test("focusPane clamps out-of-range coordinates", () => {
  const s = fresh();
  const r = focusPane(s, 99, 99);
  assert.deepEqual(focusedTab(r).focus, [0, 0]);
});

test("moveFocus is a no-op at grid edges", () => {
  const s = fresh();
  const ref = focusedTab(s).focus;
  const r = moveFocus(s, -1, -1);
  // Same reference because state didn't change.
  assert.equal(r, s);
  // Sanity: focus still [0,0].
  assert.deepEqual(focusedTab(r).focus, ref);
});

// ── vsplit / hsplit ─────────────────────────────────────────────────────────

test("vsplit inserts a new editor pane to the right and shifts focus", () => {
  const s = fresh();
  const after = vsplit(s);
  const t = focusedTab(after);
  assert.deepEqual({ rows: t.layout.length, cols: [t.layout[0].length] }, { rows: 1, cols: [2] });
  assert.deepEqual(t.focus, [0, 1]);
  assert.equal(t.layout[0][1].kind, "editor");
  assert.notEqual(t.layout[0][0].id, t.layout[0][1].id);
});

test("vsplit accepts a kind override", () => {
  const s = fresh();
  const after = vsplit(s, "context");
  const newPane = focusedPane(after);
  assert.equal(newPane.kind, "context");
  // Original editor pane untouched.
  assert.equal(focusedTab(after).layout[0][0].kind, "editor");
});

test("hsplit adds a new row below and focuses it", () => {
  const s = fresh();
  const after = hsplit(s);
  const t = focusedTab(after);
  assert.equal(t.layout.length, 2);
  assert.deepEqual(t.focus, [1, 0]);
});

test("vsplit + hsplit produces a 2D grid", () => {
  let s = fresh();
  s = vsplit(s);        // 1×2, focus [0,1]
  s = hsplit(s);        // 2 rows; new row has 1 pane; focus [1,0]
  s = vsplit(s);        // bottom row gets another pane; focus [1,1]
  const t = focusedTab(s);
  assert.equal(t.layout.length, 2);
  assert.equal(t.layout[0].length, 2);
  assert.equal(t.layout[1].length, 2);
  assert.deepEqual(t.focus, [1, 1]);
});

// ── closePane ───────────────────────────────────────────────────────────────

test("closePane on the only pane is a no-op", () => {
  const s = fresh();
  const after = closePane(s);
  assert.equal(after, s);
});

test("closePane removes the focused pane and clamps focus into the new grid", () => {
  let s = fresh();
  s = vsplit(s); // 1×2, focus [0,1]
  const after = closePane(s);
  const t = focusedTab(after);
  assert.equal(t.layout[0].length, 1);
  assert.deepEqual(t.focus, [0, 0]);
});

test("closePane collapses an empty row when it removes the last pane in that row", () => {
  let s = fresh();
  s = hsplit(s);                       // 2 rows, each with 1 pane, focus [1,0]
  const before = focusedTab(s);
  assert.equal(before.layout.length, 2);
  s = closePane(s);                    // close the only pane in row 1
  const t = focusedTab(s);
  assert.equal(t.layout.length, 1);
  assert.deepEqual(t.focus, [0, 0]);
});

// ── jumpToTile + stack ──────────────────────────────────────────────────────

test("jumpToTile on empty editor pane seeds the tile", () => {
  let s = fresh();
  s = jumpToTile(s, "tile-a");
  const p = focusedPane(s);
  assert.deepEqual(p.tiles, ["tile-a"]);
  assert.equal(p.activeIdx, 0);
});

test("jumpToTile replaces the active tile, keeping the rest of the stack", () => {
  let s = fresh();
  s = stackAdd(s, "tile-a");
  s = stackAdd(s, "tile-b");
  s = stackAdd(s, "tile-c"); // active = c, stack = [a, b, c]
  s = stackNext(s, -1);      // active = b
  s = jumpToTile(s, "tile-x");
  const p = focusedPane(s);
  assert.deepEqual(p.tiles, ["tile-a", "tile-x", "tile-c"]);
  assert.equal(p.activeIdx, 1);
});

test("jumpToTile on non-editor pane converts the pane to editor", () => {
  let s = fresh();
  s = vsplit(s, "context"); // focus is on the new context pane
  const beforeId = focusedPane(s).id;
  s = jumpToTile(s, "tile-z");
  const after = focusedPane(s);
  assert.equal(after.kind, "editor");
  // Preserves the pane id.
  assert.equal(after.id, beforeId);
  assert.deepEqual(after.tiles, ["tile-z"]);
});

test("stackAdd appends and focuses the new index", () => {
  let s = fresh();
  s = stackAdd(s, "tile-a");
  s = stackAdd(s, "tile-b");
  const p = focusedPane(s);
  assert.deepEqual(p.tiles, ["tile-a", "tile-b"]);
  assert.equal(p.activeIdx, 1);
});

test("stackAdd is idempotent — re-adding focuses the existing slot", () => {
  let s = fresh();
  s = stackAdd(s, "tile-a");
  s = stackAdd(s, "tile-b");
  s = stackAdd(s, "tile-c");
  s = stackAdd(s, "tile-a");
  const p = focusedPane(s);
  assert.deepEqual(p.tiles, ["tile-a", "tile-b", "tile-c"]);
  assert.equal(p.activeIdx, 0);
});

test("stackNext wraps around in both directions", () => {
  let s = fresh();
  s = stackAdd(s, "a");
  s = stackAdd(s, "b");
  s = stackAdd(s, "c"); // active = 2
  s = stackNext(s, 1);  // wrap to 0
  assert.equal(focusedPane(s).activeIdx, 0);
  s = stackNext(s, -1); // wrap to 2
  assert.equal(focusedPane(s).activeIdx, 2);
});

test("stackClose removes the requested tile and shifts activeIdx left if needed", () => {
  let s = fresh();
  s = stackAdd(s, "a");
  s = stackAdd(s, "b");
  s = stackAdd(s, "c"); // active = 2
  s = stackClose(s, 0); // remove 'a'; active was 2, now 1
  const p = focusedPane(s);
  assert.deepEqual(p.tiles, ["b", "c"]);
  assert.equal(p.activeIdx, 1);
});

// ── swapKind ────────────────────────────────────────────────────────────────

test("swapKind preserves the pane id while replacing fields", () => {
  let s = fresh();
  const id = focusedPane(s).id;
  s = swapKind(s, id, "widget");
  const p = focusedPane(s);
  assert.equal(p.id, id);
  assert.equal(p.kind, "widget");
  assert.equal(p.widget, "recent");
});

test("swapKind to the same kind is a no-op (preserves reference)", () => {
  const s = fresh();
  const id = focusedPane(s).id;
  const after = swapKind(s, id, "editor");
  assert.equal(after, s);
});

test("setPaneWidget points a widget pane at a new Query note", () => {
  let s = fresh();
  const id = focusedPane(s).id;
  s = swapKind(s, id, "widget");
  s = setPaneWidget(s, id, "tasks");
  assert.equal(focusedPane(s).widget, "tasks");
});

test("setPaneWidget is a no-op on a non-widget pane", () => {
  const s = fresh();
  const id = focusedPane(s).id; // editor pane
  const after = setPaneWidget(s, id, "tasks");
  assert.equal(after, s);
});

test("setPaneWidget is a no-op when the widget id is unchanged", () => {
  let s = fresh();
  const id = focusedPane(s).id;
  s = swapKind(s, id, "widget"); // widget = "recent"
  const after = setPaneWidget(s, id, "recent");
  assert.equal(after, s);
});

// ── tabs ────────────────────────────────────────────────────────────────────

test("newTab appends and switches", () => {
  let s = fresh();
  const firstId = s.activeTabId;
  s = newTab(s, "second");
  assert.equal(s.tabs.length, 2);
  assert.notEqual(s.activeTabId, firstId);
  assert.equal(focusedTab(s).name, "second");
});

test("closeTab refuses to remove the only tab", () => {
  const s = fresh();
  const after = closeTab(s, s.activeTabId);
  assert.equal(after, s);
});

test("closeTab switches active to first remaining when active tab is closed", () => {
  let s = fresh();
  s = newTab(s, "second"); // active = second
  const firstId = s.tabs[0].id;
  s = closeTab(s, s.activeTabId);
  assert.equal(s.tabs.length, 1);
  assert.equal(s.activeTabId, firstId);
});

test("switchTabByIndex jumps to the requested 0-indexed tab", () => {
  let s = fresh();
  s = newTab(s, "second");
  s = newTab(s, "third"); // active = third (index 2)
  s = switchTabByIndex(s, 0);
  assert.equal(focusedTab(s).name, "untitled");
});

test("switchTabByIndex out-of-range is a no-op", () => {
  const s = fresh();
  const after = switchTabByIndex(s, 99);
  assert.equal(after, s);
});

test("renameTab changes the name without reordering", () => {
  let s = fresh();
  s = renameTab(s, s.activeTabId, "renamed");
  assert.equal(focusedTab(s).name, "renamed");
});

test("moveTab repositions in the tabs array", () => {
  let s = fresh();
  s = newTab(s, "second");
  s = newTab(s, "third"); // [untitled, second, third]
  s = moveTab(s, 0, 2);
  assert.deepEqual(
    s.tabs.map((t) => t.name),
    ["second", "third", "untitled"],
  );
});

// ── lookups ─────────────────────────────────────────────────────────────────

test("paneById finds a pane in a non-active tab", () => {
  let s = fresh();
  const firstPaneId = s.tabs[0].layout[0][0].id;
  s = newTab(s, "second"); // active is now second tab
  const hit = paneById(s, firstPaneId);
  assert.ok(hit);
  assert.equal(hit.pane.id, firstPaneId);
  assert.equal(hit.tab.name, "untitled");
});

test("findTile locates an editor pane holding a tile, across tabs", () => {
  let s = fresh();
  s = jumpToTile(s, "alpha"); // tab 1, pane [0,0] now holds "alpha"
  const tab1Id = s.activeTabId;
  s = newTab(s, "second");
  s = jumpToTile(s, "beta"); // tab 2 holds "beta"
  // From tab 2, find a tile that lives in tab 1.
  const hit = findTile(s, "alpha");
  assert.ok(hit);
  assert.equal(hit.tabId, tab1Id);
  assert.deepEqual([hit.row, hit.col], [0, 0]);
});

test("findTile returns undefined when no pane holds the tile", () => {
  let s = fresh();
  s = jumpToTile(s, "alpha");
  assert.equal(findTile(s, "nonexistent"), undefined);
});

// ── serialization ───────────────────────────────────────────────────────────

test("serialize / deserialize round-trips the full state", () => {
  let s = fresh();
  s = vsplit(s);
  s = hsplit(s);
  s = stackAdd(s, "tile-a");
  s = stackAdd(s, "tile-b");
  s = newTab(s, "second");
  const raw = serialize(s);
  const back = deserialize(raw);
  assert.deepEqual(back, s);
});

test("deserialize rejects null / non-JSON / wrong version", () => {
  assert.equal(deserialize(null), null);
  assert.equal(deserialize(""), null);
  assert.equal(deserialize("not-json"), null);
  assert.equal(
    deserialize(JSON.stringify({ version: 999, tabs: [], activeTabId: "x" })),
    null,
  );
  assert.equal(
    deserialize(JSON.stringify({ version: STATE_VERSION, tabs: [], activeTabId: "x" })),
    null,
    "empty tabs array rejected",
  );
  assert.equal(
    deserialize(JSON.stringify({ version: STATE_VERSION, tabs: [{ id: "a" }], activeTabId: "missing" })),
    null,
    "activeTabId pointing nowhere rejected",
  );
});
