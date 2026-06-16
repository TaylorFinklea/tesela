/**
 * Phase C — pure slash-menu type-to-filter matcher.
 *
 * Thin wrapper over `scoreFuzzy` from `$lib/fuzzy`. Used by `ChordMenu` when
 * `filterMode` is on (the slash menu). Input is the flat list of rows at the
 * current breadcrumb level; output is the matched + ranked subset.
 *
 *   - Empty query → return items unchanged (full list, original order).
 *   - Non-empty query → keep `score > 0`, sort by score desc with stable
 *     tie-break on original index (so equal-score rows keep tree order).
 *
 * Pure, no Svelte, no DOM.
 */

import { scoreFuzzy } from "../fuzzy.ts";

export type SlashFilterItem = { label: string };

export function slashFilter<T extends SlashFilterItem>(items: T[], query: string): T[] {
  const q = query.trim();
  if (!q) return [...items];
  return items
    .map((item, i) => ({ item, i, score: scoreFuzzy(item.label, q).score }))
    .filter((r) => r.score > 0)
    .sort((a, b) => b.score - a.score || a.i - b.i)
    .map((r) => r.item);
}
