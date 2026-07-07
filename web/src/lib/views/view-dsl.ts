/**
 * Pure helpers for the saved-views DSL editor (saved-views spec 2026-06-10).
 *
 * Two concerns, both string-level so they unit-test without a DOM:
 *
 *  - **Validation** (`validateViewDsl`) — mirrors the server's `validate_dsl`
 *    in `crates/tesela-server/src/routes/views.rs`. The parser is TOTAL and
 *    liberal (unrecognized syntax is silently dropped), so "invalid" means a
 *    non-empty input from which the parser recognized ZERO predicates —
 *    saving it would silently create a match-everything view. Same
 *    carve-outs as the server: a lone `kind:…` selector and a bare
 *    `ORDER BY` are valid queries with an empty predicate tree. The server
 *    400s as the backstop; GrInbox still gates `canSave` on this (its
 *    QueryInput's diagnostics are additive display, not a save-gate).
 *
 *  - **Chip insertion** (`toggleClausesInDsl` / `clausesActiveInDsl`) — the
 *    inbox chip registry (`$lib/ambients/inbox/chips` CHIP_REGISTRY) knows
 *    each chip's DSL fragment(s); the views editor re-points those chips as
 *    one-tap INSERTERS into the query string. A chip is "active" when every
 *    one of its clauses appears as a token; toggling adds the missing
 *    clauses (appended at the end) or removes all of them.
 *
 * The cheap key-only autocomplete this module used to own
 * (`dslKeySuggestions` / `applyDslSuggestion` / `BASE_DSL_KEYS`) is gone —
 * superseded by the shared `QueryInput` widget's three-tier completion
 * (tesela-vp9.2; see `$lib/query-input/`).
 */
// Relative (not `$lib`) so the node test runner can resolve the value
// import without the SvelteKit alias map (mirrors the conformance test's
// direct-path import of query-language.ts).
import { parseQuery, type BoolExpr } from "../query-language.ts";

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
  // Recognize the `kind` selector in BOTH the JQL infix form (`kind = page`)
  // and the legacy colon form (`kind:page`) — it's consumed at parse time so
  // it leaves no predicate in the expr tree.
  const mentionsKind = /\bkind\b\s*[:=]/i.test(trimmed);
  if (isEmptyExpr(parsed.expr) && parsed.sort === undefined && !mentionsKind) {
    return `no predicates recognized in "${trimmed}" — use filters like status = todo, type = project, scheduled IS NULL`;
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
