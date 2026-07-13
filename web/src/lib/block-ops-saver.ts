/**
 * Per-note debounce + coalesce + in-flight-cancel for block-granular writes
 * (sync redesign 2026-06-02, S1 follow-up).
 *
 * S1 (commit 523b25d) moved in-place text edits off the whole-body PUT path
 * onto `POST /notes/{id}/blocks`, but dropped the 500ms debounce the PUT path
 * had (JournalView's `saveStates`). CM6 fires `onChange` on every keystroke,
 * so the editor was POSTing per-keystroke. This restores per-note coalescing:
 *
 *  - A burst of `enqueue` calls for note N within the window collapses into
 *    ONE trailing-edge POST. Ops are keyed by `bid` so repeated edits to the
 *    same block keep only the LATEST op (the final text), not a per-keystroke
 *    pile.
 *  - When a new coalesced POST supersedes an in-flight one, the old request is
 *    aborted (its `AbortController`) and the new controller's `signal` is
 *    threaded into the POST. An aborted POST is expected, not a failure, so it
 *    must NOT trigger the whole-body-PUT loss-avoidance fallback (that would
 *    double-write).
 *
 * Kept Svelte/DOM-free so the coalescing core is unit-tested directly (see
 * `web/tests/unit/block-ops-saver.test.mjs`) and the one-path-per-save +
 * abort-swallow contract lives in one auditable place. The Svelte component
 * (`BlockOutliner.svelte`) owns the actual `api.upsertBlocks` / whole-body-PUT
 * calls and passes them in as callbacks.
 */
import type { BlockOp } from "$lib/block-ops";

/** Match the whole-body PUT debounce (JournalView `handleContentChange`). */
export const BLOCK_OPS_DEBOUNCE_MS = 500;

/** Detect an aborted fetch. `fetch` rejects with a DOMException whose `name`
 *  is "AbortError" when its signal aborts; some environments surface a plain
 *  error with that name. Either way the abort is expected — the caller must
 *  swallow it and NOT fall back to a whole-body PUT (which would double-write
 *  the superseding edit). */
export function isAbortError(err: unknown): boolean {
  return (
    err instanceof Error && err.name === "AbortError"
  ) || (
    typeof err === "object" &&
    err !== null &&
    "name" in err &&
    (err as { name?: unknown }).name === "AbortError"
  );
}

/** What a flush does when the trailing edge fires (or a forced immediate
 *  flush lands): POST the coalesced ops for `noteId` with an abort signal.
 *  The component supplies this; tests supply a spy. The `noteId` is passed
 *  (not read live) because it can change between enqueue and a trailing flush
 *  — the POST must target the note the ops belong to. Returns the in-flight
 *  promise so the saver can attach abort-swallowing + error handling. */
export type UpsertFn = (
  noteId: string,
  ops: BlockOp[],
  signal: AbortSignal,
) => Promise<unknown>;

/** Loss-avoidance fallback for a genuine (non-abort) POST failure on
 *  `noteId`: PUT the whole body so the edit still persists. */
export type FallbackFn = (noteId: string) => void | Promise<unknown>;

interface InFlightSave {
  controller: AbortController;
  completion: Promise<void>;
}

interface NoteSaveState {
  /** Latest op per block (`bid` → op). Keyed so a burst of edits to one block
   *  collapses to a single upsert carrying the final text. */
  ops: Map<string, BlockOp>;
  timer: ReturnType<typeof setTimeout> | null;
  inFlight: InFlightSave | null;
  settlers: number;
}

/**
 * Per-note coalescing block-ops saver. One instance per BlockOutliner; keyed
 * internally by note id because `noteId` can change within a single component
 * instance (drill / Esc-back page nav) and each note's pending batch must stay
 * separate.
 */
export class BlockOpsSaver {
  #states = new Map<string, NoteSaveState>();
  #upsert: UpsertFn;
  #fallback: FallbackFn;
  #debounceMs: number;

  constructor(upsert: UpsertFn, fallback: FallbackFn, debounceMs = BLOCK_OPS_DEBOUNCE_MS) {
    this.#upsert = upsert;
    this.#fallback = fallback;
    this.#debounceMs = debounceMs;
  }

  #getState(noteId: string): NoteSaveState {
    let s = this.#states.get(noteId);
    if (!s) {
      s = { ops: new Map(), timer: null, inFlight: null, settlers: 0 };
      this.#states.set(noteId, s);
    }
    return s;
  }

  /**
   * Enqueue a coalesced batch of concrete block ops for `noteId`. Repeated
   * calls within the debounce window merge by `bid` and re-arm the
   * trailing-edge timer. The POST fires once, on the trailing edge.
   *
   * Coalescing is KIND-aware, not blind latest-wins: a `move` op carries only
   * structure (`parent_bid`/`indent_level`) while a pending `upsert` for the
   * same bid is the sole carrier of the block's typed text — replacing the
   * upsert with the move (type, then Tab within one debounce window) would
   * silently drop the last typing burst, with `lastSentBody` already advanced
   * so the own-echo guard masks the loss until a reseed reverts it. So a move
   * over a pending upsert FOLDS its structure into the upsert (upserts carry
   * both fields; the server applies text + position in one op). Every other
   * pairing keeps latest-wins: an upsert already carries structure (it
   * supersedes a pending move), and a delete supersedes everything.
   */
  enqueue(noteId: string, ops: BlockOp[]): void {
    const s = this.#getState(noteId);
    for (const op of ops) {
      const pending = s.ops.get(op.bid);
      if (op.kind === "move" && pending?.kind === "upsert") {
        s.ops.set(op.bid, {
          ...pending,
          parent_bid: op.parent_bid,
          indent_level: op.indent_level,
        });
      } else {
        s.ops.set(op.bid, op);
      }
    }
    if (s.timer) clearTimeout(s.timer);
    if (s.settlers > 0) {
      s.timer = null;
      return;
    }
    s.timer = setTimeout(() => {
      void this.flush(noteId);
    }, this.#debounceMs);
  }

  /**
   * A structural edit that needs the whole-body PUT supersedes any pending
   * block-ops batch for this note: the PUT body is a superset of the coalesced
   * text edits (the editor accumulates them in `blocks`), so re-POSTing the
   * ops too would be a redundant double-send. Cancel the pending batch (timer +
   * any in-flight POST) WITHOUT flushing it, then PUT. One path per save.
   */
  supersedeWithBody(noteId: string, put: () => void): void {
    const s = this.#states.get(noteId);
    if (s) {
      if (s.timer) {
        clearTimeout(s.timer);
        s.timer = null;
      }
      s.ops.clear();
      if (s.inFlight) {
        s.inFlight.controller.abort();
        s.inFlight = null;
      }
    }
    put();
  }

  /**
   * Flush the coalesced ops for `noteId` now (trailing-edge timer, or a forced
   * immediate flush on blur / note-change / teardown so a debounce that hasn't
   * fired never loses the last edit). Ordinarily aborts a superseded in-flight
   * POST, but while `settle` owns the note it leaves a live predecessor intact
   * and lets the settle loop serialize the queued successor behind it.
   */
  flush(noteId: string): void {
    const s = this.#states.get(noteId);
    if (!s) return;
    if (s.timer) {
      clearTimeout(s.timer);
      s.timer = null;
    }
    if (s.settlers > 0 && s.inFlight) return;
    this.#startFlush(noteId, s, s.settlers === 0);
  }

  #startFlush(noteId: string, s: NoteSaveState, abortPrevious: boolean): Promise<void> | null {
    if (s.ops.size === 0) return null;
    const ops = [...s.ops.values()];
    s.ops.clear();
    // Ordinary trailing-edge saves preserve the established latest-wins
    // behavior. `settle`, by contrast, calls this only after the existing
    // request completed, so it never uses an abort as an ordering barrier.
    if (abortPrevious && s.inFlight) s.inFlight.controller.abort();
    const controller = new AbortController();
    const inFlight: InFlightSave = {
      controller,
      completion: Promise.resolve(),
    };
    inFlight.completion = this.#upsert(noteId, ops, controller.signal)
      .then(() => undefined)
      .catch(async (err) => {
        // An abort is expected (a newer coalesced POST superseded this one).
        // Swallow it — falling back to a whole-body PUT here would double-write.
        if (isAbortError(err)) return;
        // Genuine failure (e.g. the note doesn't exist on disk yet): PUT the
        // whole body so the edit still persists.
        await this.#fallback(noteId);
      })
      .finally(() => {
        if (s.inFlight === inFlight) s.inFlight = null;
      });
    s.inFlight = inFlight;
    // Existing callers intentionally fire-and-forget. Attach a rejection
    // handler without changing the stored Promise so `settle` can still
    // observe and propagate a failed whole-body fallback.
    void inFlight.completion.catch(() => {});
    return inFlight.completion;
  }

  /**
   * Durability barrier for a relocation. Flush the queued batch, await the
   * live request rather than aborting it, then repeat if another enqueue
   * arrived while that request was in flight. A genuine POST failure is not
   * settled until its whole-body fallback completes; fallback failure rejects.
   */
  async settle(noteId: string): Promise<void> {
    const s = this.#states.get(noteId);
    if (!s) return;
    s.settlers += 1;
    try {
      while (true) {
        if (s.timer) {
          clearTimeout(s.timer);
          s.timer = null;
        }
        if (s.inFlight) {
          await s.inFlight.completion;
          continue;
        }
        const completion = this.#startFlush(noteId, s, false);
        if (completion) {
          await completion;
          continue;
        }
        return;
      }
    } finally {
      s.settlers -= 1;
      // If this settle failed while a newer batch waited behind it, restore
      // the ordinary debounce path so the queued edit is not stranded.
      if (s.settlers === 0 && s.ops.size > 0 && s.timer === null) {
        s.timer = setTimeout(() => {
          void this.flush(noteId);
        }, this.#debounceMs);
      }
    }
  }

  /** Flush every note's pending batch immediately (component teardown). */
  flushAll(): void {
    for (const noteId of [...this.#states.keys()]) this.flush(noteId);
  }

  /** Test hook: is a debounce timer currently armed for this note? */
  hasPending(noteId: string): boolean {
    const s = this.#states.get(noteId);
    return !!s && (s.timer !== null || s.ops.size > 0);
  }
}
