import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  moveSubtreeDown,
  moveSubtreeUnder,
  moveSubtreeUp,
  outdentSubtreeToRoot,
} from "../../src/lib/block-tree-move.ts";

function blk(id, indent_level) {
  return {
    id,
    bid: `${id}-bid`,
    text: id,
    raw_text: id,
    tags: [],
    inline_tags: [],
    trailing_tags: [],
    inherited_tags: [],
    properties: {},
    indent_level,
    note_id: "note",
    parent_note_type: null,
  };
}

function shape(blocks) {
  return blocks.map((b) => `${b.id}:${b.indent_level}`);
}

test("moveSubtreeUp swaps the selected block plus descendants with the previous sibling subtree", () => {
  const blocks = [
    blk("a", 0),
    blk("a1", 1),
    blk("b", 0),
    blk("b1", 1),
    blk("b1a", 2),
    blk("c", 0),
  ];

  const result = moveSubtreeUp(blocks, "b");

  assert.equal(result.changed, true);
  assert.equal(result.focusedId, "b");
  assert.deepEqual(shape(result.blocks), ["b:0", "b1:1", "b1a:2", "a:0", "a1:1", "c:0"]);
});

test("moveSubtreeDown swaps the selected block plus descendants with the next sibling subtree", () => {
  const blocks = [
    blk("a", 0),
    blk("a1", 1),
    blk("b", 0),
    blk("b1", 1),
    blk("c", 0),
    blk("c1", 1),
  ];

  const result = moveSubtreeDown(blocks, "b");

  assert.equal(result.changed, true);
  assert.equal(result.focusedId, "b");
  assert.deepEqual(shape(result.blocks), ["a:0", "a1:1", "c:0", "c1:1", "b:0", "b1:1"]);
});

test("outdentSubtreeToRoot subtracts the selected depth from the whole subtree", () => {
  const blocks = [
    blk("parent", 0),
    blk("child", 1),
    blk("grandchild", 2),
    blk("sibling", 1),
  ];

  const result = outdentSubtreeToRoot(blocks, "child");

  assert.equal(result.changed, true);
  assert.deepEqual(shape(result.blocks), ["parent:0", "child:0", "grandchild:1", "sibling:1"]);
});

test("moveSubtreeUnder moves a block plus descendants under another parent and rebases descendants", () => {
  const blocks = [
    blk("target", 0),
    blk("target-child", 1),
    blk("source", 0),
    blk("source-child", 1),
    blk("tail", 0),
  ];

  const result = moveSubtreeUnder(blocks, "source", "target");

  assert.equal(result.changed, true);
  assert.deepEqual(shape(result.blocks), ["target:0", "target-child:1", "source:1", "source-child:2", "tail:0"]);
});

test("moveSubtreeUnder refuses to move a block under its own descendant", () => {
  const blocks = [blk("source", 0), blk("child", 1), blk("tail", 0)];

  const result = moveSubtreeUnder(blocks, "source", "child");

  assert.equal(result.changed, false);
  assert.deepEqual(shape(result.blocks), ["source:0", "child:1", "tail:0"]);
});
