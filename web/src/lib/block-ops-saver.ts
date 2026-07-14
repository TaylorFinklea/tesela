/**
 * Per-note debounce + coalesce + in-flight serialization for block-granular writes
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
 *  - Once a POST is admitted, a newer coalesced batch waits behind it. Fetch
 *    cancellation cannot prove that the server stopped processing the older
 *    write, so abort-and-replace would make relocation ordering ambiguous.
 *
 * Kept Svelte/DOM-free so the coalescing core is unit-tested directly (see
 * `web/tests/unit/block-ops-saver.test.mjs`) and the one-path-per-save +
 * serialization contract lives in one auditable place. The Svelte component
 * (`BlockOutliner.svelte`) owns the actual `api.upsertBlocks` / whole-body-PUT
 * calls and passes them in as callbacks.
 */
import type { BlockOp } from "$lib/block-ops";

/** Match the whole-body PUT debounce (JournalView `handleContentChange`). */
export const BLOCK_OPS_DEBOUNCE_MS = 500;

/** What a flush does when the trailing edge fires (or a forced immediate
 *  flush lands): POST the coalesced ops for `noteId` with a request signal.
 *  The component supplies this; tests supply a spy. The `noteId` is passed
 *  (not read live) because it can change between enqueue and a trailing flush
 *  — the POST must target the note the ops belong to. Returns the in-flight
 *  promise so the saver can attach durability fallback + error handling. */
export type UpsertFn = (
  noteId: string,
  ops: BlockOp[],
  signal: AbortSignal,
) => Promise<unknown>;

/** Loss-avoidance fallback for an admitted POST failure on
 *  `noteId`: PUT the whole body so the edit still persists. */
export type FallbackFn = (noteId: string) => void | Promise<unknown>;

interface PendingMutationState {
  tail: Promise<void>;
  pending: number;
  failed: boolean;
  failure: unknown;
  reservation: object | null;
}

type ReservationListener = (reserved: boolean) => void;

export interface PerNoteMutationReservation {
  settle(): Promise<void>;
  release(): void;
}

export class PropertyMutationReservedError extends Error {
  constructor(noteId: string) {
    super(`Properties for ${noteId} are reserved for block relocation`);
    this.name = "PropertyMutationReservedError";
  }
}

/**
 * Tracks non-debounced mutations that must be durable before a note can move.
 * Mutation factories run FIFO per note, and `settle` repeats after each tail
 * snapshot so a successor registered while an earlier request is in flight
 * stays inside the same barrier. Rejections remain latched until the owning
 * page is reloaded and authoritatively reconciled: a transport rejection may
 * follow a committed property write, so neither retry nor optimistic rollback
 * can safely declare the note movable.
 */
export class PerNoteMutationBarrier {
  #states = new Map<string, PendingMutationState>();
  #listeners = new Map<string, Set<ReservationListener>>();

  #createState(): PendingMutationState {
    return {
      tail: Promise.resolve(),
      pending: 0,
      failed: false,
      failure: undefined,
      reservation: null,
    };
  }

  isReserved(noteId: string): boolean {
    return this.#states.get(noteId)?.reservation !== null
      && this.#states.get(noteId)?.reservation !== undefined;
  }

  subscribe(noteId: string, listener: ReservationListener): () => void {
    let listeners = this.#listeners.get(noteId);
    if (!listeners) {
      listeners = new Set();
      this.#listeners.set(noteId, listeners);
    }
    listeners.add(listener);
    listener(this.isReserved(noteId));
    return () => {
      const current = this.#listeners.get(noteId);
      current?.delete(listener);
      if (current?.size === 0) this.#listeners.delete(noteId);
    };
  }

  #notifyReservation(noteId: string): void {
    const reserved = this.isReserved(noteId);
    for (const listener of this.#listeners.get(noteId) ?? []) listener(reserved);
  }

  reserve(noteIds: Iterable<string>): PerNoteMutationReservation {
    const ids = [...new Set(noteIds)].filter(Boolean);
    for (const noteId of ids) {
      if (this.isReserved(noteId)) throw new PropertyMutationReservedError(noteId);
    }

    const token = {};
    for (const noteId of ids) {
      let state = this.#states.get(noteId);
      if (!state) {
        state = this.#createState();
        this.#states.set(noteId, state);
      }
      state.reservation = token;
    }
    for (const noteId of ids) this.#notifyReservation(noteId);

    let released = false;
    return {
      settle: async () => {
        if (released) throw new Error("Property mutation reservation was already released");
        await Promise.all(ids.map((noteId) => this.#settleReserved(noteId, token)));
      },
      release: () => {
        if (released) return;
        released = true;
        for (const noteId of ids) {
          const state = this.#states.get(noteId);
          if (!state || state.reservation !== token) continue;
          state.reservation = null;
          if (state.pending === 0 && !state.failed) this.#states.delete(noteId);
        }
        for (const noteId of ids) this.#notifyReservation(noteId);
      },
    };
  }

  track<T>(noteId: string, mutation: () => Promise<T>): Promise<T> {
    let state = this.#states.get(noteId);
    if (state?.reservation) {
      const rejected = Promise.reject<T>(new PropertyMutationReservedError(noteId));
      void rejected.catch(() => {});
      return rejected;
    }
    if (!state) {
      state = this.#createState();
      this.#states.set(noteId, state);
    }
    const noteState = state;
    noteState.pending += 1;

    let completion!: Promise<void>;
    const tracked = noteState.tail
      .then(mutation)
      .catch((error) => {
        if (!noteState.failed) {
          noteState.failed = true;
          noteState.failure = error;
        }
        throw error;
      })
      .finally(() => {
        noteState.pending -= 1;
        if (
          noteState.pending === 0
          && !noteState.failed
          && noteState.reservation === null
          && noteState.tail === completion
          && this.#states.get(noteId) === noteState
        ) {
          this.#states.delete(noteId);
        }
      });
    completion = tracked.then(
      () => undefined,
      () => undefined,
    );
    noteState.tail = completion;
    // Callers may intentionally fire-and-forget. Mark a rejection handled
    // without changing the Promise retained by `settle`, which must still see
    // the sticky failure after draining the note.
    void tracked.catch(() => {});
    return tracked;
  }

  async settle(noteId: string): Promise<void> {
    while (true) {
      const state = this.#states.get(noteId);
      if (!state) return;
      const tail = state.tail;
      await tail;
      if (this.#states.get(noteId) !== state || state.tail !== tail || state.pending > 0) {
        continue;
      }
      if (state.failed) throw state.failure;
      if (state.reservation === null) this.#states.delete(noteId);
      return;
    }
  }

  async #settleReserved(noteId: string, token: object): Promise<void> {
    while (true) {
      const state = this.#states.get(noteId);
      if (!state || state.reservation !== token) {
        throw new Error(`Property mutation reservation for ${noteId} is no longer active`);
      }
      const tail = state.tail;
      await tail;
      if (this.#states.get(noteId) !== state || state.reservation !== token) {
        throw new Error(`Property mutation reservation for ${noteId} is no longer active`);
      }
      if (state.tail !== tail || state.pending > 0) continue;
      if (state.failed) throw state.failure;
      return;
    }
  }
}

type SaveAdmissionDrain = () => Promise<void>;

interface SaveAdmissionEntry {
  token: object;
  drain: SaveAdmissionDrain;
}

export interface SaveAdmissionLease {
  release(): void;
}

/**
 * Non-DOM registry for active save queues that may outlive their mounted
 * editor. A queue admits one lease before its first pending mutation, retains
 * it across every predecessor/successor/fallback, and releases it only after
 * becoming durably quiet. Relocation drains every active lease before
 * reserving the API write lane.
 *
 * Generation changes cover causal hand-offs between queues: if a child queue
 * admits a parent whole-body fallback after that parent already drained, the
 * new lease bumps the generation and settlement repeats. Failed queues retain
 * their lease, so an unmounted editor cannot erase uncertain save ownership.
 */
export class PerNoteSaveAdmissionRegistry {
  #entries = new Map<string, Map<object, SaveAdmissionEntry>>();
  #generations = new Map<string, number>();

  #bump(noteId: string): void {
    this.#generations.set(noteId, (this.#generations.get(noteId) ?? 0) + 1);
  }

  admit(noteId: string, drain: SaveAdmissionDrain): SaveAdmissionLease {
    const token = {};
    let entries = this.#entries.get(noteId);
    if (!entries) {
      entries = new Map();
      this.#entries.set(noteId, entries);
    }
    entries.set(token, { token, drain });
    this.#bump(noteId);

    let active = true;
    return {
      release: () => {
        if (!active) return;
        active = false;
        const current = this.#entries.get(noteId);
        current?.delete(token);
        if (current?.size === 0) this.#entries.delete(noteId);
        this.#bump(noteId);
      },
    };
  }

  async settle(noteIds: Iterable<string>): Promise<void> {
    const ids = [...new Set(noteIds)].filter(Boolean);
    const results = await Promise.allSettled(ids.map((noteId) => this.#settleNote(noteId)));
    const failure = results.find(
      (result): result is PromiseRejectedResult => result.status === "rejected",
    );
    if (failure) throw failure.reason;
  }

  async #settleNote(noteId: string): Promise<void> {
    while (true) {
      const generation = this.#generations.get(noteId) ?? 0;
      const drains = [...(this.#entries.get(noteId)?.values() ?? [])]
        .map((entry) => entry.drain);
      const results = await Promise.allSettled(drains.map((drain) => drain()));
      const failure = results.find(
        (result): result is PromiseRejectedResult => result.status === "rejected",
      );
      if (failure) throw failure.reason;
      if ((this.#generations.get(noteId) ?? 0) === generation) return;
    }
  }
}

export const saveAdmissionRegistry = new PerNoteSaveAdmissionRegistry();

/** Shared by every API property writer and the early relocation UI freeze so
 * duplicate mounted surfaces cannot hide an in-flight or uncertain mutation. */
export const propertyMutationBarrier = new PerNoteMutationBarrier();

/** Tracks note-addressed HTTP writes independently from the early UI/property
 * freeze. Relocation reserves this only after mounted save queues drain; this
 * lets their pre-existing PUT/POST requests finish without allowing a direct
 * or already-unmounted writer to race the durable move. */
export const noteWriteBarrier = new PerNoteMutationBarrier();

export function combineMutationReservations(
  reservations: readonly PerNoteMutationReservation[],
): PerNoteMutationReservation {
  let released = false;
  return {
    settle: async () => {
      if (released) throw new Error("Mutation reservations were already released");
      const results = await Promise.allSettled(
        reservations.map((reservation) => reservation.settle()),
      );
      const failure = results.find(
        (result): result is PromiseRejectedResult => result.status === "rejected",
      );
      if (failure) throw failure.reason;
    },
    release: () => {
      if (released) return;
      released = true;
      for (const reservation of [...reservations].reverse()) reservation.release();
    },
  };
}

export function createCombinedMutationBarrier(
  earlyBarrier: PerNoteMutationBarrier,
  writeBarrier: PerNoteMutationBarrier,
): { reserve(noteIds: Iterable<string>): PerNoteMutationReservation } {
  return {
    reserve: (noteIds) => {
      const ids = [...new Set(noteIds)].filter(Boolean);
      const early = earlyBarrier.reserve(ids);
      try {
        const writes = writeBarrier.reserve(ids);
        return combineMutationReservations([early, writes]);
      } catch (error) {
        early.release();
        throw error;
      }
    },
  };
}

/** Used by reload recovery, which must reconstruct both reservations before
 * any route-level editor or direct writer can mutate an affected note. */
export const blockMoveMutationBarrier = createCombinedMutationBarrier(
  propertyMutationBarrier,
  noteWriteBarrier,
);

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
  failed: boolean;
  failure: unknown;
  admission: SaveAdmissionLease | null;
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
  #admissions: PerNoteSaveAdmissionRegistry;
  #disposePromise: Promise<void> | null = null;

  constructor(
    upsert: UpsertFn,
    fallback: FallbackFn,
    debounceMs = BLOCK_OPS_DEBOUNCE_MS,
    admissions = saveAdmissionRegistry,
  ) {
    this.#upsert = upsert;
    this.#fallback = fallback;
    this.#debounceMs = debounceMs;
    this.#admissions = admissions;
  }

  #getState(noteId: string): NoteSaveState {
    let s = this.#states.get(noteId);
    if (!s) {
      const next: NoteSaveState = {
        ops: new Map(),
        timer: null,
        inFlight: null,
        settlers: 0,
        failed: false,
        failure: undefined,
        admission: null,
      };
      this.#states.set(noteId, next);
      s = next;
    }
    return s;
  }

  #ensureAdmission(noteId: string, s: NoteSaveState): void {
    if (s.admission) return;
    s.admission = this.#admissions.admit(noteId, () => this.settle(noteId));
  }

  #releaseAdmissionIfQuiet(s: NoteSaveState): void {
    if (
      s.failed
      || s.timer !== null
      || s.inFlight !== null
      || s.ops.size > 0
      || s.settlers > 0
    ) return;
    const admission = s.admission;
    s.admission = null;
    admission?.release();
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
    this.#ensureAdmission(noteId, s);
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
   * block-ops batch for this note: the PUT body is a superset of the queued
   * text edits (the editor accumulates them in `blocks`), so re-POSTing those
   * queued ops would be a redundant double-send. Clear only the not-yet-sent
   * batch, then PUT. An already-admitted POST remains alive and the shared API
   * mutation barrier serializes the PUT behind it.
   */
  supersedeWithBody(noteId: string, put: () => void): void {
    const s = this.#states.get(noteId);
    if (s) {
      if (s.timer) {
        clearTimeout(s.timer);
        s.timer = null;
      }
      s.ops.clear();
    }
    try {
      // The parent callback admits its whole-note queue synchronously. Invoke
      // it before releasing a superseded block-op lease so the causal save
      // chain never has an unregistered gap during relocation settlement.
      put();
    } catch (error) {
      if (s && !s.failed) {
        s.failed = true;
        s.failure = error;
      }
      throw error;
    } finally {
      if (s) this.#releaseAdmissionIfQuiet(s);
    }
  }

  /**
   * Flush the coalesced ops for `noteId` now (trailing-edge timer, or a forced
   * immediate flush on blur / note-change / teardown so a debounce that hasn't
   * fired never loses the last edit). A live predecessor is never aborted; the
   * queued successor starts after it settles.
   */
  flush(noteId: string): void {
    const s = this.#states.get(noteId);
    if (!s) return;
    if (s.timer) {
      clearTimeout(s.timer);
      s.timer = null;
    }
    if (s.inFlight) return;
    this.#startFlush(noteId, s);
  }

  #startFlush(noteId: string, s: NoteSaveState): Promise<void> | null {
    if (s.ops.size === 0) return null;
    const ops = [...s.ops.values()];
    s.ops.clear();
    const controller = new AbortController();
    const inFlight: InFlightSave = {
      controller,
      completion: Promise.resolve(),
    };
    inFlight.completion = this.#upsert(noteId, ops, controller.signal)
      .then(() => undefined)
      .catch(async () => {
        // Any rejected admitted request is ambiguous, including AbortError:
        // the server may already have committed it. PUT the whole body so the
        // latest local state still persists; the API barrier separately keeps
        // relocation fail-closed until authoritative reconciliation.
        try {
          await this.#fallback(noteId);
        } catch (fallbackError) {
          if (!s.failed) {
            s.failed = true;
            s.failure = fallbackError;
          }
          throw fallbackError;
        }
      })
      .finally(() => {
        if (s.inFlight !== inFlight) return;
        s.inFlight = null;
        // If a trailing edge or forced flush fired while this request was
        // live, its timer is already cleared and the queued successor can now
        // start. During `settle`, the owning loop starts it synchronously.
        if (s.settlers === 0 && s.timer === null && s.ops.size > 0) {
          this.#startFlush(noteId, s);
        }
        this.#releaseAdmissionIfQuiet(s);
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
          try {
            await s.inFlight.completion;
          } catch {
            // The failure is latched below; continue so a successor admitted
            // before the reservation can drain before preflight rejects.
          }
          continue;
        }
        const completion = this.#startFlush(noteId, s);
        if (completion) {
          try {
            await completion;
          } catch {
            // See the live-request branch above.
          }
          continue;
        }
        if (s.failed) throw s.failure;
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
      this.#releaseAdmissionIfQuiet(s);
    }
  }

  /** Flush every note's pending batch immediately (component teardown). */
  flushAll(): void {
    for (const noteId of [...this.#states.keys()]) this.flush(noteId);
  }

  /** Drain every active note through teardown. Successful queues release
   * their admission as they become quiet; failed queues deliberately retain
   * it so relocation cannot proceed after the component disappears. */
  dispose(): Promise<void> {
    if (this.#disposePromise) return this.#disposePromise;
    const entries = [...this.#states.entries()];
    this.#disposePromise = (async () => {
      const results = await Promise.allSettled(
        entries.map(([noteId]) => this.settle(noteId)),
      );
      const failure = results.find(
        (result): result is PromiseRejectedResult => result.status === "rejected",
      );
      if (failure) throw failure.reason;
    })();
    return this.#disposePromise;
  }

  /** Test hook: is a debounce timer currently armed for this note? */
  hasPending(noteId: string): boolean {
    const s = this.#states.get(noteId);
    return !!s && (s.timer !== null || s.ops.size > 0);
  }
}
