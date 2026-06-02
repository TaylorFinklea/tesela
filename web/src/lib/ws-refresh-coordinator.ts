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
 *  3. **Own-echo re-settle** — suppressing the targeted refetch is correct
 *     mid-save, but an own-echo `note_updated` can ALSO carry a peer's
 *     concurrently-merged block (the server converged both edits before
 *     echoing). Dropping the echo outright leaves the editing client split
 *     until it edits again. So instead of dropping it we DEFER the id and
 *     re-enqueue its targeted `["note", id]` refetch once the own-echo
 *     window closes (mirrors iOS `MockMosaicService.pendingRemoteRefresh` +
 *     `scheduleSuppressionFlush`). By firing only after the window — when the
 *     client is no longer mid-save — and only the targeted id, the re-settle
 *     cannot reintroduce the mid-typing reseed this module exists to prevent.
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

/**
 * Wall-clock ms at which the own-echo window for `noteId` closes, or `null`
 * when there is no open window (never saved, or already expired). Used to
 * schedule the deferred re-settle flush exactly when suppression lifts.
 */
function ownEchoExpiryAt(noteId: string): number | null {
  const t = recentSaves.get(noteId);
  if (t === undefined) return null;
  const expiry = t + OWN_ECHO_WINDOW_MS;
  if (expiry <= Date.now()) return null;
  return expiry;
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

/** Note ids whose targeted `["note", id]` refetch was suppressed as an
 *  own-echo but must be re-settled once the own-echo window closes — the echo
 *  may have carried a peer's concurrently-merged block. Analog of iOS
 *  `pendingRemoteRefresh`. */
const deferredNoteIds = new Set<string>();
/** Single trailing-flush timer for the deferred set (mirrors iOS's lone
 *  `suppressionFlush` task). Scheduled to fire just after the earliest open
 *  own-echo window closes; re-armed if a still-suppressed id remains. */
let deferredTimer: ReturnType<typeof setTimeout> | null = null;

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
 * Own-echo note ids are held back from the targeted set here (so a mid-save
 * echo can't reseed the editor over the user's in-flight body), but `broad`
 * still fires on the coalesced pass so lists/ambients stay fresh — they don't
 * feed the actively-edited buffer, so refreshing them can't clobber an edit.
 * A suppressed id is DEFERRED (not dropped): the same echo may carry a peer's
 * concurrently-merged block, so once the own-echo window closes the id is
 * re-enqueued for its targeted refetch (see `armDeferredFlush`).
 */
export function scheduleNoteRefresh(noteId: string | null, broad: boolean): void {
  if (noteId) {
    if (isOwnEcho(noteId)) {
      // Defer the targeted refetch until the own-echo window closes; the echo
      // may have merged a peer edit the editing client must eventually see.
      deferredNoteIds.add(noteId);
      armDeferredFlush();
    } else {
      pendingNoteIds.add(noteId);
    }
  }
  if (broad) pendingBroad = true;
  if (coalesceTimer !== null) return;
  coalesceTimer = setTimeout(flushPending, COALESCE_MS);
}

/**
 * Ensure a single trailing flush is scheduled to re-settle deferred own-echo
 * ids once their suppression windows close. Mirrors iOS's lone
 * `scheduleSuppressionFlush`: at most one timer is armed at a time, set to the
 * earliest still-open window's expiry. When it fires (`flushDeferred`) it
 * re-checks each id — a fresh `recordLocalSave` may have extended the window,
 * in which case that id is re-deferred and the timer re-armed for the new
 * expiry, so the client always converges without an infinite spin (every
 * re-arm waits out a real, finite window).
 */
function armDeferredFlush(): void {
  if (deferredTimer !== null) return;
  let earliest: number | null = null;
  for (const id of deferredNoteIds) {
    const expiry = ownEchoExpiryAt(id);
    if (expiry === null) continue; // window already closed; flush picks it up
    if (earliest === null || expiry < earliest) earliest = expiry;
  }
  // +1ms so the timer fires strictly AFTER the window closes (isOwnEcho false).
  const delay = earliest === null ? 0 : Math.max(0, earliest - Date.now()) + 1;
  deferredTimer = setTimeout(flushDeferred, delay);
}

/**
 * Re-enqueue targeted `["note", id]` refetches for deferred ids whose own-echo
 * window has closed. Ids still inside an (extended) window are re-deferred and
 * a fresh timer is armed for them. Routes each ready id back through
 * `scheduleNoteRefresh(id, false)`: that re-checks `isOwnEcho` (now false, so
 * the id lands in the targeted set, no broad) and folds into the normal
 * coalesced pass — no double-fire, since a still-suppressed id never reaches
 * `pendingNoteIds`.
 */
function flushDeferred(): void {
  deferredTimer = null;
  if (deferredNoteIds.size === 0) return;
  const ready: string[] = [];
  const stillSuppressed: string[] = [];
  for (const id of deferredNoteIds) {
    if (isOwnEcho(id)) stillSuppressed.push(id);
    else ready.push(id);
  }
  deferredNoteIds.clear();
  for (const id of ready) scheduleNoteRefresh(id, false);
  // A fresh local save extended the window for these — keep deferring them and
  // re-arm the timer for the new (finite) expiry. Eventually the user stops
  // saving and the window closes, so this terminates.
  if (stillSuppressed.length > 0) {
    for (const id of stillSuppressed) deferredNoteIds.add(id);
    armDeferredFlush();
  }
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
    deferredNoteIds.clear();
    if (coalesceTimer !== null) {
      clearTimeout(coalesceTimer);
      coalesceTimer = null;
    }
    if (deferredTimer !== null) {
      clearTimeout(deferredTimer);
      deferredTimer = null;
    }
    flushCb = null;
  },
  /** Drain ready deferred ids synchronously (test-only): mirrors what the
   *  trailing timer does, without waiting out a real timer in unit tests. */
  flushDeferredNow() {
    if (deferredTimer !== null) {
      clearTimeout(deferredTimer);
      deferredTimer = null;
    }
    flushDeferred();
  },
  hasDeferred(id: string) {
    return deferredNoteIds.has(id);
  },
  /** Backdate `id`'s own-echo timestamp so its suppression window is already
   *  closed (test-only): lets a test exercise the post-expiry re-settle
   *  without waiting out the real 1500ms window. */
  expireOwnEcho(id: string) {
    recentSaves.set(id, Date.now() - OWN_ECHO_WINDOW_MS - 1);
  },
};
