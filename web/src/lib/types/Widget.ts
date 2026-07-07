/**
 * Hand-written type (not from ts-rs because widgets aren't a backend concept —
 * they're a parsed view of `note_type: Query` notes' frontmatter).
 */
import type { TableColumnConfig } from "$lib/table/table-config";

export type WidgetSection = "pinned" | "browse" | "saved";

/**
 * A rail widget. Derived from a Query note's frontmatter properties.
 */
export type Widget = {
  /** Note id (path-slug) */
  id: string;
  /** Note title — the rail label */
  title: string;
  /** DSL string from `query::` frontmatter; empty for "system" non-query widgets */
  query: string;
  /** Property/metadata key to group by; null for ungrouped */
  group: string | null;
  /** Sort spec: `key [asc|desc] (, key [asc|desc])*`; null for default */
  sort: string | null;
  /** Glyph kind hint — drives default color when `color` is unset */
  icon: string | null;
  /** Explicit color override (matches v9 palette name: rose / indigo / plum / sage / teal / amber / amber-2 / ochre) */
  color: string | null;
  /** Which rail section this widget belongs to */
  section: WidgetSection;
  /**
   * Phase 11 — view mode hint from the Query note's `view::` directive.
   * `"kanban"` flips the QWV render to a board grouped by status (or
   * whatever the query's `group::` resolves to). `null` (or any other
   * value) renders the default grouped row list.
   */
  view: string | null;
  /**
   * Marker for system-defined widgets that should be re-created on first load
   * if the user deleted them. User-authored widgets have `system: false`.
   */
  system: boolean;
  /**
   * ya4.1 — set to the saved view's id when this widget is a synthetic
   * table/kanban mount OVER a saved view (`GrInbox`'s `modeWidget`).
   * `undefined`/`null` for a plain Query-note widget. Distinguishes
   * "explicit `display_group_by` on the active saved view" (group-by
   * resolution decision 3a, persists via `updateView`) from a Query note's
   * `group::` frontmatter, which kanban does not treat as an override
   * (unchanged from before this bead).
   */
  viewId?: string | null;
  /**
   * tesela-ya4.4 — the saved view's `display_table_config` (hide/reorder/
   * sort). Like `group`, only meaningful when `viewId` marks this widget as
   * a saved-view mount; `QueryTable` ignores it otherwise (a plain
   * Query-note widget / tag page uses its own localStorage config).
   */
  tableConfig?: TableColumnConfig | null;
};
