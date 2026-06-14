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
// The singleton registry is populated as a side effect of importing v4/commands,
// so tests that need a clean registry call _reset().
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
} = mod;

test("register deduplicates by id", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "test-cmd",
    label: "Test",
    glyph: "t",
    category: "navigate",
    keywords: ["test"],
    run: () => {},
  });
  commandRegistry.register({
    id: "test-cmd",
    label: "Test 2",
    glyph: "t",
    category: "navigate",
    keywords: ["test"],
    run: () => {},
  });
  assert.equal(commandRegistry.all().length, 1);
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
