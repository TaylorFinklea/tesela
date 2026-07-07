/**
 * tesela-ya4.3 — pure column resolution for the generalized query table
 * (spec gap G4: "columns = resolved type properties"). Mirrors the
 * group-by candidate logic `kanban-group-by.ts` established for kanban
 * (decision 3c): a tag-scoped table uses the TYPE's own declared property
 * order; a non-tag-scoped query has no single type to enumerate, so
 * columns are the global properties that actually appear on ≥1 returned
 * block. Extracted so the resolution rule is unit-testable without
 * mounting `QueryTable.svelte`.
 */

export interface TableColumnCandidate {
  name: string;
  value_type: string;
  values?: string[] | null;
}

export function resolveTableColumns(params: {
  /** Non-null when the table's DSL is tag-scoped (first positive `tag:X`
   *  filter) — the type's own declared property order wins, same as
   *  Kanban's group-by candidates. */
  tagName: string | null;
  /** The tag-scoped type's declared properties, in the type's own order.
   *  Ignored when `tagName` is null. */
  typeProperties: TableColumnCandidate[];
  /** Every known Property-page definition — the candidate source for a
   *  non-tag-scoped query (no type to enumerate). */
  globalProperties: TableColumnCandidate[];
  /** Lowercased property keys actually present on ≥1 returned block. An
   *  irrelevant global property would be worse than a tight, honest
   *  column set (mirrors kanban's decision-3(c) rationale). */
  presentKeys: ReadonlySet<string>;
}): TableColumnCandidate[] {
  const { tagName, typeProperties, globalProperties, presentKeys } = params;
  if (tagName) return typeProperties;
  return globalProperties.filter((p) => presentKeys.has(p.name.toLowerCase()));
}
