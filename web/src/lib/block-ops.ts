/**
 * Block-granular write op builders (sync redesign 2026-06-02).
 *
 * Pure helpers that turn the editor's in-memory block tree into the
 * `BlockOp[]` payload for `POST /notes/{id}/blocks`. Kept free of Svelte /
 * DOM deps so they can be unit-tested directly (see
 * `web/tests/unit/block-ops.test.mjs`) and so the dual-write-path contract
 * lives in one auditable place.
 *
 * The wire shape mirrors `BlockOp` in
 * `crates/tesela-server/src/routes/notes.rs` EXACTLY: serde `tag = "kind"`,
 * `snake_case` variants, `parent_bid` optional (omit/null = top-level),
 * `indent_level` a number.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";

/** Locally-created block ids carry a `:new-<timestamp>` or `:paste-` infix;
 *  they exist in the editor but not in any server-canonical body until their
 *  first whole-body PUT round-trips. Such blocks are brand-new structural
 *  inserts that still flow through the PUT path in Stage 1 (Stage 3 migrates
 *  them to `upsert` ops), so the block-op builders skip them. Mirrors
 *  `BlockOutliner.isLocalOnlyId`. */
export function isLocalOnlyId(id: string): boolean {
  return id.includes(":new-") || id.includes(":paste-");
}

/** Mirrors the Rust `BlockOp` tagged enum. Only the variants web emits from
 *  the in-place text-edit + indent paths are modelled here; deletes still go
 *  through the dedicated `DELETE /notes/{id}/blocks/{bid}` endpoint. */
export type BlockOp =
  | {
      kind: "upsert";
      bid: string;
      text: string;
      parent_bid: string | null;
      indent_level: number;
    }
  | {
      kind: "move";
      bid: string;
      parent_bid: string | null;
      indent_level: number;
    }
  | { kind: "delete"; bid: string };

/** Strip the `<!-- bid:UUID -->` marker from a block's `raw_text` so the op
 *  carries only the human content. The engine re-stamps the marker on apply,
 *  so the wire `text` must NOT include it (else a duplicate marker lands on
 *  the line). Mirrors the strip in `BlockOutliner.buildFullContent`. */
export function stripBid(rawText: string): string {
  return rawText.replace(/\s*<!--\s*bid:[0-9a-fA-F-]{32,36}\s*-->/g, "");
}

/**
 * Derive the `parent_bid` for the block at `index` in `blocks` (document
 * order). The parent is the nearest PRECEDING block whose `indent_level` is
 * exactly one less than this block's. `null` at the top level (indent 0) or
 * when no such ancestor exists. This matches how the on-disk indentation
 * encodes the tree (the parser/`buildFullContent` use indent + document
 * order; the server engine renders document/creation order + indent).
 *
 * Returns the parent's `bid` (the server `block_id`). If the located parent
 * has no bid yet (a brand-new local-only block not round-tripped), returns
 * `null` rather than a non-canonical id — a parentless upsert lands at the
 * document root, which the next structural save reconciles.
 */
export function parentBidFor(blocks: ParsedBlock[], index: number): string | null {
  const block = blocks[index];
  if (!block) return null;
  const level = block.indent_level;
  if (level <= 0) return null;
  for (let i = index - 1; i >= 0; i--) {
    if (blocks[i].indent_level === level - 1) {
      return blocks[i].bid ?? null;
    }
  }
  return null;
}

/**
 * Build the single `upsert` op for an in-place text edit of `blockId`. The
 * block MUST already carry a server `bid` (the marker UUID) AND a server-
 * canonical id (not a `:new-`/`:paste-` local-only insert); brand-new
 * structural inserts still flow through the whole-body PUT path in Stage 1
 * (Stage 3 migrates those). Returns `null` to signal "not a block-op
 * candidate — fall back to the body path".
 */
export function upsertOpForBlock(
  blocks: ParsedBlock[],
  blockId: string,
): BlockOp | null {
  const index = blocks.findIndex((b) => b.id === blockId);
  if (index < 0) return null;
  const block = blocks[index];
  if (!block.bid || isLocalOnlyId(block.id)) return null;
  return {
    kind: "upsert",
    bid: block.bid,
    text: stripBid(block.raw_text),
    parent_bid: parentBidFor(blocks, index),
    indent_level: block.indent_level,
  };
}

/**
 * Build the `move` ops for an indent/outdent that changed the `indent_level`
 * of the blocks in `changedIds`. Returns ONE entry per affected block in
 * document order: a `move` op when the block is a block-op candidate
 * (carries a server `bid` and a canonical, non-local-only id), or `null`
 * when it is not. The `null` entries are load-bearing — the caller treats a
 * batch containing ANY `null` as "can't fully express block-granularly" and
 * falls back to the whole-body PUT for the entire indent, so a mixed subtree
 * (real blocks + a brand-new local-only one) never loses the indent on the
 * local-only members. The new `parent_bid` + `indent_level` are read from the
 * already-mutated `blocks` tree.
 */
export function moveOpsForIds(
  blocks: ParsedBlock[],
  changedIds: Set<string>,
): (BlockOp | null)[] {
  const ops: (BlockOp | null)[] = [];
  for (let i = 0; i < blocks.length; i++) {
    const b = blocks[i];
    if (!changedIds.has(b.id)) continue;
    if (!b.bid || isLocalOnlyId(b.id)) {
      ops.push(null);
      continue;
    }
    const parent_bid = parentBidFor(blocks, i);
    // The engine derives a moved block's indent from its parent's indent
    // (`parent.indent + 1`, or 0 when parentless). A non-top-level block whose
    // expected parent can't be resolved to a bid would therefore be flattened
    // to indent 0 — wrong. Signal ineligible (null) so the caller PUTs the
    // whole body instead, preserving the intended indent.
    if (b.indent_level > 0 && parent_bid === null) {
      ops.push(null);
      continue;
    }
    ops.push({
      kind: "move",
      bid: b.bid,
      parent_bid,
      indent_level: b.indent_level,
    });
  }
  return ops;
}
