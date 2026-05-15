/**
 * Currently focused block, tracked per pane.
 *
 * Originally a 2-slot model (left / right) for the legacy column-view
 * split. Prism v4 has an arbitrary number of editor panes, so the state
 * is now a `Map<paneId, ParsedBlock | null>`. Each BlockOutliner
 * publishes via its `onfocusedblockchange` callback; the surrounding
 * pane shell wires that callback to `setFocusedBlockForPane`.
 *
 * The legacy chrome still uses the left/right API — those functions are
 * kept as thin shims over reserved sentinel pane ids, so the column-view
 * split, bottom drawer, kanban, journal, and query-widget code paths are
 * untouched while both chromes coexist on the redesign-v4 branch.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import { getVSplitActiveSide } from "$lib/stores/pane-state.svelte";

/** Reserved pane ids backing the legacy left/right column-view split. */
const LEGACY_LEFT = "__legacy_left__";
const LEGACY_RIGHT = "__legacy_right__";

// Svelte 5 makes Map mutations (`set` / `delete`) reactive when the Map
// itself is `$state`-wrapped, so consumers re-run on every change.
const focusedByPane = $state(new Map<string, ParsedBlock | null>());

// ── v4 per-pane API ─────────────────────────────────────────────────────────

export function setFocusedBlockForPane(paneId: string, block: ParsedBlock | null) {
  focusedByPane.set(paneId, block);
}

export function getFocusedBlockForPane(paneId: string): ParsedBlock | null {
  return focusedByPane.get(paneId) ?? null;
}

/** Drop a pane's entry entirely — call when the pane unmounts. */
export function clearPaneFocusedBlock(paneId: string) {
  focusedByPane.delete(paneId);
}

// ── legacy left/right compat shims ──────────────────────────────────────────

export function getLeftFocusedBlock(): ParsedBlock | null {
  return focusedByPane.get(LEGACY_LEFT) ?? null;
}

export function getRightFocusedBlock(): ParsedBlock | null {
  return focusedByPane.get(LEGACY_RIGHT) ?? null;
}

export function setLeftFocusedBlock(block: ParsedBlock | null) {
  focusedByPane.set(LEGACY_LEFT, block);
}

export function setRightFocusedBlock(block: ParsedBlock | null) {
  focusedByPane.set(LEGACY_RIGHT, block);
}

/** The block the legacy bottom drawer should show — follows active side. */
export function getFocusedBlock(): ParsedBlock | null {
  const key = getVSplitActiveSide() === "right" ? LEGACY_RIGHT : LEGACY_LEFT;
  return focusedByPane.get(key) ?? null;
}

/** Legacy backwards-compat setter — writes whichever side is active. */
export function setFocusedBlock(block: ParsedBlock | null) {
  const key = getVSplitActiveSide() === "right" ? LEGACY_RIGHT : LEGACY_LEFT;
  focusedByPane.set(key, block);
}
