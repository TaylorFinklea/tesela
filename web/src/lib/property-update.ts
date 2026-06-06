/**
 * Shared utility for updating block properties in note content.
 * Used by TagTable and KanbanBoard.
 */
import { api } from "$lib/api-client";
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import type { QueryClient } from "@tanstack/svelte-query";

/**
 * Strip every case-variant of `key` from a properties map, returning a new map.
 * The on-disk parser preserves key case (`crates/tesela-core/src/block.rs`
 * `extract_properties` does not lowercase), so a block can carry `Status` while
 * we write the canonical lowercase `status`. Removing all variants first keeps
 * the optimistic value authoritative for the chip read, which does
 * `properties[name] ?? properties[name.toLowerCase()]`.
 */
function withoutKey(
  props: Record<string, string>,
  key: string,
): Record<string, string> {
  const lower = key.toLowerCase();
  return Object.fromEntries(
    Object.entries(props).filter(([k]) => k.toLowerCase() !== lower),
  );
}

/**
 * Upsert a single `key:: value` property on a block via the block-granular
 * `/blocks/set-property` endpoint. Optimistically patches the
 * `["typed-blocks", tag]` cache so the chip flips BEFORE the round-trip
 * (mirrors the editor's structured funnel), then invalidates to reconcile with
 * the server-canonical value; rolls back the cache on a failed write.
 *
 * Addresses by the stale-proof `<note_id>:<bid>` when the block has a bid (the
 * line-id goes stale on a note reflow); the routes accept either form (699041b).
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
  const addr = block.bid ? `${block.note_id}:${block.bid}` : block.id;

  // Snapshot for rollback.
  const previousBlocks = queryClient.getQueryData<ParsedBlock[]>([
    "typed-blocks",
    tagName,
  ]);

  // Optimistic patch BEFORE the round-trip so the chip flips immediately.
  if (previousBlocks) {
    const optimistic = previousBlocks.map((b) =>
      b.id === block.id
        ? { ...b, properties: { ...withoutKey(b.properties, key), [key]: value } }
        : b,
    );
    queryClient.setQueryData(["typed-blocks", tagName], optimistic);
  }

  try {
    await api.setBlockProperty(addr, key, value);
    // Reconcile: discard the optimistic value and refetch server-canonical.
    // Idempotent — the refetched block sets the same key to the same value, so
    // there's no double-count.
    queryClient.invalidateQueries({ queryKey: ["typed-blocks", tagName] });
    queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
  } catch (err) {
    // Rollback: restore the pre-edit cache so the chip doesn't keep a lie.
    if (previousBlocks) {
      queryClient.setQueryData(["typed-blocks", tagName], previousBlocks);
    }
    throw err;
  }
}

/**
 * Remove a property line from a block via the block-granular
 * `/blocks/clear-property` endpoint, with the same optimistic cache patch +
 * rollback as `updateBlockProperty`.
 */
export async function clearBlockProperty(params: {
  block: ParsedBlock;
  propKey: string;
  tagName: string;
  queryClient: QueryClient;
}): Promise<void> {
  const { block, propKey, tagName, queryClient } = params;
  const key = propKey.toLowerCase();
  const addr = block.bid ? `${block.note_id}:${block.bid}` : block.id;

  const previousBlocks = queryClient.getQueryData<ParsedBlock[]>([
    "typed-blocks",
    tagName,
  ]);

  if (previousBlocks) {
    const optimistic = previousBlocks.map((b) =>
      b.id === block.id
        ? { ...b, properties: withoutKey(b.properties, key) }
        : b,
    );
    queryClient.setQueryData(["typed-blocks", tagName], optimistic);
  }

  try {
    await api.clearBlockProperty(addr, key);
    queryClient.invalidateQueries({ queryKey: ["typed-blocks", tagName] });
    queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
  } catch (err) {
    if (previousBlocks) {
      queryClient.setQueryData(["typed-blocks", tagName], previousBlocks);
    }
    throw err;
  }
}
