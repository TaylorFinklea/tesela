/**
 * Caret-context classification for the QueryInput completion popup
 * (tesela-vp9.2). Given the raw source string and a cursor offset, decides
 * which of the three completion tiers (spec decision 4) applies:
 *
 *   - "key"      — a property/meta key belongs here
 *   - "operator" — a comparison operator / combinator / sort keyword
 *   - "value"    — a value for the nearest predicate key
 *
 * Built on `tokenize()` spans + `classifyTokens()`'s per-token role, never
 * on the AST — the parser doesn't retain token-level spans for successfully
 * parsed predicates (only `Diagnostic`s carry spans, for DROPPED tokens).
 */
import { tokenize } from "../query-language.ts";
import { classifyTokens, type ClassifiedToken } from "./classify.ts";

export type CompletionTier = "key" | "operator" | "value" | "none";

export type CaretContext = {
  tier: CompletionTier;
  /** [from, to) range in the source string an accepted item replaces. */
  from: number;
  to: number;
  /** Already-typed text in [from, to) — filters/ranks completion items. */
  prefix: string;
  /** Lowercased governing predicate key. Only meaningful for tier "value". */
  key: string | null;
};

const NONE: CaretContext = { tier: "none", from: 0, to: 0, prefix: "", key: null };

/** Reserved words whose keyword role signals "a value follows here". */
function keywordIntroducesValue(word: string): boolean {
  const w = word.toLowerCase();
  return w === "in" || w === "like" || w === "between";
}

export function caretContext(input: string, cursor: number): CaretContext {
  if (cursor < 0 || cursor > input.length) return NONE;
  // Mid-word caret — don't interrupt editing inside an existing token
  // (mirrors the pre-vp9.2 `dslKeySuggestions` convention in view-dsl.ts).
  if (cursor < input.length && /\S/.test(input[cursor])) return NONE;

  const tokens = tokenize(input);
  const classified = classifyTokens(tokens);

  // The partial word being typed, if the cursor sits immediately after a
  // word token with no gap (`stat|`, `-stat|`, `status:backlog,todo|`).
  // `-` always tokenizes as its own `minus` token (never merged into a
  // following word — see query-language.ts's tokenizer), so no separate
  // leading-dash stripping is needed here.
  const partial = classified.find((c) => c.span.tok.t === "word" && c.span.end === cursor);
  const from = partial ? partial.span.start : cursor;
  const prefix = partial && partial.span.tok.t === "word" ? partial.span.tok.v : "";

  // The nearest classified token strictly before the partial word (or the
  // caret, when there's no partial word).
  let prev: ClassifiedToken | null = null;
  for (const c of classified) {
    if (c.span.start >= from) break;
    prev = c;
  }

  if (prev === null) return { tier: "key", from, to: cursor, prefix, key: null };

  switch (prev.role) {
    case "key":
      return {
        tier: "operator",
        from,
        to: cursor,
        prefix,
        key: prev.span.tok.t === "word" ? prev.span.tok.v.toLowerCase() : null,
      };
    case "operator":
      return { tier: "value", from, to: cursor, prefix, key: prev.key };
    case "comma":
      return { tier: "value", from, to: cursor, prefix, key: prev.key };
    case "paren":
      return prev.key
        ? { tier: "value", from, to: cursor, prefix, key: prev.key }
        : { tier: "key", from, to: cursor, prefix, key: null };
    case "keyword": {
      const w = prev.span.tok.t === "word" ? prev.span.tok.v : "";
      if (keywordIntroducesValue(w)) {
        return { tier: "value", from, to: cursor, prefix, key: prev.key };
      }
      return { tier: "key", from, to: cursor, prefix, key: null };
    }
    case "value":
    default:
      // A predicate just finished — the natural next thing is a new
      // predicate (implicit AND) or an explicit combinator.
      return { tier: "key", from, to: cursor, prefix, key: null };
  }
}
