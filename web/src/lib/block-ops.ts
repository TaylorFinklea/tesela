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
 * Build the `upsert` op for a STRUCTURAL block — a brand-new block just
 * minted by Enter / new-block-above / paste, or the edited-original half of
 * an Enter split. Unlike `upsertOpForBlock` (the in-place text-edit path),
 * this does NOT reject a local-only id: structural inserts mint a canonical
 * `bid` client-side (`crypto.randomUUID`) up front, so the op carries that
 * stable bid even though the block's editor-`id` is still a `:new-`/`:paste-`
 * /`:split-` placeholder until the server's echo round-trips. The engine's
 * `BlockUpsert` creates-if-absent (appending at document END — see the spec's
 * mid-insert ordering caveat) or updates in place.
 *
 * Returns `null` only when the block is missing or has no `bid` (which would
 * force a server re-stamp); the caller treats `null` as "fall back to the
 * whole-body PUT for this save" so nothing is silently dropped.
 */
export function upsertOpForStructuralBlock(
  blocks: ParsedBlock[],
  blockId: string,
): BlockOp | null {
  const index = blocks.findIndex((b) => b.id === blockId);
  if (index < 0) return null;
  const block = blocks[index];
  if (!block.bid) return null;
  return {
    kind: "upsert",
    bid: block.bid,
    text: stripBid(block.raw_text),
    parent_bid: parentBidFor(blocks, index),
    indent_level: block.indent_level,
  };
}

/**
 * Build the converged op batch for a backspace-merge: the SURVIVING (previous)
 * block absorbs the current block's text, and the current block is removed.
 * Emits the survivor `upsert` and the absorbed-block `delete` together so the
 * server applies BOTH in one `POST /notes/{id}/blocks` call — the file
 * materializes (and the single WS fan-out fires) only after both ops land, so
 * there is no half-applied window where the merge is visible with the absorbed
 * block still present.
 *
 * `survivorId` is the merged block's editor id (its `bid` is the previous
 * block's existing, canonical bid — carried through the merge). `absorbedBid`
 * is the canonical bid of the block being merged away. Returns `null` when the
 * survivor can't be expressed as an upsert (missing / no bid) so the caller
 * falls back to the whole-body PUT for the whole merge — one path per save.
 */
export function mergeOpsForBackspace(
  blocks: ParsedBlock[],
  survivorId: string,
  absorbedBid: string,
): BlockOp[] | null {
  const survivor = upsertOpForStructuralBlock(blocks, survivorId);
  if (survivor === null) return null;
  return [survivor, { kind: "delete", bid: absorbedBid }];
}

/**
 * Build the `delete` op batch for a pure block deletion (backspace into an
 * empty block, `dd`, or a visual-mode multi-block delete). Emits ONE
 * `{ kind:"delete", bid }` op per removed block that the server has actually
 * seen — i.e. that carries a server-canonical `bid` AND is not a brand-new
 * local-only insert (`:new-`/`:paste-`). A never-round-tripped local-only
 * block has no server row, so dropping it from the editor IS the whole
 * deletion; no op is emitted for it (mirrors how `mergeOpsForBackspace` omits
 * the delete for a local-only absorbed block).
 *
 * Returns `[]` when every removed block was local-only — the caller treats an
 * empty batch as "nothing to send; local removal is the whole delete" and does
 * NOT fall back to a whole-body PUT (which would re-assert every surviving
 * block, reintroducing the clobber). `deletedBlocks` are the `ParsedBlock`s as
 * they existed before removal (so their `bid`/`id` are still readable).
 */
export function deleteOpsFor(deletedBlocks: ParsedBlock[]): BlockOp[] {
  const ops: BlockOp[] = [];
  for (const block of deletedBlocks) {
    if (!block) continue;
    if (isLocalOnlyId(block.id) || !block.bid) continue;
    ops.push({ kind: "delete", bid: block.bid });
  }
  return ops;
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
