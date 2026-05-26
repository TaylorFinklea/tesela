/**
 * WebSocket client for tesela-server /ws endpoint.
 * Uses Svelte 5 runes for reactive connection state.
 *
 * Ported from the React version (originally from Swift WebSocketClient).
 */
import type { Note } from "$lib/types/Note";

export type DeadlineApproachingEvent = {
  event: "deadline_approaching";
  block_id: string;
  title: string;
  note_id: string;
  deadline_iso: string;
  lead_minutes: number;
};
export type ScheduledFiresEvent = {
  event: "scheduled_fires";
  block_id: string;
  title: string;
  note_id: string;
  scheduled_iso: string;
};
export type RecurringRolledEvent = {
  event: "recurring_rolled";
  block_id: string;
  title: string;
  note_id: string;
  next_deadline: string;
};

type WsEvent =
  | { event: "note_created"; note: Note }
  | { event: "note_updated"; note: Note }
  | { event: "note_deleted"; id: string }
  | DeadlineApproachingEvent
  | ScheduledFiresEvent
  | RecurringRolledEvent;

// Same-origin path; vite dev server proxies `/ws` → tesela-server's WS at
// 127.0.0.1:7474. Computed at runtime so LAN clients (phones, etc.) connect
// to whatever host they loaded the page from.
function wsUrl(): string {
  if (typeof window === "undefined") return "ws://127.0.0.1:7474/ws";
  const proto = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${window.location.host}/ws`;
}
const MIN_RETRY_MS = 1_000;
const MAX_RETRY_MS = 30_000;

let socket: WebSocket | null = null;
let retryDelayMs = MIN_RETRY_MS;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let intentionallyStopped = false;
let connectionId = 0;

// Reactive state via Svelte 5 runes
let connected = $state(false);

export function getConnected() {
  return connected;
}

// Handlers
let onNoteCreated: ((note: Note) => void) | null = null;
let onNoteUpdated: ((note: Note) => void) | null = null;
let onNoteDeleted: ((id: string) => void) | null = null;
let onDeadlineApproaching: ((e: DeadlineApproachingEvent) => void) | null = null;
let onScheduledFires: ((e: ScheduledFiresEvent) => void) | null = null;
let onRecurringRolled: ((e: RecurringRolledEvent) => void) | null = null;
/// Fires after a reconnect (NOT after the first connect). Hosts use
/// this to invalidate any cached query state that might have missed
/// events while the socket was down. Without this, an iOS push that
/// arrives at the server during the reconnect-backoff window stays
/// invisible until the user manually refreshes the page.
let onReconnected: (() => void) | null = null;
/// Set true once the first connect succeeds. After that, every
/// subsequent open is a "reconnect" and fires `onReconnected`.
let hasEverConnected = false;

export function setHandlers(handlers: {
  onNoteCreated?: (note: Note) => void;
  onNoteUpdated?: (note: Note) => void;
  onNoteDeleted?: (id: string) => void;
  onDeadlineApproaching?: (e: DeadlineApproachingEvent) => void;
  onScheduledFires?: (e: ScheduledFiresEvent) => void;
  onRecurringRolled?: (e: RecurringRolledEvent) => void;
  onReconnected?: () => void;
}) {
  onNoteCreated = handlers.onNoteCreated ?? null;
  onNoteUpdated = handlers.onNoteUpdated ?? null;
  onNoteDeleted = handlers.onNoteDeleted ?? null;
  onDeadlineApproaching = handlers.onDeadlineApproaching ?? null;
  onScheduledFires = handlers.onScheduledFires ?? null;
  onRecurringRolled = handlers.onRecurringRolled ?? null;
  onReconnected = handlers.onReconnected ?? null;
}

export function connect() {
  intentionallyStopped = false;
  if (socket && socket.readyState !== WebSocket.CLOSED) return;
  cancelReconnectTimer();
  retryDelayMs = MIN_RETRY_MS;
  openConnection();
}

export function disconnect() {
  intentionallyStopped = true;
  cancelReconnectTimer();
  if (socket) {
    socket.close(1000, "client requested");
    socket = null;
  }
  connected = false;
}

function openConnection() {
  const myId = ++connectionId;
  let ws: WebSocket;
  try {
    ws = new WebSocket(wsUrl());
  } catch {
    onSocketClosed(myId);
    return;
  }
  socket = ws;

  ws.addEventListener("open", () => {
    if (myId !== connectionId) return;
    retryDelayMs = MIN_RETRY_MS;
    connected = true;
    if (hasEverConnected) {
      // Reconnect after a drop — events that arrived at the server
      // during the gap were missed. Tell the host to invalidate so
      // the freshly-reopened socket starts seeing a re-fetched UI.
      onReconnected?.();
    } else {
      hasEverConnected = true;
    }
  });

  ws.addEventListener("message", (ev) => {
    if (myId !== connectionId) return;
    handleMessage(ev.data);
  });

  ws.addEventListener("close", () => onSocketClosed(myId));
  ws.addEventListener("error", () => {});
}

function onSocketClosed(myId: number) {
  if (myId !== connectionId) return;
  socket = null;
  connected = false;
  if (intentionallyStopped) return;
  scheduleReconnect();
}

function scheduleReconnect() {
  cancelReconnectTimer();
  const delay = retryDelayMs;
  retryDelayMs = Math.min(retryDelayMs * 2, MAX_RETRY_MS);
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    if (intentionallyStopped) return;
    openConnection();
  }, delay);
}

function cancelReconnectTimer() {
  if (reconnectTimer !== null) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
}

function handleMessage(raw: unknown) {
  if (typeof raw !== "string") return;
  let event: WsEvent;
  try {
    event = JSON.parse(raw) as WsEvent;
  } catch {
    return;
  }
  switch (event.event) {
    case "note_created":
      onNoteCreated?.(event.note);
      break;
    case "note_updated":
      onNoteUpdated?.(event.note);
      break;
    case "note_deleted":
      onNoteDeleted?.(event.id);
      break;
    case "deadline_approaching":
      onDeadlineApproaching?.(event);
      break;
    case "scheduled_fires":
      onScheduledFires?.(event);
      break;
    case "recurring_rolled":
      onRecurringRolled?.(event);
      break;
  }
}
