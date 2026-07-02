/**
 * tesela-ya4.1 — pure implementation of the kanban group-by resolution
 * order locked by decision 3 in
 * `.docs/ai/phases/2026-07-02-typesystem-views-spec.md`:
 *
 *   (a) explicit `display_group_by` on the active saved view
 *   (b) per-surface localStorage pref
 *   (c) first select property with ≥1 choice
 *   (d) honest empty state ("" — never a silent list fallback)
 *
 * Extracted from `KanbanBoard.svelte` so the resolution ORDER — the
 * acceptance contract — is unit-testable without mounting a Svelte
 * component.
 */

export interface GroupByCandidate {
  name: string;
  value_type: string;
  values: string[] | null | undefined;
}

/** A property is a valid kanban group-by column source only if it's
 *  select-typed AND has at least one declared choice — an empty/undeclared
 *  `values` list has nothing to build columns from. */
export function isSelectWithChoices(p: GroupByCandidate): boolean {
  return p.value_type === "select" && Array.isArray(p.values) && p.values.length > 0;
}

export function resolveKanbanGroupBy(params: {
  /** (a) — the active saved view's `display_group_by`, or `null` outside a
   *  saved-view context (a plain tag-page / Query-note widget). */
  displayGroupBy: string | null;
  /** (b) — the per-surface localStorage pref, or `null`/empty if unset. */
  storedPref: string | null;
  /** (c) candidates, in priority order — the type's own declared property
   *  order for a tag-scoped board, or the data-derived order (global
   *  select properties actually present on the returned blocks) for a
   *  non-tag-scoped query. */
  candidates: GroupByCandidate[];
  /** Resolves ANY property name (not just a `candidates` member) to its
   *  def — an (a)/(b) override must be honored even when it isn't in the
   *  (c) candidate list (decision 3a/3b outrank "does the data have it").
   *  Returns `undefined` for a name that doesn't exist or isn't
   *  select-type-with-choices. */
  resolveDef: (name: string) => GroupByCandidate | undefined;
}): string {
  const { displayGroupBy, storedPref, candidates, resolveDef } = params;
  if (displayGroupBy && resolveDef(displayGroupBy)) return displayGroupBy;
  if (storedPref && resolveDef(storedPref)) return storedPref;
  return candidates[0]?.name ?? "";
}
