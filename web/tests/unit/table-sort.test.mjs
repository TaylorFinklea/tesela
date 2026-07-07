import { test } from "node:test";
import assert from "node:assert/strict";
import { compareTableValues, sortByColumn } from "../../src/lib/table/table-sort.ts";

// tesela-ya4.3 — typed column sort. "cells typed per value_type" implies
// sort must be typed too, not a blind string compare (a text sort of
// "10" < "9" would misorder a number column).

test("text: falls back to locale string comparison", () => {
  assert.ok(compareTableValues("apple", "banana", "text") < 0);
  assert.ok(compareTableValues("banana", "apple", "text") > 0);
  assert.equal(compareTableValues("apple", "apple", "text"), 0);
});

test("number: compares numerically, not lexically (the '10' < '9' text-sort bug)", () => {
  // A naive string compare puts "10" before "9" (lexical: "1" < "9"). The
  // typed numeric compare must invert that: 9 < 10, so "9" sorts FIRST.
  assert.ok(compareTableValues("9", "10", "number") < 0, "9 must sort BEFORE 10 numerically");
  assert.ok(compareTableValues("10", "9", "number") > 0);
  assert.ok(compareTableValues("2", "10", "number") < 0);
});

test("number: a valid number sorts before an empty/non-numeric value", () => {
  assert.ok(compareTableValues("5", "", "number") < 0);
  assert.ok(compareTableValues("", "5", "number") > 0);
  assert.ok(compareTableValues("5", "n/a", "number") < 0);
});

test("checkbox: unchecked/empty sorts before checked", () => {
  assert.ok(compareTableValues("false", "true", "checkbox") < 0);
  assert.ok(compareTableValues("true", "false", "checkbox") > 0);
  assert.equal(compareTableValues("true", "true", "checkbox"), 0);
  assert.equal(compareTableValues("", "false", "checkbox"), 0, "empty is treated as unchecked");
});

test("select: ranks by declared choice order, not alphabetically", () => {
  const choices = ["low", "medium", "high"];
  assert.ok(compareTableValues("low", "high", "select", choices) < 0, "low ranks before high per choice order");
  assert.ok(compareTableValues("high", "medium", "select", choices) > 0);
  // Alphabetically "high" < "low", but choice-rank says otherwise — this
  // assertion is the one that would fail under a naive localeCompare sort.
  assert.ok(compareTableValues("high", "low", "select", choices) > 0);
});

test("select: an off-list value ranks after every declared choice", () => {
  const choices = ["low", "medium", "high"];
  assert.ok(compareTableValues("high", "unknown", "select", choices) < 0);
  assert.ok(compareTableValues("unknown", "low", "select", choices) > 0);
});

test("select: without choices, falls back to string comparison", () => {
  assert.ok(compareTableValues("a", "b", "select", null) < 0);
});

test("sortByColumn: ascending sorts a number column numerically and returns a NEW array", () => {
  const rows = [{ v: "10" }, { v: "2" }, { v: "9" }];
  const sorted = sortByColumn(rows, (r) => r.v, { value_type: "number" }, "asc");
  assert.deepEqual(sorted.map((r) => r.v), ["2", "9", "10"]);
  assert.notEqual(sorted, rows, "must not mutate the input array");
  assert.deepEqual(rows.map((r) => r.v), ["10", "2", "9"], "input order is untouched");
});

test("sortByColumn: descending reverses the ascending result", () => {
  const rows = [{ v: "10" }, { v: "2" }, { v: "9" }];
  const sorted = sortByColumn(rows, (r) => r.v, { value_type: "number" }, "desc");
  assert.deepEqual(sorted.map((r) => r.v), ["10", "9", "2"]);
});

test("sortByColumn: threads choices through for select-typed columns", () => {
  const rows = [{ v: "high" }, { v: "low" }, { v: "medium" }];
  const sorted = sortByColumn(
    rows,
    (r) => r.v,
    { value_type: "select", values: ["low", "medium", "high"] },
    "asc",
  );
  assert.deepEqual(sorted.map((r) => r.v), ["low", "medium", "high"]);
});
