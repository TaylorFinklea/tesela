/**
 * Pure helpers for the saved-views DSL editor (saved-views spec 2026-06-10;
 * chip JQL migration tesela-vp9.3).
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
 *    inbox chip registry (`$lib/ambients/inbox/chips` CHIP_REGISTRY) now
 *    writes JQL predicate strings (tesela-vp9.3, decision 5 in
 *    `.docs/ai/phases/2026-07-07-jql-authoring-spec.md`), not colon-DSL
 *    tokens — a chip's clause can be several whitespace-separated words
 *    (`status IS NULL`). Active/toggle detection is therefore PARSE-AWARE:
 *    `clausesActiveInDsl` compares the chip's predicate(s) against the
 *    DSL's TOP-LEVEL AND atoms (via `parseQuery` + a NOT/cmp ⇄ Ne
 *    canonicalization — the same normalization `flattenToLegacyFilters` in
 *    query-language.ts already applies), so `-has:status` (old, still
 *    parseable) and `status IS NULL` (new) are recognized as the SAME
 *    predicate, and a predicate nested inside an `OR` group never counts as
 *    "active". Removal is span-based: a small token-walker (mirroring
 *    query-language.ts's predicate grammar, built only from the exported
 *    `tokenize()`) finds the exact source span of each matched top-level
 *    predicate and slices it out, preserving every other span's text
 *    byte-for-byte and collapsing the join back down to single spaces.
 *
 * The cheap key-only autocomplete this module used to own
 * (`dslKeySuggestions` / `applyDslSuggestion` / `BASE_DSL_KEYS`) is gone —
 * superseded by the shared `QueryInput` widget's three-tier completion
 * (tesela-vp9.2; see `$lib/query-input/`).
 */
// Relative (not `$lib`) so the node test runner can resolve the value
// import without the SvelteKit alias map (mirrors the conformance test's
// direct-path import of query-language.ts).
import {
  parseQuery,
  tokenize,
  type BoolExpr,
  type Predicate,
  type QueryOp,
  type Spanned,
  type Token,
} from "../query-language.ts";

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

// ────────────────────────────────────────────────────────────────────
// Parse-aware predicate containment (chip active/toggle detection)
// ────────────────────────────────────────────────────────────────────

/** Mirrors query-language.ts's private `invertOp` — needed to canonicalize
 *  `NOT (key OP value)` down to the single inverted-op atom the JQL keyword
 *  sugar (`IS NULL`, `!=`, …) produces directly, so equivalence checks
 *  compare like-for-like regardless of which surface syntax authored the
 *  predicate. */
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

/** Unwrap `NOT (cmp)` → the inverted-op `cmp` atom, and `NOT (in)` → the
 *  negation-flipped `in` atom — the same shapes the parser's own IS-NULL /
 *  NOT-IN sugar produces directly, so a hand-typed `-has:status` and a
 *  chip-written `status IS NULL` normalize to the identical signature. */
function normalizeAtom(expr: BoolExpr): BoolExpr {
  if (expr.op === "not" && expr.arg.op === "atom") {
    const pred = expr.arg.pred;
    if (pred.kind === "cmp") {
      return { op: "atom", pred: { ...pred, op: invertOp(pred.op) } };
    }
    return { op: "atom", pred: { ...pred, negated: !pred.negated } };
  }
  return expr;
}

/** Stable, case-folded signature for ONE top-level atom — used to compare
 *  predicates structurally regardless of authored casing or NOT-wrapping. */
function atomSignature(expr: BoolExpr): string {
  const n = normalizeAtom(expr);
  if (n.op === "atom") {
    const pred: Predicate = n.pred;
    if (pred.kind === "cmp") {
      return `cmp:${pred.key.toLowerCase()}:${pred.op}:${pred.value.toLowerCase()}`;
    }
    return `in:${pred.key.toLowerCase()}:${pred.negated}:${pred.values.map((v) => v.toLowerCase()).join(",")}`;
  }
  // Anything else (nested and/or, an unresolved `not`) — rare for
  // chip-shaped fragments; fall back to a structural dump so it still
  // compares consistently with itself.
  return `raw:${JSON.stringify(n)}`;
}

/**
 * Top-level AND-conjuncts of a parsed expression — mirrors the posture
 * `flattenToLegacyFilters` (query-language.ts) already uses: a flat `and`
 * contributes its args, a lone atom/not is itself a one-element AND, and an
 * `or` at the root contributes NOTHING (a predicate nested inside an OR
 * group must never read as "present" for chip purposes).
 */
function topLevelAtoms(expr: BoolExpr): BoolExpr[] {
  if (expr.op === "and") return expr.args;
  if (expr.op === "or") return [];
  return [expr];
}

/**
 * True when EVERY predicate atom of every clause fragment in `clauses` is
 * present among `dsl`'s top-level AND atoms. Semantic (AST-based), not
 * textual — `status IS NULL` and the legacy `-has:status` both parse to the
 * same normalized atom, so either surfaces as "active" for the other.
 */
export function clausesActiveInDsl(dsl: string, clauses: readonly string[]): boolean {
  if (clauses.length === 0) return false;
  const dslSignatures = new Set(topLevelAtoms(parseQuery(dsl).expr).map(atomSignature));
  for (const clause of clauses) {
    const clauseAtoms = topLevelAtoms(parseQuery(clause).expr);
    if (clauseAtoms.length === 0) return false; // malformed/empty clause
    for (const atom of clauseAtoms) {
      if (!dslSignatures.has(atomSignature(atom))) return false;
    }
  }
  return true;
}

// ────────────────────────────────────────────────────────────────────
// Span-based removal — a minimal token-walker mirroring query-language.ts's
// predicate grammar (key/operator/value consumption only — no BoolExpr
// construction), used ONLY to locate where each top-level predicate begins
// and ends in the ORIGINAL source string so it can be sliced out verbatim.
// ────────────────────────────────────────────────────────────────────

function isWordKeyword(sp: Spanned | undefined, kw: string): boolean {
  return sp?.tok.t === "word" && sp.tok.v.toLowerCase() === kw;
}

const VALUE_ADJACENT_TYPES = new Set<Token["t"]>([
  "word",
  "colon",
  "eq",
  "ne",
  "lt",
  "lte",
  "gt",
  "gte",
  "minus",
]);

/** Consume one value (a word/quoted token, plus any byte-adjacent
 *  continuation tokens) — mirrors `parseValue`'s adjacency-merge loop, for
 *  POSITION tracking only. Returns the unchanged index when there's no
 *  value token to consume. */
function consumeValue(toks: Spanned[], i: number): number {
  const first = toks[i];
  if (!first) return i;
  if (first.tok.t === "quoted") return i + 1;
  if (first.tok.t !== "word") return i;
  let j = i + 1;
  let endOffset = first.end;
  while (j < toks.length && toks[j].start === endOffset && VALUE_ADJACENT_TYPES.has(toks[j].tok.t)) {
    endOffset = toks[j].end;
    j += 1;
  }
  return j;
}

/** Consume a parenthesized value list `(a, b, c)` — used for `IN (…)`.
 *  Tolerates stray tokens the same way the real parser's list loop does. */
function consumeParenList(toks: Spanned[], i: number): number {
  if (toks[i]?.tok.t !== "lparen") return i;
  let j = i + 1;
  while (j < toks.length && toks[j].tok.t !== "rparen") {
    if (toks[j].tok.t === "word" || toks[j].tok.t === "quoted") {
      j = consumeValue(toks, j);
      continue;
    }
    j += 1;
  }
  return j < toks.length ? j + 1 : j;
}

const INFIX_OP_TYPES = new Set<Token["t"]>(["eq", "ne", "lt", "lte", "gt", "gte"]);
const LEGACY_COLON_OP_TYPES = new Set<Token["t"]>(["ne", "lte", "gte", "lt", "gt"]);

/**
 * Consume one top-level "unary" worth of tokens starting at `start` —
 * mirrors `parseUnary` + `parsePredicate`'s branch order (leading `-`/NOT,
 * parens, `key-in:`, `IN`/`NOT IN`, `LIKE`/`NOT LIKE`, `IS [NOT] NULL|EMPTY`,
 * `BETWEEN … AND …`, infix comparison, legacy colon + tight comma list) —
 * but only for POSITION advancement; the actual BoolExpr comes from
 * re-parsing the resulting substring with the real `parseQuery`. Returns
 * `start` unchanged if nothing recognizable begins there.
 */
function consumeUnit(toks: Spanned[], start: number): number {
  let i = start;
  for (;;) {
    if (toks[i]?.tok.t === "minus") {
      i += 1;
      continue;
    }
    if (isWordKeyword(toks[i], "not")) {
      i += 1;
      continue;
    }
    break;
  }
  if (toks[i]?.tok.t === "lparen") {
    let depth = 1;
    i += 1;
    while (i < toks.length && depth > 0) {
      if (toks[i].tok.t === "lparen") depth += 1;
      else if (toks[i].tok.t === "rparen") depth -= 1;
      i += 1;
    }
    return i;
  }
  const keyTok = toks[i];
  if (keyTok?.tok.t !== "word") return start;
  const key = keyTok.tok.v.toLowerCase();
  i += 1;

  if (key.endsWith("-in") && toks[i]?.tok.t === "colon") {
    i += 1;
    for (;;) {
      const t = toks[i]?.tok.t;
      if (t === "word" || t === "quoted") {
        i = consumeValue(toks, i);
        continue;
      }
      if (t === "comma") {
        i += 1;
        continue;
      }
      break;
    }
    return i;
  }
  if (isWordKeyword(toks[i], "in")) {
    return consumeParenList(toks, i + 1);
  }
  if (isWordKeyword(toks[i], "not")) {
    const save = i;
    const j = i + 1;
    if (isWordKeyword(toks[j], "in")) return consumeParenList(toks, j + 1);
    if (isWordKeyword(toks[j], "like")) return consumeValue(toks, j + 1);
    i = save; // rollback — not NOT-IN / NOT-LIKE, fall through
  }
  if (isWordKeyword(toks[i], "like")) {
    return consumeValue(toks, i + 1);
  }
  if (isWordKeyword(toks[i], "is")) {
    const save = i;
    let j = i + 1;
    if (isWordKeyword(toks[j], "not")) j += 1;
    if (isWordKeyword(toks[j], "null") || isWordKeyword(toks[j], "empty")) return j + 1;
    i = save; // rollback
  }
  if (isWordKeyword(toks[i], "between")) {
    const save = i;
    const afterLow = consumeValue(toks, i + 1);
    if (afterLow > i + 1 && isWordKeyword(toks[afterLow], "and")) {
      const afterHigh = consumeValue(toks, afterLow + 1);
      if (afterHigh > afterLow + 1) return afterHigh;
    }
    i = save; // rollback
  }
  if (toks[i] && INFIX_OP_TYPES.has(toks[i].tok.t)) {
    return consumeValue(toks, i + 1);
  }
  if (toks[i]?.tok.t === "colon") {
    i += 1;
    if (toks[i] && LEGACY_COLON_OP_TYPES.has(toks[i].tok.t)) i += 1;
    const beforeValue = i;
    i = consumeValue(toks, i);
    if (i === beforeValue) return i; // no value — matches the parser's drop
    for (;;) {
      const comma = toks[i];
      if (!comma || comma.tok.t !== "comma") break;
      if (toks[i - 1].end !== comma.start) break; // whitespace before ',' — not tight
      const next = toks[i + 1];
      if (!next || (next.tok.t !== "word" && next.tok.t !== "quoted") || next.start !== comma.end) break;
      i = consumeValue(toks, i + 1);
    }
    return i;
  }
  return i; // bare key, no operator — the real parser drops it too
}

type Chunk = { start: number; end: number };

/** Every top-level chunk's [start, end) source span, in left-to-right
 *  order, skipping bare `AND`/`OR` keywords between them. */
function topLevelChunkRanges(dsl: string): Chunk[] {
  const toks = tokenize(dsl);
  const ranges: Chunk[] = [];
  let i = 0;
  while (i < toks.length) {
    if (isWordKeyword(toks[i], "and") || isWordKeyword(toks[i], "or")) {
      i += 1;
      continue;
    }
    const chunkStart = toks[i].start;
    const next = consumeUnit(toks, i);
    if (next === i) break; // no progress — malformed tail, stop scanning
    ranges.push({ start: chunkStart, end: toks[next - 1].end });
    i = next;
  }
  return ranges;
}

type ChunkInfo = Chunk & { signature: string | null };

/** Each top-level chunk's span plus its atom signature — `null` when the
 *  chunk doesn't reparse to EXACTLY one atom (a `kind:` selector consumed
 *  for its side effect, or a parenthesized OR group), which protects it
 *  from ever being matched for removal — the same "top-level AND atoms
 *  only" rule `clausesActiveInDsl` enforces. */
function analyzeChunks(dsl: string): ChunkInfo[] {
  return topLevelChunkRanges(dsl).map(({ start, end }) => {
    const atoms = topLevelAtoms(parseQuery(dsl.slice(start, end)).expr);
    return { start, end, signature: atoms.length === 1 ? atomSignature(atoms[0]) : null };
  });
}

/** Remove every top-level chunk whose signature matches one of `clauses`'
 *  atoms, then rejoin the survivors with single spaces (collapsing any
 *  doubled whitespace / dropped `AND` keywords) — each survivor's own text
 *  is sliced verbatim from the original string. */
function removeClausesFromDsl(dsl: string, clauses: readonly string[]): string {
  const targets = new Set<string>();
  for (const clause of clauses) {
    for (const atom of topLevelAtoms(parseQuery(clause).expr)) targets.add(atomSignature(atom));
  }
  const kept = analyzeChunks(dsl).filter((c) => c.signature === null || !targets.has(c.signature));
  return kept
    .map((c) => dsl.slice(c.start, c.end))
    .join(" ")
    .trim();
}

/** Append the clause fragments not already present (per-fragment, matching
 *  the granularity `clauses` is authored at), space-separated. */
function appendMissingClauses(dsl: string, clauses: readonly string[]): string {
  const dslSignatures = new Set(topLevelAtoms(parseQuery(dsl).expr).map(atomSignature));
  const missing = clauses.filter((clause) => {
    const atoms = topLevelAtoms(parseQuery(clause).expr);
    return !(atoms.length > 0 && atoms.every((a) => dslSignatures.has(atomSignature(a))));
  });
  if (missing.length === 0) return dsl.trim();
  const trimmed = dsl.trim();
  return trimmed.length === 0 ? missing.join(" ") : `${trimmed} ${missing.join(" ")}`;
}

/**
 * Toggle a chip's clause fragment(s) in the DSL string.
 *
 * - All clauses' predicates already present (as top-level AND atoms) →
 *   remove every matching predicate's SPAN from the string (the chip turns
 *   off).
 * - Otherwise → append the missing clause fragments at the end,
 *   space-separated (the chip turns on; fragments already present are not
 *   duplicated).
 *
 * Untouched spans are preserved byte-for-byte; whitespace between spans is
 * normalized to single spaces — the editor must never reshuffle a
 * hand-written query's own predicate text just because a chip was tapped.
 */
export function toggleClausesInDsl(dsl: string, clauses: readonly string[]): string {
  if (clausesActiveInDsl(dsl, clauses)) {
    return removeClausesFromDsl(dsl, clauses);
  }
  return appendMissingClauses(dsl, clauses);
}
