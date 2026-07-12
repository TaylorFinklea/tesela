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
import { stripStructuralBid } from "./block-parser.ts";

/** Locally-created block ids carry a `:new-<timestamp>` or `:paste-` infix;
 *  they exist in the editor but not in any server-canonical body until their
 *  first whole-body PUT round-trips. Such blocks are brand-new structural
 *  inserts that still flow through the PUT path in Stage 1 (Stage 3 migrates
 *  them to `upsert` ops), so the block-op builders skip them. Mirrors
 *  `BlockOutliner.isLocalOnlyId`. */
export function isLocalOnlyId(id: string): boolean {
  return id.includes(":new-") || id.includes(":paste-");
}

/** EVERY client-minted block-id infix — the superset of `isLocalOnlyId`.
 *  Besides `:new-`/`:paste-`, the editor mints `:split-` (the edited-original
 *  half of an Enter split), `:merged-` (a backspace-merge survivor), and
 *  `:tmpl-` (template-inserted blocks). None of these ids exist in any
 *  server-canonical body (the parser mints `<noteId>:<lineNumber>`, numeric
 *  trailing segment), so a reseed can't find them by id.
 *
 *  This is the predicate for DIRTY-GUARD checks ("does the editor hold local
 *  structural edits a stale reseed could revert?") — `BlockOutliner`'s
 *  `hasUnsavedLocalEdits` and its focused-block reseed protection. It is NOT
 *  for op-builder gating: a `:split-`/`:merged-`/`:tmpl-` block carries a
 *  canonical `bid` and IS a valid op target (`upsertOpForStructuralBlock`),
 *  and an absorbed `:merged-`/`:split-` block's server row still needs a
 *  delete op — keep using `isLocalOnlyId` there. */
export function isClientMintedId(id: string): boolean {
  return (
    id.includes(":new-") ||
    id.includes(":paste-") ||
    id.includes(":split-") ||
    id.includes(":merged-") ||
    id.includes(":tmpl-")
  );
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
      /** Predecessor block id: when this upsert CREATES a new block on the
       *  server, insert it immediately AFTER this block (so a mid-note
       *  split's new half lands adjacent, not at document end). Omitted
       *  (undefined) for an in-place text edit of an existing block — the
       *  engine never moves an existing block on upsert — and at the top of
       *  the document (no predecessor). Mirrors the server `BlockOp::Upsert`
       *  `after_bid` field; `undefined` serializes as omitted = append
       *  (backward compatible). */
      after_bid?: string;
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
export function stripBid(rawText: string, expectedBid?: string): string {
  const newline = rawText.indexOf("\n");
  const firstLine = newline >= 0 ? rawText.slice(0, newline) : rawText;
  const rest = newline >= 0 ? rawText.slice(newline) : "";
  return stripStructuralBid(firstLine, expectedBid).text + rest;
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
 * Derive the `after_bid` positional hint for the block at `index`: the
 * `bid` of the immediately PRECEDING block in document order. Returns
 * `undefined` at the top of the document (no predecessor) or when the
 * predecessor hasn't been stamped with a `bid` yet (a brand-new local-only
 * block) — in both cases the new block appends at document end, which is
 * the loss-free fallback. The engine inserts a NEW block right after this
 * predecessor; an existing block updated in place ignores the hint.
 */
export function afterBidFor(blocks: ParsedBlock[], index: number): string | undefined {
  if (index <= 0) return undefined;
  return blocks[index - 1]?.bid ?? undefined;
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
    text: stripBid(block.raw_text, block.bid),
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
  // Positional hint: a structural insert (Enter split / new-block-above /
  // paste) should land ADJACENT to the block it follows, so peers render it
  // in place instead of at document end. The predecessor is the block one
  // position earlier; `undefined` at the top means append. The engine only
  // honors the hint when this op CREATES the block — re-upserting the
  // edited-original half of a split (an existing block) ignores it.
  const after_bid = afterBidFor(blocks, index);
  return {
    kind: "upsert",
    bid: block.bid,
    text: stripBid(block.raw_text, block.bid),
    parent_bid: parentBidFor(blocks, index),
    indent_level: block.indent_level,
    ...(after_bid !== undefined ? { after_bid } : {}),
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

/**
 * Diff a `prev` block tree against a `next` block tree (an undo/redo restore)
 * into the block ops that transform prev → next. Used to migrate the
 * `saveSnapshotRestore` (undo/redo) path off the whole-body PUT: instead of
 * re-asserting EVERY surviving block from a possibly-stale view (the clobber),
 * emit only the blocks the restore actually changed.
 *
 *  - A block present in `next` but absent in `prev` (by `bid`) → `upsert`
 *    (re-create the block the restore brings back).
 *  - A block present in both whose `raw_text`, `indent_level`, or computed
 *    `parent_bid` differ → `upsert` (apply the restored text/structure).
 *  - A block present in `prev` but absent in `next` → `delete` (remove the
 *    block the restore took away).
 *
 * Blocks are keyed by `bid` (the stable server/client-minted id), so re-
 * ordering alone is invisible to the diff — v1 doesn't reorder server-side
 * (the engine ignores order_key), matching the rest of the block-ops paths.
 *
 * Returns `null` when ANY block on EITHER side lacks a `bid` (a brand-new
 * local-only insert the server has never stamped). Such a block can't be
 * expressed as a stable op, so the caller must fall back to the whole-body
 * PUT (with a base) for the whole restore — one path per save, nothing
 * silently dropped. The common case (undo/redo over blocks that have all
 * round-tripped) returns a clean op batch and never PUTs.
 */
export function diffOpsForSnapshot(
  prev: ParsedBlock[],
  next: ParsedBlock[],
): BlockOp[] | null {
  // Any bid-less block on either side ⇒ not fully expressible block-granularly.
  if (prev.some((b) => !b.bid) || next.some((b) => !b.bid)) return null;
  const prevByBid = new Map<string, ParsedBlock>();
  for (const b of prev) prevByBid.set(b.bid!, b);
  const nextByBid = new Map<string, ParsedBlock>();
  for (const b of next) nextByBid.set(b.bid!, b);

  const ops: BlockOp[] = [];
  // Upserts for added or changed blocks (walk `next` in document order so the
  // `parent_bid` lookups read the restored tree).
  for (let i = 0; i < next.length; i++) {
    const b = next[i];
    const parent_bid = parentBidFor(next, i);
    const before = prevByBid.get(b.bid!);
    const text = stripBid(b.raw_text, b.bid!);
    if (
      before &&
      stripBid(before.raw_text, before.bid!) === text &&
      before.indent_level === b.indent_level &&
      parentBidFor(prev, prev.indexOf(before)) === parent_bid
    ) {
      continue; // unchanged — no op, so a concurrent peer edit to it survives.
    }
    // For a block the restore RE-CREATES (absent in prev), carry the
    // positional hint so it lands adjacent to its predecessor in the
    // restored tree; an existing block being updated ignores it.
    const after_bid = before ? undefined : afterBidFor(next, i);
    ops.push({
      kind: "upsert",
      bid: b.bid!,
      text,
      parent_bid,
      indent_level: b.indent_level,
      ...(after_bid !== undefined ? { after_bid } : {}),
    });
  }
  // Deletes for blocks the restore removed.
  for (const b of prev) {
    if (!nextByBid.has(b.bid!)) ops.push({ kind: "delete", bid: b.bid! });
  }
  return ops;
}
