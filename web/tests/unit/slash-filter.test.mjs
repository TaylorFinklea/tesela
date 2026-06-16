import assert from "node:assert/strict";
import test from "node:test";
import { slashFilter } from "../../src/lib/editor/slash-filter.ts";

const tree = [
  { label: "Heading" },
  { label: "Task" },
  { label: "Link" },
  { label: "Tag picker" },
  { label: "Date" },
  { label: "Template" },
  { label: "Query" },
  { label: "Collection" },
  { label: "Properties" },
];

test("empty query returns all items in original order", () => {
  const out = slashFilter(tree, "");
  assert.deepEqual(out.map((i) => i.label), tree.map((i) => i.label));
});

test("prefix match ranks above subsequence; Enter target is [0]", () => {
  const out = slashFilter(tree, "ta");
  // "Tag picker" + "Task" are prefix (score 1000); "Template" is subseq (lower).
  // Both should appear before Template, and [0] should be one of the prefix matches.
  assert.ok(out[0].label === "Task" || out[0].label === "Tag picker", `unexpected [0]: ${out[0].label}`);
  assert.ok(out.length >= 2);
  // Template must come AFTER the two prefix matches in the ranked list.
  const idxTemplate = out.findIndex((i) => i.label === "Template");
  const idxTask = out.findIndex((i) => i.label === "Task");
  const idxTag = out.findIndex((i) => i.label === "Tag picker");
  assert.ok(idxTemplate > idxTask && idxTemplate > idxTag, "Template should rank below Task/Tag picker");
});

test("query 'prop' surfaces Properties", () => {
  assert.equal(slashFilter(tree, "prop")[0].label, "Properties");
});

test("no-match query returns empty", () => {
  assert.deepEqual(slashFilter(tree, "zzzz"), []);
});

test("equal-score ties keep original tree order (stable)", () => {
  const eq = [{ label: "Date" }, { label: "Deadline" }];
  // Both "Date" and "Deadline" match prefix on "d" with score 1000 — keep original order.
  assert.deepEqual(slashFilter(eq, "d").map((i) => i.label), ["Date", "Deadline"]);
});

test("query 'head' returns Heading as [0]", () => {
  const out = slashFilter(tree, "head");
  assert.equal(out[0].label, "Heading");
});

test("whitespace-only query is treated as empty", () => {
  const out = slashFilter(tree, "   ");
  assert.equal(out.length, tree.length);
});
