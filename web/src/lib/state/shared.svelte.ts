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
const RECENT_KEY = "tesela:workspace:recent";
const RECENT_LIMIT = 50;

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

let pinned = $state<string[]>(readArray(PINNED_KEY));
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
