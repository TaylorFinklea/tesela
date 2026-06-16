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

test("config bucket: settings panes nest under ',' as unique-keyed children (no bare-',' leaf)", () => {
  // Regression for the leader-overlay crash: when `general` was a bare-`,` leaf
  // while devices/sync/mosaic/data were `,x` branches, buildChordTree emitted
  // TWO root nodes keyed `,` (a leaf + its subtree). GrLeaderOverlay's keyed
  // {#each ... (node.key)} then threw Svelte's each_key_duplicate and crashed
  // the WHOLE leader on every open. The fix homes `general` at `,g`, making `,`
  // a pure config bucket with all panes keyboard-reachable.
  commandRegistry._reset();
  reg("general", [",", "g"], "General");
  reg("devices", [",", "d"], "Devices");
  reg("sync", [",", "s"], "Sync");
  reg("mosaic", [",", "m"], "Mosaic");
  reg("data", [",", "a"], "Data");
  const tree = getLeaderTree();
  const commaNodes = tree.filter((n) => n.key === ",");
  assert.equal(commaNodes.length, 1, "',' must be ONE bucket node, not a leaf+subtree pair");
  const bucket = commaNodes[0];
  assert.ok(bucket.children, "',' bucket must have children");
  assert.deepEqual(
    bucket.children.map((c) => c.key).sort(),
    ["a", "d", "g", "m", "s"],
    "all 5 settings panes must be reachable as unique-keyed children",
  );
});

test("getLeaderTree siblings always have a unique render key (key+label) — overlay-safe", () => {
  // GrLeaderOverlay keys its {#each} by `${node.key} ${node.label}`. Even a
  // mis-homed bare-key leaf colliding with a bucket must produce distinct render
  // keys so the keyed each never throws each_key_duplicate.
  commandRegistry._reset();
  reg("leaf", [","], "General"); // bare-',' leaf
  reg("branch", [",", "d"], "Devices"); // ',d' branch → forces a leaf+subtree pair
  const walk = (nodes) => {
    const renderKeys = nodes.map((n) => `${n.key} ${n.label}`);
    assert.equal(
      new Set(renderKeys).size,
      renderKeys.length,
      `render keys must be unique among siblings: ${JSON.stringify(renderKeys)}`,
    );
    for (const n of nodes) if (n.children) walk(n.children);
  };
  walk(getLeaderTree());
});
