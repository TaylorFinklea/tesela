// Unit tests for the block-granular write op builders.
//
// These pure helpers turn the editor's in-memory block tree into the
// `BlockOp[]` payload for `POST /notes/{id}/blocks` (sync redesign
// 2026-06-02). The wire shape must mirror the Rust `BlockOp` enum in
// `crates/tesela-server/src/routes/notes.rs` EXACTLY: tag `kind`, snake_case
// variants, `parent_bid` nullable, `indent_level` numeric.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  stripBid,
  parentBidFor,
  upsertOpForBlock,
  moveOpsForIds,
  isLocalOnlyId,
} from "../../src/lib/block-ops.ts";

/** Minimal ParsedBlock factory — only the fields the op builders read. */
function blk(id, { bid = null, raw_text = "", indent_level = 0 } = {}) {
  return {
    id,
    bid,
    text: raw_text,
    raw_text,
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

test("stripBid removes the bid marker (with leading whitespace)", () => {
  assert.equal(
    stripBid("hello world <!-- bid:11111111-2222-3333-4444-555555555555 -->"),
    "hello world",
  );
});

test("stripBid is a no-op when there is no marker", () => {
  assert.equal(stripBid("just text"), "just text");
});

test("isLocalOnlyId flags :new- and :paste- ids", () => {
  assert.equal(isLocalOnlyId("note:new-seed"), true);
  assert.equal(isLocalOnlyId("note:paste-123"), true);
  assert.equal(isLocalOnlyId("note:5"), false);
});

test("parentBidFor: top-level block has null parent", () => {
  const blocks = [blk("note:0", { bid: "a", indent_level: 0 })];
  assert.equal(parentBidFor(blocks, 0), null);
});

test("parentBidFor: nearest preceding block at indent-1 is the parent", () => {
  const blocks = [
    blk("note:0", { bid: "p", indent_level: 0 }),
    blk("note:1", { bid: "c", indent_level: 1 }),
  ];
  assert.equal(parentBidFor(blocks, 1), "p");
});

test("parentBidFor: skips same/deeper-level intervening blocks", () => {
  const blocks = [
    blk("note:0", { bid: "p", indent_level: 0 }),
    blk("note:1", { bid: "c1", indent_level: 1 }),
    blk("note:2", { bid: "gc", indent_level: 2 }),
    blk("note:3", { bid: "c2", indent_level: 1 }),
  ];
  // c2 (index 3, indent 1) → parent is p (the nearest preceding indent-0).
  assert.equal(parentBidFor(blocks, 3), "p");
  // gc (index 2, indent 2) → parent is c1 (nearest preceding indent-1).
  assert.equal(parentBidFor(blocks, 2), "c1");
});

test("upsertOpForBlock: single upsert with bid-stripped text + derived parent", () => {
  const blocks = [
    blk("note:0", { bid: "p", raw_text: "parent", indent_level: 0 }),
    blk("note:1", {
      bid: "cccccccc-2222-3333-4444-555555555555",
      raw_text: "child text <!-- bid:cccccccc-2222-3333-4444-555555555555 -->",
      indent_level: 1,
    }),
  ];
  const op = upsertOpForBlock(blocks, "note:1");
  assert.deepEqual(op, {
    kind: "upsert",
    bid: "cccccccc-2222-3333-4444-555555555555",
    text: "child text",
    parent_bid: "p",
    indent_level: 1,
  });
});

test("upsertOpForBlock: top-level block → parent_bid null", () => {
  const blocks = [blk("note:0", { bid: "a", raw_text: "x", indent_level: 0 })];
  const op = upsertOpForBlock(blocks, "note:0");
  assert.equal(op.parent_bid, null);
});

test("upsertOpForBlock: block with no bid → null (PUT fallback)", () => {
  const blocks = [blk("note:0", { bid: null, raw_text: "x" })];
  assert.equal(upsertOpForBlock(blocks, "note:0"), null);
});

test("upsertOpForBlock: brand-new local-only block → null (PUT fallback)", () => {
  const blocks = [blk("note:new-123", { bid: "fresh", raw_text: "x" })];
  assert.equal(upsertOpForBlock(blocks, "note:new-123"), null);
});

test("upsertOpForBlock: unknown id → null", () => {
  const blocks = [blk("note:0", { bid: "a" })];
  assert.equal(upsertOpForBlock(blocks, "note:nope"), null);
});

test("moveOpsForIds: one move op per changed real block, parent derived", () => {
  // Indent of a parent+child subtree (both shifted by +1).
  const blocks = [
    blk("note:0", { bid: "root", indent_level: 0 }),
    blk("note:1", { bid: "p", indent_level: 1 }),
    blk("note:2", { bid: "c", indent_level: 2 }),
  ];
  const ops = moveOpsForIds(blocks, new Set(["note:1", "note:2"]));
  assert.deepEqual(ops, [
    { kind: "move", bid: "p", parent_bid: "root", indent_level: 1 },
    { kind: "move", bid: "c", parent_bid: "p", indent_level: 2 },
  ]);
});

test("moveOpsForIds: outdent to top level → parent_bid null", () => {
  const blocks = [blk("note:0", { bid: "a", indent_level: 0 })];
  const ops = moveOpsForIds(blocks, new Set(["note:0"]));
  assert.deepEqual(ops, [
    { kind: "move", bid: "a", parent_bid: null, indent_level: 0 },
  ]);
});

test("moveOpsForIds: local-only changed block → null entry (forces PUT fallback)", () => {
  const blocks = [
    blk("note:0", { bid: "a", indent_level: 0 }),
    blk("note:new-9", { bid: "fresh", indent_level: 1 }),
  ];
  const ops = moveOpsForIds(blocks, new Set(["note:0", "note:new-9"]));
  // Real block yields a move; local-only yields null so the caller PUTs.
  assert.equal(ops.length, 2);
  assert.deepEqual(ops[0], {
    kind: "move",
    bid: "a",
    parent_bid: null,
    indent_level: 0,
  });
  assert.equal(ops[1], null);
});

test("moveOpsForIds: indented block with unresolvable parent → null (PUT fallback)", () => {
  // child is indent 1 but its expected parent (a local-only block) has no
  // usable bid → the engine would flatten it to indent 0, so signal null.
  const blocks = [
    blk("note:new-1", { bid: null, indent_level: 0 }),
    blk("note:1", { bid: "c", indent_level: 1 }),
  ];
  const ops = moveOpsForIds(blocks, new Set(["note:1"]));
  assert.deepEqual(ops, [null]);
});
