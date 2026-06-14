import { describe, it } from "node:test";
import assert from "node:assert/strict";

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

import { eventToShortcutGlyph } from "../../src/lib/shortcut-glyph.ts";

describe("eventToShortcutGlyph", () => {
  // Test all shortcuts from commands.ts
  it("normalizes ⌘\\ (vsplit)", () => {
    const e = new KeyboardEvent("keydown", { key: "\\", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘\\");
  });

  it("normalizes ⌘- (hsplit)", () => {
    const e = new KeyboardEvent("keydown", { key: "-", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘-");
  });

  it("normalizes ⌘⇧H (move-left)", () => {
    const e = new KeyboardEvent("keydown", { key: "H", metaKey: true, shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘⇧H");
  });

  it("normalizes ⌘⇧L (move-right)", () => {
    const e = new KeyboardEvent("keydown", { key: "L", metaKey: true, shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘⇧L");
  });

  it("normalizes ⌘⇧K (move-up)", () => {
    const e = new KeyboardEvent("keydown", { key: "K", metaKey: true, shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘⇧K");
  });

  it("normalizes ⌘⇧J (move-down)", () => {
    const e = new KeyboardEvent("keydown", { key: "J", metaKey: true, shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘⇧J");
  });

  it("normalizes ⌘I (peek)", () => {
    const e = new KeyboardEvent("keydown", { key: "i", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘I");
  });

  it("normalizes ⌘G (fullscreen-graph)", () => {
    const e = new KeyboardEvent("keydown", { key: "g", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘G");
  });

  it("normalizes ⌘K (command-station)", () => {
    const e = new KeyboardEvent("keydown", { key: "k", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘K");
  });

  // Test modifier key combinations
  it("normalizes lowercase letters to uppercase", () => {
    const e = new KeyboardEvent("keydown", { key: "a", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘A");
  });

  it("normalizes uppercase letters with shift", () => {
    const e = new KeyboardEvent("keydown", { key: "A", metaKey: true, shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘⇧A");
  });

  it("normalizes alt key (⌥)", () => {
    const e = new KeyboardEvent("keydown", { key: "a", altKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌥A");
  });

  it("normalizes ctrl key (⌃)", () => {
    const e = new KeyboardEvent("keydown", { key: "a", ctrlKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌃A");
  });

  it("normalizes multiple modifiers in correct order (⌃⌥⌘⇧)", () => {
    const e = new KeyboardEvent("keydown", { 
      key: "A", 
      ctrlKey: true, 
      altKey: true, 
      metaKey: true, 
      shiftKey: true 
    });
    assert.equal(eventToShortcutGlyph(e), "⌃⌥⌘⇧A");
  });

  it("normalizes ⌃⌘ combination", () => {
    const e = new KeyboardEvent("keydown", { key: "a", ctrlKey: true, metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌃⌘A");
  });

  it("normalizes ⌥⇧ combination", () => {
    const e = new KeyboardEvent("keydown", { key: "a", altKey: true, shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌥⇧A");
  });

  // Test edge cases
  it("returns null for events without modifier keys", () => {
    const e = new KeyboardEvent("keydown", { key: "a" });
    assert.equal(eventToShortcutGlyph(e), null);
  });

  it("returns null for modifier-only key presses (Control)", () => {
    const e = new KeyboardEvent("keydown", { key: "Control", ctrlKey: true });
    assert.equal(eventToShortcutGlyph(e), null);
  });

  it("returns null for modifier-only key presses (Meta)", () => {
    const e = new KeyboardEvent("keydown", { key: "Meta", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), null);
  });

  it("returns null for modifier-only key presses (Alt)", () => {
    const e = new KeyboardEvent("keydown", { key: "Alt", altKey: true });
    assert.equal(eventToShortcutGlyph(e), null);
  });

  it("returns null for modifier-only key presses (Shift)", () => {
    const e = new KeyboardEvent("keydown", { key: "Shift", shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), null);
  });

  it("returns null for shift-only key presses", () => {
    const e = new KeyboardEvent("keydown", { key: "A", shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), null);
  });

  // Test special characters
  it("preserves backslash character", () => {
    const e = new KeyboardEvent("keydown", { key: "\\", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘\\");
  });

  it("preserves hyphen character", () => {
    const e = new KeyboardEvent("keydown", { key: "-", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘-");
  });

  // Test browser-reserved keys (should still normalize correctly)
  it("normalizes ⌘T (browser reserved)", () => {
    const e = new KeyboardEvent("keydown", { key: "t", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘T");
  });

  it("normalizes ⌘W (browser reserved)", () => {
    const e = new KeyboardEvent("keydown", { key: "w", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘W");
  });

  it("normalizes ⌘⇧W (browser reserved)", () => {
    const e = new KeyboardEvent("keydown", { key: "W", metaKey: true, shiftKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘⇧W");
  });

  it("normalizes ⌘N (browser reserved)", () => {
    const e = new KeyboardEvent("keydown", { key: "n", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘N");
  });

  it("normalizes ⌘Q (browser reserved)", () => {
    const e = new KeyboardEvent("keydown", { key: "q", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘Q");
  });

  it("normalizes ⌘R (browser reserved)", () => {
    const e = new KeyboardEvent("keydown", { key: "r", metaKey: true });
    assert.equal(eventToShortcutGlyph(e), "⌘R");
  });
});
