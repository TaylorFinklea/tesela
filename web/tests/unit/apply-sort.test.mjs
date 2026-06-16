import { test } from "node:test";
import assert from "node:assert/strict";
import { applySort } from "../../src/lib/query-language.ts";

// Rows are plain {id, p:{...}} objects; the resolver reads p[key].
const rows = [
  { id: "ship", p: { points: "10", deadline: "2026-08-31", type: "task" } },
  { id: "typo", p: { points: "2", deadline: "2026-06-30", type: "task" } },
  { id: "est", p: { points: "3.0", deadline: "2026-07-15", type: "task" } },
];
const fv = (r, key) => r.p[key] ?? "";
const ids = (xs) => xs.map((x) => x.id);

const NUM = new Map([["points", "number"]]);
const DATE = new Map([["deadline", "date"]]);

test("no sort string → original order, new array (not mutated)", () => {
  const out = applySort(rows, null, fv);
  assert.deepEqual(ids(out), ["ship", "typo", "est"]);
  assert.notEqual(out, rows); // copy
  assert.deepEqual(ids(rows), ["ship", "typo", "est"]); // input untouched
});

test("typed numeric sort — points asc orders 2 < 3 < 10 (NOT string)", () => {
  assert.deepEqual(ids(applySort(rows, "points asc", fv, NUM)), ["typo", "est", "ship"]);
});

test("typed numeric sort — points desc orders 10 > 3 > 2", () => {
  assert.deepEqual(ids(applySort(rows, "points desc", fv, NUM)), ["ship", "est", "typo"]);
});

test("default direction is ascending when omitted", () => {
  assert.deepEqual(ids(applySort(rows, "points", fv, NUM)), ["typo", "est", "ship"]);
});

test("WITHOUT a number type, points sorts lexicographically (10 < 2 < 3)", () => {
  // Proves the typed registry is what makes numeric order correct.
  assert.deepEqual(ids(applySort(rows, "points asc", fv)), ["ship", "typo", "est"]);
});

test("typed date sort — ISO deadlines order chronologically", () => {
  assert.deepEqual(ids(applySort(rows, "deadline asc", fv, DATE)), ["typo", "est", "ship"]);
  assert.deepEqual(ids(applySort(rows, "deadline desc", fv, DATE)), ["ship", "est", "typo"]);
});

test("multi-key sort — first key wins, second breaks ties", () => {
  const r = [
    { id: "a", p: { g: "x", points: "2" } },
    { id: "b", p: { g: "x", points: "1" } },
    { id: "c", p: { g: "y", points: "9" } },
  ];
  // g asc, then points desc within g=x
  const out = applySort(r, "g asc, points desc", (x, k) => x.p[k] ?? "", new Map([["points", "number"]]));
  assert.deepEqual(out.map((x) => x.id), ["a", "b", "c"]);
});

test("stable — equal keys preserve input order", () => {
  const r = [
    { id: "first", p: { type: "task" } },
    { id: "second", p: { type: "task" } },
    { id: "third", p: { type: "task" } },
  ];
  assert.deepEqual(
    applySort(r, "type asc", (x, k) => x.p[k] ?? "").map((x) => x.id),
    ["first", "second", "third"],
  );
});
