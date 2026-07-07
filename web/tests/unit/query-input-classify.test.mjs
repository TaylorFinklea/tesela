// Unit tests for the JQL token-role classifier
// (web/src/lib/query-input/classify.ts) — tesela-vp9.2. A best-effort state
// machine over tokenize() output, NOT the grammar; these tests pin the
// role/key assignment the syntax-highlighting overlay and the completion
// caret-context both build on.
import { test } from "node:test";
import assert from "node:assert/strict";

import { tokenize } from "../../src/lib/query-language.ts";
import { classifyTokens } from "../../src/lib/query-input/classify.ts";

function roles(input) {
  return classifyTokens(tokenize(input)).map((c) => c.role);
}

function rolesAndKeys(input) {
  return classifyTokens(tokenize(input)).map((c) => [c.role, c.key]);
}

test("simple infix predicate: key op value", () => {
  assert.deepEqual(rolesAndKeys("status = todo"), [
    ["key", null],
    ["operator", "status"],
    ["value", "status"],
  ]);
});

test("legacy colon predicate", () => {
  assert.deepEqual(rolesAndKeys("status:todo"), [
    ["key", null],
    ["operator", "status"],
    ["value", "status"],
  ]);
});

test("tight comma multi-value sugar: every value shares the key", () => {
  assert.deepEqual(rolesAndKeys("status:backlog,todo"), [
    ["key", null],
    ["operator", "status"],
    ["value", "status"],
    ["comma", "status"],
    ["value", "status"],
  ]);
});

test("AND combinator resets the active key for the next predicate", () => {
  assert.deepEqual(rolesAndKeys("status = todo AND priority = high"), [
    ["key", null],
    ["operator", "status"],
    ["value", "status"],
    ["keyword", null], // AND
    ["key", null],
    ["operator", "priority"],
    ["value", "priority"],
  ]);
});

test("unary NOT / leading minus is a keyword, doesn't disturb the key that follows", () => {
  assert.deepEqual(rolesAndKeys("-status:done"), [
    ["keyword", null], // minus
    ["key", null],
    ["operator", "status"],
    ["value", "status"],
  ]);
  assert.deepEqual(roles("NOT status = done"), ["keyword", "key", "operator", "value"]);
});

test("IN (...) list: every item scoped to the key, parens tagged too", () => {
  assert.deepEqual(rolesAndKeys("tag IN (a, b, c)"), [
    ["key", null],
    ["keyword", "tag"], // IN
    ["paren", "tag"], // (
    ["value", "tag"],
    ["comma", "tag"],
    ["value", "tag"],
    ["comma", "tag"],
    ["value", "tag"],
    ["paren", null], // )
  ]);
});

test("NOT IN — NOT stays a bare keyword, IN still carries the key", () => {
  assert.deepEqual(rolesAndKeys("tag NOT IN (a, b)"), [
    ["key", null],
    ["keyword", null], // NOT
    ["keyword", "tag"], // IN
    ["paren", "tag"],
    ["value", "tag"],
    ["comma", "tag"],
    ["value", "tag"],
    ["paren", null],
  ]);
});

test("LIKE predicate with a quoted value", () => {
  assert.deepEqual(rolesAndKeys('text LIKE "%foo%"'), [
    ["key", null],
    ["keyword", "text"], // LIKE
    ["value", "text"],
  ]);
});

test("IS NULL / IS NOT NULL — meta keywords, no value tokens", () => {
  assert.deepEqual(roles("deadline IS NULL"), ["key", "keyword", "keyword"]);
  assert.deepEqual(roles("deadline IS NOT NULL"), ["key", "keyword", "keyword", "keyword"]);
});

test("BETWEEN a AND b — both bounds scoped to the key, internal AND is a keyword", () => {
  assert.deepEqual(rolesAndKeys("points BETWEEN 1 AND 10"), [
    ["key", null],
    ["keyword", "points"], // BETWEEN
    ["value", "points"], // 1
    ["keyword", null], // AND (internal, not a combinator)
    ["value", "points"], // 10
  ]);
});

test("grouping parens don't carry a key", () => {
  assert.deepEqual(rolesAndKeys("(status = todo OR status = doing)"), [
    ["paren", null],
    ["key", null],
    ["operator", "status"],
    ["value", "status"],
    ["keyword", null], // OR
    ["key", null],
    ["operator", "status"],
    ["value", "status"],
    ["paren", null],
  ]);
});

test("ORDER BY — every sort key classified 'key', not 'value', including after a comma", () => {
  assert.deepEqual(roles("ORDER BY points DESC, created ASC"), [
    "keyword", // ORDER
    "keyword", // BY
    "key", // points
    "keyword", // DESC
    "comma",
    "key", // created
    "keyword", // ASC
  ]);
});

test("empty and whitespace-only input classify to nothing", () => {
  assert.deepEqual(classifyTokens(tokenize("")), []);
  assert.deepEqual(classifyTokens(tokenize("   ")), []);
});
