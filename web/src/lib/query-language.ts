/**
 * Token-style query language for filtering blocks and pages.
 *
 * Faithful TS port of `crates/tesela-core/src/query.rs` (parser +
 * in-memory matcher). Rust is the source of truth — the shared
 * conformance fixture `crates/tesela-core/tests/fixtures/
 * query-conformance.json` (consumed here by
 * `web/tests/unit/query-conformance.test.mjs`) pins matching semantics
 * across the Rust / web-TS / iOS-Swift implementations. Where this file
 * disagrees with Rust, fix THIS file, never the fixture.
 *
 * Grammar (mirrors the Rust recursive-descent parser):
 *   or        := and ("OR" and)*
 *   and       := unary (("AND" | implicit whitespace) unary)*
 *   unary     := ("NOT" | "-") unary | "(" or ")" | predicate
 *   predicate := key ( ":" legacy-op? value
 *                    | infix-op value
 *                    | "IN" "(" list ")" | "NOT IN" "(" list ")"
 *                    | "LIKE" value | "NOT LIKE" value
 *                    | "IS" ["NOT"] ("NULL" | "EMPTY")
 *                    | "BETWEEN" value "AND" value )
 *   trailing  := "ORDER BY" key [ASC|DESC] ("," key [ASC|DESC])*
 *
 * Comma multi-value sugar: `key:v1,v2` desugars to `key IN (v1, v2)` —
 * OR within the key — but only for TIGHT commas (no whitespace on either
 * side). A trailing comma degrades to single-value equality; a loose
 * comma is stray punctuation that ends the list. See
 * `peekTightCommaContinuation` (mirrors Rust's
 * `peek_tight_comma_continuation`).
 *
 * Special pseudo-keys (see `filterMatches`):
 *   - `kind:block | kind:page` — narrows the result set; consumed into
 *     ParsedQuery.kind (default `block`), NOT a filter on rows.
 *   - `has:foo` — property presence regardless of value; `-has:foo` absence.
 *   - `tag:foo` / `type:foo` / `pagetag:foo` / `blocktag:foo` — resolved
 *     tag chain membership (blocktag excludes inherited).
 *   - `tag-in:a,b` — legacy any-member alias for `tag:a,b`.
 *   - `on:daily-page` / `on:system-pages` — containing-page identity.
 *   - `is:heading` — markdown heading blocks.
 *   - `text:foo` — display-text match; `page:` / `block:` — id match.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import type { ParsedQuery } from "$lib/types/ParsedQuery";
import type { BoolExpr } from "$lib/types/BoolExpr";
import type { Predicate } from "$lib/types/Predicate";
import type { QueryFilter } from "$lib/types/QueryFilter";
import type { QueryOp } from "$lib/types/QueryOp";
import type { Kind } from "$lib/types/Kind";

export type { ParsedQuery, BoolExpr, Predicate, QueryFilter, QueryOp, Kind };

/**
 * The seeded built-in Inbox view's DSL — mirrors `INBOX_VIEW_DSL` in
 * `query.rs`. The conformance fixture gates exactly this string.
 */
export const INBOX_VIEW_DSL = "status:backlog,todo -has:scheduled -has:deadline";

// ────────────────────────────────────────────────────────────────────
// Tokenizer (mirrors query.rs `tokenize`)
// ────────────────────────────────────────────────────────────────────

/**
 * Exported (tesela-vp9.1) so authoring UI — highlighting overlays,
 * completion popups — can tokenize the SAME way the parser does, with
 * zero drift between "what lights up" and "what parses". No second
 * lexer: this is the identical function `parseQuery` calls internally.
 */
export type Token =
  | { t: "word"; v: string }
  | { t: "quoted"; v: string }
  | { t: "lparen" }
  | { t: "rparen" }
  | { t: "comma" }
  | { t: "colon" }
  | { t: "eq" }
  | { t: "ne" }
  | { t: "lt" }
  | { t: "lte" }
  | { t: "gt" }
  | { t: "gte" }
  | { t: "minus" };

/**
 * Token paired with its source span (`end` exclusive). Adjacency is
 * `prev.end === next.start` — the parser uses it to slurp colons /
 * digits / dashes that belong to a single value (`block:python:5`) and
 * to detect TIGHT commas for the `key:v1,v2` multi-value sugar.
 */
export type Spanned = { tok: Token; start: number; end: number };

function isWordChar(c: string): boolean {
  return /[A-Za-z0-9_-]/.test(c);
}

export function tokenize(input: string): Spanned[] {
  const tokens: Spanned[] = [];
  const n = input.length;
  let i = 0;
  while (i < n) {
    const c = input[i];
    if (/\s/.test(c)) {
      i += 1;
      continue;
    }
    const start = i;
    let tok: Token;
    if (c === "(") {
      tok = { t: "lparen" };
      i += 1;
    } else if (c === ")") {
      tok = { t: "rparen" };
      i += 1;
    } else if (c === ",") {
      tok = { t: "comma" };
      i += 1;
    } else if (c === ":") {
      tok = { t: "colon" };
      i += 1;
    } else if (c === "=") {
      tok = { t: "eq" };
      i += 1;
    } else if (c === "!" && input[i + 1] === "=") {
      tok = { t: "ne" };
      i += 2;
    } else if (c === "<" && input[i + 1] === "=") {
      tok = { t: "lte" };
      i += 2;
    } else if (c === ">" && input[i + 1] === "=") {
      tok = { t: "gte" };
      i += 2;
    } else if (c === "<") {
      tok = { t: "lt" };
      i += 1;
    } else if (c === ">") {
      tok = { t: "gt" };
      i += 1;
    } else if (c === '"') {
      const valStart = i + 1;
      let j = valStart;
      while (j < n && input[j] !== '"') j += 1;
      tok = { t: "quoted", v: input.slice(valStart, j) };
      i = j < n ? j + 1 : j;
    } else if (c === "-") {
      // Leading '-' is a standalone Minus (unary NOT shorthand); a '-'
      // INSIDE a word is consumed by the word branch below since '-' is
      // a word char. Order matches the Rust tokenizer.
      tok = { t: "minus" };
      i += 1;
    } else if (isWordChar(c)) {
      let j = i;
      while (j < n && isWordChar(input[j])) j += 1;
      tok = { t: "word", v: input.slice(i, j) };
      i = j;
    } else {
      // Unknown char — skip silently so malformed input never throws.
      i += 1;
      continue;
    }
    tokens.push({ tok, start, end: i });
  }
  return tokens;
}

// ────────────────────────────────────────────────────────────────────
// Recursive-descent parser (mirrors query.rs `Parser`)
// ────────────────────────────────────────────────────────────────────

type ParserState = {
  /** Source string — `parseValue` re-extracts raw byte ranges from it. */
  input: string;
  tokens: Spanned[];
  pos: number;
  /** `kind:block` / `kind:page` is plucked out of the predicate stream. */
  kind: Kind;
  /**
   * Authoring-only diagnostics sink (tesela-vp9.1). `null` for the
   * plain `parseQuery` path — every recording call below is a no-op
   * `?.push` in that case, so parsing stays exactly as it was. Non-null
   * only under `parseQueryWithDiagnostics`.
   */
  diagnostics: Diagnostic[] | null;
};

function atom(pred: Predicate): BoolExpr {
  return { op: "atom", pred };
}

function emptyAnd(): BoolExpr {
  return { op: "and", args: [] };
}

/**
 * One authoring-UI hint about a span the parser silently dropped or
 * left dangling during its re-sync (see the module doc + tesela-vp9.1).
 * `got` is the raw source slice at `[start, end)`; `hint` is a short
 * human-readable explanation, omitted (empty string) where none is
 * cheaply derivable.
 */
export type Diagnostic = { start: number; end: number; got: string; hint: string };

function recordDrop(
  p: ParserState,
  start: number,
  end: number,
  got: string,
  hint: string,
): void {
  p.diagnostics?.push({ start, end, got, hint });
}

/**
 * A `"quoted"` token is well-formed only if its span accounts for both
 * the opening AND closing `"` (`end - start === v.length + 2`); when
 * the tokenizer ran off the end of input looking for the closer, the
 * span is one byte short (`v.length + 1`). Pure token-local arithmetic
 * — no re-scan of `input` needed.
 */
function recordUnclosedQuotes(tokens: Spanned[], input: string, diagnostics: Diagnostic[]): void {
  for (const sp of tokens) {
    if (sp.tok.t !== "quoted") continue;
    const closed = sp.end - sp.start === sp.tok.v.length + 2;
    if (!closed) {
      diagnostics.push({
        start: sp.start,
        end: sp.end,
        got: input.slice(sp.start, sp.end),
        hint: "unclosed quoted string",
      });
    }
  }
}

function parseQueryInternal(input: string, diagnostics: Diagnostic[] | null): ParsedQuery {
  const tokens = tokenize(input);
  if (diagnostics) recordUnclosedQuotes(tokens, input, diagnostics);
  const p: ParserState = { input, tokens, pos: 0, kind: "block", diagnostics };
  const expr = parseOr(p) ?? emptyAnd();
  const sort = parseOrderBy(p);
  if (diagnostics && p.pos < p.tokens.length) {
    const start = p.tokens[p.pos].start;
    const end = p.tokens[p.tokens.length - 1].end;
    recordDrop(p, start, end, input.slice(start, end), "unexpected trailing input");
  }
  const filters = flattenToLegacyFilters(expr);
  const out: ParsedQuery = { kind: p.kind, expr, filters };
  if (sort !== null) out.sort = sort;
  return out;
}

export function parseQuery(input: string): ParsedQuery {
  return parseQueryInternal(input, null);
}

/**
 * Authoring-only sibling of `parseQuery` (tesela-vp9.1). Returns the
 * SAME `ParsedQuery` `parseQuery` would (byte-for-byte — proven by the
 * `query-diagnostics.test.mjs` invariant test, on top of the shared
 * conformance fixture pinning `parseQuery` itself) plus a list of spans
 * the parser silently dropped or left dangling while re-syncing.
 * Diagnostics are additive UI metadata only — never authoritative, and
 * the 182-case conformance fixture is NOT extended for them (decision 3
 * in `.docs/ai/phases/2026-07-07-jql-authoring-spec.md`).
 */
export function parseQueryWithDiagnostics(input: string): {
  parsed: ParsedQuery;
  diagnostics: Diagnostic[];
} {
  const diagnostics: Diagnostic[] = [];
  const parsed = parseQueryInternal(input, diagnostics);
  return { parsed, diagnostics };
}

function peek(p: ParserState): Token | null {
  return p.tokens[p.pos]?.tok ?? null;
}

function bump(p: ParserState): Token | null {
  const t = p.tokens[p.pos]?.tok ?? null;
  if (t !== null) p.pos += 1;
  return t;
}

function peekKeyword(p: ParserState, kw: string): boolean {
  const t = peek(p);
  return t?.t === "word" && asciiLower(t.v) === kw;
}

/** Is the upcoming two-token sequence `ORDER BY`? */
function peekOrderBy(p: ParserState): boolean {
  const a = p.tokens[p.pos]?.tok;
  const b = p.tokens[p.pos + 1]?.tok;
  return (
    a?.t === "word" &&
    asciiLower(a.v) === "order" &&
    b?.t === "word" &&
    asciiLower(b.v) === "by"
  );
}

/**
 * Parse a trailing `ORDER BY field1 [ASC|DESC][, field2 …]` clause into
 * the comma-separated string shape the server's `apply_sort` accepts.
 */
function parseOrderBy(p: ParserState): string | null {
  if (!peekOrderBy(p)) return null;
  bump(p); // ORDER
  bump(p); // BY
  const parts: string[] = [];
  for (;;) {
    const t = bump(p);
    if (t?.t !== "word") break;
    const key = asciiLower(t.v);
    let suffix = "";
    if (peekKeyword(p, "desc")) {
      bump(p);
      suffix = " desc";
    } else if (peekKeyword(p, "asc")) {
      bump(p);
      suffix = " asc";
    }
    parts.push(key + suffix);
    if (peek(p)?.t !== "comma") break;
    bump(p); // comma
  }
  return parts.length === 0 ? null : parts.join(", ");
}

/** Does the upcoming token start a new unary expression? */
function peekStartsUnary(p: ParserState): boolean {
  const t = peek(p);
  if (t === null) return false;
  if (t.t !== "lparen" && t.t !== "word" && t.t !== "minus") return false;
  // `OR` / `AND` keywords don't start a unary — they belong to a
  // higher-level rule.
  return !peekKeyword(p, "or") && !peekKeyword(p, "and");
}

function parseOr(p: ParserState): BoolExpr | null {
  let left = parseAnd(p);
  if (left === null) return null;
  const alts: BoolExpr[] = [];
  while (peekKeyword(p, "or")) {
    const orSpanned = p.tokens[p.pos];
    bump(p);
    const rhs = parseAnd(p);
    if (rhs !== null) {
      if (alts.length === 0) alts.push(left);
      alts.push(rhs);
    } else {
      recordDrop(p, orSpanned.start, orSpanned.end, "OR", "'OR' has no right-hand predicate");
    }
  }
  if (alts.length > 0) left = { op: "or", args: alts };
  return left;
}

function parseAnd(p: ParserState): BoolExpr | null {
  let left = parseUnary(p);
  if (left === null) return null;
  const args: BoolExpr[] = [];
  for (;;) {
    let andSpanned: Spanned | null = null;
    if (peekKeyword(p, "and")) {
      andSpanned = p.tokens[p.pos];
      bump(p);
    } else if (!peekStartsUnary(p)) {
      break;
    }
    const rhs = parseUnary(p);
    if (rhs === null) {
      if (andSpanned !== null) {
        recordDrop(p, andSpanned.start, andSpanned.end, "AND", "'AND' has no right-hand predicate");
      }
      break;
    }
    if (args.length === 0) args.push(left);
    args.push(rhs);
  }
  if (args.length > 0) left = { op: "and", args };
  return left;
}

function parseUnary(p: ParserState): BoolExpr | null {
  // Loop so we can keep trying after `kind:value` predicates that get
  // consumed for their side-effect (mutating `p.kind`) but produce no
  // expression — mirrors the Rust parser exactly.
  for (;;) {
    // Stop at a trailing `ORDER BY` — picked up by `parseOrderBy` at
    // the top level.
    if (peekOrderBy(p)) return null;
    if (peekKeyword(p, "not")) {
      const notSpanned = p.tokens[p.pos];
      bump(p);
      const inner = parseUnary(p);
      if (inner === null) {
        recordDrop(p, notSpanned.start, notSpanned.end, "NOT", "'NOT' has no operand");
        return null;
      }
      return { op: "not", arg: inner };
    }
    const t = peek(p);
    if (t?.t === "minus") {
      const minusSpanned = p.tokens[p.pos];
      bump(p);
      const inner = parseUnary(p);
      if (inner === null) {
        recordDrop(p, minusSpanned.start, minusSpanned.end, "-", "'-' has no operand");
        return null;
      }
      return { op: "not", arg: inner };
    }
    if (t?.t === "lparen") {
      const lparenSpanned = p.tokens[p.pos];
      bump(p);
      const inner = parseOr(p) ?? emptyAnd();
      if (peek(p)?.t === "rparen") {
        bump(p);
      } else {
        const end = p.pos < p.tokens.length ? p.tokens[p.pos].start : p.input.length;
        recordDrop(p, lparenSpanned.start, end, p.input.slice(lparenSpanned.start, end), "unclosed '('");
      }
      return inner;
    }
    const startPos = p.pos;
    const e = parsePredicate(p);
    if (e !== null) return e;
    if (p.pos === startPos) {
      // No progress — give up to avoid an infinite loop.
      return null;
    }
    // Predicate consumed for side-effect (likely `kind:foo`); eat an
    // explicit AND so `kind:block AND status:todo` doesn't stall.
    if (peekKeyword(p, "and")) bump(p);
    if (!peekStartsUnary(p)) return null;
  }
}

/**
 * Parse one predicate. Backward-compat: every legacy form
 * (`key:value`, `key:>=N`, `tag-in:a,b,c`, `has:foo`) produces the same
 * predicate the Rust parser produces.
 */
function parsePredicate(p: ParserState): BoolExpr | null {
  const keySpanned = p.tokens[p.pos];
  const keyTok = bump(p);
  if (keyTok === null) return null;
  // A standalone quoted string or punctuation at predicate position is
  // malformed — drop it and re-synchronize on the next token.
  if (keyTok.t !== "word") {
    recordDrop(
      p,
      keySpanned.start,
      keySpanned.end,
      p.input.slice(keySpanned.start, keySpanned.end),
      "expected a predicate key here",
    );
    return null;
  }
  const key = asciiLower(keyTok.v);

  // `kind:` is meta — consume the value, set p.kind, return null so the
  // token doesn't end up in the expression tree.
  if (key === "kind") {
    if (peek(p)?.t === "colon") bump(p);
    const v = parseValue(p);
    if (v !== null) {
      const lv = asciiLower(v);
      p.kind = lv === "page" || lv === "pages" ? "page" : "block";
    }
    return null;
  }

  // Legacy `tag-in:a,b,c` shape — equivalent to `tag IN (a, b, c)`.
  if (key.endsWith("-in") && peek(p)?.t === "colon") {
    bump(p); // ':'
    const realKey = key.slice(0, key.length - "-in".length);
    const values = parseCommaListUntilWhitespace(p);
    return atom({ kind: "in", key: realKey, values, negated: false });
  }

  // New-style infix `key IN (…)` / `key NOT IN (…)`.
  if (peekKeyword(p, "in")) {
    bump(p);
    const values = parseParenValueList(p);
    return atom({ kind: "in", key, values, negated: false });
  }
  if (peekKeyword(p, "not")) {
    // Tentatively consume NOT; commit only if followed by IN or LIKE.
    const save = p.pos;
    bump(p);
    if (peekKeyword(p, "in")) {
      bump(p);
      const values = parseParenValueList(p);
      return atom({ kind: "in", key, values, negated: true });
    }
    if (peekKeyword(p, "like")) {
      const likeSpanned = p.tokens[p.pos];
      bump(p);
      const raw = parseValue(p);
      if (raw === null) {
        recordDrop(p, likeSpanned.start, likeSpanned.end, "LIKE", "'LIKE' has no operand");
      }
      return atom({ kind: "cmp", key, op: "NotLike", value: raw ?? "" });
    }
    p.pos = save;
  }

  // `key LIKE "pattern"` — SQL-style wildcard match.
  if (peekKeyword(p, "like")) {
    const likeSpanned = p.tokens[p.pos];
    bump(p);
    const raw = parseValue(p);
    if (raw === null) {
      recordDrop(p, likeSpanned.start, likeSpanned.end, "LIKE", "'LIKE' has no operand");
    }
    return atom({ kind: "cmp", key, op: "Like", value: raw ?? "" });
  }

  // `key IS [NOT] NULL|EMPTY` — sugar for `-has:key` / `has:key`.
  if (peekKeyword(p, "is")) {
    const save = p.pos;
    bump(p); // IS
    let negated = false;
    if (peekKeyword(p, "not")) {
      bump(p);
      negated = true;
    }
    if (peekKeyword(p, "null") || peekKeyword(p, "empty")) {
      bump(p);
      return atom({
        kind: "cmp",
        key: "has",
        // IS NOT NULL → present → has:key → Eq; IS NULL → absent → Ne.
        op: negated ? "Eq" : "Ne",
        value: key,
      });
    }
    p.pos = save;
  }

  // `key BETWEEN a AND b` — sugar for `key >= a AND key <= b`.
  if (peekKeyword(p, "between")) {
    const save = p.pos;
    bump(p); // BETWEEN
    const low = parseValue(p);
    if (low !== null && peekKeyword(p, "and")) {
      bump(p);
      const high = parseValue(p);
      if (high !== null) {
        return {
          op: "and",
          args: [
            atom({ kind: "cmp", key, op: "Gte", value: low }),
            atom({ kind: "cmp", key, op: "Lte", value: high }),
          ],
        };
      }
    }
    p.pos = save;
  }

  // Infix comparison operator: `key = value`, `key != value`, etc.
  const infixSpanned = p.tokens[p.pos];
  const infix = consumeInfixOp(p);
  if (infix !== null) {
    const raw = parseValue(p);
    if (raw === null) {
      const opText = p.input.slice(infixSpanned.start, infixSpanned.end);
      recordDrop(p, infixSpanned.start, infixSpanned.end, opText, `'${opText}' has no operand`);
    }
    return atom({ kind: "cmp", key, op: infix, value: raw ?? "" });
  }

  // Legacy colon syntax: `key:value`, `key:>=N`, etc. `has:foo` is the
  // one legitimate "no value" form; for everything else an empty value
  // drops the predicate.
  if (peek(p)?.t === "colon") {
    const colonSpanned = p.tokens[p.pos];
    bump(p);
    const op = consumeLegacyColonOp(p) ?? "Eq";
    const value = parseValue(p) ?? "";
    if (key !== "has" && value === "") {
      recordDrop(
        p,
        keySpanned.start,
        colonSpanned.end,
        p.input.slice(keySpanned.start, colonSpanned.end),
        `'${key}:' has no value`,
      );
      return null;
    }
    // `key:v1,v2,…` — comma multi-value sugar: OR within the key,
    // desugared to the same `in` predicate the `key IN (…)` form and
    // the legacy `tag-in:` shape produce. Eq-only, and the commas must
    // be TIGHT — see `peekTightCommaContinuation`.
    if (op === "Eq" && value !== "") {
      const values = [value];
      while (peekTightCommaContinuation(p)) {
        bump(p); // ','
        const v = parseValue(p);
        if (v !== null && v !== "") values.push(v);
        else break;
      }
      if (values.length > 1) {
        return atom({ kind: "in", key, values, negated: false });
      }
    }
    return atom({ kind: "cmp", key, op, value });
  }

  // A bareword with no operator at all isn't a valid predicate.
  recordDrop(
    p,
    keySpanned.start,
    keySpanned.end,
    p.input.slice(keySpanned.start, keySpanned.end),
    `unknown word '${key}' — expected an operator after it`,
  );
  return null;
}

function consumeInfixOp(p: ParserState): QueryOp | null {
  const t = peek(p);
  let op: QueryOp | null = null;
  if (t?.t === "eq") op = "Eq";
  else if (t?.t === "ne") op = "Ne";
  else if (t?.t === "lt") op = "Lt";
  else if (t?.t === "lte") op = "Lte";
  else if (t?.t === "gt") op = "Gt";
  else if (t?.t === "gte") op = "Gte";
  if (op !== null) bump(p);
  return op;
}

function consumeLegacyColonOp(p: ParserState): QueryOp | null {
  const t = peek(p);
  let op: QueryOp | null = null;
  if (t?.t === "ne") op = "Ne";
  else if (t?.t === "lte") op = "Lte";
  else if (t?.t === "gte") op = "Gte";
  else if (t?.t === "lt") op = "Lt";
  else if (t?.t === "gt") op = "Gt";
  if (op !== null) bump(p);
  return op;
}

/**
 * Parse a value. Quoted strings are self-contained; barewords slurp
 * every ADJACENT (no whitespace gap) value-like token so values that
 * legitimately contain `:` (block ids like `python:5`) survive.
 * Mirrors Rust's `parse_value` — including consuming (and discarding)
 * a non-value token at the cursor.
 */
function parseValue(p: ParserState): string | null {
  const first = p.tokens[p.pos];
  if (!first) return null;
  p.pos += 1;
  if (first.tok.t === "quoted") return first.tok.v;
  if (first.tok.t !== "word") return null;
  let buf = first.tok.v;
  let endOffset = first.end;
  while (p.pos < p.tokens.length) {
    const span = p.tokens[p.pos];
    if (span.start !== endOffset) break; // whitespace gap → value ends
    const tt = span.tok.t;
    if (
      tt === "word" ||
      tt === "colon" ||
      tt === "eq" ||
      tt === "ne" ||
      tt === "lt" ||
      tt === "lte" ||
      tt === "gt" ||
      tt === "gte" ||
      tt === "minus"
    ) {
      // Append the raw source slice — preserves the exact characters.
      buf += p.input.slice(span.start, span.end);
      endOffset = span.end;
      p.pos += 1;
    } else {
      // Quoted / paren / comma terminate the value.
      break;
    }
  }
  return buf;
}

/**
 * Is the cursor at a TIGHT comma-list continuation — a `,` with no
 * whitespace on either side, followed by a value token? Drives the
 * `key:v1,v2` multi-value OR sugar. Tightness matters: a comma touching
 * whitespace (`status:a, b` / `status:a ,b`) is stray punctuation, not
 * a list separator. The legacy `tag-in:` path keeps its looser
 * whitespace-tolerant list parsing for back-compat.
 */
function peekTightCommaContinuation(p: ParserState): boolean {
  if (p.pos === 0) return false;
  const comma = p.tokens[p.pos];
  if (!comma || comma.tok.t !== "comma") return false;
  if (p.tokens[p.pos - 1].end !== comma.start) return false; // ws before ','
  const next = p.tokens[p.pos + 1];
  if (!next) return false;
  return (next.tok.t === "word" || next.tok.t === "quoted") && next.start === comma.end;
}

/** Parse `(a, b, c)` — used for `IN (…)`. Tolerates missing parens. */
function parseParenValueList(p: ParserState): string[] {
  const out: string[] = [];
  if (peek(p)?.t !== "lparen") return out;
  bump(p);
  for (;;) {
    const t = peek(p);
    if (t?.t === "rparen") {
      bump(p);
      break;
    }
    if (t?.t === "comma") {
      bump(p);
      continue;
    }
    if (t?.t === "word" || t?.t === "quoted") {
      const v = parseValue(p);
      if (v !== null) out.push(v);
      continue;
    }
    break;
  }
  return out;
}

/**
 * Parse `a,b,c` (legacy `tag-in:a,b,c` shape — no parens). Stops at the
 * first token that can't be part of a comma list. Whitespace-tolerant.
 */
function parseCommaListUntilWhitespace(p: ParserState): string[] {
  const out: string[] = [];
  for (;;) {
    const t = peek(p);
    if (t?.t === "word" || t?.t === "quoted") {
      const v = parseValue(p);
      if (v !== null) out.push(v);
      continue;
    }
    if (t?.t === "comma") {
      bump(p);
      continue;
    }
    break;
  }
  return out.filter((s) => s !== "");
}

/**
 * Flatten a `BoolExpr` into the legacy flat-AND `QueryFilter[]` view —
 * only when the expression is a flat conjunction of simple `cmp` atoms
 * (a `not(cmp)` becomes a flipped op). Empty for anything richer.
 */
function flattenToLegacyFilters(expr: BoolExpr): QueryFilter[] {
  let atoms: BoolExpr[];
  if (expr.op === "and") atoms = expr.args;
  else if (expr.op === "or") return [];
  else atoms = [expr];
  const out: QueryFilter[] = [];
  for (const a of atoms) {
    if (a.op === "atom" && a.pred.kind === "cmp") {
      out.push({ key: a.pred.key, op: a.pred.op, value: a.pred.value });
    } else if (a.op === "not" && a.arg.op === "atom" && a.arg.pred.kind === "cmp") {
      out.push({
        key: a.arg.pred.key,
        op: invertOp(a.arg.pred.op),
        value: a.arg.pred.value,
      });
    } else {
      return [];
    }
  }
  return out;
}

function invertOp(op: QueryOp): QueryOp {
  switch (op) {
    case "Eq":
      return "Ne";
    case "Ne":
      return "Eq";
    case "Gt":
      return "Lte";
    case "Lt":
      return "Gte";
    case "Gte":
      return "Lt";
    case "Lte":
      return "Gt";
    case "Like":
      return "NotLike";
    case "NotLike":
      return "Like";
  }
}

// ────────────────────────────────────────────────────────────────────
// Matcher (mirrors query.rs `block_matches` / `filter_matches`)
// ────────────────────────────────────────────────────────────────────

function asciiLower(s: string): string {
  return s.replace(/[A-Z]/g, (c) => String.fromCharCode(c.charCodeAt(0) + 32));
}

function eqIgnoreAsciiCase(a: string, b: string): boolean {
  return asciiLower(a) === asciiLower(b);
}

/** Full-string numeric literal check — mirrors Rust's `str::parse::<f64>`. */
function numericValue(s: string): number | null {
  const t = s.trim();
  if (t === "" || !/^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/.test(t)) return null;
  return Number(t);
}

function isIsoDate(s: string): boolean {
  return (
    s.length >= 10 &&
    s[4] === "-" &&
    s[7] === "-" &&
    /^\d{4}$/.test(s.slice(0, 4)) &&
    /^\d{2}$/.test(s.slice(5, 7)) &&
    /^\d{2}$/.test(s.slice(8, 10))
  );
}

/** Canonical `YYYY-MM-DD` daily-note id check — drives `on:daily-page`. */
function isDailyNoteId(noteId: string): boolean {
  return noteId.length === 10 && isIsoDate(noteId);
}

/** System page types — drives `on:system-pages`. Case-sensitive, like Rust. */
function isSystemNoteType(noteType: string): boolean {
  return (
    noteType === "Tag" ||
    noteType === "Property" ||
    noteType === "Query" ||
    noteType === "Template"
  );
}

/**
 * First non-whitespace run is 1–6 `#`s followed by whitespace
 * (CommonMark heading). Drives `is:heading`. `#urgent` (no whitespace
 * after the `#`s) is a hashtag, not a heading; 7+ `#`s aren't either.
 */
function isHeadingText(text: string): boolean {
  const trimmed = text.trimStart();
  let hashes = 0;
  for (const ch of trimmed) {
    if (ch === "#") {
      hashes += 1;
      if (hashes > 6) return false;
    } else {
      return hashes >= 1 && /\s/.test(ch);
    }
  }
  return false; // all-#s (or empty) — no heading body
}

/** Comparison helper: number → ISO date → ASCII-lowercased string. */
function compare(a: string, b: string): number {
  const an = numericValue(a);
  const bn = numericValue(b);
  if (an !== null && bn !== null) return an < bn ? -1 : an > bn ? 1 : 0;
  if (isIsoDate(a) && isIsoDate(b)) return a < b ? -1 : a > b ? 1 : 0; // lexicographic
  const al = asciiLower(a);
  const bl = asciiLower(b);
  return al < bl ? -1 : al > bl ? 1 : 0;
}

/**
 * SQL `LIKE` matcher: `%` = any run, `_` = one char; case-insensitive,
 * anchored, regex metas treated as literals. Mirrors `like_matches`.
 */
function likeMatches(actual: string, pattern: string): boolean {
  let out = "^";
  for (const ch of pattern) {
    if (ch === "%") out += ".*";
    else if (ch === "_") out += ".";
    else if (/[.+*?()|[\]{}^$\\]/.test(ch)) out += "\\" + ch;
    else out += ch;
  }
  out += "$";
  let re: RegExp;
  try {
    re = new RegExp(out, "i");
  } catch {
    return false; // malformed pattern → no match, never throw
  }
  return re.test(actual);
}

function applyOp(actual: string, op: QueryOp, expected: string): boolean {
  if (op === "Eq") return eqIgnoreAsciiCase(actual, expected);
  if (op === "Ne") return !eqIgnoreAsciiCase(actual, expected);
  if (op === "Like") return likeMatches(actual, expected);
  if (op === "NotLike") return !likeMatches(actual, expected);
  const cmp = compare(actual, expected);
  if (op === "Gt") return cmp > 0;
  if (op === "Lt") return cmp < 0;
  if (op === "Gte") return cmp >= 0;
  return cmp <= 0; // Lte
}

/**
 * Map a `value_type` string onto the four comparison buckets. Both the
 * server `ValueType` vocabulary (`multiselect`, `node`, …) and the web
 * `PropertyType` vocabulary (`multi-select`, `email`, `phone`, `object`)
 * collapse here — only number / date-like / checkbox differ from the
 * default string bucket. Mirror of `query.rs:compare_typed`'s match arms.
 */
function valueTypeBucket(vt: string): "number" | "date" | "checkbox" | "string" {
  switch (vt.toLowerCase()) {
    case "number":
      return "number";
    case "date":
    case "datetime":
      return "date";
    case "checkbox":
      return "checkbox";
    default:
      return "string"; // text/url/select/multi-select/multiselect/node/email/phone/object
  }
}

/** Registry-typed comparison (L5) — mirror of `query.rs:compare_typed`. */
function compareTyped(a: string, b: string, vt: string): number {
  switch (valueTypeBucket(vt)) {
    case "number": {
      const an = numericValue(a);
      const bn = numericValue(b);
      if (an !== null && bn !== null) return an < bn ? -1 : an > bn ? 1 : 0;
      // either side isn't a number → case-folded string (no date promotion)
      const al = asciiLower(a);
      const bl = asciiLower(b);
      return al < bl ? -1 : al > bl ? 1 : 0;
    }
    case "checkbox": {
      const ab = eqIgnoreAsciiCase(a.trim(), "true");
      const bb = eqIgnoreAsciiCase(b.trim(), "true");
      return ab === bb ? 0 : ab ? 1 : -1; // false < true
    }
    case "date": {
      if (isIsoDate(a) && isIsoDate(b)) return a < b ? -1 : a > b ? 1 : 0;
      const al = asciiLower(a);
      const bl = asciiLower(b);
      return al < bl ? -1 : al > bl ? 1 : 0;
    }
    default: {
      // string bucket — NO numeric promotion (a select "10" stays text).
      const al = asciiLower(a);
      const bl = asciiLower(b);
      return al < bl ? -1 : al > bl ? 1 : 0;
    }
  }
}

/** `applyOp` routed through `compareTyped` — mirror of `query.rs:apply_op_typed`. */
function applyOpTyped(actual: string, op: QueryOp, expected: string, vt: string): boolean {
  if (op === "Like") return likeMatches(actual, expected);
  if (op === "NotLike") return !likeMatches(actual, expected);
  if (op === "Eq") return compareTyped(actual, expected, vt) === 0;
  if (op === "Ne") return compareTyped(actual, expected, vt) !== 0;
  const cmp = compareTyped(actual, expected, vt);
  if (op === "Gt") return cmp > 0;
  if (op === "Lt") return cmp < 0;
  if (op === "Gte") return cmp >= 0;
  return cmp <= 0; // Lte
}

/** Shared empty registry for the untyped `blockMatches` path. */
const EMPTY_TYPES: ReadonlyMap<string, string> = new Map();

/**
 * Check whether a parsed block matches the query's expression tree.
 *
 * `types` (L5) maps a **lowercased** property name to its declared
 * `value_type` string; when present, property comparisons are typed
 * (numeric/date/bool) instead of string-guessed. Omit it (or pass an empty
 * map) for the registry-free heuristic — identical to the pre-L5 behavior.
 * Mirror of `query.rs:block_matches` / `block_matches_typed`.
 */
export function blockMatches(
  block: ParsedBlock,
  query: ParsedQuery,
  types: ReadonlyMap<string, string> = EMPTY_TYPES,
): boolean {
  return evalExpr(block, query.expr, types);
}

/**
 * Stably sort rows by an `ORDER BY` sort string — the `"field [asc|desc], …"`
 * shape `parseQuery` puts in `ParsedQuery.sort`. Multi-key (first key wins;
 * ties fall through to the next), default ascending.
 *
 * Field resolution is the caller's job (`fieldValue(row, key)` → the raw string
 * for that key — title/text builtins, property lookup, `[[…]]` stripped — the
 * same keys the server's `apply_sort` resolves). The COMPARISON is L5-typed via
 * `compareTyped`, so `points` orders numerically and ISO dates chronologically
 * — the inline query block sorts CORRECTLY where the server's string-only
 * `apply_sort` does not (server-side sort parity is a tracked follow-up).
 */
export function applySort<T>(
  rows: readonly T[],
  sort: string | null | undefined,
  fieldValue: (row: T, key: string) => string,
  types: ReadonlyMap<string, string> = EMPTY_TYPES,
): T[] {
  const out = [...rows];
  if (!sort) return out;
  const keys: Array<{ key: string; desc: boolean }> = [];
  for (const tok of sort.split(",")) {
    const parts = tok.trim().split(/\s+/).filter((p) => p.length > 0);
    if (parts.length === 0) continue;
    keys.push({ key: parts[0].toLowerCase(), desc: (parts[1] ?? "").toLowerCase() === "desc" });
  }
  if (keys.length === 0) return out;
  // Decorate-sort-undecorate for a stable sort (preserve filter order on ties).
  return out
    .map((row, i) => ({ row, i }))
    .sort((a, b) => {
      for (const { key, desc } of keys) {
        const vt = types.get(key) ?? "string";
        const cmp = compareTyped(fieldValue(a.row, key), fieldValue(b.row, key), vt);
        if (cmp !== 0) return desc ? -cmp : cmp;
      }
      return a.i - b.i;
    })
    .map((d) => d.row);
}

/** Walk the tree, short-circuiting. Empty `and` matches everything. */
function evalExpr(block: ParsedBlock, expr: BoolExpr, types: ReadonlyMap<string, string>): boolean {
  if (expr.op === "and") return expr.args.every((a) => evalExpr(block, a, types));
  if (expr.op === "or") return expr.args.some((a) => evalExpr(block, a, types));
  if (expr.op === "not") return !evalExpr(block, expr.arg, types);
  return predMatches(block, expr.pred, types);
}

function predMatches(
  block: ParsedBlock,
  pred: Predicate,
  types: ReadonlyMap<string, string>,
): boolean {
  if (pred.kind === "cmp") {
    return filterMatches(block, { key: pred.key, op: pred.op, value: pred.value }, types);
  }
  // `key in (a, b, c)` is OR over `key = v`; `not in` negates. Routes
  // through the same per-key matcher so semantics line up exactly.
  const anyMatch = pred.values.some((v) =>
    filterMatches(block, { key: pred.key, op: "Eq", value: v }, types),
  );
  return pred.negated ? !anyMatch : anyMatch;
}

function filterMatches(
  block: ParsedBlock,
  f: QueryFilter,
  types: ReadonlyMap<string, string>,
): boolean {
  // `tag:` (default), `type:` (alias), `pagetag:` (frontmatter alias),
  // `blocktag:` (excludes inherited).
  if (f.key === "tag" || f.key === "type" || f.key === "pagetag" || f.key === "blocktag") {
    const needle = asciiLower(f.value);
    const includeInherited = f.key !== "blocktag";
    const chain = includeInherited
      ? [...block.tags, ...block.inherited_tags]
      : block.tags;
    const hasTag = chain.some((t) => asciiLower(t) === needle);
    if (f.op === "Eq") return hasTag;
    if (f.op === "Ne") return !hasTag;
    return false; // comparison ops not meaningful for tags
  }
  if (f.key === "has-link") {
    // Block contains `[[<value>]]` (case-insensitive) anywhere in raw_text.
    const needle = asciiLower(`[[${f.value}]]`);
    const present = asciiLower(block.raw_text).includes(needle);
    if (f.op === "Eq") return present;
    if (f.op === "Ne") return !present;
    return false;
  }
  if (f.key === "has") {
    // `has:foo` checks property presence regardless of value.
    const needle = asciiLower(f.value);
    const present = Object.keys(block.properties).some((k) => asciiLower(k) === needle);
    if (f.op === "Eq") return present;
    if (f.op === "Ne") return !present;
    return false;
  }
  if (f.key === "page") {
    // `page:<note_id>` — containing note id. `-page:foo` is the common
    // form ("Hide all from this page").
    const matched = eqIgnoreAsciiCase(block.note_id, f.value);
    if (f.op === "Eq") return matched;
    if (f.op === "Ne") return !matched;
    return false;
  }
  if (f.key === "block") {
    // `block:<id>` — deterministic block id (`<note_id>:<line>`).
    const matched = eqIgnoreAsciiCase(block.id, f.value);
    if (f.op === "Eq") return matched;
    if (f.op === "Ne") return !matched;
    return false;
  }
  if (f.key === "tag-in") {
    // Back-compat: a `tag-in` Cmp filter (value carries the comma list).
    // The parser normally desugars `tag-in:a,b` into an `in` predicate,
    // but programmatically-built filters can still take this path.
    const needles = f.value
      .split(",")
      .map((s) => asciiLower(s.trim()))
      .filter((s) => s !== "");
    const matched =
      needles.length === 0
        ? false
        : [...block.tags, ...block.inherited_tags].some((t) =>
            needles.includes(asciiLower(t)),
          );
    if (f.op === "Eq") return matched;
    if (f.op === "Ne") return !matched;
    return false;
  }
  if (f.key === "on") {
    // `on:daily-page` / `on:system-pages` — containing page identity.
    // Unknown value → false-on-Eq / true-on-Ne (graceful degrade).
    const v = asciiLower(f.value);
    let matched = false;
    if (v === "daily-page") matched = isDailyNoteId(block.note_id);
    else if (v === "system-pages") {
      const nt = block.parent_note_type ?? null;
      matched = nt !== null && isSystemNoteType(nt);
    }
    if (f.op === "Eq") return matched;
    if (f.op === "Ne") return !matched;
    return false;
  }
  if (f.key === "text") {
    // Display text (first line, tags stripped) — what users see.
    return applyOp(block.text, f.op, f.value);
  }
  if (f.key === "is") {
    // `is:heading`; unknown values degrade gracefully like `on:`.
    const matched = asciiLower(f.value) === "heading" ? isHeadingText(block.text) : false;
    if (f.op === "Eq") return matched;
    if (f.op === "Ne") return !matched;
    return false;
  }
  // Property lookup — case-insensitive key match. Missing property
  // matches any `Ne` ("missing != value") and nothing else.
  const entry = Object.entries(block.properties).find(([k]) => asciiLower(k) === f.key);
  if (!entry) return f.op === "Ne";
  // L5: if the registry declares this property's type, compare typed
  // (numeric/date/bool); otherwise keep the string heuristic.
  const vt = types.get(asciiLower(f.key));
  return vt !== undefined
    ? applyOpTyped(entry[1], f.op, f.value, vt)
    : applyOp(entry[1], f.op, f.value);
}
