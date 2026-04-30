/**
 * Hand-written type (not from ts-rs because widgets aren't a backend concept —
 * they're a parsed view of `note_type: Query` notes' frontmatter).
 */
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
   * Marker for system-defined widgets that should be re-created on first load
   * if the user deleted them. User-authored widgets have `system: false`.
   */
  system: boolean;
};
