/**
 * tesela-ya4.4 — pure table column display config: hide / reorder / sort,
 * persisted per spec gap G5 (decision 4's write-back contract, mirrored
 * from `display_group_by`). Extracted so the projection/reorder/sort
 * mutation rules are unit-testable without mounting `QueryTable.svelte`
 * (mirrors why `table-columns.ts`/`table-sort.ts` were extracted).
 *
 * Mirrors `tesela_sync::TableColumnConfig`'s serde shape verbatim (flat
 * snake_case fields) so a `ViewRecord.display_table_config` payload from
 * `GET /views` / `updateView` deserializes directly into this type with no
 * translation layer.
 */
import type { TableColumnCandidate } from "./table-columns";
import type { SortDirection } from "./table-sort";

export interface TableColumnConfig {
  /** Property names hidden from the table. */
  hidden: string[];
  /** Explicit column display order (property names). Columns not
   *  mentioned here render after the ordered ones, in their naturally
   *  resolved order. */
  order: string[];
  /** Property name currently sorted by, if any. */
  sort_by: string | null;
  /** Sort direction; only meaningful when `sort_by` is set. */
  sort_dir: SortDirection | null;
}

/** The "no override" config — every saved view / tag page starts here. */
export const EMPTY_TABLE_CONFIG: TableColumnConfig = {
  hidden: [],
  order: [],
  sort_by: null,
  sort_dir: null,
};

/** Project `columns` (already resolved by `resolveTableColumns`) through a
 *  `TableColumnConfig`: drop hidden columns, then apply the explicit
 *  `order` override. Columns not named in `order` (new columns since the
 *  config was last saved, or an empty `order`) append after the ordered
 *  ones, in their originally-resolved order — so a stale/partial config
 *  never HIDES a column it doesn't mention, only reorders what it names. */
export function applyTableConfig(
  columns: TableColumnCandidate[],
  config: TableColumnConfig,
): TableColumnCandidate[] {
  const visible = columns.filter((c) => !config.hidden.includes(c.name));
  if (config.order.length === 0) return visible;

  const byName = new Map(visible.map((c) => [c.name, c] as const));
  const ordered: TableColumnCandidate[] = [];
  for (const name of config.order) {
    const c = byName.get(name);
    if (c) {
      ordered.push(c);
      byName.delete(name);
    }
  }
  for (const c of visible) {
    if (byName.has(c.name)) ordered.push(c);
  }
  return ordered;
}

/** Toggle a column's hidden state (immutable — returns a NEW config). */
export function toggleColumnHidden(config: TableColumnConfig, name: string): TableColumnConfig {
  const hidden = config.hidden.includes(name)
    ? config.hidden.filter((n) => n !== name)
    : [...config.hidden, name];
  return { ...config, hidden };
}

/** Move `name` one slot left/right within `visibleColumnNames` (the
 *  CURRENTLY rendered, post-`applyTableConfig` column-name order), and
 *  return the resulting order as the config's new explicit `order`
 *  override. A no-op (returns the input unchanged) when `name` isn't
 *  found or is already at the boundary in that direction — there is
 *  nowhere to move it. */
export function moveColumnInConfig(
  visibleColumnNames: string[],
  name: string,
  direction: "left" | "right",
): string[] {
  const idx = visibleColumnNames.indexOf(name);
  if (idx < 0) return visibleColumnNames;
  const targetIdx = direction === "left" ? idx - 1 : idx + 1;
  if (targetIdx < 0 || targetIdx >= visibleColumnNames.length) return visibleColumnNames;
  const next = [...visibleColumnNames];
  [next[idx], next[targetIdx]] = [next[targetIdx], next[idx]];
  return next;
}

/** Sort-column toggle mirroring `QueryTable`'s old local-`$state` logic
 *  (ya4.3), now folded into the persisted config: clicking/`s`-ing the
 *  currently-sorted column flips direction; picking a new column resets
 *  to ascending. */
export function toggleSortInConfig(config: TableColumnConfig, name: string): TableColumnConfig {
  if (config.sort_by === name) {
    return { ...config, sort_dir: config.sort_dir === "asc" ? "desc" : "asc" };
  }
  return { ...config, sort_by: name, sort_dir: "asc" };
}
