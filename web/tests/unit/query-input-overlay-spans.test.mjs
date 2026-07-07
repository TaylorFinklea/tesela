// Unit tests for QueryInput's overlay span builder
// (web/src/lib/query-input/overlay-spans.ts) — tesela-vp9.2. Pure: given the
// raw source string plus optional diagnostics, returns the FULL span list
// (including whitespace gaps) the colored underlay renders — glyph-for-
// glyph identical to the real transparent-text input on top of it.
import { test } from "node:test";
import assert from "node:assert/strict";

import { buildOverlaySpans } from "../../src/lib/query-input/overlay-spans.ts";
import { parseQueryWithDiagnostics } from "../../src/lib/query-language.ts";

/** Every span's `text`, concatenated, must reconstruct the source exactly —
 *  the underlay's fundamental correctness invariant (no dropped/duplicated
 *  characters, since the input's real glyphs must line up with it). */
function assertReconstructs(input, spans) {
  assert.equal(spans.map((s) => s.text).join(""), input);
}

test("empty input — no spans", () => {
  assert.deepEqual(buildOverlaySpans(""), []);
});

test("whitespace-only input — a single 'text' gap span", () => {
  const spans = buildOverlaySpans("   ");
  assert.deepEqual(spans, [{ start: 0, end: 3, text: "   ", role: "text", diagnostic: false }]);
});

test("spans reconstruct the source exactly, across a representative query", () => {
  const input = 'status = "in progress" AND points BETWEEN 1 AND 10 ORDER BY points DESC';
  assertReconstructs(input, buildOverlaySpans(input));
});

test("adjacent tokens with no gap produce no 'text' filler between them", () => {
  // "status:todo" — key, colon, value are all glyph-adjacent (no whitespace).
  const spans = buildOverlaySpans("status:todo");
  assert.deepEqual(
    spans.map((s) => s.role),
    ["key", "operator", "value"],
  );
});

test("whitespace between tokens becomes an explicit 'text' gap span", () => {
  const spans = buildOverlaySpans("status = todo");
  assert.deepEqual(
    spans.map((s) => [s.role, s.text]),
    [
      ["key", "status"],
      ["text", " "],
      ["operator", "="],
      ["text", " "],
      ["value", "todo"],
    ],
  );
});

test("a quoted value's span includes its surrounding quote characters", () => {
  const spans = buildOverlaySpans('text = "hello world"');
  const value = spans.find((s) => s.role === "value");
  assert.equal(value.text, '"hello world"');
});

// ── diagnostics underline spans ───────────────────────────────────────────

test("no diagnostics — nothing marked", () => {
  const spans = buildOverlaySpans("status = todo", []);
  assert.ok(spans.every((s) => s.diagnostic === false));
});

test("a diagnostic spanning a dropped token marks exactly that token's span", () => {
  const input = "status:todo AND";
  const { diagnostics } = parseQueryWithDiagnostics(input);
  assert.equal(diagnostics.length, 1); // dangling AND
  const spans = buildOverlaySpans(input, diagnostics);
  const flagged = spans.filter((s) => s.diagnostic);
  assert.equal(flagged.length, 1);
  assert.equal(flagged[0].text, "AND");
});

test("a diagnostic overlapping a whitespace gap flags that gap span too", () => {
  // Unclosed paren diagnostic spans from '(' to end of input, crossing the
  // whitespace between ':' and 'todo'.
  const input = "(status: todo";
  const { diagnostics } = parseQueryWithDiagnostics(input);
  assert.equal(diagnostics.length, 1);
  const spans = buildOverlaySpans(input, diagnostics);
  const gap = spans.find((s) => s.role === "text");
  assert.ok(gap, "expected a whitespace gap span");
  assert.equal(gap.diagnostic, true);
});

test("diagnostics list is independent of role assignment — a clean query still colors normally", () => {
  const input = "status = todo";
  const { diagnostics } = parseQueryWithDiagnostics(input);
  assert.deepEqual(diagnostics, []);
  const spans = buildOverlaySpans(input, diagnostics);
  assert.deepEqual(
    spans.map((s) => s.role),
    ["key", "text", "operator", "text", "value"],
  );
});
