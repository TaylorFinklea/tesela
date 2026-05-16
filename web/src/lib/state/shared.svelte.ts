/**
 * Prism v5 — shared workspace state for pinned · recent · search.
 *
 * - **Pinned**: user-curated, disk-persisted as a small JSON file. Phase 7
 *   ships a localStorage-backed stub so the UI works; a proper file-based
 *   pinned store lands when the backend exposes a workspace-state API.
 *   TanStack Query is the cache layer; the consumer-side wrapper exposes
 *   reactive `$state` values that surfaces (sidebar + Station) both read
 *   from one source.
 *
 * - **Recent**: LRU capped at 50, populated on every `focusPane(page)`.
 *   Local-only (each device sees its own recent queue). localStorage-
 *   backed.
 *
 * - **Search**: not held here. Surfaces issue a TanStack Query keyed by
 *   the query string; uses `api.listNotes` with title-substring filter
 *   for now. A real index lands later.
 */

const PINNED_KEY = "tesela:workspace:pinned";
const PINNED_BLOCKS_KEY = "tesela:workspace:pinned-blocks";
const RECENT_KEY = "tesela:workspace:recent";
const RECENT_LIMIT = 50;

export type PinnedBlock = {
  pageId: string;
  blockId: string;
  preview: string;
};

function readArray(key: string): string[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    return Array.isArray(arr) ? (arr as string[]).filter((s) => typeof s === "string") : [];
  } catch {
    return [];
  }
}

function writeArray(key: string, arr: string[]): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(key, JSON.stringify(arr));
  } catch (e) {
    console.warn("shared workspace state persist failed", e);
  }
}

function readBlocks(): PinnedBlock[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(PINNED_BLOCKS_KEY);
    if (!raw) return [];
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return [];
    return arr.filter(
      (b): b is PinnedBlock =>
        b &&
        typeof b.pageId === "string" &&
        typeof b.blockId === "string" &&
        typeof b.preview === "string",
    );
  } catch {
    return [];
  }
}

function writeBlocks(arr: PinnedBlock[]): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(PINNED_BLOCKS_KEY, JSON.stringify(arr));
  } catch (e) {
    console.warn("pinned-blocks persist failed", e);
  }
}

let pinned = $state<string[]>(readArray(PINNED_KEY));
let pinnedBlocks = $state<PinnedBlock[]>(readBlocks());
let recent = $state<string[]>(readArray(RECENT_KEY));

// ── pinned ─────────────────────────────────────────────────────────────────

export function getPinned(): string[] {
  return pinned;
}

export function isPinned(pageId: string): boolean {
  return pinned.includes(pageId);
}

export function togglePin(pageId: string): void {
  if (!pageId) return;
  if (pinned.includes(pageId)) {
    pinned = pinned.filter((p) => p !== pageId);
  } else {
    pinned = [pageId, ...pinned];
  }
  writeArray(PINNED_KEY, pinned);
}

export function pin(pageId: string): void {
  if (!pageId || pinned.includes(pageId)) return;
  pinned = [pageId, ...pinned];
  writeArray(PINNED_KEY, pinned);
}

export function unpin(pageId: string): void {
  if (!pinned.includes(pageId)) return;
  pinned = pinned.filter((p) => p !== pageId);
  writeArray(PINNED_KEY, pinned);
}

// ── pinned blocks ──────────────────────────────────────────────────────────

export function getPinnedBlocks(): PinnedBlock[] {
  return pinnedBlocks;
}

export function isPinnedBlock(pageId: string, blockId: string): boolean {
  return pinnedBlocks.some(
    (b) => b.pageId === pageId && b.blockId === blockId,
  );
}

export function togglePinBlock(
  pageId: string,
  blockId: string,
  preview: string,
): void {
  if (!pageId || !blockId) return;
  const exists = pinnedBlocks.some(
    (b) => b.pageId === pageId && b.blockId === blockId,
  );
  if (exists) {
    pinnedBlocks = pinnedBlocks.filter(
      (b) => !(b.pageId === pageId && b.blockId === blockId),
    );
  } else {
    pinnedBlocks = [{ pageId, blockId, preview }, ...pinnedBlocks];
  }
  writeBlocks(pinnedBlocks);
}

export function unpinBlock(pageId: string, blockId: string): void {
  pinnedBlocks = pinnedBlocks.filter(
    (b) => !(b.pageId === pageId && b.blockId === blockId),
  );
  writeBlocks(pinnedBlocks);
}

// ── recent ─────────────────────────────────────────────────────────────────

export function getRecent(): string[] {
  return recent;
}

/**
 * Push a page id onto the recent LRU. Called by the focusPane chokepoint
 * when a page buffer gains focus. Idempotent on no-op (re-focusing the
 * current top item just moves the timestamp implicitly via list ordering).
 */
export function touchRecent(pageId: string): void {
  if (!pageId) return;
  const next = [pageId, ...recent.filter((p) => p !== pageId)].slice(
    0,
    RECENT_LIMIT,
  );
  if (next.length === recent.length && next.every((v, i) => v === recent[i])) return;
  recent = next;
  writeArray(RECENT_KEY, recent);
}

export function clearRecent(): void {
  recent = [];
  writeArray(RECENT_KEY, []);
}
