/**
 * Completion-list assembly for QueryInput (tesela-vp9.2, spec decision 4).
 * Pure — takes a `CaretContext` (see `caret-context.ts`) plus a source of
 * properties/types and returns the FULL candidate item list for that tier.
 * Prefix filtering/ranking is deliberately NOT done here — QueryInput feeds
 * `ctx.prefix` straight into `AutocompleteMenu`'s own fuzzy `filter` prop,
 * so this module always returns the tier's complete candidate set.
 */
import type { CaretContext } from "./caret-context.ts";

/** Structural subset of `PropertyDef` (types/PropertyDef.ts) — a real
 *  PropertyDef satisfies this without mapping; tests pass plain fakes. */
export type PropertySource = { name: string; value_type: string; values: string[] | null };

/** Structural subset of `TypeDefinition` (types/TypeDefinition.ts). */
export type TypeSource = { name: string };

export type CompletionSources = {
  properties: readonly PropertySource[];
  types: readonly TypeSource[];
};

export type CompletionItem = { id: string; label: string; secondary?: string };

/** Meta keys every query understands, per the vp9 spec's decision 4 list —
 *  distinct from registered properties (which come from `sources.properties`). */
export const META_KEYS: readonly string[] = [
  "type",
  "kind",
  "tag",
  "status",
  "has",
  "is",
  "on",
  "text",
  "page",
  "block",
];

/** The full operator/combinator/sort-keyword menu offered right after a
 *  key (spec decision 4 + the task's tier-(b) list) — NOT filtered by
 *  what's grammatically valid at the exact caret position; the fixed menu
 *  is the spec'd simplification. */
export const OPERATOR_ITEMS: readonly string[] = [
  "=",
  "!=",
  "<",
  "<=",
  ">",
  ">=",
  ":",
  "IN",
  "NOT IN",
  "LIKE",
  "NOT LIKE",
  "BETWEEN",
  "IS NULL",
  "IS NOT NULL",
  "AND",
  "OR",
  "ORDER BY",
  "ASC",
  "DESC",
];

/** Both the Rust `ValueType` spelling (`multiselect`) and the web
 *  `PropertyType` spelling (`multi-select`) — see query-language.ts's
 *  `valueTypeBucket` doc for why both exist. */
function isSelectType(valueType: string): boolean {
  const v = valueType.toLowerCase();
  return v === "select" || v === "multi-select" || v === "multiselect";
}

/**
 * Build the full completion candidate list for `ctx`'s tier. Returns `[]`
 * for tier "none", for a VALUE tier whose key isn't a select-typed
 * property (and isn't `type`/`kind`), or when there's nothing to offer.
 */
export function buildCompletions(ctx: CaretContext, sources: CompletionSources): CompletionItem[] {
  if (ctx.tier === "key") {
    const seen = new Set<string>();
    const items: CompletionItem[] = [];
    for (const p of sources.properties) {
      const lower = p.name.toLowerCase();
      if (seen.has(lower)) continue;
      seen.add(lower);
      items.push({ id: p.name, label: p.name, secondary: p.value_type });
    }
    for (const key of META_KEYS) {
      if (seen.has(key)) continue;
      seen.add(key);
      items.push({ id: key, label: key, secondary: "meta" });
    }
    return items;
  }

  if (ctx.tier === "operator") {
    return OPERATOR_ITEMS.map((op) => ({ id: op, label: op }));
  }

  if (ctx.tier === "value") {
    const key = (ctx.key ?? "").toLowerCase();
    if (key.length === 0) return [];
    if (key === "type" || key === "kind") {
      return sources.types.map((t) => ({ id: t.name, label: t.name }));
    }
    const prop = sources.properties.find((p) => p.name.toLowerCase() === key);
    if (prop && isSelectType(prop.value_type) && prop.values) {
      return prop.values.map((v) => ({ id: v, label: v }));
    }
    return [];
  }

  return [];
}
