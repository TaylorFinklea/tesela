// Prism v5 v4→v5 migration tests. Idempotent + golden-file coverage of the
// 5-kind → 3-kind buffer mapping.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { migrate } from "../../src/lib/buffer/migration.ts";

function v4State({
  tabs,
  activeTabId = tabs[0]?.id ?? "tab-1",
  version = 2,
} = {}) {
  return { version, tabs, activeTabId };
}

function v4LeafEditor(tiles = ["page-x"], activeIdx = 0, id = "p1") {
  return { kind: "leaf", pane: { id, kind: "editor", tiles, activeIdx } };
}
function v4LeafWidget(widget, id = "p1") {
  return { kind: "leaf", pane: { id, kind: "widget", widget } };
}
function v4LeafContext(tile = null, id = "p1") {
  return { kind: "leaf", pane: { id, kind: "context", tile } };
}
function v4LeafGraph(id = "p1") {
  return { kind: "leaf", pane: { id, kind: "graph" } };
}
function v4LeafDashboard(id = "p1") {
  return { kind: "leaf", pane: { id, kind: "dashboard" } };
}
function v4Split(dir, children, sizes) {
  return {
    id: "s1",
    kind: "split",
    dir,
    children,
    sizes: sizes ?? children.map(() => 1 / children.length),
  };
}
function v4Tab(layout, focus, id = "tab-1", name = "untitled") {
  return { id, name, layout, focus };
}

// ── editor → page ──────────────────────────────────────────────────────────

test("editor pane → page buffer using activeIdx tile", () => {
  const state = v4State({
    tabs: [v4Tab(v4LeafEditor(["x", "y", "z"], 1, "p1"), "p1")],
  });
  const { workspace, report } = migrate(state);
  assert.equal(workspace._v, 3);
  assert.equal(workspace.tabs.length, 1);
  const leaf = workspace.tabs[0].layout;
  assert.equal(leaf.type, "leaf");
  assert.equal(leaf.buffer.kind, "page");
  assert.equal(leaf.buffer.pageId, "y"); // activeIdx 1
  assert.equal(report.droppedExtraEditorTiles, 2);
});

test("editor pane with empty tiles is dropped (returns fallback tab)", () => {
  const state = v4State({
    tabs: [v4Tab(v4LeafEditor([], 0, "p1"), "p1")],
  });
  const { workspace } = migrate(state);
  // The leaf was dropped; fallback tab created.
  assert.equal(workspace.tabs.length, 1);
  assert.equal(workspace.tabs[0].layout.type, "leaf");
});

// ── widget → ambient ───────────────────────────────────────────────────────

test("widget pane (calendar) → ambient calendar", () => {
  const state = v4State({
    tabs: [v4Tab(v4LeafWidget("calendar"), "p1")],
  });
  const { workspace, report } = migrate(state);
  const leaf = workspace.tabs[0].layout;
  assert.equal(leaf.buffer.kind, "ambient");
  assert.equal(leaf.buffer.ambientName, "calendar");
  assert.equal(report.convertedWidgetAmbient, 1);
});

test("widget pane with unmapped widget id → ambient workspace-dashboard + reported", () => {
  const state = v4State({
    tabs: [v4Tab(v4LeafWidget("custom-thing"), "p1")],
  });
  const { workspace, report } = migrate(state);
  const leaf = workspace.tabs[0].layout;
  assert.equal(leaf.buffer.kind, "ambient");
  assert.equal(leaf.buffer.ambientName, "workspace-dashboard");
  assert.deepEqual(report.unmappedWidgets, ["custom-thing"]);
});

// ── context → derived ──────────────────────────────────────────────────────

test("context pane → derived backlinks-of-page with follow binding", () => {
  const state = v4State({
    tabs: [v4Tab(v4LeafContext(null, "p1"), "p1")],
  });
  const { workspace, report } = migrate(state);
  const leaf = workspace.tabs[0].layout;
  assert.equal(leaf.buffer.kind, "derived");
  assert.equal(leaf.buffer.rendererName, "backlinks-of-page");
  assert.equal(leaf.buffer.binding.mode, "follow");
  assert.equal(report.convertedContextDerived, 1);
});

// ── graph → dropped ────────────────────────────────────────────────────────

test("graph pane → dropped + reported", () => {
  const state = v4State({
    tabs: [v4Tab(v4LeafGraph("p1"), "p1")],
  });
  const { workspace, report } = migrate(state);
  // Whole tab was just a graph pane; fallback tab now.
  assert.equal(workspace.tabs.length, 1);
  assert.equal(report.droppedGraph, 1);
});

test("graph pane mixed with editor → only graph dropped", () => {
  const state = v4State({
    tabs: [
      v4Tab(
        v4Split("vertical", [
          v4LeafEditor(["x"], 0, "p1"),
          v4LeafGraph("p2"),
        ]),
        "p1",
      ),
    ],
  });
  const { workspace, report } = migrate(state);
  const root = workspace.tabs[0].layout;
  // Only one leaf survives → split collapses into the surviving leaf.
  assert.equal(root.type, "leaf");
  assert.equal(root.buffer.kind, "page");
  assert.equal(root.buffer.pageId, "x");
  assert.equal(report.droppedGraph, 1);
});

// ── dashboard → ambient ────────────────────────────────────────────────────

test("dashboard pane → ambient workspace-dashboard", () => {
  const state = v4State({
    tabs: [v4Tab(v4LeafDashboard("p1"), "p1")],
  });
  const { workspace, report } = migrate(state);
  const leaf = workspace.tabs[0].layout;
  assert.equal(leaf.buffer.kind, "ambient");
  assert.equal(leaf.buffer.ambientName, "workspace-dashboard");
  assert.equal(report.convertedDashboardAmbient, 1);
});

// ── n-ary → binary right-lean ──────────────────────────────────────────────

test("n-ary v4 split of 3 leaves becomes right-leaning binary tree", () => {
  const state = v4State({
    tabs: [
      v4Tab(
        v4Split(
          "vertical",
          [
            v4LeafEditor(["a"], 0, "pa"),
            v4LeafEditor(["b"], 0, "pb"),
            v4LeafEditor(["c"], 0, "pc"),
          ],
          [0.5, 0.3, 0.2],
        ),
        "pa",
      ),
    ],
  });
  const { workspace } = migrate(state);
  const root = workspace.tabs[0].layout;
  assert.equal(root.type, "split");
  assert.equal(root.dir, "v");
  // First child = leaf A; second child = binary split of B + C
  assert.equal(root.children[0].type, "leaf");
  assert.equal(root.children[0].buffer.pageId, "a");
  assert.equal(root.children[1].type, "split");
  assert.equal(root.children[1].children[0].buffer.pageId, "b");
  assert.equal(root.children[1].children[1].buffer.pageId, "c");
  // Ratio of the root split should reflect A's share (0.5 of total 1.0).
  assert.equal(root.ratio, 0.5);
});

// ── focus mapping ──────────────────────────────────────────────────────────

test("migration preserves focus when the focused v4 pane survives", () => {
  const state = v4State({
    tabs: [
      v4Tab(
        v4Split("vertical", [
          v4LeafEditor(["a"], 0, "pa"),
          v4LeafEditor(["b"], 0, "pb"),
        ]),
        "pb",
      ),
    ],
  });
  const { workspace } = migrate(state);
  const tab = workspace.tabs[0];
  // The new leaf id for the previously-focused 'pb' should be the focus.
  const focused = tab.layout.children[1];
  assert.equal(tab.lastFocusedLeafId, focused.id);
  assert.equal(tab.lastFocusedPageId, "b");
});

test("migration falls back to first surviving leaf when focused pane is dropped", () => {
  const state = v4State({
    tabs: [
      v4Tab(
        v4Split("vertical", [
          v4LeafEditor(["a"], 0, "pa"),
          v4LeafGraph("pg"),
        ]),
        "pg", // focused on the graph pane that gets dropped
      ),
    ],
  });
  const { workspace } = migrate(state);
  const tab = workspace.tabs[0];
  assert.ok(tab.lastFocusedLeafId);
  assert.equal(tab.lastFocusedPageId, "a");
});

// ── idempotency ────────────────────────────────────────────────────────────

test("migrate is idempotent on a v3 workspace", () => {
  const state = v4State({
    tabs: [v4Tab(v4LeafEditor(["x"], 0, "p1"), "p1")],
  });
  const { workspace: first } = migrate(state);
  const { workspace: second } = migrate(first);
  // Same _v + same tab count + same focused page.
  assert.equal(first._v, 3);
  assert.equal(second._v, 3);
  assert.equal(first.tabs.length, second.tabs.length);
  assert.equal(first.tabs[0].lastFocusedPageId, second.tabs[0].lastFocusedPageId);
});

test("migrate of null/garbage produces a fresh v3 workspace", () => {
  const { workspace: a } = migrate(null);
  const { workspace: b } = migrate({});
  const { workspace: c } = migrate({ totally: "wrong" });
  for (const ws of [a, b, c]) {
    assert.equal(ws._v, 3);
    assert.equal(ws.tabs.length, 1);
  }
});
