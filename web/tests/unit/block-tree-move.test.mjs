import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  BLOCK_MOVE_MIME,
  classifyDropPlacement,
  decodeBlockMoveDragPayload,
  encodeBlockMoveDragPayload,
  extractSubtree,
  moveSubtreeDown,
  moveSubtreeUnder,
  moveSubtreeUp,
  outdentSubtreeToRoot,
  planBlockMove,
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

test("extractSubtree uses stable bids and includes collapsed descendants", () => {
  const blocks = [blk("root", 0), blk("child", 1), blk("grandchild", 2), blk("tail", 0)];
  assert.deepEqual(
    extractSubtree(blocks, "root-bid").map((b) => b.bid),
    ["root-bid", "child-bid", "grandchild-bid"],
  );
});

test("planBlockMove computes before, inside, after, and append placement", () => {
  const sourceBlocks = [blk("source", 1), blk("source-child", 2), blk("source-grandchild", 3)];
  const destinationBlocks = [
    blk("destination-parent", 0),
    blk("target", 1),
    blk("target-child", 2),
    blk("tail", 0),
  ];
  const cases = [
    {
      placement: "before",
      targetBid: "target-bid",
      expected: {
        insertionIndex: 1,
        destinationIndent: 1,
        destinationParentBid: "destination-parent-bid",
      },
    },
    {
      placement: "inside",
      targetBid: "target-bid",
      expected: {
        insertionIndex: 3,
        destinationIndent: 2,
        destinationParentBid: "target-bid",
      },
    },
    {
      placement: "after",
      targetBid: "target-bid",
      expected: {
        insertionIndex: 3,
        destinationIndent: 1,
        destinationParentBid: "destination-parent-bid",
      },
    },
    {
      placement: "append",
      targetBid: null,
      expected: {
        insertionIndex: 4,
        destinationIndent: 0,
        destinationParentBid: null,
      },
    },
  ];

  for (const { placement, targetBid, expected } of cases) {
    const plan = planBlockMove({
      sourceBlocks,
      rootBid: "source-bid",
      destinationBlocks,
      targetBid,
      placement,
      sameNote: false,
    });

    assert.deepEqual(plan.subtreeBids, [
      "source-bid",
      "source-child-bid",
      "source-grandchild-bid",
    ]);
    assert.deepEqual(
      {
        insertionIndex: plan.insertionIndex,
        destinationIndent: plan.destinationIndent,
        destinationParentBid: plan.destinationParentBid,
      },
      expected,
      placement,
    );
    assert.equal(plan.noOp, false, placement);
  }
});

test("planBlockMove preserves descendant indentation relative to the moved root", () => {
  const sourceBlocks = [blk("source", 2), blk("child", 3), blk("grandchild", 4)];
  const destinationBlocks = [blk("target", 1)];

  const plan = planBlockMove({
    sourceBlocks,
    rootBid: "source-bid",
    destinationBlocks,
    targetBid: "target-bid",
    placement: "inside",
    sameNote: false,
  });
  const subtree = extractSubtree(sourceBlocks, "source-bid");
  const projectedIndents = subtree.map(
    (block) => plan.destinationIndent + block.indent_level - subtree[0].indent_level,
  );

  assert.deepEqual(projectedIndents, [2, 3, 4]);
});

test("planBlockMove adjusts same-note insertion after conceptually removing the source", () => {
  const blocks = [
    blk("head", 0),
    blk("source", 0),
    blk("source-child", 1),
    blk("target", 0),
    blk("target-child", 1),
    blk("tail", 0),
  ];

  const plan = planBlockMove({
    sourceBlocks: blocks,
    rootBid: "source-bid",
    destinationBlocks: blocks,
    targetBid: "target-bid",
    placement: "after",
    sameNote: true,
  });

  assert.equal(plan.insertionIndex, 3);
  assert.equal(plan.noOp, false);
});

test("planBlockMove marks an already-satisfied same-note placement as a no-op", () => {
  const blocks = [blk("head", 0), blk("source", 0), blk("source-child", 1), blk("target", 0)];

  const plan = planBlockMove({
    sourceBlocks: blocks,
    rootBid: "source-bid",
    destinationBlocks: blocks,
    targetBid: "target-bid",
    placement: "before",
    sameNote: true,
  });

  assert.equal(plan.insertionIndex, 1);
  assert.equal(plan.noOp, true);
});

test("planBlockMove rejects self and descendant targets", () => {
  const blocks = [blk("source", 0), blk("child", 1), blk("grandchild", 2), blk("tail", 0)];

  for (const targetBid of ["source-bid", "child-bid", "grandchild-bid"]) {
    assert.throws(
      () => planBlockMove({
        sourceBlocks: blocks,
        rootBid: "source-bid",
        destinationBlocks: blocks,
        targetBid,
        placement: "inside",
        sameNote: true,
      }),
      /source subtree/,
      targetBid,
    );
  }
});

test("planBlockMove enforces target requirements for each placement", () => {
  const sourceBlocks = [blk("source", 0)];
  const destinationBlocks = [blk("target", 0)];

  for (const placement of ["before", "inside", "after"]) {
    assert.throws(
      () => planBlockMove({
        sourceBlocks,
        rootBid: "source-bid",
        destinationBlocks,
        targetBid: null,
        placement,
        sameNote: false,
      }),
      /requires a target bid/,
      placement,
    );
  }
  assert.throws(
    () => planBlockMove({
      sourceBlocks,
      rootBid: "source-bid",
      destinationBlocks,
      targetBid: "target-bid",
      placement: "append",
      sameNote: false,
    }),
    /requires a null target bid/,
  );
});

test("classifyDropPlacement divides a block row into exact vertical thirds", () => {
  const rect = { top: 100, height: 90 };
  const cases = [
    [99, "before"],
    [129.999, "before"],
    [130, "inside"],
    [159.999, "inside"],
    [160, "after"],
    [191, "after"],
  ];

  for (const [clientY, expected] of cases) {
    assert.equal(classifyDropPlacement(clientY, rect), expected, String(clientY));
  }
});

test("decodeBlockMoveDragPayload rejects external and malformed drag data", () => {
  const payload = {
    move_id: "11111111-1111-4111-8111-111111111111",
    source_note_id: "2026-07-12",
    root_bid: "22222222-2222-4222-8222-222222222222",
  };
  const raw = JSON.stringify(payload);
  assert.equal(decodeBlockMoveDragPayload(["text/plain"], raw), null);
  assert.deepEqual(decodeBlockMoveDragPayload([BLOCK_MOVE_MIME], raw), payload);
  assert.equal(decodeBlockMoveDragPayload([BLOCK_MOVE_MIME], "{"), null);
});

test("block move drag payload parsing requires exact fields and UUID locators", () => {
  const payload = {
    move_id: "11111111-1111-4111-8111-111111111111",
    source_note_id: "2026-07-12",
    root_bid: "22222222-2222-4222-8222-222222222222",
  };
  const invalidPayloads = [
    { ...payload, extra: true },
    { source_note_id: payload.source_note_id, root_bid: payload.root_bid },
    { ...payload, move_id: "not-a-uuid" },
    { ...payload, root_bid: "not-a-uuid" },
    { ...payload, source_note_id: "" },
    null,
    [],
  ];

  assert.equal(
    decodeBlockMoveDragPayload([`${BLOCK_MOVE_MIME}; charset=utf-8`], JSON.stringify(payload)),
    null,
  );
  for (const invalid of invalidPayloads) {
    assert.equal(
      decodeBlockMoveDragPayload([BLOCK_MOVE_MIME], JSON.stringify(invalid)),
      null,
    );
  }
});

test("encodeBlockMoveDragPayload serializes only a valid internal locator", () => {
  const payload = {
    move_id: "11111111-1111-4111-8111-111111111111",
    source_note_id: "2026-07-12",
    root_bid: "22222222-2222-4222-8222-222222222222",
  };

  assert.deepEqual(JSON.parse(encodeBlockMoveDragPayload(payload)), payload);
  assert.throws(
    () => encodeBlockMoveDragPayload({ ...payload, move_id: "not-a-uuid" }),
    /valid block move drag payload/,
  );
});
