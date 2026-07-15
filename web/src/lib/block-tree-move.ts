import type { ParsedBlock } from "./types/ParsedBlock";

export type MovePlacement = "before" | "inside" | "after" | "append";

/** Stable identity for a rendered block/editor. Canonical bids survive line
 * shifts and reorders; legacy bid-less blocks fall back to their namespaced
 * local id so the two key domains cannot collide. */
export function stableBlockKey(block: Pick<ParsedBlock, "id" | "bid">): string {
  return block.bid ? `bid:${block.bid}` : `id:${block.id}`;
}

export type FocusRestorationOptions<T> = {
  maxAttempts: number;
  stableAttempts?: number;
  findTarget: () => T | null | Promise<T | null>;
  waitForRetry: () => Promise<void>;
  isTargetFocused?: (target: T) => boolean;
  focusTarget: (target: T) => void;
};

export type FocusRestorationClaim = {
  readonly __focusRestorationClaim: never;
};

export type FocusRestorationController = {
  claim(): FocusRestorationClaim;
  restore<T>(
    claim: FocusRestorationClaim,
    options: FocusRestorationOptions<T>,
  ): Promise<boolean>;
  revoke(): void;
  dispose(): void;
};

/** Owns delayed focus restoration so a stale retry cannot overwrite newer
 * user intent. A logical restoration claims its lease before any prerequisite
 * work; revocation and teardown invalidate it without consuming user input. */
export function createFocusRestorationController(): FocusRestorationController {
  let lease = 0;
  let disposed = false;
  const claims = new WeakMap<FocusRestorationClaim, number>();
  const owns = (claim: FocusRestorationClaim) =>
    !disposed && claims.get(claim) === lease;

  return {
    claim() {
      const claim = {} as FocusRestorationClaim;
      claims.set(claim, ++lease);
      return claim;
    },
    async restore<T>(
      claim: FocusRestorationClaim,
      {
        maxAttempts,
        stableAttempts = 1,
        findTarget,
        waitForRetry,
        isTargetFocused,
        focusTarget,
      }: FocusRestorationOptions<T>,
    ): Promise<boolean> {
      let stable = 0;
      for (let attempt = 0; attempt < maxAttempts; attempt++) {
        if (!owns(claim)) return false;
        const target = await findTarget();
        if (target !== null) {
          if (!owns(claim)) return false;
          if (!isTargetFocused?.(target)) {
            focusTarget(target);
            stable = 1;
          } else {
            stable += 1;
          }
          if (stable >= stableAttempts) return true;
        } else {
          stable = 0;
        }
        if (attempt + 1 >= maxAttempts) break;
        if (!owns(claim)) return false;
        await waitForRetry();
      }
      return false;
    },
    revoke() {
      lease += 1;
    },
    dispose() {
      disposed = true;
      lease += 1;
    },
  };
}

export type BlockMoveRequest = {
  move_id: string;
  source_note_id: string;
  root_bid: string;
  destination_note_id: string;
  target_bid: string | null;
  placement: MovePlacement;
};

export function isBlockMoveRequest(value: unknown): value is BlockMoveRequest {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const request = value as Partial<BlockMoveRequest>;
  if (
    typeof request.move_id !== "string"
    || !UUID_PATTERN.test(request.move_id)
    || typeof request.source_note_id !== "string"
    || request.source_note_id.length === 0
    || typeof request.root_bid !== "string"
    || !UUID_PATTERN.test(request.root_bid)
    || typeof request.destination_note_id !== "string"
    || request.destination_note_id.length === 0
    || !["before", "inside", "after", "append"].includes(request.placement ?? "")
  ) return false;
  return request.placement === "append"
    ? request.target_bid === null
    : typeof request.target_bid === "string" && UUID_PATTERN.test(request.target_bid);
}

export type BlockMoveSession = {
  phase: "idle" | "selecting" | "pending" | "retryable";
  request: BlockMoveRequest | null;
  targetBid: string | null;
  targetNoteId: string | null;
  placement: MovePlacement | null;
};

export type BlockMoveSessionAction =
  | { type: "start"; request: BlockMoveRequest }
  | { type: "target"; noteId: string; bid: string | null; placement: MovePlacement }
  | { type: "submit" }
  | { type: "success" | "cancel" | "ordinary-error" }
  | { type: "recoverable-error" };

export const IDLE_BLOCK_MOVE_SESSION: BlockMoveSession = {
  phase: "idle",
  request: null,
  targetBid: null,
  targetNoteId: null,
  placement: null,
};

export function isBlockRelocationTarget(
  targetBid: string | null,
  blockBid: string | null | undefined,
): boolean {
  return targetBid !== null && blockBid != null && targetBid === blockBid;
}

export function reduceBlockMoveSession(
  state: BlockMoveSession,
  action: BlockMoveSessionAction,
): BlockMoveSession {
  switch (action.type) {
    case "start":
      if (state.phase !== "idle") return state;
      return {
        phase: "selecting",
        request: action.request,
        targetBid: null,
        targetNoteId: null,
        placement: null,
      };
    case "target":
      if (state.phase !== "selecting" || !state.request) return state;
      return {
        phase: "selecting",
        request: {
          ...state.request,
          destination_note_id: action.noteId,
          target_bid: action.bid,
          placement: action.placement,
        },
        targetBid: action.bid,
        targetNoteId: action.noteId,
        placement: action.placement,
      };
    case "submit":
      if ((state.phase !== "selecting" && state.phase !== "retryable") || !state.request) {
        return state;
      }
      return { ...state, phase: "pending" };
    case "recoverable-error":
      return state.phase === "pending" && state.request
        ? { ...state, phase: "retryable" }
        : state;
    case "cancel":
      return state.phase === "selecting" ? IDLE_BLOCK_MOVE_SESSION : state;
    case "success":
    case "ordinary-error":
      return IDLE_BLOCK_MOVE_SESSION;
  }
}

export type BlockMoveResponse<TNote extends { id: string }> = {
  move_id: string;
  notes: TNote[];
};

export type BlockMoveFailureClassification =
  | { kind: "definitive"; message: string; blockingMoveId: null }
  | { kind: "retryable"; message: string; blockingMoveId: null }
  | { kind: "blocked-by-other"; message: string; blockingMoveId: string }
  | { kind: "ambiguous"; message: string | null; blockingMoveId: null };

export function classifyBlockMoveFailure(
  status: number | undefined,
  body: string | undefined,
  expectedMoveId: string,
): BlockMoveFailureClassification {
  let parsed: { error?: unknown; move_id?: unknown; retry_safe?: unknown };
  try {
    parsed = JSON.parse(body ?? "") as typeof parsed;
  } catch {
    return { kind: "ambiguous", message: null, blockingMoveId: null };
  }
  const message = typeof parsed.error === "string" ? parsed.error : null;
  if ((status === 400 || status === 404 || status === 409) && message) {
    return { kind: "definitive", message, blockingMoveId: null };
  }
  if (
    status === 503
    && parsed.retry_safe === true
    && typeof parsed.move_id === "string"
    && UUID_PATTERN.test(parsed.move_id)
  ) {
    return parsed.move_id === expectedMoveId
      ? {
          kind: "retryable",
          message: message ?? "Block move requires an exact-request retry",
          blockingMoveId: null,
        }
      : {
          kind: "blocked-by-other",
          message: message ?? "An earlier block move requires recovery",
          blockingMoveId: parsed.move_id,
        };
  }
  return { kind: "ambiguous", message, blockingMoveId: null };
}

export function isDefinitiveBlockMoveRejection(
  status: number | undefined,
  body: string | undefined,
): boolean {
  return classifyBlockMoveFailure(status, body, "").kind === "definitive";
}

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
const BLOCK_MOVE_TEXT_MIME = "text/plain";
const BLOCK_MOVE_TEXT_PREFIX = "tesela-block-move:";

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
  const edgeRail = Math.min(6, rect.height / 4);
  const firstBoundary = rect.top + edgeRail;
  const secondBoundary = rect.top + rect.height - edgeRail;
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

type WritableBlockMoveDataTransfer = {
  clearData: () => void;
  setData: (format: string, data: string) => void;
  effectAllowed: string;
};

export function seedBlockMoveDragData(
  transfer: WritableBlockMoveDataTransfer,
  payload: BlockMoveDragPayload,
): boolean {
  const encoded = encodeBlockMoveDragPayload(payload);
  try {
    transfer.clearData();
  } catch {
    // A writable drag store can still accept the two formats below.
  }

  let seeded = false;
  for (const [format, data] of [
    [BLOCK_MOVE_MIME, encoded],
    [BLOCK_MOVE_TEXT_MIME, `${BLOCK_MOVE_TEXT_PREFIX}${payload.move_id}`],
  ] as const) {
    try {
      transfer.setData(format, data);
      seeded = true;
    } catch {
      // WKWebView may reject custom formats while accepting text/plain.
    }
  }
  if (!seeded) return false;
  try {
    transfer.effectAllowed = "move";
  } catch {
    // The move session remains identified by its exact payload/marker.
  }
  return true;
}

export function blockMoveDragHasSupportedType(types: readonly string[]): boolean {
  return types.includes(BLOCK_MOVE_MIME) || types.includes(BLOCK_MOVE_TEXT_MIME);
}

export function blockMoveDragMatchesRequest(
  types: readonly string[],
  readData: (format: string) => string,
  request: Pick<BlockMoveRequest, "move_id" | "source_note_id" | "root_bid">,
): boolean {
  if (types.includes(BLOCK_MOVE_MIME)) {
    try {
      const payload = decodeBlockMoveDragPayload(types, readData(BLOCK_MOVE_MIME));
      if (
        payload?.move_id === request.move_id
        && payload.source_note_id === request.source_note_id
        && payload.root_bid === request.root_bid
      ) return true;
    } catch {
      // Fall through to the text marker used by embedded WebKit.
    }
  }
  if (!types.includes(BLOCK_MOVE_TEXT_MIME)) return false;
  try {
    return readData(BLOCK_MOVE_TEXT_MIME) === `${BLOCK_MOVE_TEXT_PREFIX}${request.move_id}`;
  } catch {
    return false;
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

export function sameNoteMoveRequestForAction(
  blocks: ParsedBlock[],
  focusedBid: string,
  noteId: string,
  action: "up" | "down" | "indent",
  moveId: string,
  previousVisibleBid?: string,
): BlockMoveRequest | null {
  const sourceStart = blocks.findIndex((block) => block.bid === focusedBid);
  if (sourceStart < 0 || !focusedBid || !noteId || !moveId) return null;

  let targetIndex: number;
  let placement: Exclude<MovePlacement, "append">;
  if (action === "up") {
    targetIndex = previousSiblingStart(blocks, sourceStart);
    placement = "before";
  } else if (action === "down") {
    targetIndex = nextSiblingStart(blocks, sourceStart);
    placement = "after";
  } else {
    targetIndex = previousVisibleBid
      ? blocks.findIndex((block) => block.bid === previousVisibleBid)
      : sourceStart - 1;
    placement = "inside";
  }
  if (targetIndex < 0) return null;
  const targetBid = blocks[targetIndex]?.bid;
  if (typeof targetBid !== "string" || targetBid.length === 0) return null;

  try {
    const plan = planBlockMove({
      sourceBlocks: blocks,
      rootBid: focusedBid,
      destinationBlocks: blocks,
      targetBid,
      placement,
      sameNote: true,
    });
    if (plan.noOp) return null;
  } catch {
    return null;
  }

  return {
    move_id: moveId,
    source_note_id: noteId,
    root_bid: focusedBid,
    destination_note_id: noteId,
    target_bid: targetBid,
    placement,
  };
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
