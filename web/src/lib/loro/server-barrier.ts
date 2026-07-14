export type LoroBarrierAcknowledgement = {
  event: "loro_barrier_ack";
  barrier_id: string;
  ok: boolean;
};

export interface ServerBarrierTrackerDeps<S> {
  createId(): string;
  isOpen(socket: S): boolean;
  sendText(socket: S, text: string): boolean;
  scheduleTimeout(cb: () => void): unknown;
  clearTimeout(handle: unknown): void;
}

export interface ServerBarrierTransaction {
  acknowledge(): boolean | void;
  reject(): void;
}

export interface ServerBarrierTransactionDeps {
  /** Synchronously hand the affected document deltas to the captured socket. */
  prepare(): void | ServerBarrierTransaction;
  /** The socket and generation captured by the caller are still current. */
  isConnectionCurrent(): boolean;
  /** Request and await the connection-local server acknowledgement. */
  request(): Promise<void>;
}

export interface ServerBarrierRetryQueueDeps<K> {
  run(keys: readonly K[]): Promise<void>;
  schedule(cb: () => void, delayMs: number): unknown;
  cancelSchedule(handle: unknown): void;
  initialDelayMs: number;
  maxDelayMs: number;
}

/** Coalesces documents awaiting durable proof and retries with bounded
 * exponential backoff. A generation per key prevents a successful in-flight
 * batch from erasing a newer retry request for the same document. */
export class ServerBarrierRetryQueue<K> {
  #deps: ServerBarrierRetryQueueDeps<K>;
  #pending = new Map<K, number>();
  #generation = 0;
  #handle: unknown | null = null;
  #running = false;
  #delayMs: number;

  constructor(deps: ServerBarrierRetryQueueDeps<K>) {
    this.#deps = deps;
    this.#delayMs = deps.initialDelayMs;
  }

  enqueue(keys: Iterable<K>): void {
    for (const key of keys) this.#pending.set(key, ++this.#generation);
    this.#scheduleIfNeeded();
  }

  /** A barrier outside this queue proved these keys while a retry was queued. */
  resolve(keys: Iterable<K>): void {
    for (const key of keys) this.#pending.delete(key);
    if (this.#pending.size === 0) {
      this.#delayMs = this.#deps.initialDelayMs;
      if (this.#handle !== null) {
        this.#deps.cancelSchedule(this.#handle);
        this.#handle = null;
      }
    }
  }

  #scheduleIfNeeded(): void {
    if (this.#pending.size === 0 || this.#running || this.#handle !== null) return;
    this.#handle = this.#deps.schedule(() => {
      this.#handle = null;
      void this.#run();
    }, this.#delayMs);
  }

  async #run(): Promise<void> {
    if (this.#running || this.#pending.size === 0) return;
    this.#running = true;
    const batch = [...this.#pending.entries()];
    let failed = false;
    try {
      for (const [key, generation] of batch) {
        try {
          await this.#deps.run([key]);
          if (this.#pending.get(key) === generation) this.#pending.delete(key);
        } catch {
          failed = true;
        }
      }
    } finally {
      if (failed && this.#pending.size > 0) {
        this.#delayMs = Math.min(
          this.#deps.maxDelayMs,
          Math.max(this.#deps.initialDelayMs, this.#delayMs * 2),
        );
      } else {
        this.#delayMs = this.#deps.initialDelayMs;
      }
      this.#running = false;
      this.#scheduleIfNeeded();
    }
  }
}

/** Coordinate the optimistic document handoff with its durable server ack.
 * Every failure after preparation rolls the document transaction back before
 * it escapes to the caller, so reconnect/release flushes can replay it. */
export async function runServerBarrierTransaction(
  deps: ServerBarrierTransactionDeps,
): Promise<void> {
  const transaction = deps.prepare();
  if (transaction && typeof (transaction as unknown as { then?: unknown }).then === "function") {
    throw new Error("Loro barrier flush callback must be synchronous");
  }
  if (!deps.isConnectionCurrent()) {
    transaction?.reject();
    throw new Error("WebSocket changed during Loro barrier preparation");
  }
  try {
    await deps.request();
  } catch (error) {
    transaction?.reject();
    throw error;
  }
  if (transaction?.acknowledge() === false) {
    throw new Error("Loro document changed while the server barrier was pending");
  }
}

interface PendingBarrier<S> {
  socket: S;
  generation: number;
  timer: unknown;
  resolve(): void;
  reject(error: Error): void;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function acknowledgement(value: unknown): LoroBarrierAcknowledgement | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  const record = value as Record<string, unknown>;
  const keys = Object.keys(record).sort();
  if (keys.join(",") !== "barrier_id,event,ok") return null;
  if (record.event !== "loro_barrier_ack") return null;
  if (typeof record.barrier_id !== "string" || !UUID.test(record.barrier_id)) return null;
  if (typeof record.ok !== "boolean") return null;
  return record as LoroBarrierAcknowledgement;
}

/** Connection-local tracker for server-applied Loro barriers. The caller
 * captures the WebSocket and its generation before requesting; only an exact
 * acknowledgement received from that same connection can settle the Promise. */
export class ServerBarrierTracker<S> {
  #deps: ServerBarrierTrackerDeps<S>;
  #pending = new Map<string, PendingBarrier<S>>();

  constructor(deps: ServerBarrierTrackerDeps<S>) {
    this.#deps = deps;
  }

  request(socket: S, generation: number): Promise<void> {
    if (!this.#deps.isOpen(socket)) {
      return Promise.reject(new Error("Loro barrier socket is not open"));
    }
    const barrierId = this.#deps.createId();
    if (!UUID.test(barrierId) || this.#pending.has(barrierId)) {
      return Promise.reject(new Error("Loro barrier id is invalid or already pending"));
    }

    return new Promise<void>((resolve, reject) => {
      const pending: PendingBarrier<S> = {
        socket,
        generation,
        timer: null,
        resolve,
        reject,
      };
      pending.timer = this.#deps.scheduleTimeout(() => {
        this.#fail(barrierId, new Error("Loro server barrier timed out"));
      });
      this.#pending.set(barrierId, pending);
      let sent = false;
      try {
        sent = this.#deps.sendText(
          socket,
          JSON.stringify({ event: "loro_barrier", barrier_id: barrierId }),
        );
      } catch {
        sent = false;
      }
      if (!sent) this.#fail(barrierId, new Error("Loro server barrier send was dropped"));
    });
  }

  handleAcknowledgement(socket: S, generation: number, value: unknown): boolean {
    const ack = acknowledgement(value);
    if (!ack) return false;
    const pending = this.#pending.get(ack.barrier_id);
    if (!pending || pending.socket !== socket || pending.generation !== generation) return false;
    this.#pending.delete(ack.barrier_id);
    this.#deps.clearTimeout(pending.timer);
    if (ack.ok) pending.resolve();
    else pending.reject(new Error("Loro server rejected the barrier"));
    return true;
  }

  rejectConnection(socket: S, generation: number, error: Error): void {
    for (const [barrierId, pending] of this.#pending) {
      if (pending.socket === socket && pending.generation === generation) {
        this.#fail(barrierId, error);
      }
    }
  }

  pendingCount(): number {
    return this.#pending.size;
  }

  #fail(barrierId: string, error: Error): void {
    const pending = this.#pending.get(barrierId);
    if (!pending) return;
    this.#pending.delete(barrierId);
    this.#deps.clearTimeout(pending.timer);
    pending.reject(error);
  }
}
