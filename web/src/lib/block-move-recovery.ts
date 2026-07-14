import { isBlockMoveRequest, type BlockMoveRequest } from "./block-tree-move.ts";
import type { PerNoteMutationReservation } from "./block-ops-saver.ts";

const STORAGE_KEY = "tesela:block-move-recovery:v1";

export type BlockMoveRecoveryStatus = "submitting" | "retryable";

export type BlockMoveRecoveryState = {
  request: BlockMoveRequest;
  status: BlockMoveRecoveryStatus;
  message: string | null;
  blockingMoveId: string | null;
};

export interface BlockMoveRecoveryStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export interface BlockMoveReservationBarrier {
  reserve(noteIds: Iterable<string>): PerNoteMutationReservation;
}

type RecoveryListener = (state: BlockMoveRecoveryState | null) => void;

function copyRequest(request: BlockMoveRequest): BlockMoveRequest {
  return {
    move_id: request.move_id,
    source_note_id: request.source_note_id,
    root_bid: request.root_bid,
    destination_note_id: request.destination_note_id,
    target_bid: request.target_bid,
    placement: request.placement,
  };
}

function copyState(state: BlockMoveRecoveryState | null): BlockMoveRecoveryState | null {
  return state ? { ...state, request: copyRequest(state.request) } : null;
}

export class BlockMoveRecoveryOwner {
  #barrier: BlockMoveReservationBarrier;
  #storage: BlockMoveRecoveryStorage | null;
  #scope: string;
  #reservation: PerNoteMutationReservation | null = null;
  #state: BlockMoveRecoveryState | null = null;
  #listeners = new Set<RecoveryListener>();

  constructor(
    barrier: BlockMoveReservationBarrier,
    storage: BlockMoveRecoveryStorage | null,
    scope: string,
  ) {
    this.#barrier = barrier;
    this.#storage = storage;
    this.#scope = scope;
    this.#restore();
  }

  current(): BlockMoveRecoveryState | null {
    return copyState(this.#state);
  }

  subscribe(listener: RecoveryListener): () => void {
    this.#listeners.add(listener);
    listener(this.current());
    return () => this.#listeners.delete(listener);
  }

  adopt(request: BlockMoveRequest, reservation: PerNoteMutationReservation): void {
    if (this.#state || this.#reservation) {
      throw new Error("Another submitted block move already owns recovery");
    }
    if (!isBlockMoveRequest(request)) throw new Error("Invalid block move recovery request");
    if (!this.#storage) throw new Error("Block move recovery storage is unavailable");
    const exactRequest = copyRequest(request);
    this.#storage.setItem(STORAGE_KEY, JSON.stringify({
      version: 1,
      scope: this.#scope,
      request: exactRequest,
    }));
    this.#reservation = reservation;
    this.#state = {
      request: exactRequest,
      status: "submitting",
      message: null,
      blockingMoveId: null,
    };
    this.#notify();
  }

  markSubmitting(moveId: string): boolean {
    if (!this.#owns(moveId) || this.#state?.status !== "retryable") return false;
    this.#state = {
      ...this.#state!,
      status: "submitting",
      message: null,
      blockingMoveId: null,
    };
    this.#notify();
    return true;
  }

  markRetryable(moveId: string, message: string, blockingMoveId: string | null): boolean {
    if (!this.#owns(moveId) || this.#state?.status !== "submitting") return false;
    this.#state = {
      ...this.#state!,
      status: "retryable",
      message,
      blockingMoveId,
    };
    this.#notify();
    return true;
  }

  complete(moveId: string): boolean {
    if (!this.#owns(moveId)) return false;
    try {
      this.#storage?.removeItem(STORAGE_KEY);
      if (this.#storage?.getItem(STORAGE_KEY) !== null) {
        throw new Error("Block move recovery marker still exists after removal");
      }
    } catch {
      this.#state = {
        ...this.#state!,
        status: "retryable",
        message: "The move finished, but its recovery marker could not be cleared",
        blockingMoveId: null,
      };
      this.#notify();
      return false;
    }
    const reservation = this.#reservation;
    this.#reservation = null;
    this.#state = null;
    reservation?.release();
    this.#notify();
    return true;
  }

  #owns(moveId: string): boolean {
    return this.#state?.request.move_id === moveId && this.#reservation !== null;
  }

  #restore(): void {
    if (!this.#storage) return;
    let parsed: unknown;
    try {
      const raw = this.#storage.getItem(STORAGE_KEY);
      if (!raw) return;
      parsed = JSON.parse(raw);
    } catch {
      this.#discardStoredRequest();
      return;
    }
    const record = parsed as { version?: unknown; scope?: unknown; request?: unknown };
    const recordKeys = parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? Object.keys(parsed).sort()
      : [];
    const requestKeys = record.request && typeof record.request === "object"
      && !Array.isArray(record.request)
      ? Object.keys(record.request).sort()
      : [];
    if (
      recordKeys.join(",") !== "request,scope,version"
      || requestKeys.join(",")
        !== "destination_note_id,move_id,placement,root_bid,source_note_id,target_bid"
      || record.version !== 1
      || record.scope !== this.#scope
      || !isBlockMoveRequest(record.request)
    ) {
      this.#discardStoredRequest();
      return;
    }
    try {
      this.#reservation = this.#barrier.reserve([
        ...new Set([
          record.request.source_note_id,
          record.request.destination_note_id,
        ]),
      ]);
    } catch {
      return;
    }
    this.#state = {
      request: copyRequest(record.request),
      status: "retryable",
      message: "A submitted block move needs an exact-request retry",
      blockingMoveId: null,
    };
  }

  #discardStoredRequest(): void {
    try {
      this.#storage?.removeItem(STORAGE_KEY);
    } catch {
      // A broken storage implementation is equivalent to unavailable recovery.
    }
  }

  #notify(): void {
    for (const listener of this.#listeners) listener(this.current());
  }
}
