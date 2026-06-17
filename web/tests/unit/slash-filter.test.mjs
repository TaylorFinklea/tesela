import assert from "node:assert/strict";
import test from "node:test";
import {
  slashFilter,
  flattenTree,
  flattenedSlashFilter,
} from "../../src/lib/editor/slash-filter.ts";

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

// ── Deep slash search (Logseq-style: /p1 jumps straight to a buried leaf) ──

const deepTree = [
  { label: "Heading" },
  { label: "Query" },
  {
    label: "Properties",
    children: [
      {
        label: "Priority",
        children: [{ label: "p1" }, { label: "p2" }, { label: "p3" }],
      },
      {
        label: "Status",
        children: [{ label: "todo" }, { label: "done" }],
      },
    ],
  },
];

test("flattenTree emits every group AND descendant leaf with its path + fullLabel", () => {
  const flat = flattenTree(deepTree);
  const byLabel = (l) => flat.find((e) => e.node.label === l);
  // top-level leaf: empty path, fullLabel == label
  assert.deepEqual(byLabel("Heading").path, []);
  assert.equal(byLabel("Heading").fullLabel, "Heading");
  // group node present
  assert.equal(byLabel("Priority").fullLabel, "Properties › Priority");
  // deep leaf carries full ancestor path + breadcrumb label
  const p1 = byLabel("p1");
  assert.deepEqual(p1.path, ["Properties", "Priority"]);
  assert.equal(p1.fullLabel, "Properties › Priority › p1");
  // every node (3 top + Priority + 3 + Status + 2 = 10) is flattened
  assert.equal(flat.length, 10);
});

test("/p1 surfaces the deep Priority value without manual descent", () => {
  const out = flattenedSlashFilter(deepTree, "p1");
  assert.equal(out[0].node.label, "p1");
  assert.equal(out[0].fullLabel, "Properties › Priority › p1");
});

test("typing a property name surfaces the group and its values", () => {
  const labels = flattenedSlashFilter(deepTree, "priority").map((e) => e.fullLabel);
  assert.ok(labels.includes("Properties › Priority"), "group node matches");
  assert.ok(labels.includes("Properties › Priority › p1"), "child value matches via path");
});

test("top-level verbs still match under deep flatten (no regression)", () => {
  assert.equal(flattenedSlashFilter(deepTree, "head")[0].node.label, "Heading");
});

test("empty query returns the full flattened tree in order", () => {
  assert.equal(flattenedSlashFilter(deepTree, "").length, 10);
  assert.equal(flattenedSlashFilter(deepTree, "")[0].node.label, "Heading");
});

test("no-match deep query returns empty", () => {
  assert.deepEqual(flattenedSlashFilter(deepTree, "zzzz"), []);
});
