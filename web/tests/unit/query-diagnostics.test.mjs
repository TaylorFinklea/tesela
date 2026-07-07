// Unit tests for the authoring-only diagnostics pass (tesela-vp9.1).
//
// `parseQueryWithDiagnostics` wraps the SAME recursive-descent parser
// `parseQuery` uses (query-language.ts `parseQueryInternal`) and records
// `{start, end, got, hint}` spans wherever the parser silently drops a
// token or leaves an operator dangling while re-syncing. Diagnostics are
// additive UI metadata only — they must never change what `parseQuery`
// itself returns; that invariant is asserted directly below, on top of
// the shared conformance fixture (query-conformance.test.mjs) which pins
// `parseQuery`'s well-formed-input behavior separately.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { parseQuery, parseQueryWithDiagnostics } from "../../src/lib/query-language.ts";

// ────────────────────────────────────────────────────────────────────
// Clean JQL → zero diagnostics
// ────────────────────────────────────────────────────────────────────

const CLEAN_QUERIES = [
  "points > 5",
  "points >= 5 AND points <= 10",
  "status != done",
  "tag = urgent",
  "tag IN (urgent, blocked)",
  "tag NOT IN (urgent, blocked)",
  'text LIKE "%foo%"',
  'text NOT LIKE "%foo%"',
  "points BETWEEN 1 AND 10",
  "deadline IS NULL",
  "deadline IS NOT NULL",
  "(status:todo OR status:doing) AND priority:high",
  "status:todo AND priority:high",
  "status:todo OR priority:high",
  "ORDER BY points DESC, created ASC",
  "status:todo ORDER BY points DESC, created ASC",
  "status:backlog,todo -has:scheduled -has:deadline",
  "tag-in:a,b,c",
  "kind:page status:todo",
  "-status:done",
  'text:"hello world"',
  "",
  "   ",
];

for (const q of CLEAN_QUERIES) {
  test(`clean JQL produces zero diagnostics: ${JSON.stringify(q)}`, () => {
    const { diagnostics } = parseQueryWithDiagnostics(q);
    assert.deepEqual(
      diagnostics,
      [],
      `expected no diagnostics for ${JSON.stringify(q)}, got ${JSON.stringify(diagnostics)}`,
    );
  });
}

// ────────────────────────────────────────────────────────────────────
// Malformed inputs → diagnostics with correct spans
// ────────────────────────────────────────────────────────────────────

test("dangling AND records a diagnostic spanning the AND token", () => {
  const dsl = "status:todo AND";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(dsl.slice(d.start, d.end), "AND");
  assert.equal(d.got, "AND");
  assert.match(d.hint, /AND/);
  // The dangling AND is dropped — the predicate before it still parses.
  assert.deepEqual(parsed, parseQuery(dsl));
  assert.equal(parsed.filters.length, 1);
  assert.equal(parsed.filters[0].key, "status");
});

test("dangling OR records a diagnostic spanning the OR token", () => {
  const dsl = "status:todo OR";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(dsl.slice(d.start, d.end), "OR");
  assert.equal(d.got, "OR");
  assert.deepEqual(parsed, parseQuery(dsl));
});

test("unclosed paren records a diagnostic spanning from '(' to end of input", () => {
  const dsl = "(status:todo";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(d.start, 0);
  assert.equal(d.end, dsl.length);
  assert.equal(d.got, dsl);
  assert.match(d.hint, /unclosed/i);
  // Content inside the unclosed paren still parses as if closed.
  assert.deepEqual(parsed, parseQuery(dsl));
  assert.equal(parsed.filters.length, 1);
  assert.equal(parsed.filters[0].key, "status");
});

test("unclosed quote records a diagnostic spanning the quoted token", () => {
  const dsl = 'text:"foo';
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(d.start, 5); // index of the opening '"'
  assert.equal(d.end, dsl.length);
  assert.equal(d.got, '"foo');
  assert.match(d.hint, /unclosed/i);
  assert.deepEqual(parsed, parseQuery(dsl));
  // The unterminated quote still yields its content as the value.
  assert.equal(parsed.filters[0].value, "foo");
});

test("bare unknown word between predicates records a diagnostic and is dropped", () => {
  const dsl = "status:todo blah status:done";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(dsl.slice(d.start, d.end), "blah");
  assert.equal(d.got, "blah");
  assert.match(d.hint, /unknown word/i);
  assert.deepEqual(parsed, parseQuery(dsl));
  // "blah" dropped; both status predicates survive as an implicit AND.
  assert.equal(parsed.filters.length, 2);
});

test("infix operator with no operand ('points >') records a diagnostic", () => {
  const dsl = "points >";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(dsl.slice(d.start, d.end), ">");
  assert.equal(d.got, ">");
  assert.match(d.hint, /no operand/i);
  assert.deepEqual(parsed, parseQuery(dsl));
  // Predicate still produced, with an empty value (matches parseQuery).
  assert.equal(parsed.filters.length, 1);
  assert.equal(parsed.filters[0].value, "");
});

test("colon predicate with no value ('status:' alone) records a diagnostic and drops", () => {
  const dsl = "status: AND priority:high";
  // "AND" is itself a word token so it's slurped as the value here —
  // use a punctuation boundary instead to force a genuinely missing value.
  const dsl2 = "status:)";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl2);
  assert.ok(diagnostics.length >= 1);
  const colonDiag = diagnostics.find((d) => d.hint.includes("has no value"));
  assert.ok(colonDiag, `expected a "has no value" diagnostic, got ${JSON.stringify(diagnostics)}`);
  assert.equal(dsl2.slice(colonDiag.start, colonDiag.end), "status:");
  assert.deepEqual(parsed, parseQuery(dsl2));
  void dsl; // kept for documentation of the AND-slurp gotcha above
});

test("LIKE with no operand records a diagnostic", () => {
  const dsl = "text LIKE";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(dsl.slice(d.start, d.end), "LIKE");
  assert.match(d.hint, /no operand/i);
  assert.deepEqual(parsed, parseQuery(dsl));
});

test("dangling NOT records a diagnostic", () => {
  const dsl = "NOT";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  assert.equal(dsl.slice(diagnostics[0].start, diagnostics[0].end), "NOT");
  assert.deepEqual(parsed, parseQuery(dsl));
});

test("dangling '-' (minus/NOT shorthand) records a diagnostic", () => {
  const dsl = "status:todo -";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(dsl.slice(d.start, d.end), "-");
  assert.deepEqual(parsed, parseQuery(dsl));
});

test("stray trailing token after a well-formed expression records a diagnostic", () => {
  const dsl = "status:todo)";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.equal(diagnostics.length, 1);
  const d = diagnostics[0];
  assert.equal(dsl.slice(d.start, d.end), ")");
  assert.match(d.hint, /trailing/i);
  assert.deepEqual(parsed, parseQuery(dsl));
});

test("malformed token at predicate position (stray punctuation) records a diagnostic", () => {
  const dsl = "()";
  const { parsed, diagnostics } = parseQueryWithDiagnostics(dsl);
  assert.ok(diagnostics.length >= 1);
  assert.deepEqual(parsed, parseQuery(dsl));
});

// ────────────────────────────────────────────────────────────────────
// Invariant: parseQuery(s) === parseQueryWithDiagnostics(s).parsed
// for a broad corpus of malformed strings.
// ────────────────────────────────────────────────────────────────────

const MALFORMED_CORPUS = [
  "status:todo AND",
  "status:todo OR",
  "(status:todo",
  "((status:todo)",
  "status:todo)",
  "status:todo))",
  'text:"foo',
  'text:"',
  "status:todo blah status:done",
  "blah",
  "points >",
  "points >=",
  "status:",
  "status:)",
  "status:,",
  "text LIKE",
  "text NOT LIKE",
  "NOT",
  "-",
  "status:todo -",
  "()",
  "(",
  ")",
  ",",
  ":",
  "AND",
  "OR",
  "AND OR",
  "status:todo AND OR priority:high",
  "points BETWEEN 5",
  "points BETWEEN",
  "kind:",
  "ORDER BY",
  "status:todo ORDER BY",
  "tag IN",
  "tag IN (",
  "tag NOT",
  '"unterminated',
  "status:todo AND blah OR (priority:high",
  "   status:todo   ",
];

for (const dsl of MALFORMED_CORPUS) {
  test(`invariant: parseQuery matches parseQueryWithDiagnostics().parsed for ${JSON.stringify(dsl)}`, () => {
    const plain = parseQuery(dsl);
    const { parsed } = parseQueryWithDiagnostics(dsl);
    assert.deepEqual(parsed, plain);
  });
}
