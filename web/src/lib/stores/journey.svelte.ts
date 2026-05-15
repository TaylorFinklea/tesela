/**
 * Prism v4 — Journey breadcrumb store.
 *
 * Independent of `lib/stores/navigation.svelte.ts` (which tracks URL
 * paths for the legacy browser back/forward). Journey tracks *tiles*
 * — every `jumpToTile` call funnels through here, giving the Journey
 * bar a chronological trail of where the user has been. ⌘[ walks
 * back one step; clicking a chip jumps directly.
 *
 * Capped at MAX entries (FIFO) so a long session doesn't bloat. Not
 * persisted — the layout snapshot in localStorage is the durable
 * record; Journey is the in-session history.
 */

export type JourneyEntry = {
  tileId: string;
  /** Source of the navigation: `palette`, `peek`, `back`, `url`, etc.
   *  Mostly cosmetic — surfaces as a tiny tag chip on hover. */
  via: string;
  ts: number;
};

const MAX = 10;

let entries = $state<JourneyEntry[]>([]);
let cursor = $state(-1);
/** When true, the next push is consumed silently — used by `goBack` /
 *  `goForward` so the resulting `jumpToTile` doesn't re-record itself. */
let suppressNextPush = false;

export function pushJourney(tileId: string, via: string = "manual") {
  if (suppressNextPush) {
    suppressNextPush = false;
    // The jump came from a back/forward; just move the cursor to it.
    const idx = entries.findIndex((e) => e.tileId === tileId);
    if (idx >= 0) cursor = idx;
    return;
  }
  // Dedupe consecutive landings on the same tile (avoids loop spam from
  // effects that re-run on tab switches).
  const cur = entries[cursor];
  if (cur && cur.tileId === tileId) return;

  // Drop any forward history past the cursor.
  if (cursor < entries.length - 1) {
    entries.splice(cursor + 1);
  }
  entries.push({ tileId, via, ts: Date.now() });
  if (entries.length > MAX) {
    entries.splice(0, entries.length - MAX);
  }
  cursor = entries.length - 1;
}

export function getJourneyEntries(): readonly JourneyEntry[] {
  return entries;
}

export function getJourneyCursor(): number {
  return cursor;
}

export function canGoBackInJourney(): boolean {
  return cursor > 0;
}

export function canGoForwardInJourney(): boolean {
  return cursor >= 0 && cursor < entries.length - 1;
}

/** Returns the tile to jump to, or undefined if there's nothing to walk
 *  back to. The caller is responsible for actually calling jumpToTile —
 *  this function only updates the cursor + flips the suppression flag.
 */
export function goBackInJourney(): string | undefined {
  if (!canGoBackInJourney()) return undefined;
  cursor -= 1;
  suppressNextPush = true;
  return entries[cursor].tileId;
}

export function goForwardInJourney(): string | undefined {
  if (!canGoForwardInJourney()) return undefined;
  cursor += 1;
  suppressNextPush = true;
  return entries[cursor].tileId;
}

/** Jump to an arbitrary entry by index (clicking a chip). */
export function jumpToJourneyEntry(idx: number): string | undefined {
  if (idx < 0 || idx >= entries.length) return undefined;
  cursor = idx;
  suppressNextPush = true;
  return entries[idx].tileId;
}

export function clearJourney() {
  entries = [];
  cursor = -1;
  suppressNextPush = false;
}
