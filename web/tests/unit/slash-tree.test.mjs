import assert from "node:assert/strict";
import test from "node:test";
import { buildSlashTree } from "../../src/lib/editor/slash-tree.ts";

// 8 insertion verbs in the order they appear in the slash menu (mirrors
// registry registration order: heading, task, link, tag, date, template,
// query, collection). Widget is intentionally absent — it moved to the
// leader `new` bucket in Phase C.
const verbLeaves = [
  { key: "h", label: "Heading",   action: () => {} },
  { key: "t", label: "Task",      action: () => {} },
  { key: "l", label: "Link",      action: () => {} },
  { key: "T", label: "Tag picker", action: () => {} },
  { key: "d", label: "Date",      action: () => {} },
  { key: "m", label: "Template",  action: () => {} },
  { key: "q", label: "Query",     action: () => {} },
  { key: "c", label: "Collection", action: () => {} },
];

test("untyped block: 8 verbs + Properties→Manual leaf, no hoisted props, no /s", () => {
  const tree = buildSlashTree({
    verbLeaves,
    propertyChildren: [{ key: "k", label: "Manual key:: value", action: () => {} }],
  });
  assert.equal(tree.length, 9);
  assert.equal(tree[8].label, "Properties");
  assert.equal(tree[8].key, "p");
  assert.deepEqual(tree[8].children.map((c) => c.label), ["Manual key:: value"]);
  assert.equal(tree.find((n) => n.key === "s"), undefined); // /s fallback dropped
  assert.equal(tree.find((n) => n.label === "New widget"), undefined); // widget gone
  // The 8 verbs occupy slots 0..7 in registration order.
  assert.deepEqual(
    tree.slice(0, 8).map((n) => n.label),
    ["Heading", "Task", "Link", "Tag picker", "Date", "Template", "Query", "Collection"],
  );
});

test("#Task block: Properties children are the tag-scoped defs, NOT hoisted to top level", () => {
  const taskProps = [
    { key: "s", label: "Status" },
    { key: "p", label: "Priority" },
    { key: "d", label: "Deadline" },
    { key: "S", label: "Scheduled" },
    { key: "o", label: "Points" },
  ];
  const tree = buildSlashTree({ verbLeaves, propertyChildren: taskProps });
  // Properties is ONE top-level row; the 5 defs live UNDER it, not hoisted.
  assert.equal(tree.length, 9);
  const props = tree.find((n) => n.label === "Properties");
  assert.deepEqual(
    props.children.map((c) => c.label),
    ["Status", "Priority", "Deadline", "Scheduled", "Points"],
  );
  // none of the defs leaked to the top level
  for (const label of ["Status", "Priority", "Deadline", "Scheduled", "Points"]) {
    assert.equal(
      tree.filter((n) => n.label === label).length,
      0,
      `def "${label}" should not be at the top level`,
    );
  }
});
