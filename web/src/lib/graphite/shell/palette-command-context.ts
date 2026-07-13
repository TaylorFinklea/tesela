import type { CommandContext } from "$lib/command-registry.svelte";

export type PaletteCommandContextState = Readonly<{
  open: boolean;
  context: CommandContext | null;
}>;

export const CLOSED_PALETTE_COMMAND_CONTEXT: PaletteCommandContextState = {
  open: false,
  context: null,
};

function snapshotCommandContext(context: CommandContext): CommandContext {
  return {
    ...context,
    focusedBlock: context.focusedBlock
      ? {
          ...context.focusedBlock,
          properties: { ...context.focusedBlock.properties },
        }
      : context.focusedBlock,
  };
}

export function transitionPaletteCommandContext(
  state: PaletteCommandContextState,
  open: boolean,
  currentContext: CommandContext,
): PaletteCommandContextState {
  if (!open) {
    return state.open ? CLOSED_PALETTE_COMMAND_CONTEXT : state;
  }
  if (state.open) return state;
  return {
    open: true,
    context: snapshotCommandContext(currentContext),
  };
}
