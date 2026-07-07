/**
 * Best-effort per-token role classifier for JQL authoring UI (tesela-vp9.2).
 *
 * A lightweight state machine over `tokenize()`'s output — NOT a second
 * parser. It drives two things that both need "what role does this token
 * play, and which predicate key does it belong to" without re-implementing
 * `query-language.ts`'s recursive-descent grammar:
 *
 *   - cosmetic syntax-highlighting (`overlay-spans.ts`)
 *   - completion caret-context tiers (`caret-context.ts`)
 *
 * Advisory only, same spirit as the diagnostics pass (see the vp9 spec,
 * `.docs/ai/phases/2026-07-07-jql-authoring-spec.md`, decision 3): it only
 * needs to be right often enough to look correct and to place the
 * completion popup sensibly. The real grammar — and the sole source of
 * truth for what a query MEANS — stays `query-language.ts`'s parser.
 */
import type { Spanned } from "../query-language.ts";

export type TokenRole = "key" | "keyword" | "operator" | "value" | "paren" | "comma";

export type ClassifiedToken = {
  span: Spanned;
  role: TokenRole;
  /**
   * Lowercased predicate key this token is scoped to — set on the
   * operator/value/comma tokens of a `key OP value` predicate, and on the
   * `IN`/`LIKE`/`BETWEEN` keyword that introduces a value. `null` for key
   * tokens themselves, combinators (`AND`/`OR`/`NOT`), and punctuation
   * outside any predicate's value region.
   */
  key: string | null;
};

const VALUE_KEYWORDS = new Set(["in", "like", "between", "is"]);
const KEYWORDS = new Set([
  "and",
  "or",
  "not",
  "in",
  "like",
  "between",
  "is",
  "null",
  "empty",
  "order",
  "by",
  "asc",
  "desc",
]);

/**
 * Classify every token in `tokens` (the output of `tokenize()`) into a
 * role + governing key. See the module doc for what "best-effort" means
 * here — it is not the grammar.
 */
export function classifyTokens(tokens: readonly Spanned[]): ClassifiedToken[] {
  const out: ClassifiedToken[] = [];
  let state: "key" | "op" | "val" = "key";
  let activeKey: string | null = null;
  let pendingBetween = false;
  let inOrderBy = false;

  for (let i = 0; i < tokens.length; i++) {
    const sp = tokens[i];
    const t = sp.tok;

    if (t.t === "lparen" || t.t === "rparen") {
      out.push({ span: sp, role: "paren", key: state === "val" ? activeKey : null });
      if (t.t === "rparen") state = "key";
      continue;
    }
    if (t.t === "comma") {
      out.push({ span: sp, role: "comma", key: inOrderBy ? null : activeKey });
      if (!inOrderBy) state = "val";
      continue;
    }
    if (
      t.t === "colon" ||
      t.t === "eq" ||
      t.t === "ne" ||
      t.t === "lt" ||
      t.t === "lte" ||
      t.t === "gt" ||
      t.t === "gte"
    ) {
      out.push({ span: sp, role: "operator", key: activeKey });
      state = "val";
      continue;
    }
    if (t.t === "minus") {
      out.push({ span: sp, role: state === "key" ? "keyword" : "operator", key: activeKey });
      continue;
    }
    if (t.t === "quoted") {
      out.push({ span: sp, role: "value", key: activeKey });
      state = "key";
      continue;
    }

    // word
    const lower = t.v.toLowerCase();
    if (KEYWORDS.has(lower)) {
      out.push({ span: sp, role: "keyword", key: VALUE_KEYWORDS.has(lower) ? activeKey : null });
      if (lower === "order") {
        const next = tokens[i + 1]?.tok;
        if (next?.t === "word" && next.v.toLowerCase() === "by") inOrderBy = true;
        state = "key";
        activeKey = null;
      } else if (lower === "by") {
        state = "key";
      } else if (lower === "and" || lower === "or") {
        if (pendingBetween) {
          pendingBetween = false;
          state = "val";
        } else {
          state = "key";
          activeKey = null;
        }
      } else if (lower === "in" || lower === "like") {
        state = "val";
      } else if (lower === "between") {
        state = "val";
        pendingBetween = true;
      } else if (lower === "is") {
        state = "val";
      } else if (lower === "null" || lower === "empty") {
        state = "key";
      } else if (lower === "asc" || lower === "desc") {
        state = "key";
      }
      // "not" — leave state untouched; it either prefixes a unary (state
      // already "key") or a NOT IN / NOT LIKE (state already "op").
      continue;
    }

    if (inOrderBy) {
      out.push({ span: sp, role: "key", key: null });
      state = "key";
      continue;
    }
    if (state === "val") {
      out.push({ span: sp, role: "value", key: activeKey });
      state = "key";
    } else {
      out.push({ span: sp, role: "key", key: null });
      activeKey = lower;
      state = "op";
    }
  }
  return out;
}
