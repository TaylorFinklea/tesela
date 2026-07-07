// Unit tests for QueryInput's completion caret-context classification
// (web/src/lib/query-input/caret-context.ts) — tesela-vp9.2. Given the raw
// source string and a cursor offset, decides which completion tier (key /
// operator / value / none) applies, the replace range, and — for the value
// tier — which predicate key governs it.
import { test } from "node:test";
import assert from "node:assert/strict";

import { caretContext } from "../../src/lib/query-input/caret-context.ts";

/** Cursor at the very end of `input` — the common "still typing" case. */
function atEnd(input) {
  return caretContext(input, input.length);
}

test("empty input at the start — key tier, nothing typed yet", () => {
  const c = atEnd("");
  assert.equal(c.tier, "key");
  assert.equal(c.from, 0);
  assert.equal(c.to, 0);
  assert.equal(c.prefix, "");
  assert.equal(c.key, null);
});

test("typing a fresh key from scratch — key tier, prefix is what's typed", () => {
  const c = atEnd("sta");
  assert.equal(c.tier, "key");
  assert.equal(c.from, 0);
  assert.equal(c.to, 3);
  assert.equal(c.prefix, "sta");
});

test("right after a completed key (trailing space) — operator tier", () => {
  const input = "status ";
  const c = atEnd(input);
  assert.equal(c.tier, "operator");
  assert.equal(c.from, input.length);
  assert.equal(c.to, input.length);
  assert.equal(c.prefix, "");
});

test("caret at the end of an as-yet-unfinished key — key tier (still typing it)", () => {
  // The word itself IS the partial being typed; there's no "previous"
  // token yet for the caret to react to.
  const c = atEnd("status");
  assert.equal(c.tier, "key");
  assert.equal(c.prefix, "status");
});

test("after an operator + space — value tier, key carried from the operator", () => {
  const c = atEnd("status = ");
  assert.equal(c.tier, "value");
  assert.equal(c.key, "status");
  assert.equal(c.prefix, "");
});

test("after an operator, mid-value-typing — value tier with the partial prefix", () => {
  const c = atEnd("status = tod");
  assert.equal(c.tier, "value");
  assert.equal(c.key, "status");
  assert.equal(c.prefix, "tod");
});

test("legacy colon operator also resolves to value tier", () => {
  const c = atEnd("status:");
  assert.equal(c.tier, "value");
  assert.equal(c.key, "status");
});

test("tight comma continuation (key:v1,v2 sugar) — value tier, same key", () => {
  const c = atEnd("status:backlog,");
  assert.equal(c.tier, "value");
  assert.equal(c.key, "status");
  assert.equal(c.prefix, "");
});

test("mid-typing the second value after a tight comma", () => {
  const c = atEnd("status:backlog,t");
  assert.equal(c.tier, "value");
  assert.equal(c.key, "status");
  assert.equal(c.prefix, "t");
});

test("after IN ( — value tier, key carried through the paren", () => {
  const c = atEnd("tag IN (");
  assert.equal(c.tier, "value");
  assert.equal(c.key, "tag");
});

test("after a closed predicate + space — key tier again (implicit AND)", () => {
  const c = atEnd("status = todo ");
  assert.equal(c.tier, "key");
  assert.equal(c.key, null);
});

test("after AND — key tier for the next predicate", () => {
  const c = atEnd("status = todo AND ");
  assert.equal(c.tier, "key");
});

test("after a grouping ')' — key tier", () => {
  const c = atEnd("(status = todo) ");
  assert.equal(c.tier, "key");
});

// ── quoted strings ──────────────────────────────────────────────────────

test("cursor right after a closed quoted value — key tier (predicate finished)", () => {
  const c = atEnd('status = "todo" ');
  assert.equal(c.tier, "key");
});

test("cursor inside an open quoted string — mid-token, no suggestion", () => {
  const input = 'status = "in prog'; // unterminated quote, still being typed
  const c = caretContext(input, input.length - 2); // caret inside "prog, not at its end
  assert.equal(c.tier, "none");
});

// ── mid-token caret ──────────────────────────────────────────────────────

test("caret strictly inside an existing word — no suggestion", () => {
  const c = caretContext("status", 3); // sta|tus
  assert.equal(c.tier, "none");
});

test("out-of-range cursor is rejected defensively", () => {
  assert.equal(caretContext("status", -1).tier, "none");
  assert.equal(caretContext("status", 99).tier, "none");
});

// ── leading negation dash ──────────────────────────────────────────────

test("leading '-' (NOT shorthand) doesn't get swallowed into the key prefix", () => {
  const c = atEnd("-stat");
  assert.equal(c.tier, "key");
  assert.equal(c.from, 1); // after the '-'
  assert.equal(c.prefix, "stat");
});
