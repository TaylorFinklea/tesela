// Prism v5 binary pane-tree algebra tests. Node 22+ strips TypeScript
// types natively. Run via `pnpm test:unit` from web/.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  closeFocused,
  defaultWorkspace,
  findLeaf,
  findParent,
  leaves,
  makeAmbientBuffer,
  makeDerivedBuffer,
  makePageBuffer,
  makeLeaf,
  makeTab,
  movePaneToEdge,
  nextFocusedLeaf,
  replaceLeafBuffer,
  setSplitRatio,
  splitFocused,
} from "../../src/lib/buffer/tree.ts";
import { asPageId } from "../../src/lib/buffer/types.ts";

const pageA = () => makePageBuffer(asPageId("a"));
const pageB = () => makePageBuffer(asPageId("b"));
const pageC = () => makePageBuffer(asPageId("c"));

function tabOfBuffer(buffer) {
  return makeTab("t", buffer);
}

// ── factories ──────────────────────────────────────────────────────────────

test("makeTab seeds a single-leaf layout with focus on the leaf", () => {
  const t = tabOfBuffer(pageA());
  assert.equal(t.layout.type, "leaf");
  assert.equal(t.lastFocusedLeafId, t.layout.id);
  assert.equal(t.lastFocusedPageId, asPageId("a"));
});

test("defaultWorkspace yields v3 envelope + one tab", () => {
  const ws = defaultWorkspace(pageA());
  assert.equal(ws._v, 3);
  assert.equal(ws.tabs.length, 1);
  assert.equal(ws.activeTabId, ws.tabs[0].id);
  assert.equal(ws.sidebar.activeSurface, "tree");
  assert.equal(ws.sidebar.collapsed, false);
});

// ── traversal ──────────────────────────────────────────────────────────────

test("leaves iterates left-then-right of a binary split", () => {
  const a = makeLeaf(pageA());
  const b = makeLeaf(pageB());
  const split = {
    type: "split",
    id: "s1",
    dir: "v",
    ratio: 0.5,
    children: [a, b],
  };
  const ids = Array.from(leaves(split)).map((l) => l.id);
  assert.deepEqual(ids, [a.id, b.id]);
});

test("findLeaf finds a deep leaf", () => {
  const t = tabOfBuffer(pageA());
  const { layout, newLeafId } = splitFocused(
    t.layout,
    t.lastFocusedLeafId,
    "v",
    pageB(),
  );
  const hit = findLeaf(layout, newLeafId);
  assert.ok(hit);
  assert.equal(hit.buffer.kind, "page");
});

test("findParent of root leaf returns undefined", () => {
  const t = tabOfBuffer(pageA());
  assert.equal(findParent(t.layout, t.lastFocusedLeafId), undefined);
});

// ── split ──────────────────────────────────────────────────────────────────

test("splitFocused vertical creates split with focused leaf first", () => {
  const t = tabOfBuffer(pageA());
  const { layout, newLeafId } = splitFocused(
    t.layout,
    t.lastFocusedLeafId,
    "v",
    pageB(),
  );
  assert.equal(layout.type, "split");
  assert.equal(layout.dir, "v");
  assert.equal(layout.children[0].type, "leaf");
  assert.equal(layout.children[0].id, t.lastFocusedLeafId);
  assert.equal(layout.children[1].id, newLeafId);
});

test("splitFocused on an unknown leaf is a no-op", () => {
  const t = tabOfBuffer(pageA());
  const { layout, newLeafId } = splitFocused(
    t.layout,
    "nope",
    "v",
    pageB(),
  );
  assert.equal(layout, t.layout);
  assert.equal(newLeafId, "nope");
});

test("nested splits grow depth — vsplit then hsplit creates two-level tree", () => {
  const t = tabOfBuffer(pageA());
  const r1 = splitFocused(t.layout, t.lastFocusedLeafId, "v", pageB());
  const r2 = splitFocused(r1.layout, r1.newLeafId, "h", pageC());
  // root is vertical, its right child is horizontal
  assert.equal(r2.layout.type, "split");
  assert.equal(r2.layout.dir, "v");
  assert.equal(r2.layout.children[1].type, "split");
  assert.equal(r2.layout.children[1].dir, "h");
});

// ── close ──────────────────────────────────────────────────────────────────

test("closeFocused on a vsplit promotes the surviving child", () => {
  const t = tabOfBuffer(pageA());
  const { layout, newLeafId } = splitFocused(
    t.layout,
    t.lastFocusedLeafId,
    "v",
    pageB(),
  );
  const r = closeFocused(layout, newLeafId);
  assert.ok(r);
  assert.equal(r.layout.type, "leaf");
  assert.equal(r.layout.id, t.lastFocusedLeafId);
  assert.equal(r.nextFocusId, t.lastFocusedLeafId);
});

test("closeFocused on a root leaf returns undefined", () => {
  const t = tabOfBuffer(pageA());
  assert.equal(closeFocused(t.layout, t.lastFocusedLeafId), undefined);
});

test("closeFocused preserves left sibling when right closes in nested tree", () => {
  // build vsplit(A, hsplit(B, C)); close C → vsplit(A, B)
  const t = tabOfBuffer(pageA());
  const r1 = splitFocused(t.layout, t.lastFocusedLeafId, "v", pageB());
  const r2 = splitFocused(r1.layout, r1.newLeafId, "h", pageC());
  const r3 = closeFocused(r2.layout, r2.newLeafId);
  assert.ok(r3);
  assert.equal(r3.layout.type, "split");
  assert.equal(r3.layout.dir, "v");
  assert.equal(r3.layout.children[0].type, "leaf");
  assert.equal(r3.layout.children[1].type, "leaf");
});

// ── move pane to edge ──────────────────────────────────────────────────────

test("movePaneToEdge wraps the survivor + focused into a new split at the edge", () => {
  const t = tabOfBuffer(pageA());
  const r1 = splitFocused(t.layout, t.lastFocusedLeafId, "v", pageB());
  // Move the new leaf to the bottom edge → horizontal split with B at the bottom
  const next = movePaneToEdge(r1.layout, r1.newLeafId, "down");
  assert.equal(next.type, "split");
  assert.equal(next.dir, "h");
  assert.equal(next.children[1].type, "leaf");
  assert.equal(next.children[1].id, r1.newLeafId);
});

test("movePaneToEdge on a single-leaf root is a no-op", () => {
  const t = tabOfBuffer(pageA());
  assert.equal(movePaneToEdge(t.layout, t.lastFocusedLeafId, "right"), t.layout);
});

// ── focus motion ───────────────────────────────────────────────────────────

test("nextFocusedLeaf finds horizontal neighbor right of vsplit-left", () => {
  const t = tabOfBuffer(pageA());
  const r = splitFocused(t.layout, t.lastFocusedLeafId, "v", pageB());
  const right = nextFocusedLeaf(r.layout, t.lastFocusedLeafId, "right");
  assert.equal(right, r.newLeafId);
});

test("nextFocusedLeaf returns undefined when no neighbor exists", () => {
  const t = tabOfBuffer(pageA());
  assert.equal(nextFocusedLeaf(t.layout, t.lastFocusedLeafId, "left"), undefined);
});

test("nextFocusedLeaf goes up the tree to find a matching-axis ancestor", () => {
  // vsplit(A, hsplit(B, C)). Starting on B, "right" must climb past the
  // horizontal split to the vertical root and land on... nothing right of
  // the right subtree.
  const t = tabOfBuffer(pageA());
  const r1 = splitFocused(t.layout, t.lastFocusedLeafId, "v", pageB());
  const r2 = splitFocused(r1.layout, r1.newLeafId, "h", pageC());
  // r1.newLeafId is B (the leaf split into the hsplit's child[0]).
  const right = nextFocusedLeaf(r2.layout, r1.newLeafId, "right");
  // No neighbor to the right of the right subtree.
  assert.equal(right, undefined);
});

// ── replace buffer ─────────────────────────────────────────────────────────

test("replaceLeafBuffer swaps the buffer without changing leaf id", () => {
  const t = tabOfBuffer(pageA());
  const next = replaceLeafBuffer(t.layout, t.lastFocusedLeafId, pageB());
  assert.equal(next.type, "leaf");
  assert.equal(next.id, t.lastFocusedLeafId);
  assert.equal(next.buffer.pageId, "b");
});

test("replaceLeafBuffer can change buffer kind", () => {
  const t = tabOfBuffer(pageA());
  const next = replaceLeafBuffer(
    t.layout,
    t.lastFocusedLeafId,
    makeAmbientBuffer("calendar"),
  );
  assert.equal(next.buffer.kind, "ambient");
});

// ── setSplitRatio ─────────────────────────────────────────────────────────

test("setSplitRatio adjusts the specified split's ratio", () => {
  const t = tabOfBuffer(pageA());
  const r = splitFocused(t.layout, t.lastFocusedLeafId, "v", pageB());
  const split = r.layout;
  const next = setSplitRatio(r.layout, split.id, 0.7);
  assert.equal(next.ratio, 0.7);
  // Children unchanged structurally
  assert.equal(next.children[0].id, split.children[0].id);
  assert.equal(next.children[1].id, split.children[1].id);
});

test("setSplitRatio on unknown split id is a no-op", () => {
  const t = tabOfBuffer(pageA());
  const r = splitFocused(t.layout, t.lastFocusedLeafId, "v", pageB());
  assert.equal(setSplitRatio(r.layout, "nope", 0.7), r.layout);
});

// ── structural sharing ────────────────────────────────────────────────────

test("setSplitRatio preserves sibling identity (structural sharing)", () => {
  const t = tabOfBuffer(pageA());
  const r1 = splitFocused(t.layout, t.lastFocusedLeafId, "v", pageB());
  const r2 = splitFocused(r1.layout, r1.newLeafId, "h", pageC());
  const splitId = r2.layout.id; // root split, vertical
  const next = setSplitRatio(r2.layout, splitId, 0.8);
  // Left child (leaf A) should be referentially the same object — its
  // subtree wasn't touched.
  assert.equal(next.children[0], r2.layout.children[0]);
});

// ── buffer factories ──────────────────────────────────────────────────────

test("makeAmbientBuffer / makeDerivedBuffer produce correctly-shaped Buffers", () => {
  const a = makeAmbientBuffer("calendar");
  assert.equal(a.kind, "ambient");
  assert.equal(a.ambientName, "calendar");
  const d = makeDerivedBuffer("backlinks-of-page", { mode: "follow" });
  assert.equal(d.kind, "derived");
  assert.equal(d.rendererName, "backlinks-of-page");
  assert.equal(d.binding.mode, "follow");
});
