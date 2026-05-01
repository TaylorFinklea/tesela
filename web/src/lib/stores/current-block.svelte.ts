/**
 * Currently focused block in the active note's outliner.
 *
 * Phase 9.5b — per-side state for the column-view split. The drawer's
 * Properties / Outline tabs follow whichever side is active per
 * `pane-state.svelte`'s `vSplitActiveSide`. Single-pane state always sets
 * active = "right", so the drawer reads `rightFocusedBlock` in that case.
 *
 * BlockOutliner publishes via its `onfocusedblockchange` callback; the note
 * page wires the callback to the side-specific setter
 * (`setLeftFocusedBlock` / `setRightFocusedBlock`).
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import { getVSplitActiveSide } from "$lib/stores/pane-state.svelte";

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

/** The block the bottom drawer should show — follows active side. */
export function getFocusedBlock(): ParsedBlock | null {
  if (getVSplitActiveSide() === "right") return rightFocusedBlock;
  return leftFocusedBlock;
}

/** Backwards-compat setter — writes whichever side is active. */
export function setFocusedBlock(block: ParsedBlock | null) {
  if (getVSplitActiveSide() === "right") {
    rightFocusedBlock = block;
  } else {
    leftFocusedBlock = block;
  }
}
