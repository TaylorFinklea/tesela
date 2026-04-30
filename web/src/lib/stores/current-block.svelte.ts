/**
 * Currently focused block in the active note's outliner.
 *
 * Phase 9.5 — bifurcated into per-side state for the vertical split. Callers
 * that don't care about which side is active should use `getFocusedBlock()`
 * (alias for `getActiveFocusedBlock`); callers that need to write should use
 * `setFocusedBlock` (writes the side that's currently active per
 * `pane-state.svelte`).
 *
 * The bottom drawer reads `getFocusedBlock()` so its Properties / Outline
 * tabs follow the active outliner side. BlockOutliner publishes via its
 * `onfocusedblockchange` callback; the note page wires the callback to the
 * side-specific setter (`setLeftFocusedBlock` / `setRightFocusedBlock`).
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import { getVSplitActiveSide, isVSplitOpen } from "$lib/stores/pane-state.svelte";

let leftFocusedBlock = $state<ParsedBlock | null>(null);
let rightFocusedBlock = $state<ParsedBlock | null>(null);

export function getLeftFocusedBlock(): ParsedBlock | null {
  return leftFocusedBlock;
}

export function getRightFocusedBlock(): ParsedBlock | null {
  return rightFocusedBlock;
}

export function setLeftFocusedBlock(block: ParsedBlock | null) {
  leftFocusedBlock = block;
}

export function setRightFocusedBlock(block: ParsedBlock | null) {
  rightFocusedBlock = block;
}

/**
 * The block that the bottom drawer should show. When the vsplit is closed (or
 * left side active), returns the left side. When right is active, returns
 * the right side.
 */
export function getFocusedBlock(): ParsedBlock | null {
  if (isVSplitOpen() && getVSplitActiveSide() === "right") return rightFocusedBlock;
  return leftFocusedBlock;
}

/**
 * Backwards-compatible single-side setter. Writes whichever side is active.
 * Existing callers (pre-9.5) keep working without changes.
 */
export function setFocusedBlock(block: ParsedBlock | null) {
  if (isVSplitOpen() && getVSplitActiveSide() === "right") {
    rightFocusedBlock = block;
  } else {
    leftFocusedBlock = block;
  }
}
