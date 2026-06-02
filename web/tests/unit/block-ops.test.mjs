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
  upsertOpForStructuralBlock,
  mergeOpsForBackspace,
  moveOpsForIds,
  deleteOpsFor,
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

// ----- Structural-edit op builders (Stage 3: insert / split / paste / merge) -----

test("upsertOpForStructuralBlock: a brand-new local-only block IS upserted (its client-minted bid carries)", () => {
  // Enter on the last block: the new block has a `:new-` id but a real bid.
  // Unlike the in-place path's `upsertOpForBlock`, the structural builder must
  // NOT reject it — the bid is canonical and the engine creates-if-absent.
  const blocks = [
    blk("note:0", { bid: "first", raw_text: "first", indent_level: 0 }),
    blk("note:new-123", {
      bid: "11111111-2222-3333-4444-555555555555",
      raw_text: "fresh line",
      indent_level: 0,
    }),
  ];
  const op = upsertOpForStructuralBlock(blocks, "note:new-123");
  assert.deepEqual(op, {
    kind: "upsert",
    bid: "11111111-2222-3333-4444-555555555555",
    text: "fresh line",
    parent_bid: null,
    indent_level: 0,
  });
});

test("upsertOpForStructuralBlock: split-original (changed id, inherited bid) strips its marker + derives parent", () => {
  // After an Enter split the original block keeps its bid (spread) but gets a
  // `:split-` id and its text shrinks to the pre-cursor portion.
  const blocks = [
    blk("note:0", { bid: "p", raw_text: "parent", indent_level: 0 }),
    blk("note:split-9", {
      bid: "aaaaaaaa-2222-3333-4444-555555555555",
      raw_text: "before <!-- bid:aaaaaaaa-2222-3333-4444-555555555555 -->",
      indent_level: 1,
    }),
  ];
  const op = upsertOpForStructuralBlock(blocks, "note:split-9");
  assert.deepEqual(op, {
    kind: "upsert",
    bid: "aaaaaaaa-2222-3333-4444-555555555555",
    text: "before",
    parent_bid: "p",
    indent_level: 1,
  });
});

test("upsertOpForStructuralBlock: a child new-block nests under its (local-only but bid-carrying) parent", () => {
  // A freshly-created parent (local-only id, real bid) with a child created
  // under it: the child's parent_bid resolves to the parent's client-minted
  // bid, so the nesting survives the upsert.
  const blocks = [
    blk("note:new-1", { bid: "parent-bid", raw_text: "p", indent_level: 0 }),
    blk("note:new-2", { bid: "child-bid", raw_text: "c", indent_level: 1 }),
  ];
  const op = upsertOpForStructuralBlock(blocks, "note:new-2");
  assert.equal(op.parent_bid, "parent-bid");
  assert.equal(op.indent_level, 1);
});

test("upsertOpForStructuralBlock: no bid → null (PUT fallback / server would re-stamp)", () => {
  const blocks = [blk("note:new-1", { bid: null, raw_text: "x" })];
  assert.equal(upsertOpForStructuralBlock(blocks, "note:new-1"), null);
});

test("upsertOpForStructuralBlock: unknown id → null", () => {
  const blocks = [blk("note:0", { bid: "a" })];
  assert.equal(upsertOpForStructuralBlock(blocks, "note:nope"), null);
});

test("mergeOpsForBackspace: survivor upsert + absorbed delete, in that order", () => {
  // prev absorbs current's text; merged block keeps prev's bid (new `:merged-`
  // id). The absorbed block's canonical bid is deleted.
  const blocks = [
    blk("note:merged-1", {
      bid: "prev-bid",
      raw_text: "prevtext-and-currenttext",
      indent_level: 0,
    }),
    blk("note:1", { bid: "tail", raw_text: "tail", indent_level: 0 }),
  ];
  const ops = mergeOpsForBackspace(blocks, "note:merged-1", "current-bid");
  assert.deepEqual(ops, [
    {
      kind: "upsert",
      bid: "prev-bid",
      text: "prevtext-and-currenttext",
      parent_bid: null,
      indent_level: 0,
    },
    { kind: "delete", bid: "current-bid" },
  ]);
});

test("mergeOpsForBackspace: survivor has no bid → null (PUT fallback)", () => {
  const blocks = [blk("note:merged-1", { bid: null, raw_text: "x" })];
  assert.equal(mergeOpsForBackspace(blocks, "note:merged-1", "current-bid"), null);
});

// ----- Pure-delete op builder (S4: backspace-into-empty / dd / visual delete) -----

test("deleteOpsFor: a single server-known block → one delete op (no PUT)", () => {
  const removed = [
    blk("note:3", { bid: "dead-beef-2222-3333-4444-555555555555", raw_text: "gone" }),
  ];
  assert.deepEqual(deleteOpsFor(removed), [
    { kind: "delete", bid: "dead-beef-2222-3333-4444-555555555555" },
  ]);
});

test("deleteOpsFor: multiple server-known blocks → a batch of delete ops in order", () => {
  const removed = [
    blk("note:1", { bid: "aaaa-1111", raw_text: "a" }),
    blk("note:2", { bid: "bbbb-2222", raw_text: "b" }),
    blk("note:3", { bid: "cccc-3333", raw_text: "c" }),
  ];
  assert.deepEqual(deleteOpsFor(removed), [
    { kind: "delete", bid: "aaaa-1111" },
    { kind: "delete", bid: "bbbb-2222" },
    { kind: "delete", bid: "cccc-3333" },
  ]);
});

test("deleteOpsFor: a local-only removed block → no op (local removal IS the delete)", () => {
  const removed = [blk("note:new-99", { bid: "fresh-bid", raw_text: "x" })];
  assert.deepEqual(deleteOpsFor(removed), []);
});

test("deleteOpsFor: a paste-only removed block → no op", () => {
  const removed = [blk("note:paste-7", { bid: "paste-bid", raw_text: "x" })];
  assert.deepEqual(deleteOpsFor(removed), []);
});

test("deleteOpsFor: a removed block with no bid → no op (server never saw it)", () => {
  const removed = [blk("note:2", { bid: null, raw_text: "x" })];
  assert.deepEqual(deleteOpsFor(removed), []);
});

test("deleteOpsFor: mixed batch → ops only for the server-known blocks", () => {
  const removed = [
    blk("note:1", { bid: "real-1", raw_text: "real" }),
    blk("note:new-2", { bid: "fresh", raw_text: "local-only" }),
    blk("note:3", { bid: "real-3", raw_text: "real2" }),
  ];
  assert.deepEqual(deleteOpsFor(removed), [
    { kind: "delete", bid: "real-1" },
    { kind: "delete", bid: "real-3" },
  ]);
});

test("deleteOpsFor: empty input → empty batch (no PUT, no op)", () => {
  assert.deepEqual(deleteOpsFor([]), []);
});

// ----- Bulk visual-mode status/tag + template-insert batch emission -----
//
// These mirror exactly what `BlockOutliner`'s `bulkCycleStatus` /
// `bulkToggleTag` / `insertTemplateAfter` build: a single coalesced batch of
// upsert ops, ONE per affected block (status/tag change a block's text;
// template insert adds brand-new blocks). The component's `saveBlocksViaOps`
// then POSTs them in one call, or falls back to the whole-body PUT if the
// batch carries a `null` (a non-candidate block). The op-construction is the
// load-bearing, testable part, so assert it against the same builders the
// handlers call.

test("bulk status/tag: N changed blocks → N upsert ops in one batch, correct bid+text", () => {
  // After a bulk status cycle the editor mutated three blocks' raw_text in
  // place; the handler emits `[...changedIds].map((id) => upsertOpForBlock(...))`.
  const blocks = [
    blk("note:0", {
      bid: "aaaaaaaa-0000-0000-0000-000000000000",
      raw_text: "DONE alpha <!-- bid:aaaaaaaa-0000-0000-0000-000000000000 -->",
      indent_level: 0,
    }),
    blk("note:1", {
      bid: "bbbbbbbb-0000-0000-0000-000000000000",
      raw_text: "DONE beta <!-- bid:bbbbbbbb-0000-0000-0000-000000000000 -->",
      indent_level: 0,
    }),
    blk("note:2", {
      bid: "cccccccc-0000-0000-0000-000000000000",
      raw_text: "DONE gamma <!-- bid:cccccccc-0000-0000-0000-000000000000 -->",
      indent_level: 0,
    }),
  ];
  const changedIds = ["note:0", "note:1", "note:2"];
  const ops = changedIds.map((id) => upsertOpForBlock(blocks, id));
  assert.equal(ops.length, 3);
  assert.ok(ops.every((o) => o !== null));
  assert.deepEqual(ops, [
    { kind: "upsert", bid: "aaaaaaaa-0000-0000-0000-000000000000", text: "DONE alpha", parent_bid: null, indent_level: 0 },
    { kind: "upsert", bid: "bbbbbbbb-0000-0000-0000-000000000000", text: "DONE beta", parent_bid: null, indent_level: 0 },
    { kind: "upsert", bid: "cccccccc-0000-0000-0000-000000000000", text: "DONE gamma", parent_bid: null, indent_level: 0 },
  ]);
});

test("bulk tag toggle: only the flipped blocks are upserted, an unchanged one is NOT re-asserted", () => {
  // `bulkToggleTag` builds `changedIds` from ONLY the blocks it actually
  // flipped (the add/remove-bias guard skips some); a skipped block keeps its
  // old text and must not appear in the batch (re-asserting it would clobber a
  // concurrent peer edit).
  const blocks = [
    blk("note:0", {
      bid: "11111111-0000-0000-0000-000000000000",
      raw_text: "alpha #Task <!-- bid:11111111-0000-0000-0000-000000000000 -->",
      indent_level: 0,
    }),
    blk("note:1", {
      bid: "22222222-0000-0000-0000-000000000000",
      raw_text: "beta (already had it, skipped) <!-- bid:22222222-0000-0000-0000-000000000000 -->",
      indent_level: 0,
    }),
  ];
  // Only note:0 was flipped this round.
  const changedIds = ["note:0"];
  const ops = changedIds.map((id) => upsertOpForBlock(blocks, id));
  assert.deepEqual(ops, [
    { kind: "upsert", bid: "11111111-0000-0000-0000-000000000000", text: "alpha #Task", parent_bid: null, indent_level: 0 },
  ]);
  // note:1 (skipped) is absent — not re-asserted.
  assert.ok(!ops.some((o) => o.bid === "22222222-0000-0000-0000-000000000000"));
});

test("bulk status/tag: a non-candidate (local-only / no-bid) block yields a null in the batch → PUT fallback", () => {
  // If a selected block has no server bid yet, its op is null; the handler's
  // `saveBlocksViaOps` sees the null and falls back to the whole-body PUT for
  // the entire batch (one path per save).
  const blocks = [
    blk("note:0", { bid: "real-0", raw_text: "DOING a", indent_level: 0 }),
    blk("note:new-9", { bid: "fresh", raw_text: "DOING b", indent_level: 0 }),
  ];
  const ops = ["note:0", "note:new-9"].map((id) => upsertOpForBlock(blocks, id));
  assert.equal(ops.length, 2);
  assert.deepEqual(ops[0], {
    kind: "upsert",
    bid: "real-0",
    text: "DOING a",
    parent_bid: null,
    indent_level: 0,
  });
  // local-only insert → null → forces the whole-body PUT for the batch.
  assert.equal(ops[1], null);
});

test("template insert: each inserted block → one structural upsert (fresh bid, re-based indent)", () => {
  // `insertTemplateAfter` mints a fresh bid per inserted block (like paste) and
  // re-bases indents so the template's outermost blocks nest under the parent;
  // it then emits `inserted.map((b) => upsertOpForStructuralBlock(...))`.
  const blocks = [
    blk("note:0", { bid: "parent-bid", raw_text: "parent", indent_level: 0 }),
    blk("note:tmpl-100-0", {
      bid: "tmpl-bid-1",
      raw_text: "heading",
      indent_level: 1,
    }),
    blk("note:tmpl-100-1", {
      bid: "tmpl-bid-2",
      raw_text: "nested item",
      indent_level: 2,
    }),
    blk("note:tail", { bid: "tail-bid", raw_text: "tail", indent_level: 0 }),
  ];
  const inserted = [blocks[1], blocks[2]];
  const ops = inserted.map((b) => upsertOpForStructuralBlock(blocks, b.id));
  assert.equal(ops.length, 2);
  assert.deepEqual(ops, [
    { kind: "upsert", bid: "tmpl-bid-1", text: "heading", parent_bid: "parent-bid", indent_level: 1 },
    { kind: "upsert", bid: "tmpl-bid-2", text: "nested item", parent_bid: "tmpl-bid-1", indent_level: 2 },
  ]);
});

test("template insert: a bid-less inserted block → null (would force PUT fallback)", () => {
  // Defensive: if a template block somehow lacks a bid, the structural builder
  // returns null so the handler's `saveBlocksViaOps` falls back rather than
  // emitting a partial batch the server would re-stamp.
  const blocks = [
    blk("note:0", { bid: "p", raw_text: "parent", indent_level: 0 }),
    blk("note:tmpl-1-0", { bid: null, raw_text: "no bid", indent_level: 1 }),
  ];
  const ops = [blocks[1]].map((b) => upsertOpForStructuralBlock(blocks, b.id));
  assert.deepEqual(ops, [null]);
});
