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

/** A node in a slash/chord tree: a label, optional children to recurse into. */
export type FlattenableNode = { label: string; children?: FlattenableNode[] };
/** A flattened tree node carrying its ancestor path + a "Path › Label" string. */
export type FlatMatch<T> = { node: T; path: string[]; fullLabel: string };

/**
 * Recursively flatten a slash tree, emitting BOTH each group node and every
 * descendant leaf, each with its ancestor path and a `Path › Label` fullLabel.
 * So a value buried two levels deep (Properties › Priority › p1) becomes a
 * single flat entry that a query can match directly.
 */
export function flattenTree<T extends FlattenableNode>(level: T[], path: string[] = []): FlatMatch<T>[] {
  const out: FlatMatch<T>[] = [];
  for (const node of level) {
    const fullLabel = path.length === 0 ? node.label : `${path.join(" › ")} › ${node.label}`;
    out.push({ node, path: [...path], fullLabel });
    if (node.children) out.push(...flattenTree(node.children as T[], [...path, node.label]));
  }
  return out;
}

/**
 * Logseq-style deep slash match: flatten the WHOLE tree under `level`, then
 * fuzzy-rank by `fullLabel` so typing `/p1` surfaces the deep leaf without
 * manual descent. Empty query → the full flattened tree in original order.
 * Ranking matches `slashFilter`: score desc, stable tie-break on tree order.
 */
export function flattenedSlashFilter<T extends FlattenableNode>(level: T[], query: string): FlatMatch<T>[] {
  const flat = flattenTree(level);
  const q = query.trim();
  if (!q) return flat;
  return flat
    .map((e, i) => ({ e, i, score: scoreFuzzy(e.fullLabel, q).score }))
    .filter((r) => r.score > 0)
    .sort((a, b) => b.score - a.score || a.i - b.i)
    .map((r) => r.e);
}
