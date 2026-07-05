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
const { getLeaderTree } = await import("../../src/lib/leader/leader-tree.svelte.ts");
const keybindings = await import("../../src/lib/stores/keybindings.svelte.ts");

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

test("editor-category leaf dispatches tesela:run-editor-command (not run in place)", () => {
  // The shell ctx has no live `editor`, so a leader editor verb must hand off
  // to the focused BlockEditor via the event rather than calling run() (which
  // would no-op: `if (!ed) return`).
  commandRegistry._reset();
  let ran = false;
  let dispatched = null;
  globalThis.CustomEvent = class { constructor(type, init) { this.type = type; this.detail = init?.detail; } };
  globalThis.document = { dispatchEvent: (e) => { dispatched = e; return true; } };
  commandRegistry.register({
    id: "editor.heading", label: "Heading", glyph: "#", category: "editor",
    surface: "global", slashKey: "h", chord: ["i", "h"], keywords: [],
    run: () => { ran = true; },
  });
  const tree = getLeaderTree({ editorFocused: true });
  const h = tree.find((n) => n.key === "i")?.children?.find((c) => c.key === "h");
  assert.ok(h?.action, "i → h leaf must exist");
  h.action();
  assert.equal(ran, false, "editor command must NOT run in place from the leader");
  assert.ok(dispatched, "an event must be dispatched");
  assert.equal(dispatched.type, "tesela:run-editor-command");
  assert.equal(dispatched.detail.id, "editor.heading");
});

test("non-editor leaf runs in place (no event dispatched)", () => {
  commandRegistry._reset();
  let ran = false;
  let dispatched = null;
  globalThis.document = { dispatchEvent: (e) => { dispatched = e; return true; } };
  commandRegistry.register({
    id: "nav.daily", label: "Daily", glyph: "d", category: "navigate",
    chord: ["g", "d"], keywords: [], run: () => { ran = true; },
  });
  const d = getLeaderTree({}).find((n) => n.key === "g")?.children?.find((c) => c.key === "d");
  assert.ok(d?.action, "g → d leaf must exist");
  d.action();
  assert.equal(ran, true, "non-editor command runs in place");
  assert.equal(dispatched, null, "no event dispatched for a non-editor command");
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

// ── user config: group-label + hide-per-surface overrides (tesela-cmdd.4) ──

test("a user group-label override replaces the compiled-in bucket label", () => {
  commandRegistry._reset();
  keybindings.resetAll();
  reg("daily", ["g", "d"], "Today's daily note");
  keybindings.setGroupLabel("g", "Jump around…");
  const g = getLeaderTree().find((n) => n.key === "g");
  assert.ok(g);
  assert.equal(g.label, "Jump around…");
  keybindings.resetAll();
});

test("a group-label override is scoped to its own chord-path prefix, not other buckets", () => {
  commandRegistry._reset();
  keybindings.resetAll();
  reg("daily", ["g", "d"], "Today's daily note");
  reg("vsplit", ["w", "v"], "Split vertically");
  keybindings.setGroupLabel("g", "Jump around…");
  const tree = getLeaderTree();
  assert.equal(tree.find((n) => n.key === "g").label, "Jump around…");
  assert.equal(tree.find((n) => n.key === "w").label, "windows…"); // untouched
  keybindings.resetAll();
});

test("a nested group-label override applies at its own depth (path-keyed, not just top-level key)", () => {
  commandRegistry._reset();
  keybindings.resetAll();
  reg("general", [",", "g"], "General");
  reg("devices", [",", "d"], "Devices");
  keybindings.setGroupLabel(",", "Settings…");
  const bucket = getLeaderTree().find((n) => n.key === ",");
  assert.equal(bucket.label, "Settings…");
  keybindings.resetAll();
});

test("resetGroupLabel restores the compiled-in label", () => {
  commandRegistry._reset();
  keybindings.resetAll();
  reg("daily", ["g", "d"], "Today's daily note");
  keybindings.setGroupLabel("g", "Custom");
  assert.equal(getLeaderTree().find((n) => n.key === "g").label, "Custom");
  keybindings.resetGroupLabel("g");
  assert.equal(getLeaderTree().find((n) => n.key === "g").label, "go to…");
  keybindings.resetAll();
});

test("a command hidden on the leader surface (no ctx) is excluded from the tree", () => {
  commandRegistry._reset();
  keybindings.resetAll();
  reg("daily", ["g", "d"], "Today's daily note");
  reg("graph", ["g", "g"], "Fullscreen graph");
  keybindings.setHidden("daily", ["leader"]);
  const g = getLeaderTree().find((n) => n.key === "g");
  // "daily" hidden leaves only "graph" under g — a single-child bucket
  // collapses its own g/d slot away, so g's only remaining leaf is "graph".
  assert.ok(g);
  const leaves = [];
  const walk = (nodes) => {
    for (const n of nodes) {
      if (n.action) leaves.push(n.key);
      if (n.children) walk(n.children);
    }
  };
  walk([g]);
  assert.deepEqual(leaves, ["g"]); // graph's leaf key within the g bucket
  keybindings.resetAll();
});

test("a command hidden on the leader surface (with ctx) is excluded from the tree; non-hidden still renders", () => {
  // Regression for the qwen review finding on tesela-cmdd.4: the per-surface
  // hidden[] config must filter the leader tree regardless of whether a
  // CommandContext is provided. The production caller (GrLeaderOverlay) always
  // passes a ctx, so this path is the one the user actually sees. The fix
  // makes the explicit `isHiddenOn` filter in getLeaderTree unconditional
  // (it was previously gated on `!ctx`, relying on `availableOn` to filter
  // implicitly in the with-ctx branch — fragile, easy to silently regress).
  commandRegistry._reset();
  keybindings.resetAll();
  reg("daily", ["g", "d"], "Today's daily note");
  reg("graph", ["g", "g"], "Fullscreen graph");
  keybindings.setHidden("daily", ["leader"]);
  // Walk the whole tree and collect every leaf key, so the assertion
  // is independent of the single-child-bucket collapse that simplifies
  // the no-ctx test above.
  const leaves = [];
  const walk = (nodes) => {
    for (const n of nodes) {
      if (n.action) leaves.push(n.key);
      if (n.children) walk(n.children);
    }
  };
  walk(getLeaderTree({}));
  assert.ok(
    !leaves.includes("d"),
    `hidden command "daily" (leaf key "d") must NOT appear in the tree; got ${JSON.stringify(leaves)}`,
  );
  assert.ok(
    leaves.includes("g"),
    `non-hidden command "graph" (leaf key "g") must still appear in the tree; got ${JSON.stringify(leaves)}`,
  );
  keybindings.resetAll();
});
