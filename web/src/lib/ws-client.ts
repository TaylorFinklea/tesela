/**
 * WebSocket client for tesela-server's /ws endpoint.
 *
 * Ported from app/Tesela/Tesela/Services/WebSocketClient.swift.
 * Preserves the original semantics:
 *   - Exponential backoff reconnect (1s → 30s, doubling)
 *   - `intentionallyStopped` flag that suppresses reconnect until connect() is called again
 *   - Reset backoff to 1s on every successful open attempt
 *   - Connection-id guard so a stale receive loop can't stomp on a fresh connection
 *
 * The server broadcasts WsEvent JSON messages (see crates/tesela-server/src/state.rs).
 * WsEvent is not exported via ts-rs because it lives in tesela-server, not tesela-core —
 * the shape below must stay in sync with WsEvent manually.
 */

import type { Note } from "@/lib/types/Note";

/** Mirror of crates/tesela-server/src/state.rs::WsEvent (tagged union via serde tag="event"). */
export type WsEvent =
  | { event: "note_created"; note: Note }
  | { event: "note_updated"; note: Note }
  | { event: "note_deleted"; id: string };

export interface WsClientOptions {
  url?: string;
  minRetryMs?: number;
  maxRetryMs?: number;
}

export interface WsHandlers {
  onNoteCreated?: (note: Note) => void;
  onNoteUpdated?: (note: Note) => void;
  onNoteDeleted?: (id: string) => void;
  onConnectionStateChanged?: (connected: boolean) => void;
}

export class WsClient {
  private readonly url: string;
  private readonly minRetryMs: number;
  private readonly maxRetryMs: number;

  private socket: WebSocket | null = null;
  private retryDelayMs: number;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionallyStopped = false;
  /** Incremented on every openConnection() attempt so stale handlers can bail. */
  private connectionId = 0;
  private handlers: WsHandlers = {};

  constructor(opts: WsClientOptions = {}) {
    this.url = opts.url ?? "ws://127.0.0.1:7474/ws";
    this.minRetryMs = opts.minRetryMs ?? 1_000;
    this.maxRetryMs = opts.maxRetryMs ?? 30_000;
    this.retryDelayMs = this.minRetryMs;
  }

  /** Register event handlers. Safe to call multiple times; later calls merge. */
  setHandlers(handlers: WsHandlers): void {
    this.handlers = { ...this.handlers, ...handlers };
  }

  get isConnected(): boolean {
    return this.socket?.readyState === WebSocket.OPEN;
  }

  /**
   * Begin connecting. Idempotent: does nothing if already connected.
   * Clears the intentionally-stopped latch so reconnect loops are permitted.
   */
  connect(): void {
    this.intentionallyStopped = false;
    if (this.socket && this.socket.readyState !== WebSocket.CLOSED) return;

    this.cancelReconnectTimer();
    this.retryDelayMs = this.minRetryMs;
    this.openConnection();
  }

  /**
   * Permanently disconnect. Suppresses all future automatic reconnects
   * until connect() is called again.
   */
  disconnect(): void {
    this.intentionallyStopped = true;
    this.cancelReconnectTimer();
    if (this.socket) {
      this.socket.close(1000, "client requested");
      this.socket = null;
    }
    this.handlers.onConnectionStateChanged?.(false);
  }

  private openConnection(): void {
    const myId = ++this.connectionId;
    let ws: WebSocket;
    try {
      ws = new WebSocket(this.url);
    } catch {
      // Construction itself failed (malformed URL, etc.). Fall back to backoff.
      this.onSocketClosed(myId);
      return;
    }
    this.socket = ws;

    ws.addEventListener("open", () => {
      if (myId !== this.connectionId) return;
      this.retryDelayMs = this.minRetryMs;
      this.handlers.onConnectionStateChanged?.(true);
    });

    ws.addEventListener("message", (ev) => {
      if (myId !== this.connectionId) return;
      this.handleMessage(ev.data);
    });

    ws.addEventListener("close", () => this.onSocketClosed(myId));
    ws.addEventListener("error", () => {
      // The close handler will fire right after; don't schedule twice.
    });
  }

  private onSocketClosed(myId: number): void {
    // Ignore stale closures from a connection that's already been replaced.
    if (myId !== this.connectionId) return;

    this.socket = null;
    this.handlers.onConnectionStateChanged?.(false);

    if (this.intentionallyStopped) return;
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    this.cancelReconnectTimer();
    const delay = this.retryDelayMs;
    this.retryDelayMs = Math.min(this.retryDelayMs * 2, this.maxRetryMs);
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      if (this.intentionallyStopped) return;
      this.openConnection();
    }, delay);
  }

  private cancelReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  private handleMessage(raw: unknown): void {
    if (typeof raw !== "string") return;
    let event: WsEvent;
    try {
      event = JSON.parse(raw) as WsEvent;
    } catch {
      return;
    }
    switch (event.event) {
      case "note_created":
        this.handlers.onNoteCreated?.(event.note);
        break;
      case "note_updated":
        this.handlers.onNoteUpdated?.(event.note);
        break;
      case "note_deleted":
        this.handlers.onNoteDeleted?.(event.id);
        break;
    }
  }
}

/** Default singleton for convenience. */
export const wsClient = new WsClient();
