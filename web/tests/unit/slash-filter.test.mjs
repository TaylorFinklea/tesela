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
