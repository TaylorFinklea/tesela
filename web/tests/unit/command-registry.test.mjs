import assert from "node:assert/strict";
import test from "node:test";

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
