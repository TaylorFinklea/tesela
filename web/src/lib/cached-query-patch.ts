/**
 * Pure cache-shape-agnostic property patch used by `property-update.ts`'s
 * optimistic-update step. Extracted (no `$lib`/`@tanstack` imports) so the
 * shape-branching logic — the actual root cause of a tesela-ya4.1 fix-round
 * regression — is unit-testable without mounting a component or resolving
 * SvelteKit's `$lib` alias (mirrors why `kanban-group-by.ts` was extracted).
 *
 * `queryKey` callers currently point at one of two shapes:
 *  - `ParsedBlock[]` — the tag-page `getTypedBlocks` cache.
 *  - `QueryResult` (`{ groups: [{ items: QueryItem[] }] }`) — KanbanBoard's
 *    and QueryTable's generalized `executeQuery`-backed sources
 *    (tesela-ya4.1/ya4.3): the RAW `api.executeQuery` response, never a
 *    flat array.
 *
 * Assuming every cache was the flat-array shape threw
 * `TypeError: previousBlocks.map is not a function` for every kanban card
 * move (drag-drop, `m` picker, `H`/`L`) once KanbanBoard started pointing
 * `queryKey` at the raw `QueryResult` cache — the write was caught by the
 * caller's try/catch and silently logged, never reaching
 * `api.setBlockProperty`/`clearBlockProperty`. `patchCachedProperty` below
 * duck-types on `Array.isArray` and patches each shape via its own matching
 * key (`ParsedBlock.id` vs `QueryItem.block_id`), so one shared patch path
 * works for both instead of assuming a single shape.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import type { QueryResult } from "$lib/types/QueryResult";

export type CachedQueryData = ParsedBlock[] | QueryResult;

export function isQueryResult(data: CachedQueryData): data is QueryResult {
  return !Array.isArray(data);
}

/**
 * Strip every case-variant of `key` from a properties map, returning a new map.
 * The on-disk parser preserves key case (`crates/tesela-core/src/block.rs`
 * `extract_properties` does not lowercase), so a block can carry `Status` while
 * we write the canonical lowercase `status`. Removing all variants first keeps
 * the optimistic value authoritative for the chip read, which does
 * `properties[name] ?? properties[name.toLowerCase()]`.
 */
export function withoutKey(
  props: Record<string, string>,
  key: string,
): Record<string, string> {
  const lower = key.toLowerCase();
  return Object.fromEntries(
    Object.entries(props).filter(([k]) => k.toLowerCase() !== lower),
  );
}

/** Apply a property set (`value` non-null) or clear (`value === null`) to
 *  the block/item matching `blockId`, preserving the cache's own shape. */
export function patchCachedProperty(
  data: CachedQueryData,
  blockId: string,
  key: string,
  value: string | null,
): CachedQueryData {
  const applyTo = (props: Record<string, string>) =>
    value === null
      ? withoutKey(props, key)
      : { ...withoutKey(props, key), [key]: value };

  if (isQueryResult(data)) {
    return {
      groups: data.groups.map((g) => ({
        ...g,
        items: g.items.map((item) =>
          item.block_id === blockId
            ? { ...item, properties: applyTo(item.properties) }
            : item,
        ),
      })),
    };
  }

  return data.map((b) =>
    b.id === blockId ? { ...b, properties: applyTo(b.properties) } : b,
  );
}
