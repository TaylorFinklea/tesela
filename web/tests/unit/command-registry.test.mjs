import assert from "node:assert/strict";
import test from "node:test";

// Mock KeyboardEvent for Node.js test environment
class KeyboardEvent {
  constructor(type, init = {}) {
    this.type = type;
    this.key = init.key || "";
    this.ctrlKey = init.ctrlKey || false;
    this.altKey = init.altKey || false;
    this.metaKey = init.metaKey || false;
    this.shiftKey = init.shiftKey || false;
  }
}
globalThis.KeyboardEvent = KeyboardEvent;

// We test the pure registry utilities by importing the module's named exports.
// This module alone never registers anything as an import side effect (the
// builtin V4 command set is registered explicitly via registerBuiltinCommands(),
// called once from the root layout) — the singleton registry starts empty, but
// tests still call _reset() defensively for isolation from each other.
const mod = await import("../../src/lib/command-registry.svelte.ts");
const { BUILTIN_SLASH_CHORDS } = await import("../../src/lib/chord-keys.ts");

const {
  commandRegistry,
  buildKeymapIndex,
  findConflicts,
  BROWSER_RESERVED_KEYS,
  effectiveShortcut,
  effectiveChord,
  checkRebind,
  resolveShortcut,
  surfacesFor,
  matchesCommand,
} = mod;

test("register throws on duplicate id (dev — includes plain node test runs)", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "test-cmd",
    label: "Test",
    glyph: "t",
    category: "navigate",
    keywords: ["test"],
    run: () => {},
  });
  assert.throws(() => {
    commandRegistry.register({
      id: "test-cmd",
      label: "Test 2",
      glyph: "t",
      category: "navigate",
      keywords: ["test"],
      run: () => {},
    });
  }, /registered twice/);
  // The first registration wins; the throwing duplicate never landed.
  assert.equal(commandRegistry.all().length, 1);
  assert.equal(commandRegistry.get("test-cmd")?.label, "Test");
});

test("buildKeymapIndex groups shortcuts and chords", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "a",
    label: "A",
    glyph: "a",
    category: "navigate",
    shortcut: "⌘A",
    chord: ["g", "a"],
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "b",
    label: "B",
    glyph: "b",
    category: "navigate",
    chord: ["g", "b"],
    keywords: [],
    run: () => {},
  });

  const idx = buildKeymapIndex();
  assert.equal(idx.shortcuts.get("⌘A")?.length, 1);
  assert.equal(idx.chords.get("g a")?.length, 1);
  assert.equal(idx.chords.get("g b")?.length, 1);
});

test("buildKeymapIndex includes builtin slash chords", () => {
  commandRegistry._reset();
  const idx = buildKeymapIndex();

  for (const [key, label] of BUILTIN_SLASH_CHORDS) {
    const commands = idx.chords.get(`/ ${key}`);
    assert.ok(commands, `expected slash chord / ${key}`);
    assert.equal(commands[0].id, `slash:${key}`);
    assert.equal(commands[0].label, label);
  }
});

test("findConflicts detects duplicate shortcuts and chords", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "a",
    label: "A",
    glyph: "a",
    category: "navigate",
    shortcut: "⌘X",
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "b",
    label: "B",
    glyph: "b",
    category: "navigate",
    shortcut: "⌘X",
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "c",
    label: "C",
    glyph: "c",
    category: "navigate",
    chord: ["g", "d"],
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "d",
    label: "D",
    glyph: "d",
    category: "navigate",
    chord: ["g", "d"],
    keywords: [],
    run: () => {},
  });

  const conflicts = findConflicts();
  const shortcutConflict = conflicts.find((c) => c.kind === "shortcut" && c.key === "⌘X");
  const chordConflict = conflicts.find((c) => c.kind === "chord" && c.key === "g d");
  assert.ok(shortcutConflict, "expected shortcut conflict");
  assert.equal(shortcutConflict.commands.length, 2);
  assert.ok(chordConflict, "expected chord conflict");
  assert.equal(chordConflict.commands.length, 2);
});

test("findConflicts flags browser-reserved keys", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "new-tab",
    label: "New tab",
    glyph: "+",
    category: "tab",
    shortcut: "⌘T",
    keywords: [],
    run: () => {},
  });

  const conflicts = findConflicts();
  const reserved = conflicts.find((c) => c.kind === "browser-reserved" && c.key === "⌘T");
  assert.ok(reserved, "expected browser-reserved conflict for ⌘T");
  assert.ok(BROWSER_RESERVED_KEYS.has("⌘T"));
});

test("findByVerb resolves by verb or id", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "my-cmd",
    verb: "myverb",
    label: "My",
    glyph: "m",
    category: "navigate",
    keywords: [],
    run: () => {},
  });
  assert.equal(commandRegistry.findByVerb("myverb")?.id, "my-cmd");
  assert.equal(commandRegistry.findByVerb("my-cmd")?.id, "my-cmd");
});

test("available filters by when predicate", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "always",
    label: "Always",
    glyph: "a",
    category: "navigate",
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "only-page",
    label: "Only Page",
    glyph: "p",
    category: "navigate",
    keywords: [],
    when: (ctx) => ctx.bufferKind === "page",
    run: () => {},
  });

  assert.equal(commandRegistry.available({}).length, 1);
  assert.equal(commandRegistry.available({ bufferKind: "page" }).length, 2);
  assert.equal(commandRegistry.available({ bufferKind: "ambient" }).length, 1);
});

test("available filters editor-surface commands without editor context", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "global-cmd",
    label: "Global",
    glyph: "g",
    category: "navigate",
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "editor-cmd",
    label: "Editor",
    glyph: "e",
    category: "editor",
    surface: "editor",
    keywords: [],
    run: () => {},
  });

  assert.deepEqual(commandRegistry.available({}).map((cmd) => cmd.id), ["global-cmd"]);
  assert.deepEqual(commandRegistry.available({ editor: {} }).map((cmd) => cmd.id), ["global-cmd", "editor-cmd"]);
});

// ── effectiveShortcut / effectiveChord / checkRebind / resolveShortcut ─────

test("effectiveShortcut returns override when present", () => {
  commandRegistry._reset();
  const cmd = {
    id: "x",
    label: "X",
    glyph: "x",
    category: "navigate",
    shortcut: "⌘A",
    keywords: [],
    run: () => {},
    registeredAt: 0,
  };
  const overrides = { x: { shortcut: "⌘Z" } };
  assert.equal(effectiveShortcut(cmd, overrides), "⌘Z");
});

test("effectiveShortcut falls back to command default", () => {
  const cmd = {
    id: "x",
    label: "X",
    glyph: "x",
    category: "navigate",
    shortcut: "⌘A",
    keywords: [],
    run: () => {},
    registeredAt: 0,
  };
  assert.equal(effectiveShortcut(cmd, {}), "⌘A");
});

test("effectiveShortcut returns undefined when override is null", () => {
  const cmd = {
    id: "x",
    label: "X",
    glyph: "x",
    category: "navigate",
    shortcut: "⌘A",
    keywords: [],
    run: () => {},
    registeredAt: 0,
  };
  const overrides = { x: { shortcut: null } };
  assert.equal(effectiveShortcut(cmd, overrides), undefined);
});

test("effectiveChord returns override when present", () => {
  const cmd = {
    id: "x",
    label: "X",
    glyph: "x",
    category: "navigate",
    chord: ["g", "a"],
    keywords: [],
    run: () => {},
    registeredAt: 0,
  };
  const overrides = { x: { chord: ["h", "b"] } };
  assert.deepEqual(effectiveChord(cmd, overrides), ["h", "b"]);
});

test("effectiveChord falls back to command default", () => {
  const cmd = {
    id: "x",
    label: "X",
    glyph: "x",
    category: "navigate",
    chord: ["g", "a"],
    keywords: [],
    run: () => {},
    registeredAt: 0,
  };
  assert.deepEqual(effectiveChord(cmd, {}), ["g", "a"]);
});

test("effectiveChord returns undefined when override is null", () => {
  const cmd = {
    id: "x",
    label: "X",
    glyph: "x",
    category: "navigate",
    chord: ["g", "a"],
    keywords: [],
    run: () => {},
    registeredAt: 0,
  };
  const overrides = { x: { chord: null } };
  assert.equal(effectiveChord(cmd, overrides), undefined);
});

test("checkRebind returns reserved for browser-reserved shortcut", () => {
  const result = checkRebind("x", "shortcut", "⌘W", {});
  assert.deepEqual(result, { ok: false, reason: "reserved" });
});

test("checkRebind returns ok for non-reserved shortcut", () => {
  const result = checkRebind("x", "shortcut", "⌘A", {});
  assert.deepEqual(result, { ok: true });
});

test("checkRebind always returns ok for chord", () => {
  const result = checkRebind("x", "chord", "g a", {});
  assert.deepEqual(result, { ok: true });
});

test("checkRebind reports taken when another command holds the binding", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "a", label: "A", glyph: "a", category: "navigate",
    shortcut: "⌘J", keywords: [], run: () => {},
  });
  commandRegistry.register({
    id: "b", label: "B", glyph: "b", category: "navigate",
    shortcut: "⌘K", keywords: [], run: () => {},
  });
  // Rebinding b onto a's shortcut → taken (soft warn), holders = [a].
  const taken = checkRebind("b", "shortcut", "⌘J", {});
  assert.equal(taken.ok, false);
  assert.equal(taken.reason, "taken");
  assert.deepEqual(taken.by.map((c) => c.id), ["a"]);
  // b's own current shortcut and a free key are both ok.
  assert.deepEqual(checkRebind("b", "shortcut", "⌘K", {}), { ok: true });
  assert.deepEqual(checkRebind("b", "shortcut", "⌘Z", {}), { ok: true });
});

test("buildKeymapIndex with no overrides is unchanged", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "a",
    label: "A",
    glyph: "a",
    category: "navigate",
    shortcut: "⌘A",
    chord: ["g", "a"],
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "b",
    label: "B",
    glyph: "b",
    category: "navigate",
    chord: ["g", "b"],
    keywords: [],
    run: () => {},
  });

  const idx = buildKeymapIndex();
  assert.equal(idx.shortcuts.get("⌘A")?.length, 1);
  assert.equal(idx.chords.get("g a")?.length, 1);
  assert.equal(idx.chords.get("g b")?.length, 1);
});

test("buildKeymapIndex with overrides uses effective shortcuts", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "a",
    label: "A",
    glyph: "a",
    category: "navigate",
    shortcut: "⌘A",
    chord: ["g", "a"],
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "b",
    label: "B",
    glyph: "b",
    category: "navigate",
    chord: ["g", "b"],
    keywords: [],
    run: () => {},
  });

  const overrides = {
    a: { shortcut: "⌘Z", chord: ["h", "x"] },
    b: { chord: null }, // unbind chord
  };

  const idx = buildKeymapIndex(commandRegistry, overrides);
  assert.equal(idx.shortcuts.get("⌘A"), undefined); // old shortcut gone
  assert.equal(idx.shortcuts.get("⌘Z")?.length, 1); // new shortcut
  assert.equal(idx.chords.get("g a"), undefined); // old chord gone
  assert.equal(idx.chords.get("h x")?.length, 1); // new chord
  assert.equal(idx.chords.get("g b"), undefined); // chord unbound
});

test("resolveShortcut returns matching command", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "test-cmd",
    label: "Test",
    glyph: "t",
    category: "navigate",
    shortcut: "⌘P",
    keywords: [],
    run: () => {},
  });

  const e = new KeyboardEvent("keydown", { key: "p", metaKey: true });
  const result = resolveShortcut(e, {}, {});
  assert.equal(result?.id, "test-cmd");
});

test("resolveShortcut returns undefined for browser-reserved key", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "test-cmd",
    label: "Test",
    glyph: "t",
    category: "navigate",
    shortcut: "⌘W",
    keywords: [],
    run: () => {},
  });

  const e = new KeyboardEvent("keydown", { key: "w", metaKey: true });
  const result = resolveShortcut(e, {}, {});
  assert.equal(result, undefined);
});

test("resolveShortcut returns undefined when no modifier held", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "test-cmd",
    label: "Test",
    glyph: "t",
    category: "navigate",
    shortcut: "⌘P",
    keywords: [],
    run: () => {},
  });

  const e = new KeyboardEvent("keydown", { key: "p" });
  const result = resolveShortcut(e, {}, {});
  assert.equal(result, undefined);
});

test("resolveShortcut respects overrides", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "cmd-a",
    label: "A",
    glyph: "a",
    category: "navigate",
    shortcut: "⌘A",
    keywords: [],
    run: () => {},
  });
  commandRegistry.register({
    id: "cmd-b",
    label: "B",
    glyph: "b",
    category: "navigate",
    shortcut: "⌘B",
    keywords: [],
    run: () => {},
  });

  // Rebind cmd-a to ⌘Z
  const overrides = { "cmd-a": { shortcut: "⌘Z" } };

  // Old shortcut no longer resolves
  const e1 = new KeyboardEvent("keydown", { key: "a", metaKey: true });
  assert.equal(resolveShortcut(e1, {}, overrides), undefined);

  // New shortcut resolves to cmd-a
  const e2 = new KeyboardEvent("keydown", { key: "z", metaKey: true });
  const result = resolveShortcut(e2, {}, overrides);
  assert.equal(result?.id, "cmd-a");

  // cmd-b still resolves to ⌘B
  const e3 = new KeyboardEvent("keydown", { key: "b", metaKey: true });
  const result2 = resolveShortcut(e3, {}, overrides);
  assert.equal(result2?.id, "cmd-b");
});

// ── surfacesFor (back-compat derivation) ──────────────────────────────────

test("surfacesFor returns explicit surfaces verbatim when present", () => {
  const cmd = {
    id: "x", label: "X", glyph: "x", category: "editor",
    surfaces: new Set(["slash", "leader"]),
    slashKey: "h", surface: "global", chord: ["i", "h"], // all ignored
    keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("slash"), true);
  assert.equal(s.has("leader"), true);
  assert.equal(s.has("colon"), false);
  assert.equal(s.has("palette"), false);
});

test("surfacesFor derives slash from slashKey + always palette/colon (back-compat)", () => {
  // A bare editor insertion verb today: slashKey present, no surface flag.
  const cmd = {
    id: "ins", label: "Ins", glyph: "+", category: "editor",
    slashKey: "h", keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("slash"), true);
  assert.equal(s.has("palette"), true);
  assert.equal(s.has("colon"), true);
});

test("surfacesFor: surface:'global' yields all four surfaces", () => {
  // The editor.heading shape: surface:'global' + slashKey — leaks everywhere today.
  const cmd = {
    id: "editor.heading", label: "Heading", glyph: "#", category: "editor",
    surface: "global", slashKey: "h", keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.deepEqual(
    [...["slash", "colon", "leader", "palette"]].filter((k) => s.has(k)).sort(),
    ["colon", "leader", "palette", "slash"],
  );
});

test("surfacesFor: surface:'editor' yields slash only", () => {
  const cmd = {
    id: "ed", label: "Ed", glyph: "e", category: "editor",
    surface: "editor", keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("slash"), true);
  assert.equal(s.has("colon"), false);
  assert.equal(s.has("leader"), false);
  assert.equal(s.has("palette"), false);
});

test("surfacesFor: chord puts command in the leader bucket", () => {
  const cmd = {
    id: "go-daily", label: "Daily", glyph: "d", category: "navigate",
    chord: ["g", "d"], keywords: [], run: () => {},
  };
  assert.equal(surfacesFor(cmd).has("leader"), true);
});

test("surfacesFor: plain command (no flags) is palette + colon", () => {
  const cmd = {
    id: "plain", label: "Plain", glyph: "p", category: "navigate",
    keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("palette"), true);
  assert.equal(s.has("colon"), true);
  assert.equal(s.has("slash"), false);
  assert.equal(s.has("leader"), false);
});

// ── availableOn (per-surface filter over available) ───────────────────────

test("availableOn filters available() by derived surface", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "slashy", label: "Slashy", glyph: "+", category: "editor",
    slashKey: "h", keywords: [], run: () => {},
  });
  commandRegistry.register({
    id: "leadery", label: "Leadery", glyph: "g", category: "navigate",
    chord: ["g", "d"], keywords: [], run: () => {},
  });
  commandRegistry.register({
    id: "plain", label: "Plain", glyph: "p", category: "navigate",
    keywords: [], run: () => {},
  });

  // slashy has slashKey → slash; also palette+colon. NOT leader.
  assert.deepEqual(
    commandRegistry.availableOn("slash", {}).map((c) => c.id),
    ["slashy"],
  );
  // leadery has chord → leader. plain has none.
  assert.deepEqual(
    commandRegistry.availableOn("leader", {}).map((c) => c.id),
    ["leadery"],
  );
  // palette/colon include every non-editor command.
  assert.deepEqual(
    commandRegistry.availableOn("palette", {}).map((c) => c.id),
    ["slashy", "leadery", "plain"],
  );
  assert.deepEqual(
    commandRegistry.availableOn("colon", {}).map((c) => c.id),
    ["slashy", "leadery", "plain"],
  );
});

test("availableOn respects explicit surfaces (authoritative)", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "scoped", label: "Scoped", glyph: "s", category: "navigate",
    surfaces: new Set(["leader"]),
    slashKey: "x", chord: ["g", "z"], // would-be derivations ignored
    keywords: [], run: () => {},
  });
  assert.deepEqual(commandRegistry.availableOn("leader", {}).map((c) => c.id), ["scoped"]);
  assert.deepEqual(commandRegistry.availableOn("slash", {}).map((c) => c.id), []);
  assert.deepEqual(commandRegistry.availableOn("palette", {}).map((c) => c.id), []);
});

test("availableOn still honors when() and editor gate from available()", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "page-only", label: "Page", glyph: "p", category: "navigate",
    chord: ["g", "p"], when: (ctx) => ctx.bufferKind === "page",
    keywords: [], run: () => {},
  });
  commandRegistry.register({
    id: "editor-ins", label: "Ins", glyph: "+", category: "editor",
    surface: "editor", slashKey: "h", keywords: [], run: () => {},
  });

  // when() gate: leader bucket hides page-only off a page.
  assert.deepEqual(commandRegistry.availableOn("leader", {}).map((c) => c.id), []);
  assert.deepEqual(
    commandRegistry.availableOn("leader", { bufferKind: "page" }).map((c) => c.id),
    ["page-only"],
  );
  // editor gate: editor-ins absent without ctx.editor, present with it.
  assert.deepEqual(commandRegistry.availableOn("slash", {}).map((c) => c.id), []);
  assert.deepEqual(
    commandRegistry.availableOn("slash", { editor: {} }).map((c) => c.id),
    ["editor-ins"],
  );
});

test("availableOn('slash') excludes editor.widget (Phase C — widget moves to leader)", () => {
  commandRegistry._reset();
  // editor.widget is the canonical editor-insertion command that Phase C
  // removes from the slash surface. It still exists in the registry (the
  // leader's `n` bucket needs it), but `availableOn('slash', …)` must not
  // return it. editor.task stays as a control (should remain on slash).
  commandRegistry.register({
    id: "editor.widget", label: "New widget", glyph: "w", category: "editor",
    surface: "editor", slashKey: "w", chord: ["n", "w"],
    surfaces: new Set(["leader"]), // explicit: only leader, not slash
    keywords: [], run: () => {},
  });
  commandRegistry.register({
    id: "editor.task", label: "Task", glyph: "t", category: "editor",
    surface: "global", slashKey: "t", keywords: [], run: () => {},
  });

  const slash = commandRegistry.availableOn("slash", { editor: {} }).map((c) => c.id);
  assert.ok(!slash.includes("editor.widget"), "editor.widget must not appear on slash surface");
  assert.ok(slash.includes("editor.task"), "editor.task must still appear on slash surface");
});

test("BUILTIN_SLASH_CHORDS no longer carries widget (w) or All-properties (p)", () => {
  // Phase C drops the hard-coded `w` (New widget) and `p` (All properties)
  // rows. Widget moved to the leader `new` bucket; `p` is now the single
  // context-aware Properties entry in getSlashTree, not a top-level builtin.
  assert.equal(BUILTIN_SLASH_CHORDS.has("w"), false, "BUILTIN_SLASH_CHORDS dropped w (New widget)");
  assert.equal(BUILTIN_SLASH_CHORDS.has("p"), false, "BUILTIN_SLASH_CHORDS dropped p (All properties)");
});

// ── Phase D: colon narrows to exact verbs + folds peek/graph builtins ─────

test("availableOn('colon') includes peek and graph verbs from the registry", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "peek",
    verb: "peek",
    label: "Toggle Peek popover",
    glyph: "i",
    category: "tile",
    chord: ["p"],
    shortcut: "⌘I",
    keywords: ["peek"],
    run: () => {},
  });
  commandRegistry.register({
    id: "fullscreen-graph",
    verb: "graph",
    label: "Fullscreen graph",
    glyph: "✦",
    category: "navigate",
    chord: ["g", "g"],
    shortcut: "⌘G",
    keywords: ["graph"],
    run: () => {},
  });

  const colon = commandRegistry.availableOn("colon", {});
  const verbs = colon.map((c) => c.verb);
  assert.ok(verbs.includes("peek"), "colon surface includes :peek from registry");
  assert.ok(verbs.includes("graph"), "colon surface includes :graph from registry");
  assert.equal(commandRegistry.findByVerb("peek")?.id, "peek");
  assert.equal(commandRegistry.findByVerb("graph")?.id, "fullscreen-graph");
});

test("availableOn('colon') excludes a palette-only command", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "palette-only",
    verb: "palonly",
    label: "Palette Only",
    glyph: "x",
    category: "navigate",
    surfaces: new Set(["palette"]),
    keywords: [],
    run: () => {},
  });
  const colonVerbs = commandRegistry.availableOn("colon", {}).map((c) => c.verb);
  assert.ok(!colonVerbs.includes("palonly"), "palette-only command is not a colon verb");
  const palVerbs = commandRegistry.availableOn("palette", {}).map((c) => c.verb);
  assert.ok(palVerbs.includes("palonly"));
});

// ── B (leader→editor): editor commands reach the leader when a block is focused ──

test("surfacesFor: surface:'editor' WITH a chord ALSO yields leader (leader→editor)", () => {
  // The link/date/tag/etc. shape: editor surface + an i/p chord. They must now
  // appear in the leader (i/p buckets), not just slash. (No chord → slash-only,
  // covered by the existing test above.)
  const cmd = {
    id: "editor.link", label: "Link", glyph: "[[ ]]", category: "editor",
    surface: "editor", slashKey: "l", chord: ["i", "l"], keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("slash"), true);
  assert.equal(s.has("leader"), true); // NEW
  assert.equal(s.has("colon"), false);
  assert.equal(s.has("palette"), false);
});

test("available: editorFocused admits surface:'editor' commands without a full editor ctx", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "editor-cmd", label: "Editor", glyph: "e", category: "editor",
    surface: "editor", chord: ["i", "l"], keywords: [], run: () => {},
  });
  // Neither editor nor editorFocused → dropped (unchanged behavior).
  assert.deepEqual(commandRegistry.available({}).map((c) => c.id), []);
  // editorFocused presence (the leader path) admits it without a full editor ctx.
  assert.deepEqual(commandRegistry.available({ editorFocused: true }).map((c) => c.id), ["editor-cmd"]);
  // A full editor ctx (the slash path) still admits it.
  assert.deepEqual(commandRegistry.available({ editor: {} }).map((c) => c.id), ["editor-cmd"]);
});

test("availableOn('leader'): editor+chord command shows only when a block is focused", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "editor.link", label: "Link", glyph: "[[ ]]", category: "editor",
    surface: "editor", slashKey: "l", chord: ["i", "l"], keywords: [], run: () => {},
  });
  // Hidden from the leader with no focused block...
  assert.deepEqual(commandRegistry.availableOn("leader", {}).map((c) => c.id), []);
  // ...shown once a block is focused (editorFocused), so the i bucket populates.
  assert.deepEqual(
    commandRegistry.availableOn("leader", { editorFocused: true }).map((c) => c.id),
    ["editor.link"],
  );
  // Slash surface unchanged: still gated on a real editor ctx.
  assert.deepEqual(commandRegistry.availableOn("slash", { editor: {} }).map((c) => c.id), ["editor.link"]);
});

// ── matchesCommand (moved into the registry so palette+colon stop reaching
// around it into v4/commands.ts) ───────────────────────────────────────────

test("matchesCommand: empty query matches everything", () => {
  const cmd = { id: "x", label: "X", glyph: "x", category: "navigate", keywords: [], run: () => {} };
  assert.equal(matchesCommand(cmd, ""), true);
});

test("matchesCommand: matches label, verb, or keywords case-insensitively", () => {
  const cmd = {
    id: "daily", verb: "daily", label: "Today's daily note", glyph: "☀",
    category: "navigate", keywords: ["journal"], run: () => {},
  };
  assert.equal(matchesCommand(cmd, "DAILY"), true);
  assert.equal(matchesCommand(cmd, "journal"), true);
  assert.equal(matchesCommand(cmd, "today"), true);
  assert.equal(matchesCommand(cmd, "nope"), false);
});
