import type { ParsedBlock } from "./types/ParsedBlock";

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
