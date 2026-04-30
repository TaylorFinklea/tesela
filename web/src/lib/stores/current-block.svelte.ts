/**
 * Currently focused block in the active note's outliner.
 *
 * The bottom drawer reads this to drive its Outline / Properties tabs without
 * prop-drilling through the layout. BlockOutliner publishes via its
 * `onfocusedblockchange` callback; the note page wires that callback to
 * `setFocusedBlock`.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";

let focusedBlock = $state<ParsedBlock | null>(null);

export function getFocusedBlock(): ParsedBlock | null {
  return focusedBlock;
}

export function setFocusedBlock(block: ParsedBlock | null) {
  focusedBlock = block;
}
