import type { ParsedBlock } from "./types/ParsedBlock";

export type MovePlacement = "before" | "inside" | "after" | "append";

export type BlockMoveRequest = {
  move_id: string;
  source_note_id: string;
  root_bid: string;
  destination_note_id: string;
  target_bid: string | null;
  placement: MovePlacement;
};

export type BlockMoveResponse<TNote extends { id: string }> = {
  move_id: string;
  notes: TNote[];
};

export type BlockMoveExecutorDependencies = {
  post: <T>(path: string, body: unknown, signal?: AbortSignal) => Promise<T>;
  recordLocalSave: (id: string) => void;
};

export async function executeBlockSubtreeRelocation<TNote extends { id: string }>(
  req: BlockMoveRequest,
  signal: AbortSignal | undefined,
  dependencies: BlockMoveExecutorDependencies,
): Promise<BlockMoveResponse<TNote>> {
  dependencies.recordLocalSave(req.source_note_id);
  dependencies.recordLocalSave(req.destination_note_id);
  const response = await dependencies.post<BlockMoveResponse<TNote>>(
    "/blocks/move-subtree",
    req,
    signal,
  );
  for (const note of response.notes) dependencies.recordLocalSave(note.id);
  return response;
}

export type BlockMoveDragPayload = {
  move_id: string;
  source_note_id: string;
  root_bid: string;
};

export type BlockMovePlan = {
  subtreeBids: string[];
  insertionIndex: number;
  destinationIndent: number;
  destinationParentBid: string | null;
  noOp: boolean;
};

export const BLOCK_MOVE_MIME = "application/x-tesela-block-move";

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export type TreeMoveResult = {
  blocks: ParsedBlock[];
  focusedId: string | null;
  changed: boolean;
};

function unchanged(blocks: ParsedBlock[], focusedId: string | null = null): TreeMoveResult {
  return { blocks, focusedId, changed: false };
}

function subtreeEnd(blocks: ParsedBlock[], start: number): number {
  const root = blocks[start];
  if (!root) return start;
  for (let i = start + 1; i < blocks.length; i++) {
    if (blocks[i].indent_level <= root.indent_level) return i;
  }
  return blocks.length;
}

function parentBid(blocks: ParsedBlock[], start: number): string | null {
  const block = blocks[start];
  if (!block || block.indent_level === 0) return null;
  for (let i = start - 1; i >= 0; i--) {
    const candidate = blocks[i];
    if (candidate.indent_level < block.indent_level) {
      if (typeof candidate.bid !== "string" || candidate.bid.length === 0) {
        throw new Error("destination parent is missing a stable bid");
      }
      return candidate.bid;
    }
  }
  return null;
}

function requireStableBids(blocks: ParsedBlock[]): string[] {
  return blocks.map((block) => {
    if (typeof block.bid !== "string" || block.bid.length === 0) {
      throw new Error("source subtree contains a block without a stable bid");
    }
    return block.bid;
  });
}

function isBlockMoveDragPayload(value: unknown): value is BlockMoveDragPayload {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  if (keys.length !== 3 || keys[0] !== "move_id" || keys[1] !== "root_bid" || keys[2] !== "source_note_id") {
    return false;
  }
  return (
    typeof record.move_id === "string"
    && UUID_PATTERN.test(record.move_id)
    && typeof record.source_note_id === "string"
    && record.source_note_id.trim().length > 0
    && typeof record.root_bid === "string"
    && UUID_PATTERN.test(record.root_bid)
  );
}

export function extractSubtree(blocks: ParsedBlock[], rootBid: string): ParsedBlock[] {
  const start = blocks.findIndex((block) => block.bid === rootBid);
  if (start < 0) return [];
  return blocks.slice(start, subtreeEnd(blocks, start));
}

export function planBlockMove(args: {
  sourceBlocks: ParsedBlock[];
  rootBid: string;
  destinationBlocks: ParsedBlock[];
  targetBid: string | null;
  placement: MovePlacement;
  sameNote: boolean;
}): BlockMovePlan {
  const { sourceBlocks, rootBid, destinationBlocks, targetBid, placement, sameNote } = args;
  if (!(["before", "inside", "after", "append"] as string[]).includes(placement)) {
    throw new Error(`unsupported move placement: ${String(placement)}`);
  }
  if (placement === "append") {
    if (targetBid !== null) throw new Error("append placement requires a null target bid");
  } else if (typeof targetBid !== "string" || targetBid.length === 0) {
    throw new Error(`${placement} placement requires a target bid`);
  }

  const sourceStart = sourceBlocks.findIndex((block) => block.bid === rootBid);
  if (sourceStart < 0) throw new Error("source root bid was not found");
  const subtree = sourceBlocks.slice(sourceStart, subtreeEnd(sourceBlocks, sourceStart));
  const subtreeBids = requireStableBids(subtree);
  if (targetBid !== null && subtreeBids.includes(targetBid)) {
    throw new Error("move target cannot be inside the source subtree");
  }

  let availableDestination = destinationBlocks;
  let originalRootIndex = -1;
  let originalRootIndent = -1;
  let originalParentBid: string | null = null;
  if (sameNote) {
    originalRootIndex = destinationBlocks.findIndex((block) => block.bid === rootBid);
    if (originalRootIndex < 0) throw new Error("same-note destination is missing the source root bid");
    originalRootIndent = destinationBlocks[originalRootIndex].indent_level;
    originalParentBid = parentBid(destinationBlocks, originalRootIndex);
    const originalRootEnd = subtreeEnd(destinationBlocks, originalRootIndex);
    availableDestination = [
      ...destinationBlocks.slice(0, originalRootIndex),
      ...destinationBlocks.slice(originalRootEnd),
    ];
  }

  let insertionIndex: number;
  let destinationIndent: number;
  let destinationParentBid: string | null;
  if (placement === "append") {
    insertionIndex = availableDestination.length;
    destinationIndent = 0;
    destinationParentBid = null;
  } else {
    const targetIndex = availableDestination.findIndex((block) => block.bid === targetBid);
    if (targetIndex < 0) throw new Error("destination target bid was not found");
    const target = availableDestination[targetIndex];
    if (placement === "before") {
      insertionIndex = targetIndex;
      destinationIndent = target.indent_level;
      destinationParentBid = parentBid(availableDestination, targetIndex);
    } else if (placement === "inside") {
      insertionIndex = subtreeEnd(availableDestination, targetIndex);
      destinationIndent = target.indent_level + 1;
      if (typeof target.bid !== "string" || target.bid.length === 0) {
        throw new Error("destination target is missing a stable bid");
      }
      destinationParentBid = target.bid;
    } else {
      insertionIndex = subtreeEnd(availableDestination, targetIndex);
      destinationIndent = target.indent_level;
      destinationParentBid = parentBid(availableDestination, targetIndex);
    }
  }

  return {
    subtreeBids,
    insertionIndex,
    destinationIndent,
    destinationParentBid,
    noOp: sameNote
      && insertionIndex === originalRootIndex
      && destinationIndent === originalRootIndent
      && destinationParentBid === originalParentBid,
  };
}

export function classifyDropPlacement(
  clientY: number,
  rect: Pick<DOMRect, "top" | "height">,
): Exclude<MovePlacement, "append"> {
  const firstBoundary = rect.top + rect.height / 3;
  const secondBoundary = rect.top + rect.height * 2 / 3;
  if (clientY < firstBoundary) return "before";
  if (clientY < secondBoundary) return "inside";
  return "after";
}

export function encodeBlockMoveDragPayload(payload: BlockMoveDragPayload): string {
  if (!isBlockMoveDragPayload(payload)) {
    throw new TypeError("expected a valid block move drag payload");
  }
  return JSON.stringify(payload);
}

export function decodeBlockMoveDragPayload(
  types: readonly string[],
  raw: string,
): BlockMoveDragPayload | null {
  if (!types.includes(BLOCK_MOVE_MIME)) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    return isBlockMoveDragPayload(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function previousSiblingStart(blocks: ParsedBlock[], start: number): number {
  const block = blocks[start];
  if (!block) return -1;
  const level = block.indent_level;
  for (let i = start - 1; i >= 0; i--) {
    const candidateLevel = blocks[i].indent_level;
    if (candidateLevel < level) return -1;
    if (candidateLevel === level) return i;
  }
  return -1;
}

function nextSiblingStart(blocks: ParsedBlock[], start: number): number {
  const block = blocks[start];
  if (!block) return -1;
  const end = subtreeEnd(blocks, start);
  const next = blocks[end];
  if (!next || next.indent_level !== block.indent_level) return -1;
  return end;
}

function rebaseSubtree(subtree: ParsedBlock[], delta: number): ParsedBlock[] {
  if (delta === 0) return subtree;
  return subtree.map((b) => ({
    ...b,
    indent_level: Math.max(0, b.indent_level + delta),
  }));
}

export function moveSubtreeUp(blocks: ParsedBlock[], blockId: string): TreeMoveResult {
  const start = blocks.findIndex((b) => b.id === blockId);
  if (start < 0) return unchanged(blocks, blockId);
  const prevStart = previousSiblingStart(blocks, start);
  if (prevStart < 0) return unchanged(blocks, blockId);
  const prevEnd = subtreeEnd(blocks, prevStart);
  const end = subtreeEnd(blocks, start);
  const selected = blocks.slice(start, end);
  const previous = blocks.slice(prevStart, prevEnd);
  return {
    blocks: [
      ...blocks.slice(0, prevStart),
      ...selected,
      ...previous,
      ...blocks.slice(end),
    ],
    focusedId: blockId,
    changed: true,
  };
}

export function moveSubtreeDown(blocks: ParsedBlock[], blockId: string): TreeMoveResult {
  const start = blocks.findIndex((b) => b.id === blockId);
  if (start < 0) return unchanged(blocks, blockId);
  const nextStart = nextSiblingStart(blocks, start);
  if (nextStart < 0) return unchanged(blocks, blockId);
  const end = subtreeEnd(blocks, start);
  const nextEnd = subtreeEnd(blocks, nextStart);
  const selected = blocks.slice(start, end);
  const next = blocks.slice(nextStart, nextEnd);
  return {
    blocks: [
      ...blocks.slice(0, start),
      ...next,
      ...selected,
      ...blocks.slice(nextEnd),
    ],
    focusedId: blockId,
    changed: true,
  };
}

export function outdentSubtreeToRoot(blocks: ParsedBlock[], blockId: string): TreeMoveResult {
  const start = blocks.findIndex((b) => b.id === blockId);
  const block = blocks[start];
  if (!block) return unchanged(blocks, blockId);
  if (block.indent_level === 0) return unchanged(blocks, blockId);
  const end = subtreeEnd(blocks, start);
  const delta = -block.indent_level;
  return {
    blocks: [
      ...blocks.slice(0, start),
      ...rebaseSubtree(blocks.slice(start, end), delta),
      ...blocks.slice(end),
    ],
    focusedId: blockId,
    changed: true,
  };
}

export function moveSubtreeUnder(
  blocks: ParsedBlock[],
  blockId: string,
  parentId: string,
): TreeMoveResult {
  if (blockId === parentId) return unchanged(blocks, blockId);
  const start = blocks.findIndex((b) => b.id === blockId);
  const parentStart = blocks.findIndex((b) => b.id === parentId);
  const block = blocks[start];
  const parent = blocks[parentStart];
  if (!block || !parent) return unchanged(blocks, blockId);
  const end = subtreeEnd(blocks, start);
  if (parentStart > start && parentStart < end) return unchanged(blocks, blockId);

  const subtree = blocks.slice(start, end);
  const withoutSubtree = [
    ...blocks.slice(0, start),
    ...blocks.slice(end),
  ];
  const parentIndex = withoutSubtree.findIndex((b) => b.id === parentId);
  if (parentIndex < 0) return unchanged(blocks, blockId);
  const insertAt = subtreeEnd(withoutSubtree, parentIndex);
  const delta = parent.indent_level + 1 - block.indent_level;
  const moved = rebaseSubtree(subtree, delta);

  return {
    blocks: [
      ...withoutSubtree.slice(0, insertAt),
      ...moved,
      ...withoutSubtree.slice(insertAt),
    ],
    focusedId: blockId,
    changed: true,
  };
}
