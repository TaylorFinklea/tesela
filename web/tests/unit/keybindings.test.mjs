import { describe, it, beforeEach } from "node:test";
import assert from "node:assert/strict";

// Mock $state for Node.js test environment (Svelte 5 rune)
globalThis.$state = (initial) => initial;

// Mock localStorage for Node.js test environment
const localStorageMock = (() => {
  let store = {};
  return {
    getItem: (key) => store[key] ?? null,
    setItem: (key, value) => { store[key] = value; },
    removeItem: (key) => { delete store[key]; },
    clear: () => { store = {}; },
  };
})();

// @ts-ignore - mock globalThis.localStorage for Node.js
globalThis.localStorage = localStorageMock;

// Import after mocking localStorage
const keybindings = await import("../../src/lib/stores/keybindings.svelte.ts");

describe("keybindings store", () => {
  beforeEach(() => {
    localStorageMock.clear();
    keybindings.resetAll();
  });

  describe("get", () => {
    it("returns undefined for unknown command id", () => {
      assert.equal(keybindings.get("unknown-cmd"), undefined);
    });

    it("returns override after setShortcut", () => {
      keybindings.setShortcut("test-cmd", "⌘A");
      const override = keybindings.get("test-cmd");
      assert.deepEqual(override, { shortcut: "⌘A" });
    });

    it("returns override after setChord", () => {
      keybindings.setChord("test-cmd", ["g", "a"]);
      const override = keybindings.get("test-cmd");
      assert.deepEqual(override, { chord: ["g", "a"] });
    });

    it("returns both shortcut and chord when both are set", () => {
      keybindings.setShortcut("test-cmd", "⌘B");
      keybindings.setChord("test-cmd", ["g", "b"]);
      const override = keybindings.get("test-cmd");
      assert.deepEqual(override, { shortcut: "⌘B", chord: ["g", "b"] });
    });
  });

  describe("setShortcut", () => {
    it("sets shortcut to a string value", () => {
      keybindings.setShortcut("cmd1", "⌘X");
      assert.deepEqual(keybindings.get("cmd1"), { shortcut: "⌘X" });
    });

    it("sets shortcut to null (explicitly unbound)", () => {
      keybindings.setShortcut("cmd1", null);
      assert.deepEqual(keybindings.get("cmd1"), { shortcut: null });
    });

    it("persists to localStorage", () => {
      keybindings.setShortcut("cmd1", "⌘Y");
      const stored = localStorageMock.getItem("tesela:keybindings");
      assert.ok(stored);
      const parsed = JSON.parse(stored);
      assert.deepEqual(parsed, { cmd1: { shortcut: "⌘Y" } });
    });

    it("updates existing shortcut", () => {
      keybindings.setShortcut("cmd1", "⌘Z");
      keybindings.setShortcut("cmd1", "⌘W");
      assert.deepEqual(keybindings.get("cmd1"), { shortcut: "⌘W" });
    });

    it("preserves chord when setting shortcut", () => {
      keybindings.setChord("cmd1", ["a", "b"]);
      keybindings.setShortcut("cmd1", "⌘C");
      assert.deepEqual(keybindings.get("cmd1"), { shortcut: "⌘C", chord: ["a", "b"] });
    });
  });

  describe("setChord", () => {
    it("sets chord to an array value", () => {
      keybindings.setChord("cmd1", ["x", "y"]);
      assert.deepEqual(keybindings.get("cmd1"), { chord: ["x", "y"] });
    });

    it("sets chord to null (explicitly unbound)", () => {
      keybindings.setChord("cmd1", null);
      assert.deepEqual(keybindings.get("cmd1"), { chord: null });
    });

    it("persists to localStorage", () => {
      keybindings.setChord("cmd1", ["g", "z"]);
      const stored = localStorageMock.getItem("tesela:keybindings");
      assert.ok(stored);
      const parsed = JSON.parse(stored);
      assert.deepEqual(parsed, { cmd1: { chord: ["g", "z"] } });
    });

    it("updates existing chord", () => {
      keybindings.setChord("cmd1", ["a"]);
      keybindings.setChord("cmd1", ["b", "c"]);
      assert.deepEqual(keybindings.get("cmd1"), { chord: ["b", "c"] });
    });

    it("preserves shortcut when setting chord", () => {
      keybindings.setShortcut("cmd1", "⌘D");
      keybindings.setChord("cmd1", ["e", "f"]);
      assert.deepEqual(keybindings.get("cmd1"), { shortcut: "⌘D", chord: ["e", "f"] });
    });
  });

  describe("reset", () => {
    it("removes a single command override", () => {
      keybindings.setShortcut("cmd1", "⌘E");
      keybindings.setShortcut("cmd2", "⌘F");
      keybindings.reset("cmd1");
      assert.equal(keybindings.get("cmd1"), undefined);
      assert.deepEqual(keybindings.get("cmd2"), { shortcut: "⌘F" });
    });

    it("persists removal to localStorage", () => {
      keybindings.setShortcut("cmd1", "⌘G");
      keybindings.setShortcut("cmd2", "⌘H");
      keybindings.reset("cmd1");
      const stored = localStorageMock.getItem("tesela:keybindings");
      assert.ok(stored);
      const parsed = JSON.parse(stored);
      assert.deepEqual(parsed, { cmd2: { shortcut: "⌘H" } });
    });

    it("is idempotent for non-existent command", () => {
      keybindings.setShortcut("cmd1", "⌘I");
      keybindings.reset("unknown-cmd");
      assert.deepEqual(keybindings.get("cmd1"), { shortcut: "⌘I" });
    });
  });

  describe("resetAll", () => {
    it("clears all overrides", () => {
      keybindings.setShortcut("cmd1", "⌘J");
      keybindings.setShortcut("cmd2", "⌘K");
      keybindings.setChord("cmd3", ["g"]);
      keybindings.resetAll();
      assert.equal(keybindings.get("cmd1"), undefined);
      assert.equal(keybindings.get("cmd2"), undefined);
      assert.equal(keybindings.get("cmd3"), undefined);
    });

    it("removes localStorage key", () => {
      keybindings.setShortcut("cmd1", "⌘L");
      keybindings.resetAll();
      const stored = localStorageMock.getItem("tesela:keybindings");
      assert.equal(stored, null);
    });

    it("is idempotent when already empty", () => {
      keybindings.resetAll();
      assert.equal(keybindings.get("cmd1"), undefined);
    });
  });

  describe("tri-state behavior", () => {
    it("absent key inherits compiled-in default (returns undefined)", () => {
      // No override set - should return undefined to indicate "use default"
      assert.equal(keybindings.get("cmd1"), undefined);
    });

    it("null value means explicitly unbound", () => {
      keybindings.setShortcut("cmd1", null);
      const override = keybindings.get("cmd1");
      assert.ok(override);
      assert.equal(override.shortcut, null);
      assert.ok("shortcut" in override);
    });

    it("string value means rebound", () => {
      keybindings.setShortcut("cmd1", "⌘M");
      const override = keybindings.get("cmd1");
      assert.ok(override);
      assert.equal(override.shortcut, "⌘M");
    });

    it("can distinguish absent from null", () => {
      // cmd1 has no override (absent)
      const absent = keybindings.get("cmd1");
      assert.equal(absent, undefined);

      // cmd2 has explicit null (unbound)
      keybindings.setShortcut("cmd2", null);
      const unbound = keybindings.get("cmd2");
      assert.ok(unbound);
      assert.equal(unbound.shortcut, null);
      assert.ok("shortcut" in unbound);

      // cmd3 has a value (rebound)
      keybindings.setShortcut("cmd3", "⌘N");
      const rebound = keybindings.get("cmd3");
      assert.ok(rebound);
      assert.equal(rebound.shortcut, "⌘N");
    });
  });

  describe("snapshot", () => {
    it("returns a plain object copy of all overrides", () => {
      keybindings.setShortcut("cmd1", "⌘O");
      keybindings.setChord("cmd2", ["h"]);
      const snap = keybindings.snapshot();
      assert.deepEqual(snap, {
        cmd1: { shortcut: "⌘O" },
        cmd2: { chord: ["h"] },
      });
    });

    it("returns empty object when no overrides", () => {
      const snap = keybindings.snapshot();
      assert.deepEqual(snap, {});
    });
  });
});
