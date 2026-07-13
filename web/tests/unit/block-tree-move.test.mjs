import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

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
import * as blockTreeMove from "../../src/lib/block-tree-move.ts";

const blockOutlinerSource = readFileSync(
  new URL("../../src/lib/components/BlockOutliner.svelte", import.meta.url),
  "utf8",
);
const blockEditorSource = readFileSync(
  new URL("../../src/lib/components/BlockEditor.svelte", import.meta.url),
  "utf8",
);
const journalViewSource = readFileSync(
  new URL("../../src/lib/components/JournalView.svelte", import.meta.url),
  "utf8",
);

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

function sourceBetween(source, start, end) {
  return source.slice(source.indexOf(start), source.indexOf(end));
}

function focusRestorationController() {
  assert.equal(
    typeof blockTreeMove.createFocusRestorationController,
    "function",
    "focus restoration must be guarded by a revocable ownership lease",
  );
  return blockTreeMove.createFocusRestorationController();
}

test("a revoked planned focus restoration cannot reclaim ownership at completion", async () => {
  const controller = focusRestorationController();
  assert.equal(typeof controller.claim, "function");
  const claim = controller.claim();
  const focused = [];

  controller.revoke();
  const restored = await controller.restore(claim, {
    maxAttempts: 1,
    findTarget: () => ({ bid: "completed-move" }),
    waitForRetry: async () => {},
    focusTarget: (value) => focused.push(value),
  });

  assert.equal(restored, false);
  assert.deepEqual(focused, []);
});

test("focus restoration stops when its lease is canceled before the target appears", async () => {
  const controller = focusRestorationController();
  const claim = controller.claim();
  let releaseRetry;
  let target = null;
  const focused = [];
  const retry = new Promise((resolve) => { releaseRetry = resolve; });

  const restoration = controller.restore(claim, {
    maxAttempts: 2,
    findTarget: () => target,
    waitForRetry: () => retry,
    focusTarget: (value) => focused.push(value),
  });

  controller.revoke();
  target = { bid: "moved-root" };
  releaseRetry();

  assert.equal(await restoration, false);
  assert.deepEqual(focused, []);
});

test("uncontested focus restoration retries and focuses exactly once", async () => {
  const controller = focusRestorationController();
  const claim = controller.claim();
  const target = { bid: "moved-root" };
  const focused = [];
  let attempts = 0;

  const restored = await controller.restore(claim, {
    maxAttempts: 3,
    findTarget: () => ++attempts === 1 ? null : target,
    waitForRetry: async () => {},
    focusTarget: (value) => focused.push(value),
  });

  assert.equal(restored, true);
  assert.equal(attempts, 2);
  assert.deepEqual(focused, [target]);
});

test("a newer focus restoration supersedes an older retrying restoration", async () => {
  const controller = focusRestorationController();
  let releaseOldRetry;
  let oldTarget = null;
  const newTarget = { bid: "new-owner" };
  const focused = [];
  const oldRetry = new Promise((resolve) => { releaseOldRetry = resolve; });

  const oldClaim = controller.claim();
  const oldRestoration = controller.restore(oldClaim, {
    maxAttempts: 2,
    findTarget: () => oldTarget,
    waitForRetry: () => oldRetry,
    focusTarget: (value) => focused.push(value),
  });
  const newClaim = controller.claim();
  const newRestoration = controller.restore(newClaim, {
    maxAttempts: 1,
    findTarget: () => newTarget,
    waitForRetry: async () => {},
    focusTarget: (value) => focused.push(value),
  });

  oldTarget = { bid: "old-owner" };
  releaseOldRetry();

  assert.equal(await newRestoration, true);
  assert.equal(await oldRestoration, false);
  assert.deepEqual(focused, [newTarget]);
});

test("a newer focus restoration supersedes an owner awaiting its first lookup", async () => {
  const controller = focusRestorationController();
  let releaseOldLookup;
  const oldLookup = new Promise((resolve) => { releaseOldLookup = resolve; });
  const oldTarget = { bid: "old-owner" };
  const newTarget = { bid: "new-owner" };
  const focused = [];

  const oldClaim = controller.claim();
  const oldRestoration = controller.restore(oldClaim, {
    maxAttempts: 1,
    findTarget: async () => {
      await oldLookup;
      return oldTarget;
    },
    waitForRetry: async () => {},
    focusTarget: (value) => focused.push(value),
  });
  const newClaim = controller.claim();
  const newRestoration = controller.restore(newClaim, {
    maxAttempts: 1,
    findTarget: () => newTarget,
    waitForRetry: async () => {},
    focusTarget: (value) => focused.push(value),
  });

  releaseOldLookup();

  assert.equal(await newRestoration, true);
  assert.equal(await oldRestoration, false);
  assert.deepEqual(focused, [newTarget]);
});

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

const MOVE_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const SOURCE_BID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

function requestForSession() {
  return {
    move_id: MOVE_ID,
    source_note_id: "2026-07-12",
    root_bid: SOURCE_BID,
    destination_note_id: "2026-07-12",
    target_bid: null,
    placement: "append",
  };
}

test("block move session covers start, target, submit, and success transitions", () => {
  assert.equal(typeof blockTreeMove.reduceBlockMoveSession, "function");
  assert.ok(blockTreeMove.IDLE_BLOCK_MOVE_SESSION);
  const request = requestForSession();

  const selecting = blockTreeMove.reduceBlockMoveSession(
    blockTreeMove.IDLE_BLOCK_MOVE_SESSION,
    { type: "start", request },
  );
  assert.deepEqual(selecting, {
    phase: "selecting",
    request,
    targetBid: null,
    targetNoteId: null,
    placement: null,
  });

  const targeted = blockTreeMove.reduceBlockMoveSession(selecting, {
    type: "target",
    noteId: "2026-07-11",
    bid: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
    placement: "inside",
  });
  assert.equal(targeted.phase, "selecting");
  assert.equal(targeted.targetNoteId, "2026-07-11");
  assert.equal(targeted.targetBid, "cccccccc-cccc-4ccc-8ccc-cccccccccccc");
  assert.equal(targeted.placement, "inside");
  assert.deepEqual(targeted.request, {
    ...request,
    destination_note_id: "2026-07-11",
    target_bid: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
    placement: "inside",
  });

  const pending = blockTreeMove.reduceBlockMoveSession(targeted, { type: "submit" });
  assert.equal(pending.phase, "pending");
  assert.strictEqual(pending.request, targeted.request);
  assert.deepEqual(
    blockTreeMove.reduceBlockMoveSession(pending, { type: "success" }),
    blockTreeMove.IDLE_BLOCK_MOVE_SESSION,
  );
});

test("cancel clears only selection while ordinary error clears submitted state", () => {
  const request = requestForSession();
  const selecting = blockTreeMove.reduceBlockMoveSession(
    blockTreeMove.IDLE_BLOCK_MOVE_SESSION,
    { type: "start", request },
  );
  const pending = blockTreeMove.reduceBlockMoveSession(selecting, { type: "submit" });

  assert.deepEqual(
    blockTreeMove.reduceBlockMoveSession(selecting, { type: "cancel" }),
    blockTreeMove.IDLE_BLOCK_MOVE_SESSION,
  );
  assert.strictEqual(
    blockTreeMove.reduceBlockMoveSession(pending, { type: "cancel" }),
    pending,
  );
  for (const state of [selecting, pending]) {
    assert.deepEqual(
      blockTreeMove.reduceBlockMoveSession(state, { type: "ordinary-error" }),
      blockTreeMove.IDLE_BLOCK_MOVE_SESSION,
      state.phase,
    );
  }
});

test("recoverable error retains the exact move id for retry", () => {
  const request = requestForSession();
  const selecting = blockTreeMove.reduceBlockMoveSession(
    blockTreeMove.IDLE_BLOCK_MOVE_SESSION,
    { type: "start", request },
  );
  const pending = blockTreeMove.reduceBlockMoveSession(selecting, { type: "submit" });
  const retryable = blockTreeMove.reduceBlockMoveSession(pending, { type: "recoverable-error" });
  assert.equal(retryable.phase, "retryable");
  assert.equal(retryable.request.move_id, request.move_id);
  assert.strictEqual(retryable.request, request);

  const retried = blockTreeMove.reduceBlockMoveSession(retryable, { type: "submit" });
  assert.equal(retried.phase, "pending");
  assert.strictEqual(retried.request, request);
});

test("block move session ignores transitions that are invalid for its phase", () => {
  const request = requestForSession();
  const idle = blockTreeMove.IDLE_BLOCK_MOVE_SESSION;
  const selecting = blockTreeMove.reduceBlockMoveSession(idle, { type: "start", request });
  const targeted = blockTreeMove.reduceBlockMoveSession(selecting, {
    type: "target",
    noteId: "2026-07-11",
    bid: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
    placement: "before",
  });
  const pending = blockTreeMove.reduceBlockMoveSession(targeted, { type: "submit" });
  const retryable = blockTreeMove.reduceBlockMoveSession(pending, { type: "recoverable-error" });

  assert.strictEqual(blockTreeMove.reduceBlockMoveSession(idle, { type: "submit" }), idle);
  assert.strictEqual(blockTreeMove.reduceBlockMoveSession(idle, { type: "recoverable-error" }), idle);
  assert.strictEqual(
    blockTreeMove.reduceBlockMoveSession(idle, {
      type: "target",
      noteId: "2026-07-11",
      bid: null,
      placement: "append",
    }),
    idle,
  );
  assert.strictEqual(
    blockTreeMove.reduceBlockMoveSession(pending, {
      type: "target",
      noteId: "2026-07-10",
      bid: null,
      placement: "append",
    }),
    pending,
  );
  assert.strictEqual(
    blockTreeMove.reduceBlockMoveSession(retryable, {
      type: "target",
      noteId: "2026-07-10",
      bid: null,
      placement: "append",
    }),
    retryable,
  );
  assert.strictEqual(
    blockTreeMove.reduceBlockMoveSession(retryable, { type: "cancel" }),
    retryable,
  );
  assert.strictEqual(
    blockTreeMove.reduceBlockMoveSession(retryable, { type: "ordinary-error" }),
    idle,
  );
  const replacement = { ...request, move_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd" };
  assert.strictEqual(
    blockTreeMove.reduceBlockMoveSession(pending, { type: "start", request: replacement }),
    pending,
  );
  assert.strictEqual(
    blockTreeMove.reduceBlockMoveSession(retryable, { type: "start", request: replacement }),
    retryable,
  );
});

test("relocation preparation follows an outliner that becomes nonempty after mount", () => {
  assert.match(blockOutlinerSource, /use:relocationPrepare/);
});

test("inside preparation reveals a folded keyboard target before transport", () => {
  assert.match(journalViewSource, /expandInsideBid\s*=\s*request\.placement === "inside"/);
  assert.match(blockOutlinerSource, /prepareOutlinerForRelocation\(\s*addressedBids[^)]*expandInsideBid/s);
});

test("same-note Alt routes before marking the outliner locally dirty", () => {
  for (const [start, end] of [
    ["function handleMoveBlock", "function handleMoveUnderPrevious"],
    ["function handleMoveUnderPrevious", "function handleOutdentToRoot"],
  ]) {
    const body = blockOutlinerSource.slice(
      blockOutlinerSource.indexOf(start),
      blockOutlinerSource.indexOf(end),
    );
    assert.ok(body.indexOf("if (relocation)") < body.indexOf("lastLocalEditAt = Date.now()"));
  }
});

test("same-note Alt keeps client-minted endpoints inert", () => {
  assert.match(blockOutlinerSource, /sameNoteRelocationHasStableEndpoints/);
  assert.match(blockOutlinerSource, /\|\| !prev\.bid/);
});

test("an untouched empty seed lets an internal drop bubble to day append", () => {
  assert.match(blockOutlinerSource, /isUntouchedEmptySeed/);
  assert.match(blockOutlinerSource, /if \(isUntouchedEmptySeed\(block\)\) return;/);
});

test("a canonical moved body replaces an untouched mounted synthetic seed", () => {
  const dirtyGuard = sourceBetween(
    blockOutlinerSource,
    "function hasUnsavedLocalEdits",
    "function applyExternalReparse",
  );
  const reparse = sourceBetween(
    blockOutlinerSource,
    "function applyExternalReparse",
    "// Clear undo/redo on page navigation",
  );
  assert.match(
    dirtyGuard,
    /isClientMintedId\(b\.id\)\s*&&\s*!isUntouchedEmptySeed\(b\)/,
  );
  assert.match(
    reparse,
    /isClientMintedId\(focusedId\)\s*&&\s*!isUntouchedEmptySeed\(focusedBlock\)/,
  );
});

test("stable block keys survive line shifts and keep fallback identities collision-free", () => {
  assert.equal(typeof blockTreeMove.stableBlockKey, "function");
  const stableBlockKey = blockTreeMove.stableBlockKey;
  const beforeDelete = { id: "2026-07-12:1", bid: SOURCE_BID };
  const afterDelete = { id: "2026-07-12:0", bid: SOURCE_BID };

  assert.equal(stableBlockKey(beforeDelete), stableBlockKey(afterDelete));
  assert.notEqual(
    stableBlockKey({ id: "2026-07-12:new-a", bid: null }),
    stableBlockKey({ id: "2026-07-12:new-b", bid: null }),
  );
  assert.notEqual(
    stableBlockKey(beforeDelete),
    stableBlockKey({ id: `bid:${SOURCE_BID}`, bid: null }),
    "bid and fallback key namespaces must not collide",
  );
});

test("outliner row ownership and focused reparses use the stable block key", () => {
  const reparse = sourceBetween(
    blockOutlinerSource,
    "function applyExternalReparse",
    "// Clear undo/redo on page navigation",
  );

  assert.match(
    blockOutlinerSource,
    /\{#each visibleBlocks as block, vi \(stableBlockKey\(block\)\)\}/,
  );
  assert.match(reparse, /stableBlockKey\(focusedBlock\)/);
  assert.match(reparse, /stableBlockKey\(b\)\s*===\s*focusedKey/);
});

test("focusing a rekeyed row republishes that exact block", () => {
  const editor = sourceBetween(
    blockOutlinerSource,
    "<BlockEditor",
    "<!-- Display chips",
  );

  assert.match(
    editor,
    /onfocus=\{\(\) => \{[\s\S]*?focusedIndex = vi;[\s\S]*?onfocusedblockchange\?\.\(block\)/,
  );
});

test("focus and Loro undo routing share the row's stable editor key", () => {
  const editorProps = sourceBetween(
    blockOutlinerSource,
    "<BlockEditor",
    "<!-- Display chips",
  );
  const lifecycle = sourceBetween(
    blockEditorSource,
    "const focusBlurHandler",
    "// Leader → editor bridge",
  );

  assert.match(editorProps, /editorKey=\{stableBlockKey\(block\)\}/);
  assert.match(lifecycle, /setFocusedEditor\(editorKey\)/);
  assert.match(lifecycle, /setFocusedNoteDoc\(editorKey, noteSlug\)/);
  assert.equal((lifecycle.match(/clearFocusedEditor\(editorKey\)/g) ?? []).length, 2);
  assert.equal((lifecycle.match(/clearFocusedNoteDoc\(editorKey\)/g) ?? []).length, 2);
});

test("Loro subscription restarts on canonical block identity with owned cleanup", () => {
  const subscription = sourceBetween(
    blockEditorSource,
    "// C2.3 reactive subscription lifecycle",
    "onMount(() => {",
  );
  const editorMount = sourceBetween(
    blockEditorSource,
    "onMount(() => {",
    "// Leader → editor bridge",
  );

  assert.match(subscription, /const bindingBlockId = blockId;/);
  assert.match(subscription, /let disposed = false;/);
  assert.match(subscription, /if \(disposed \|\| !view\) return;/);
  assert.match(subscription, /subRetryTimer = setTimeout/);
  assert.match(
    subscription,
    /return \(\) => \{[\s\S]*disposed = true;[\s\S]*clearTimeout\(subRetryTimer\)[\s\S]*loroUnsub\?\.\(\)/,
  );
  assert.doesNotMatch(editorMount, /trySubscribeLoro/);
});

test("move toast cleanup never clears a newer unrelated toast", () => {
  const cleanup = sourceBetween(
    journalViewSource,
    "function clearMoveToast",
    "function dispatchMove",
  );
  assert.match(cleanup, /getToast\(\)/);
  assert.match(cleanup, /current\?\.id === moveToastId/);
  assert.match(cleanup, /clearToast\(\)/);
});

test("move instructions and recovery toasts clear when their session leaves that phase", () => {
  const cancel = sourceBetween(
    journalViewSource,
    "function cancelSelectingMove",
    "async function prepareOutliner",
  );
  const execute = sourceBetween(
    journalViewSource,
    "async function executeMove",
    "async function submitSelectedMove",
  );
  const retry = sourceBetween(
    journalViewSource,
    "function retryMove",
    "async function focusBlockBid",
  );
  assert.match(cancel, /clearMoveToast\(\)/);
  assert.match(execute, /type: "success"[\s\S]*clearMoveToast\(\)/);
  assert.match(retry, /clearMoveToast\(\)[\s\S]*type: "submit"/);
});

test("late move completions cannot publish UI after Journal teardown", () => {
  const showToast = sourceBetween(
    journalViewSource,
    "function showMoveToast",
    "function clearMoveToast",
  );
  const execute = sourceBetween(
    journalViewSource,
    "async function executeMove",
    "async function submitSelectedMove",
  );
  const focus = sourceBetween(
    journalViewSource,
    "async function focusBlockBid",
    "function relocationBindings",
  );
  const cleanup = journalViewSource.slice(journalViewSource.lastIndexOf("onMount(() => {"));

  assert.match(showToast, /if \(moveUiDisposed\) return;/);
  assert.ok((execute.match(/if \(moveUiDisposed\) return;/g) ?? []).length >= 2);
  assert.ok(
    execute.indexOf("await settleMoveResponse(request)")
      < execute.indexOf("if (moveUiDisposed) return;"),
    "durable response/cache work must finish before the success UI guard",
  );
  assert.ok((focus.match(/if \(moveUiDisposed\) return;/g) ?? []).length >= 2);
  assert.match(cleanup, /return \(\) => \{\s*moveUiDisposed = true;[\s\S]*clearMoveToast\(\)/);
});

test("Journal focus restoration yields to later pointer and keyboard input", () => {
  const focus = sourceBetween(
    journalViewSource,
    "async function focusBlockBid",
    "function relocationBindings",
  );
  const lifecycle = journalViewSource.slice(journalViewSource.lastIndexOf("onMount(() => {"));

  assert.match(journalViewSource, /createFocusRestorationController/);
  assert.match(journalViewSource, /const focusRestoration = createFocusRestorationController\(\)/);
  assert.match(focus, /focusRestoration\.restore\(focusClaim, \{/);
  assert.match(focus, /findTarget: async \(\) =>/);
  assert.match(focus, /focusTarget: \(\{ row, editor \}\) => \{/);
  assert.match(focus, /row\.scrollIntoView[\s\S]*editor\.focus\(\)/);

  assert.match(
    lifecycle,
    /const revokeFocusRestoration = \(\) => focusRestoration\.revoke\(\);/,
    "the canceling input listener must not prevent or stop the user's event",
  );
  const pointerCancel = lifecycle.indexOf(
    'document.addEventListener("pointerdown", revokeFocusRestoration, true)',
  );
  const keyCancel = lifecycle.indexOf(
    'document.addEventListener("keydown", revokeFocusRestoration, true)',
  );
  const moveKeyHandler = lifecycle.indexOf(
    'document.addEventListener("keydown", keyHandler, true)',
  );
  assert.ok(pointerCancel >= 0, "pointerdown must revoke stale restoration even on the focused editor");
  assert.ok(keyCancel >= 0 && keyCancel < moveKeyHandler, "keydown revocation must run before move handling");
  assert.match(
    lifecycle,
    /return \(\) => \{\s*moveUiDisposed = true;\s*focusRestoration\.dispose\(\)/,
  );
  assert.match(
    lifecycle,
    /document\.removeEventListener\("pointerdown", revokeFocusRestoration, true\)/,
  );
  assert.match(
    lifecycle,
    /document\.removeEventListener\("keydown", revokeFocusRestoration, true\)/,
  );
});

test("move completion restores focus only under ownership claimed before transport", () => {
  const cancel = sourceBetween(
    journalViewSource,
    "function cancelSelectingMove",
    "async function prepareOutliner",
  );
  const execute = sourceBetween(
    journalViewSource,
    "async function executeMove",
    "async function submitSelectedMove",
  );
  const focus = sourceBetween(
    journalViewSource,
    "async function focusBlockBid",
    "function relocationBindings",
  );

  const claimIndex = execute.indexOf("const focusClaim = focusRestoration.claim()");
  const preflightIndex = execute.indexOf("await prepareMove(request)");
  const transportIndex = execute.indexOf("await settleMoveResponse(request)");
  assert.ok(claimIndex >= 0, "each move attempt must claim its future focus restoration");
  assert.ok(claimIndex < preflightIndex && claimIndex < transportIndex);
  assert.match(
    execute,
    /focusBlockBid\(focusClaim, request\.destination_note_id, request\.root_bid\)/,
  );
  assert.match(
    execute,
    /focusBlockBid\(focusClaim, request\.source_note_id, request\.root_bid\)/,
  );
  assert.match(
    focus,
    /async function focusBlockBid\(\s*focusClaim: FocusRestorationClaim,/,
  );
  assert.match(focus, /focusRestoration\.restore\(focusClaim, \{/);

  const cancelDispatch = cancel.indexOf('dispatchMove({ type: "cancel" })');
  const cancelClaim = cancel.indexOf("const focusClaim = focusRestoration.claim()");
  assert.ok(cancelDispatch >= 0 && cancelDispatch < cancelClaim);
  assert.match(
    cancel,
    /focusBlockBid\(focusClaim, request\.source_note_id, request\.root_bid\)/,
  );
});

test("sameNoteMoveRequestForAction derives subtree-aware Alt-arrow requests", () => {
  assert.equal(typeof blockTreeMove.sameNoteMoveRequestForAction, "function");
  const bids = {
    a: "11111111-1111-4111-8111-111111111111",
    a1: "22222222-2222-4222-8222-222222222222",
    b: SOURCE_BID,
    b1: "33333333-3333-4333-8333-333333333333",
    c: "44444444-4444-4444-8444-444444444444",
    c1: "55555555-5555-4555-8555-555555555555",
  };
  const blocks = [
    { ...blk("a", 0), bid: bids.a },
    { ...blk("a1", 1), bid: bids.a1 },
    { ...blk("b", 0), bid: bids.b },
    { ...blk("b1", 1), bid: bids.b1 },
    { ...blk("c", 0), bid: bids.c },
    { ...blk("c1", 1), bid: bids.c1 },
  ];
  const noteId = "2026-07-12";

  assert.deepEqual(
    blockTreeMove.sameNoteMoveRequestForAction(blocks, bids.b, noteId, "up", MOVE_ID),
    {
      move_id: MOVE_ID,
      source_note_id: noteId,
      root_bid: bids.b,
      destination_note_id: noteId,
      target_bid: bids.a,
      placement: "before",
    },
  );
  assert.deepEqual(
    blockTreeMove.sameNoteMoveRequestForAction(blocks, bids.b, noteId, "down", MOVE_ID),
    {
      move_id: MOVE_ID,
      source_note_id: noteId,
      root_bid: bids.b,
      destination_note_id: noteId,
      target_bid: bids.c,
      placement: "after",
    },
  );
  assert.deepEqual(
    blockTreeMove.sameNoteMoveRequestForAction(blocks, bids.b, noteId, "indent", MOVE_ID),
    {
      move_id: MOVE_ID,
      source_note_id: noteId,
      root_bid: bids.b,
      destination_note_id: noteId,
      target_bid: bids.a1,
      placement: "inside",
    },
  );
});

test("sameNoteMoveRequestForAction rejects unavailable and unstable moves", () => {
  const only = { ...blk("only", 0), bid: SOURCE_BID };
  const noteId = "2026-07-12";
  assert.equal(
    blockTreeMove.sameNoteMoveRequestForAction([only], SOURCE_BID, noteId, "up", MOVE_ID),
    null,
  );
  assert.equal(
    blockTreeMove.sameNoteMoveRequestForAction([only], SOURCE_BID, noteId, "down", MOVE_ID),
    null,
  );
  assert.equal(
    blockTreeMove.sameNoteMoveRequestForAction([only], SOURCE_BID, noteId, "indent", MOVE_ID),
    null,
  );
  assert.equal(
    blockTreeMove.sameNoteMoveRequestForAction([{ ...only, bid: null }], SOURCE_BID, noteId, "up", MOVE_ID),
    null,
  );
  assert.equal(
    blockTreeMove.sameNoteMoveRequestForAction([only], "missing", noteId, "down", MOVE_ID),
    null,
  );
});

test("Alt-Right derives its target from the caller's visible block order", () => {
  const parentBid = "11111111-1111-4111-8111-111111111111";
  const hiddenChildBid = "22222222-2222-4222-8222-222222222222";
  const source = { ...blk("source", 0), bid: SOURCE_BID };
  const fullBlocks = [
    { ...blk("parent", 0), bid: parentBid },
    { ...blk("hidden-child", 1), bid: hiddenChildBid },
    source,
  ];
  const visibleBlocks = [fullBlocks[0], source];

  const request = blockTreeMove.sameNoteMoveRequestForAction(
    fullBlocks,
    SOURCE_BID,
    "2026-07-12",
    "indent",
    MOVE_ID,
    visibleBlocks[0].bid,
  );

  assert.equal(request.target_bid, parentBid);
  assert.equal(request.placement, "inside");
});

test("Alt-Right validates a collapsed source subtree against the full tree", () => {
  const parentBid = "11111111-1111-4111-8111-111111111111";
  const childBid = "22222222-2222-4222-8222-222222222222";
  const blocks = [
    { ...blk("parent", 0), bid: parentBid },
    { ...blk("source", 0), bid: SOURCE_BID },
    { ...blk("collapsed-child", 1), bid: childBid },
    { ...blk("tail", 0), bid: "33333333-3333-4333-8333-333333333333" },
  ];

  assert.equal(
    blockTreeMove.sameNoteMoveRequestForAction(
      blocks,
      SOURCE_BID,
      "2026-07-12",
      "indent",
      MOVE_ID,
      childBid,
    ),
    null,
  );
});

test("sameNoteMoveRequestForAction rejects missing targets and satisfied indent moves", () => {
  const parentBid = "11111111-1111-4111-8111-111111111111";
  const missingTarget = [
    { ...blk("target", 0), bid: null },
    { ...blk("source", 0), bid: SOURCE_BID },
  ];
  assert.equal(
    blockTreeMove.sameNoteMoveRequestForAction(
      missingTarget,
      SOURCE_BID,
      "2026-07-12",
      "up",
      MOVE_ID,
    ),
    null,
  );

  const alreadyInside = [
    { ...blk("parent", 0), bid: parentBid },
    { ...blk("source", 1), bid: SOURCE_BID },
  ];
  assert.equal(
    blockTreeMove.sameNoteMoveRequestForAction(
      alreadyInside,
      SOURCE_BID,
      "2026-07-12",
      "indent",
      MOVE_ID,
    ),
    null,
  );
});
