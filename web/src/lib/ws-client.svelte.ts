/**
 * WebSocket client for tesela-server /ws endpoint.
 * Uses Svelte 5 runes for reactive connection state.
 *
 * Ported from the React version (originally from Swift WebSocketClient).
 */
import type { Note } from "$lib/types/Note";

type WsEvent =
  | { event: "note_created"; note: Note }
  | { event: "note_updated"; note: Note }
  | { event: "note_deleted"; id: string };

const WS_URL = "ws://127.0.0.1:7474/ws";
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

export function setHandlers(handlers: {
  onNoteCreated?: (note: Note) => void;
  onNoteUpdated?: (note: Note) => void;
  onNoteDeleted?: (id: string) => void;
}) {
  onNoteCreated = handlers.onNoteCreated ?? null;
  onNoteUpdated = handlers.onNoteUpdated ?? null;
  onNoteDeleted = handlers.onNoteDeleted ?? null;
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
    ws = new WebSocket(WS_URL);
  } catch {
    onSocketClosed(myId);
    return;
  }
  socket = ws;

  ws.addEventListener("open", () => {
    if (myId !== connectionId) return;
    retryDelayMs = MIN_RETRY_MS;
    connected = true;
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
  }
}
