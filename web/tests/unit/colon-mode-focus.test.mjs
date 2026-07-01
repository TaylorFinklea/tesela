import assert from "node:assert/strict";
import test from "node:test";

// Mock $state for Node.js test environment (Svelte 5 rune)
globalThis.$state = (initial) => initial;

// Import the colon-mode store
const {
  isColonModeOpen,
  getColonPriorPaneId,
  openColonMode,
  closeColonMode,
} = await import("../../src/lib/stores/colon-mode.svelte.ts");

test("colon-mode: priorPaneId is stored and retrieved", () => {
  closeColonMode();
  const testPaneId = "pane-123";
  openColonMode({ priorPaneId: testPaneId });
  assert.equal(getColonPriorPaneId(), testPaneId, "should store prior pane ID");
  assert.equal(isColonModeOpen(), true, "mode should be open");
});

test("colon-mode: closeColonMode preserves priorPaneId for focus restoration", () => {
  const testPaneId = "pane-456";
  openColonMode({ priorPaneId: testPaneId });
  assert.equal(isColonModeOpen(), true, "mode should be open");

  closeColonMode();

  assert.equal(isColonModeOpen(), false, "mode should be closed");
  // Key assertion: priorPaneId should still be available after close
  // so that restoreFocus() can call focusLeaf(priorPaneId)
  assert.equal(
    getColonPriorPaneId(),
    testPaneId,
    "priorPaneId should still be available after close for restoreFocus() call"
  );
});

test("colon-mode: open without priorPaneId", () => {
  closeColonMode();
  openColonMode(); // no priorPaneId
  assert.equal(isColonModeOpen(), true, "mode should be open");
  assert.equal(getColonPriorPaneId(), undefined, "priorPaneId should be undefined");
});
