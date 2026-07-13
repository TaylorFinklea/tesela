import assert from "node:assert/strict";
import test from "node:test";

import {
  CLOSED_PALETTE_COMMAND_CONTEXT,
  transitionPaletteCommandContext,
} from "../../src/lib/graphite/shell/palette-command-context.ts";

test("palette keeps its opening editor context through blur and resets on close/reopen", () => {
  const opening = {
    bufferKind: "page",
    editorFocused: true,
    focusedBlock: { id: "source", properties: {} },
  };
  const blurred = {
    ...opening,
    editorFocused: false,
  };

  let state = transitionPaletteCommandContext(
    CLOSED_PALETTE_COMMAND_CONTEXT,
    true,
    opening,
  );
  assert.notStrictEqual(state.context, opening, "opening context must be snapshotted");
  assert.equal(state.context?.editorFocused, true);

  state = transitionPaletteCommandContext(state, true, blurred);
  assert.equal(state.context?.editorFocused, true, "palette input blur must not change availability");
  assert.equal(state.context?.focusedBlock?.id, "source");

  state = transitionPaletteCommandContext(state, false, blurred);
  assert.equal(state.context, null, "closing clears the execution context");

  state = transitionPaletteCommandContext(state, true, blurred);
  assert.equal(state.context?.editorFocused, false, "reopen captures the new context");
  assert.equal(state.context?.focusedBlock?.id, "source");
});
