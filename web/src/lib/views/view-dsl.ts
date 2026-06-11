/**
 * Pure helpers for the saved-views DSL editor (saved-views spec 2026-06-10).
 *
 * Three concerns, all string-level so they unit-test without a DOM:
 *
 *  - **Validation** (`validateViewDsl`) — mirrors the server's `validate_dsl`
 *    in `crates/tesela-server/src/routes/views.rs`. The parser is TOTAL and
 *    liberal (unrecognized syntax is silently dropped), so "invalid" means a
 *    non-empty input from which the parser recognized ZERO predicates —
 *    saving it would silently create a match-everything view. Same
 *    carve-outs as the server: a lone `kind:…` selector and a bare
 *    `ORDER BY` are valid queries with an empty predicate tree. The server
 *    400s as the backstop; this gives the editor live inline feedback.
 *
 *  - **Chip insertion** (`toggleClausesInDsl` / `clausesActiveInDsl`) — the
 *    inbox chip registry (`$lib/ambients/inbox/chips` CHIP_REGISTRY) knows
 *    each chip's DSL fragment(s); the views editor re-points those chips as
 *    one-tap INSERTERS into the query string. A chip is "active" when every
 *    one of its clauses appears as a token; toggling adds the missing
 *    clauses (appended at the end) or removes all of them.
 *
 *  - **Key autocomplete** (`dslKeySuggestions` / `applyDslSuggestion`) — the
 *    cheap version: suggest predicate KEYS (`status:`, `has:`, …) for the
 *    partial word at the caret. Values / a full LSP are explicitly out of
 *    scope.
 */
// Relative (not `$lib`) so the node test runner can resolve the value
// import without the SvelteKit alias map (mirrors the conformance test's
// direct-path import of query-language.ts).
import { parseQuery, type BoolExpr } from "../query-language.ts";

/** Built-in predicate keys the parser understands (see query-language.ts
 *  grammar header). Property keys from `GET /properties` are appended by
 *  the caller at suggestion time. */
export const BASE_DSL_KEYS: readonly string[] = [
  "status",
  "tag",
  "has",
  "is",
  "on",
  "kind",
  "text",
  "page",
  "block",
  "type",
  "tag-in",
];

function isEmptyExpr(expr: BoolExpr): boolean {
  return expr.op === "and" && expr.args.length === 0;
}

/**
 * Validate a view DSL string. Returns `null` when saveable, or a
 * human-readable error message for inline display. Mirrors the server's
 * `validate_dsl` (which remains the authoritative backstop on save).
 */
export function validateViewDsl(dsl: string): string | null {
  const trimmed = dsl.trim();
  if (trimmed.length === 0) return "query must not be empty";
  const parsed = parseQuery(trimmed);
  const mentionsKind = trimmed.toLowerCase().includes("kind:");
  if (isEmptyExpr(parsed.expr) && parsed.sort === undefined && !mentionsKind) {
    return `no predicates recognized in "${trimmed}" — use key:value filters like status:todo, tag:project, -has:scheduled`;
  }
  return null;
}

/** Whitespace tokenization — the same liberal split the chip round-trip in
 *  `chips.ts` uses; the real parser tolerates anything token-shaped. */
function tokens(dsl: string): string[] {
  return dsl
    .split(/\s+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** True when EVERY clause of a chip appears as a token in the DSL. */
export function clausesActiveInDsl(dsl: string, clauses: readonly string[]): boolean {
  if (clauses.length === 0) return false;
  const present = new Set(tokens(dsl));
  return clauses.every((c) => present.has(c));
}

/**
 * Toggle a chip's clause fragment(s) in the DSL string.
 *
 * - All clauses present → remove every occurrence of each clause token
 *   (the chip turns off).
 * - Otherwise → append the missing clauses at the end, space-separated
 *   (the chip turns on; clauses already present are not duplicated).
 *
 * Token order of the untouched parts is preserved verbatim — the editor
 * must never reshuffle a hand-written query just because a chip was
 * tapped. Whitespace is normalized to single spaces (the string is being
 * edited token-wise anyway).
 */
export function toggleClausesInDsl(dsl: string, clauses: readonly string[]): string {
  const toks = tokens(dsl);
  if (clausesActiveInDsl(dsl, clauses)) {
    const remove = new Set(clauses);
    return toks.filter((t) => !remove.has(t)).join(" ");
  }
  const present = new Set(toks);
  const missing = clauses.filter((c) => !present.has(c));
  return [...toks, ...missing].join(" ");
}

export type DslSuggestion = {
  /** Replace range [from, to) in the DSL string with the accepted item. */
  from: number;
  to: number;
  /** Completion items, each a full `key:` prefix ready to type a value. */
  items: string[];
};

/**
 * Key suggestions for the partial word at `cursor`. Returns `null` when
 * there is nothing to suggest: caret not at the end of a word, the word
 * already has its `key:` part, or no key matches. A leading `-` (negation)
 * is honored — suggestions replace only the key part after it.
 *
 * `extraKeys` lets the caller mix in property names from the registry
 * endpoint (`GET /properties`).
 */
export function dslKeySuggestions(
  dsl: string,
  cursor: number,
  extraKeys: readonly string[] = [],
): DslSuggestion | null {
  if (cursor < 0 || cursor > dsl.length) return null;
  // Mid-word caret (next char is a word char) → don't suggest; the user is
  // editing inside an existing token, not finishing one.
  if (cursor < dsl.length && /\S/.test(dsl[cursor])) return null;
  // Walk back to the start of the current whitespace-delimited token.
  let start = cursor;
  while (start > 0 && /\S/.test(dsl[start - 1])) start -= 1;
  let word = dsl.slice(start, cursor);
  if (word.startsWith("-")) {
    start += 1;
    word = word.slice(1);
  }
  if (word.length === 0) return null;
  // Already has a key: part (or any operator) → key suggestion is over.
  if (/[:=<>!(]/.test(word)) return null;
  const partial = word.toLowerCase();
  const seen = new Set<string>();
  const items: string[] = [];
  for (const key of [...BASE_DSL_KEYS, ...extraKeys.map((k) => k.toLowerCase())]) {
    if (seen.has(key)) continue;
    seen.add(key);
    if (key.startsWith(partial) && key !== partial) items.push(`${key}:`);
    else if (key === partial) items.push(`${key}:`);
  }
  if (items.length === 0) return null;
  return { from: start, to: cursor, items };
}

/**
 * Apply an accepted suggestion item to the DSL string. Returns the new
 * string and the caret position (right after the inserted `key:`).
 */
export function applyDslSuggestion(
  dsl: string,
  s: { from: number; to: number },
  item: string,
): { dsl: string; cursor: number } {
  const next = dsl.slice(0, s.from) + item + dsl.slice(s.to);
  return { dsl: next, cursor: s.from + item.length };
}
