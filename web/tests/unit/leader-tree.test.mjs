import test from "node:test";
import assert from "node:assert/strict";

// Mock Svelte 5 runes for Node.js test environment
globalThis.$state = (v) => v;
globalThis.$derived = (v) => v;
globalThis.$derived.by = (fn) => fn();
globalThis.$effect = () => {};

// Mock localStorage for keybindings.svelte.ts
const localStorageMock = (() => {
  let store = {};
  return {
    getItem: (key) => store[key] ?? null,
    setItem: (key, value) => { store[key] = value; },
    removeItem: (key) => { delete store[key]; },
    clear: () => { store = {}; },
  };
})();
globalThis.localStorage = localStorageMock;

const { commandRegistry } = await import("../../src/lib/command-registry.svelte.ts");
const { getLeaderTree } = await import("../../src/lib/v5/leader-tree.svelte.ts");

function reg(id, chord, label) {
  commandRegistry.register({
    id,
    label: label ?? id,
    glyph: "x",
    category: "navigate",
    chord,
    keywords: [],
    run: () => {},
  });
}

test("named buckets get spec labels, not joined child labels", () => {
  commandRegistry._reset();
  reg("daily", ["g", "d"], "Today's daily note");
  reg("graph", ["g", "g"], "Fullscreen graph");
  reg("vsplit", ["w", "v"], "Split vertically");
  const tree = getLeaderTree();
  const g = tree.find((n) => n.key === "g");
  assert.ok(g, "bucket 'g' should exist");
  assert.equal(g.label, "go to…"); // FROM CHORD_GROUP_LABELS, not "Today's daily note / Fullscreen graph"
  const w = tree.find((n) => n.key === "w");
  assert.ok(w, "bucket 'w' should exist");
  assert.equal(w.label, "windows…");
});

test("all 10 spec buckets are in CHORD_GROUP_LABELS", () => {
  commandRegistry._reset();
  // Register one command per bucket
  const buckets = ["g", "w", "b", "n", "i", "p", "v", "a", "t", ","];
  for (const key of buckets) {
    reg(`cmd-${key}`, [key, "x"], `cmd ${key}`);
  }
  const tree = getLeaderTree();
  for (const key of buckets) {
    const node = tree.find((n) => n.key === key);
    assert.ok(node, `bucket '${key}' should exist in tree`);
    // Each should have a named label (not just the child label)
    assert.ok(
      node.label.endsWith("…"),
      `bucket '${key}' label "${node.label}" should end with … (named from CHORD_GROUP_LABELS)`
    );
  }
});

test("every chord-carrying command is homed under exactly one bucket (no orphans)", () => {
  commandRegistry._reset();
  reg("a", ["g", "d"]);
  reg("b", ["w", "v"]);
  reg("c", ["n", "n"]);
  const tree = getLeaderTree();
  // collect every top-level key; assert each is a known bucket
  const known = new Set(["g", "w", "b", "n", "i", "p", "v", "a", "t", ",", "/", " "]);
  for (const node of tree) {
    assert.ok(known.has(node.key), `bucket key "${node.key}" not in taxonomy`);
  }
});

test("bucket labels match spec: i=insert, p=properties, v=views, a=actions, t=toggle, ,=config", () => {
  commandRegistry._reset();
  const labelMap = {
    g: "go to…",
    w: "windows…",
    b: "buffers…",
    n: "new…",
    i: "insert…",
    p: "properties…",
    v: "views…",
    a: "actions…",
    t: "toggle…",
    ",": "config…",
  };
  for (const [key, expected] of Object.entries(labelMap)) {
    reg(`cmd-${key}-1`, [key, "x"], `cmd ${key}`);
    reg(`cmd-${key}-2`, [key, "y"], `cmd ${key} 2`);
  }
  const tree = getLeaderTree();
  for (const [key, expected] of Object.entries(labelMap)) {
    const node = tree.find((n) => n.key === key);
    assert.ok(node, `bucket '${key}' should be in tree`);
    assert.equal(node.label, expected, `bucket '${key}' label should be "${expected}"`);
  }
});
