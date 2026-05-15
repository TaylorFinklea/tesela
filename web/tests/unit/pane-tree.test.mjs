// Prism v4 pane-tree binary-tree tests. Node 22+ strips TypeScript types
// natively. Run via `pnpm test:unit` from web/.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  STATE_VERSION,
  closePane,
  closeTab,
  deserialize,
  findLeafByPaneId,
  findParentOf,
  findTile,
  focusPane,
  focusedPane,
  focusedTab,
  hsplit,
  initialState,
  jumpToTile,
  leaves,
  makePane,
  moveFocus,
  moveTab,
  movePane,
  newTab,
  paneById,
  renameTab,
  serialize,
  setPaneWidget,
  setSplitSizes,
  stackAdd,
  stackClose,
  stackNext,
  swapKind,
  switchTabByIndex,
  vsplit,
} from "../../src/lib/stores/pane-tree.ts";

function fresh() {
  return initialState();
}

// ── factories ───────────────────────────────────────────────────────────────

test("initialState has one tab with a single editor leaf focused", () => {
  const s = fresh();
  assert.equal(s.version, STATE_VERSION);
  assert.equal(s.tabs.length, 1);
  const t = s.tabs[0];
  assert.equal(t.id, s.activeTabId);
  assert.equal(t.layout.kind, "leaf");
  assert.equal(t.layout.pane.kind, "editor");
  assert.equal(t.focus, t.layout.pane.id);
});

test("makePane stamps a fresh id per call", () => {
  const a = makePane("editor");
  const b = makePane("editor");
  assert.notEqual(a.id, b.id);
});

// ── vsplit / hsplit ─────────────────────────────────────────────────────────

test("vsplit wraps the root leaf in a vertical split, focuses the new leaf", () => {
  const s = fresh();
  const before = focusedTab(s).layout;
  const after = vsplit(s);
  const t = focusedTab(after);
  assert.equal(t.layout.kind, "split");
  assert.equal(t.layout.dir, "vertical");
  assert.equal(t.layout.children.length, 2);
  assert.equal(t.layout.children[0].kind, "leaf");
  assert.equal(t.layout.children[1].kind, "leaf");
  assert.equal(t.layout.children[0].pane.id, before.pane.id);
  assert.equal(t.focus, t.layout.children[1].pane.id);
});

test("vsplit accepts a kind override", () => {
  const after = vsplit(fresh(), "context");
  const p = focusedPane(after);
  assert.equal(p.kind, "context");
});

test("vsplit twice on the same axis flattens — no nested splits", () => {
  let s = fresh();
  s = vsplit(s);
  s = vsplit(s);
  const t = focusedTab(s);
  assert.equal(t.layout.kind, "split");
  assert.equal(t.layout.dir, "vertical");
  assert.equal(t.layout.children.length, 3);
  assert.ok(t.layout.children.every((c) => c.kind === "leaf"));
});

test("hsplit perpendicular to a vsplit wraps just the focused leaf", () => {
  let s = fresh();
  s = vsplit(s);
  s = hsplit(s);
  const t = focusedTab(s);
  assert.equal(t.layout.kind, "split");
  assert.equal(t.layout.dir, "vertical");
  assert.equal(t.layout.children.length, 2);
  assert.equal(t.layout.children[0].kind, "leaf");
  const right = t.layout.children[1];
  assert.equal(right.kind, "split");
  assert.equal(right.dir, "horizontal");
  assert.equal(right.children.length, 2);
});

test("sibling weights halve when vsplit adds a new pane", () => {
  let s = fresh();
  s = vsplit(s);
  const t = focusedTab(s);
  assert.deepEqual(t.layout.sizes, [0.5, 0.5]);
});

test("appending to a same-axis split halves the focused child", () => {
  let s = fresh();
  s = vsplit(s);
  s = vsplit(s);
  const t = focusedTab(s);
  assert.deepEqual(t.layout.sizes, [0.5, 0.25, 0.25]);
});

// ── closePane ───────────────────────────────────────────────────────────────

test("closePane on the only pane is a no-op", () => {
  const s = fresh();
  assert.equal(closePane(s), s);
});

test("closePane on a 2-leaf split collapses back to a single leaf", () => {
  let s = fresh();
  s = vsplit(s);
  s = closePane(s);
  assert.equal(focusedTab(s).layout.kind, "leaf");
});

test("closePane collapses chains of single-child splits", () => {
  let s = fresh();
  s = vsplit(s);
  s = hsplit(s);
  s = closePane(s);
  const t = focusedTab(s);
  assert.equal(t.layout.kind, "split");
  assert.equal(t.layout.dir, "vertical");
  assert.ok(t.layout.children.every((c) => c.kind === "leaf"));
});

// ── moveFocus ───────────────────────────────────────────────────────────────

test("moveFocus right walks to the sibling vsplit child", () => {
  let s = fresh();
  s = vsplit(s);
  const lId = s.tabs[0].layout.children[0].pane.id;
  s = focusPane(s, lId);
  s = moveFocus(s, "right");
  const t = focusedTab(s);
  assert.equal(t.focus, t.layout.children[1].pane.id);
});

test("moveFocus through a perpendicular split picks the edge leaf", () => {
  let s = fresh();
  s = vsplit(s);
  s = hsplit(s);
  const lId = s.tabs[0].layout.children[0].pane.id;
  s = focusPane(s, lId);
  s = moveFocus(s, "right");
  const t = focusedTab(s);
  const rightSubtree = t.layout.children[1];
  assert.equal(rightSubtree.kind, "split");
  assert.equal(t.focus, rightSubtree.children[0].pane.id);
});

test("moveFocus at the edge is a no-op", () => {
  const s = fresh();
  assert.equal(moveFocus(s, "left"), s);
});

// ── movePane ────────────────────────────────────────────────────────────────

test("movePane left on a vsplit'd pane moves it to the left edge", () => {
  let s = fresh();
  s = vsplit(s);
  s = movePane(s, "left");
  const t = focusedTab(s);
  assert.equal(t.layout.kind, "split");
  assert.equal(t.layout.dir, "vertical");
  assert.equal(t.layout.children.length, 2);
  assert.equal(t.focus, t.layout.children[0].pane.id);
});

test("movePane down wraps a vsplit in an outer hsplit", () => {
  let s = fresh();
  s = vsplit(s);
  s = movePane(s, "down");
  const t = focusedTab(s);
  assert.equal(t.layout.kind, "split");
  assert.equal(t.layout.dir, "horizontal");
  assert.equal(t.layout.children.length, 2);
});

// ── jumpToTile + stack ──────────────────────────────────────────────────────

test("jumpToTile on the focused empty editor seeds the tile", () => {
  let s = fresh();
  s = jumpToTile(s, "tile-a");
  const p = focusedPane(s);
  assert.deepEqual(p.tiles, ["tile-a"]);
  assert.equal(p.activeIdx, 0);
});

test("stackAdd appends and focuses the new index", () => {
  let s = fresh();
  s = stackAdd(s, "tile-a");
  s = stackAdd(s, "tile-b");
  const p = focusedPane(s);
  assert.deepEqual(p.tiles, ["tile-a", "tile-b"]);
  assert.equal(p.activeIdx, 1);
});

test("stackNext wraps in both directions", () => {
  let s = fresh();
  s = stackAdd(s, "a");
  s = stackAdd(s, "b");
  s = stackAdd(s, "c");
  s = stackNext(s, 1);
  assert.equal(focusedPane(s).activeIdx, 0);
  s = stackNext(s, -1);
  assert.equal(focusedPane(s).activeIdx, 2);
});

test("stackClose removes a tile and shifts activeIdx", () => {
  let s = fresh();
  s = stackAdd(s, "a");
  s = stackAdd(s, "b");
  s = stackAdd(s, "c");
  s = stackClose(s, 0);
  const p = focusedPane(s);
  assert.deepEqual(p.tiles, ["b", "c"]);
  assert.equal(p.activeIdx, 1);
});

// ── swapKind ────────────────────────────────────────────────────────────────

test("swapKind replaces the focused pane in place, preserves id", () => {
  let s = fresh();
  const id = focusedPane(s).id;
  s = swapKind(s, id, "widget");
  const p = focusedPane(s);
  assert.equal(p.id, id);
  assert.equal(p.kind, "widget");
  assert.equal(p.widget, "recent");
});

test("setPaneWidget points a widget pane at a new note", () => {
  let s = fresh();
  const id = focusedPane(s).id;
  s = swapKind(s, id, "widget");
  s = setPaneWidget(s, id, "tasks");
  assert.equal(focusedPane(s).widget, "tasks");
});

// ── setSplitSizes ───────────────────────────────────────────────────────────

test("setSplitSizes rewrites a split's weights when length matches", () => {
  let s = fresh();
  s = vsplit(s);
  const splitId = s.tabs[0].layout.id;
  s = setSplitSizes(s, splitId, [0.7, 0.3]);
  assert.deepEqual(focusedTab(s).layout.sizes, [0.7, 0.3]);
});

test("setSplitSizes is a no-op on length mismatch", () => {
  let s = fresh();
  s = vsplit(s);
  const splitId = s.tabs[0].layout.id;
  assert.equal(setSplitSizes(s, splitId, [0.4, 0.3, 0.3]), s);
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
  assert.equal(closeTab(s, s.activeTabId), s);
});

test("switchTabByIndex jumps to the requested 0-indexed tab", () => {
  let s = fresh();
  s = newTab(s, "second");
  s = newTab(s, "third");
  s = switchTabByIndex(s, 0);
  assert.equal(focusedTab(s).name, "untitled");
});

test("renameTab changes the name without reordering", () => {
  let s = fresh();
  s = renameTab(s, s.activeTabId, "renamed");
  assert.equal(focusedTab(s).name, "renamed");
});

test("moveTab repositions in the tabs array", () => {
  let s = fresh();
  s = newTab(s, "second");
  s = newTab(s, "third");
  s = moveTab(s, 0, 2);
  assert.deepEqual(s.tabs.map((t) => t.name), ["second", "third", "untitled"]);
});

// ── lookups ─────────────────────────────────────────────────────────────────

test("paneById finds a pane in a non-active tab", () => {
  let s = fresh();
  const firstPaneId = s.tabs[0].layout.pane.id;
  s = newTab(s, "second");
  const hit = paneById(s, firstPaneId);
  assert.ok(hit);
  assert.equal(hit.pane.id, firstPaneId);
  assert.equal(hit.tab.name, "untitled");
});

test("findTile locates an editor pane across tabs", () => {
  let s = fresh();
  s = jumpToTile(s, "alpha");
  const tab1Id = s.activeTabId;
  s = newTab(s, "second");
  s = jumpToTile(s, "beta");
  const hit = findTile(s, "alpha");
  assert.ok(hit);
  assert.equal(hit.tabId, tab1Id);
});

test("findParentOf returns undefined for the root leaf", () => {
  const s = fresh();
  const t = focusedTab(s);
  assert.equal(findParentOf(t.layout, t.focus), undefined);
});

test("findLeafByPaneId locates a leaf by pane id", () => {
  let s = fresh();
  s = vsplit(s);
  const t = focusedTab(s);
  const target = t.layout.children[0].pane.id;
  const leaf = findLeafByPaneId(t.layout, target);
  assert.ok(leaf);
  assert.equal(leaf.pane.id, target);
});

test("leaves iterates every leaf in pre-order", () => {
  let s = fresh();
  s = vsplit(s);
  s = hsplit(s);
  const t = focusedTab(s);
  const ids = [...leaves(t.layout)].map((l) => l.pane.id);
  assert.equal(ids.length, 3);
  assert.equal(new Set(ids).size, 3);
});

// ── serialization ───────────────────────────────────────────────────────────

test("serialize / deserialize round-trips the v2 tree", () => {
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
    deserialize(JSON.stringify({ version: 999, tabs: [{ id: "a", name: "x", layout: { kind: "leaf", pane: { id: "p", kind: "editor", tiles: [], activeIdx: 0 } }, focus: "p" }], activeTabId: "a" })),
    null,
  );
});

test("deserialize migrates v1 (matrix) state into the v2 tree", () => {
  const legacy = {
    version: 1,
    activeTabId: "t1",
    tabs: [
      {
        id: "t1",
        name: "legacy",
        layout: [
          [{ id: "p1", kind: "editor", tiles: ["alpha"], activeIdx: 0 }],
          [
            { id: "p2", kind: "editor", tiles: [], activeIdx: 0 },
            { id: "p3", kind: "context", tile: null },
          ],
        ],
        focus: [1, 1],
        rowSizes: [0.6, 0.4],
        colSizes: [[1], [0.5, 0.5]],
      },
    ],
  };
  const back = deserialize(JSON.stringify(legacy));
  assert.ok(back);
  assert.equal(back.version, STATE_VERSION);
  const t = back.tabs[0];
  assert.equal(t.layout.kind, "split");
  assert.equal(t.layout.dir, "horizontal");
  assert.deepEqual(t.layout.sizes, [0.6, 0.4]);
  assert.equal(t.layout.children[0].kind, "leaf");
  const bottom = t.layout.children[1];
  assert.equal(bottom.kind, "split");
  assert.equal(bottom.dir, "vertical");
  assert.deepEqual(bottom.sizes, [0.5, 0.5]);
  assert.equal(t.focus, "p3");
});
