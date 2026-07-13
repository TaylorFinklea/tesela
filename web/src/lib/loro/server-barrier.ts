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
