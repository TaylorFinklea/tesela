/**
 * Shared utility for updating block properties in note content.
 * Used by TagTable and KanbanBoard.
 */
import { api } from "$lib/api-client";
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import type { QueryClient, QueryKey } from "@tanstack/svelte-query";
import {
  patchCachedProperty,
  type CachedQueryData,
} from "$lib/cached-query-patch";

/**
 * Upsert a single `key:: value` property on a block via the block-granular
 * `/blocks/set-property` endpoint. Optimistically patches the caller's
 * `queryKey` cache (a `ParsedBlock[]` or a `QueryResult`, see
 * `CachedQueryData`) so the chip flips BEFORE the round-trip (mirrors the
 * editor's structured funnel), then invalidates to reconcile with the
 * server-canonical value; rolls back the cache on a failed write.
 *
 * `queryKey` is caller-supplied (not hardcoded to `["typed-blocks", tag]`)
 * so callers whose cache comes from a different source — e.g. KanbanBoard's
 * generalized `executeQuery`-backed board (tesela-ya4.1) — can still get the
 * optimistic patch/rollback against their own cache.
 *
 * Addresses by the stale-proof `<note_id>:<bid>` when the block has a bid (the
 * line-id goes stale on a note reflow); the routes accept either form (699041b).
 */
export async function updateBlockProperty(params: {
  block: ParsedBlock;
  propKey: string;
  value: string;
  queryKey: QueryKey;
  queryClient: QueryClient;
}): Promise<void> {
  const { block, propKey, value, queryKey, queryClient } = params;
  const key = propKey.toLowerCase();
  const addr = block.bid ? `${block.note_id}:${block.bid}` : block.id;

  // Snapshot for rollback.
  const previousData = queryClient.getQueryData<CachedQueryData>(queryKey);

  // Optimistic patch BEFORE the round-trip so the chip flips immediately.
  if (previousData) {
    queryClient.setQueryData(
      queryKey,
      patchCachedProperty(previousData, block.id, key, value),
    );
  }

  try {
    await api.setBlockProperty(addr, key, value);
    // Reconcile: discard the optimistic value and refetch server-canonical.
    // Idempotent — the refetched block sets the same key to the same value, so
    // there's no double-count.
    queryClient.invalidateQueries({ queryKey });
    queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
  } catch (err) {
    // Rollback: restore the pre-edit cache so the chip doesn't keep a lie.
    if (previousData) {
      queryClient.setQueryData(queryKey, previousData);
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
  queryKey: QueryKey;
  queryClient: QueryClient;
}): Promise<void> {
  const { block, propKey, queryKey, queryClient } = params;
  const key = propKey.toLowerCase();
  const addr = block.bid ? `${block.note_id}:${block.bid}` : block.id;

  const previousData = queryClient.getQueryData<CachedQueryData>(queryKey);

  if (previousData) {
    queryClient.setQueryData(
      queryKey,
      patchCachedProperty(previousData, block.id, key, null),
    );
  }

  try {
    await api.clearBlockProperty(addr, key);
    queryClient.invalidateQueries({ queryKey });
    queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
  } catch (err) {
    if (previousData) {
      queryClient.setQueryData(queryKey, previousData);
    }
    throw err;
  }
}
