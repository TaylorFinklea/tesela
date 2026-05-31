/**
 * WebSocket-driven refetch coordinator.
 *
 * Problem this solves
 * -------------------
 * The server echoes every `PUT /notes/{id}` back to the *editing* client as a
 * `note_updated` WsEvent (see `crates/tesela-server/src/routes/notes.rs`
 * `update_note` → `ws_tx.send(WsEvent::NoteUpdated{..})`). The web layout's
 * per-event handler responded by firing a fan of `invalidateQueries`, and
 * because `["notes"]` prefix-matches EVERY mounted `["notes", ...]` query
 * (the daily list `?tag=daily&limit=60`, the autocomplete list `?limit=200`,
 * any `?limit=500` sidebar/ambient query), a single edit triggered a full
 * multi-query refetch pass. A 5-edit burst → 5 echoes → 5 passes, none
 * coalesced. The stale list responses then reseeded the actively-edited
 * editor's `body` prop, racing the user's in-flight save (the "edits clear
 * on refresh" clobber).
 *
 * The fix mirrors the iOS client (`MockMosaicService.scheduleRemoteRefresh`'s
 * 300ms debounce + `suppressRemoteUntil` own-echo window):
 *
 *  1. **Coalesce** — a burst of WS events within ~300ms collapses into ONE
 *     refetch pass instead of N.
 *  2. **Own-echo suppression** — note ids this client just saved (within a
 *     short window, recorded by the api-client write path) do not trigger a
 *     refetch of their own `["note", id]` query, so the optimistic
 *     `setQueryData` the editor already wrote is not clobbered by a stale
 *     refetch. List queries still refresh on the coalesced pass.
 *
 * The per-note editor (`BlockOutliner`) keeps its own mid-typing clobber
 * guard (the 1200ms reparse cooldown + local-only-id protection); this module
 * is the upstream half that stops the storm before it reaches the editor.
 */

/** Window after a local save during which an incoming `note_updated` for the
 *  same id is treated as this client's own echo and its targeted `["note",id]`
 *  invalidation is skipped. Mirrors iOS `suppressRemoteUntil` (~1.5s). */
const OWN_ECHO_WINDOW_MS = 1_500;

/** Coalesce window for WS-driven refetches. Mirrors the iOS 300ms debounce. */
const COALESCE_MS = 300;

/** noteId → wall-clock ms of the most recent local save. Populated by the
 *  api-client write path (`recordLocalSave`). Pruned lazily on read. */
const recentSaves = new Map<string, number>();

/**
 * Record that this client just issued a write (PUT/POST) for `noteId`. The
 * api-client write path calls this so the WS echo that follows can be
 * recognised as our own. Safe to call with any id; cheap.
 */
export function recordLocalSave(noteId: string): void {
  recentSaves.set(noteId, Date.now());
}

/**
 * True when `noteId` was saved by this client within the own-echo window —
 * i.e. an incoming `note_updated` for it is almost certainly the server
 * echoing back our own PUT, not a genuine remote change.
 */
export function isOwnEcho(noteId: string): boolean {
  const t = recentSaves.get(noteId);
  if (t === undefined) return false;
  if (Date.now() - t > OWN_ECHO_WINDOW_MS) {
    recentSaves.delete(noteId);
    return false;
  }
  return true;
}

// ── Coalesced refetch scheduling ───────────────────────────────────────────

let coalesceTimer: ReturnType<typeof setTimeout> | null = null;
/** Note ids whose targeted `["note", id]` refresh is requested for the next
 *  coalesced pass. Ids recognised as own-echoes are filtered before enqueue,
 *  so they never land here. */
const pendingNoteIds = new Set<string>();
/** True when at least one pending event wants the broad list/ambient refresh
 *  (`["notes"]`, `["typed-blocks"]`, `["agenda"]`, `["widget","inbox"]`). */
let pendingBroad = false;
let flushCb: ((batch: { noteIds: string[]; broad: boolean }) => void) | null = null;

/**
 * Register the function that performs the actual `invalidateQueries` fan-out.
 * Called once from the layout with a closure over its `QueryClient`. The
 * coordinator owns the *timing* (coalescing); the callback owns *which*
 * queries to touch.
 */
export function setRefreshCallback(
  cb: (batch: { noteIds: string[]; broad: boolean }) => void,
): void {
  flushCb = cb;
}

/**
 * Enqueue a coalesced refetch pass for a `note_updated` / `note_created` /
 * `note_deleted` event. `noteId` is the affected note (for the targeted
 * `["note", id]` invalidation); pass `null` when there is no specific id.
 * `broad` requests the list/ambient fan-out (always true for these events
 * today, but kept explicit for callers that only want a targeted refresh).
 *
 * Own-echo note ids are dropped from the targeted set here, but `broad` still
 * fires on the coalesced pass so lists/ambients stay fresh — they don't feed
 * the actively-edited buffer, so refreshing them can't clobber an edit.
 */
export function scheduleNoteRefresh(noteId: string | null, broad: boolean): void {
  if (noteId && !isOwnEcho(noteId)) pendingNoteIds.add(noteId);
  if (broad) pendingBroad = true;
  if (coalesceTimer !== null) return;
  coalesceTimer = setTimeout(flushPending, COALESCE_MS);
}

/**
 * Force any pending coalesced pass to run immediately (e.g. on reconnect,
 * where we want to recover missed events without waiting out the debounce).
 */
export function flushNoteRefreshNow(): void {
  if (coalesceTimer !== null) {
    clearTimeout(coalesceTimer);
    coalesceTimer = null;
  }
  flushPending();
}

function flushPending(): void {
  coalesceTimer = null;
  const noteIds = [...pendingNoteIds];
  const broad = pendingBroad;
  pendingNoteIds.clear();
  pendingBroad = false;
  if (noteIds.length === 0 && !broad) return;
  flushCb?.({ noteIds, broad });
}

// Exposed for tests.
export const __test = {
  OWN_ECHO_WINDOW_MS,
  COALESCE_MS,
  reset() {
    recentSaves.clear();
    pendingNoteIds.clear();
    pendingBroad = false;
    if (coalesceTimer !== null) {
      clearTimeout(coalesceTimer);
      coalesceTimer = null;
    }
    flushCb = null;
  },
};
