/**
 * Shared utility for updating block properties in note content.
 * Used by TagTable and KanbanBoard.
 */
import { api } from "$lib/api-client";
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import type { QueryClient } from "@tanstack/svelte-query";

/**
 * Upsert a single `key:: value` property on a block via the block-granular
 * `/blocks/set-property` endpoint. The server locates the block by its
 * `<note_id>:<line>` id and rewrites only that one property line — no
 * whole-note PUT, so a concurrent peer edit to a sibling block survives.
 *
 * `block.id` is `<note_id>:<line>` (per `ParsedBlock`), which is exactly the
 * `block_id` the endpoint expects.
 */
export async function updateBlockProperty(params: {
  block: ParsedBlock;
  propKey: string;
  value: string;
  tagName: string;
  queryClient: QueryClient;
}): Promise<void> {
  const { block, propKey, value, tagName, queryClient } = params;
  const key = propKey.toLowerCase();

  await api.setBlockProperty(block.id, key, value);
  queryClient.invalidateQueries({ queryKey: ["typed-blocks", tagName] });
  queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
}

/**
 * Remove a property line from a block (for clearing/unsetting a value) via the
 * block-granular `/blocks/clear-property` endpoint. Same single-block rewrite
 * as `updateBlockProperty` — no whole-note PUT.
 */
export async function clearBlockProperty(params: {
  block: ParsedBlock;
  propKey: string;
  tagName: string;
  queryClient: QueryClient;
}): Promise<void> {
  const { block, propKey, tagName, queryClient } = params;
  const key = propKey.toLowerCase();

  await api.clearBlockProperty(block.id, key);
  queryClient.invalidateQueries({ queryKey: ["typed-blocks", tagName] });
  queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
}
