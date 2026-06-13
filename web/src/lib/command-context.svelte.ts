/**
 * Reactive command context for the unified command registry.
 *
 * B3 (2026-06-13): exposes the current editor/pane/route state so the
 * registry can filter commands by `when` predicates.
 */

import { page } from "$app/stores";
import { get } from "svelte/store";
import type { CommandContext } from "$lib/command-registry.svelte";
import { getFocusedBuffer, getActiveTab } from "$lib/buffer/state.svelte";
import { getVimMode } from "$lib/stores/pane-state.svelte";
import { getFocusedBlock } from "$lib/stores/current-block.svelte";

export function getCommandContext(): CommandContext {
  const route = get(page)?.route?.id;
  const buffer = getFocusedBuffer();
  const tab = getActiveTab();
  const block = getFocusedBlock();

  return {
    route,
    bufferKind: buffer?.kind ?? null,
    vimMode: getVimMode(),
    focusedBlock: block
      ? { id: block.id, properties: block.properties ?? {} }
      : null,
    splitOpen: tab?.layout?.type === "split",
  };
}
