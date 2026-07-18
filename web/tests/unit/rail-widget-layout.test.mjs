import assert from "node:assert/strict";
import test from "node:test";

const mod = await import("../../src/lib/graphite/rail-widget-layout.ts");

const {
  RAIL_LAYOUT_STORAGE_KEY,
  DEFAULT_RAIL_WIDGET_LAYOUT,
  normalizeRailWidgetLayout,
  loadRailWidgetLayout,
  saveRailWidgetLayout,
  addRailWidget,
  removeRailWidget,
  moveRailWidget,
  toggleRailWidgetCollapsed,
  flattenRailQueryRows,
} = mod;

function memoryStorage(initial = null) {
  const values = new Map();
  if (initial !== null) values.set(RAIL_LAYOUT_STORAGE_KEY, initial);
  return {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    value: () => values.get(RAIL_LAYOUT_STORAGE_KEY),
  };
}

test("layout falls back on corrupt storage and preserves an intentional empty layout", () => {
  assert.deepEqual(loadRailWidgetLayout(memoryStorage("not json")), DEFAULT_RAIL_WIDGET_LAYOUT);
  assert.deepEqual(normalizeRailWidgetLayout({ version: 1, placements: [] }), {
    version: 1,
    placements: [],
  });
});

test("layout validation drops malformed and duplicate ids but preserves unavailable sources", () => {
  const layout = normalizeRailWidgetLayout({
    version: 1,
    placements: [
      { id: "query:work", fallbackTitle: "Work", collapsed: true },
      { id: "query:work", fallbackTitle: "Duplicate" },
      { id: "view:deleted-view", fallbackTitle: "Old view" },
      { id: "oops", fallbackTitle: "Bad" },
    ],
  });
  assert.deepEqual(layout.placements.map((item) => item.id), ["query:work", "view:deleted-view"]);
  assert.equal(layout.placements[0].collapsed, true);
  assert.equal(layout.placements[1].fallbackTitle, "Old view");
});

test("add/remove/move/collapse are stable and boundary-safe", () => {
  let layout = { version: 1, placements: [] };
  const candidate = {
    id: "query:work",
    kind: "query",
    sourceId: "work",
    title: "Work",
    fallbackTitle: "Work",
    subtitle: "Query note",
    icon: "search",
    collapsed: false,
  };
  layout = addRailWidget(layout, candidate);
  assert.strictEqual(addRailWidget(layout, candidate), layout, "duplicate add is a no-op");
  layout = addRailWidget(layout, { ...candidate, id: "view:next", sourceId: "next", title: "Next" });
  assert.strictEqual(moveRailWidget(layout, "query:work", -1), layout, "top boundary is a no-op");
  layout = moveRailWidget(layout, "view:next", -1);
  assert.deepEqual(layout.placements.map((item) => item.id), ["view:next", "query:work"]);
  layout = toggleRailWidgetCollapsed(layout, "query:work");
  assert.equal(layout.placements[1].collapsed, true);
  layout = removeRailWidget(layout, "view:next");
  assert.deepEqual(layout.placements.map((item) => item.id), ["query:work"]);
});

test("save and reload round-trip versioned placement state", () => {
  const storage = memoryStorage();
  const layout = {
    version: 1,
    placements: [{ id: "view:inbox", fallbackTitle: "Inbox", collapsed: true }],
  };
  saveRailWidgetLayout(layout, storage);
  assert.deepEqual(loadRailWidgetLayout(storage), layout);
  assert.match(storage.value(), /\"version\":1/);
});

test("grouped query rows flatten both page and block results in source order", () => {
  const page = { page_id: "p", block_id: null, title: "Page", text: "Page" };
  const block = { page_id: "p", block_id: "b", title: "Page", text: "Task" };
  const rows = flattenRailQueryRows({
    groups: [
      { key: "a", count: 1, items: [page] },
      { key: "b", count: 1, items: [block] },
    ],
  });
  assert.deepEqual(rows, [page, block]);
});
