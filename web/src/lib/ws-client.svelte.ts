/**
 * WebSocket client for tesela-server /ws endpoint.
 * Uses Svelte 5 runes for reactive connection state.
 *
 * Ported from the React version (originally from Swift WebSocketClient).
 */
import type { Note } from "$lib/types/Note";
import type { ViewRecord } from "$lib/api-client";
import { decodeTlr2, type LoroDocUpdate } from "$lib/loro/tlr2";
import { decodePresence, type PresenceFrame } from "$lib/loro/presence";

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
/** Saved-views registry changed (any `/views` write — create / update /
 *  delete / reorder). Carries the FULL ordered registry, mirroring how
 *  `note_updated` carries the whole note, so the client refreshes the view
 *  switcher without a refetch. */
export type ViewsChangedEvent = {
  event: "views_changed";
  views: ViewRecord[];
};

type WsEvent =
  | { event: "note_created"; note: Note }
  | { event: "note_updated"; note: Note }
  | { event: "note_deleted"; id: string }
  | DeadlineApproachingEvent
  | ScheduledFiresEvent
  | RecurringRolledEvent
  | ViewsChangedEvent;

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
let onViewsChanged: ((views: ViewRecord[]) => void) | null = null;
/// Fires after a reconnect (NOT after the first connect). Hosts use
/// this to invalidate any cached query state that might have missed
/// events while the socket was down. Without this, an iOS push that
/// arrives at the server during the reconnect-backoff window stays
/// invisible until the user manually refreshes the page.
let onReconnected: (() => void) | null = null;
/// Fires when an inbound BINARY WS frame decodes as a TLR2 Loro-delta
/// payload (`crates/tesela-sync` protocol v2). C2.1 infrastructure: the
/// server already broadcasts these on every edit; web now decodes them.
/// The decoded per-doc updates are NOT applied to any Loro doc yet — that
/// wiring lands in C2.2/C2.3. Foreign/short binary frames (non-TLR2) are
/// dropped silently and never reach this handler.
let onBinaryDelta: ((updates: LoroDocUpdate[]) => void) | null = null;
/// Fires when an inbound BINARY WS frame decodes as an EPHEMERAL presence
/// frame (`PRES` magic) — a peer's live caret. NOT a document delta: it never
/// touches a Loro doc; the host feeds it to the remote-cursor store. Checked
/// BEFORE the TLR2 delta path in `handleBinaryFrame`.
let onPresence: ((frame: PresenceFrame) => void) | null = null;
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
  onViewsChanged?: (views: ViewRecord[]) => void;
  onReconnected?: () => void;
  onBinaryDelta?: (updates: LoroDocUpdate[]) => void;
  onPresence?: (frame: PresenceFrame) => void;
}) {
  onNoteCreated = handlers.onNoteCreated ?? null;
  onNoteUpdated = handlers.onNoteUpdated ?? null;
  onNoteDeleted = handlers.onNoteDeleted ?? null;
  onDeadlineApproaching = handlers.onDeadlineApproaching ?? null;
  onScheduledFires = handlers.onScheduledFires ?? null;
  onRecurringRolled = handlers.onRecurringRolled ?? null;
  onViewsChanged = handlers.onViewsChanged ?? null;
  onReconnected = handlers.onReconnected ?? null;
  onBinaryDelta = handlers.onBinaryDelta ?? null;
  onPresence = handlers.onPresence ?? null;
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
  // Receive binary TLR2 frames as ArrayBuffer (not Blob) so `handleMessage`
  // can decode them synchronously without an async Blob read.
  ws.binaryType = "arraybuffer";
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
  // Binary frames are TLR2 Loro-delta payloads (protocol v2). The server may
  // deliver them as an ArrayBuffer (we set `binaryType = "arraybuffer"`) or,
  // defensively, as a Blob. ArrayBuffer is handled synchronously here; a Blob
  // is read async then re-dispatched. Anything else falls through to the
  // text-JSON path below, preserving all existing event handling exactly.
  if (raw instanceof ArrayBuffer) {
    handleBinaryFrame(new Uint8Array(raw));
    return;
  }
  if (typeof Blob !== "undefined" && raw instanceof Blob) {
    void raw
      .arrayBuffer()
      .then((buf) => handleBinaryFrame(new Uint8Array(buf)))
      .catch(() => {});
    return;
  }
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
    case "views_changed":
      onViewsChanged?.(event.views);
      break;
  }
}

/// Decode an inbound binary WS frame as TLR2 and, if it's a valid v2 payload,
/// hand the per-doc updates to `onBinaryDelta`. Non-TLR2 / short frames decode
/// to `null` and are dropped. A malformed (truncated DEFLATE / postcard) v2
/// frame throws inside `decodeTlr2`; we swallow it so one bad frame can't tear
/// down the socket. The updates are NOT applied anywhere yet (C2.1 infra only).
function handleBinaryFrame(bytes: Uint8Array) {
  // Ephemeral presence (PRES) is checked FIRST — it's a transient caret, not a
  // document delta, and must never reach the Loro-apply path. A non-PRES frame
  // decodes to null and falls through to the TLR2 delta path unchanged.
  const presence = decodePresence(bytes);
  if (presence) {
    onPresence?.(presence);
    return;
  }
  let updates: LoroDocUpdate[] | null;
  try {
    updates = decodeTlr2(bytes);
  } catch {
    return;
  }
  if (updates === null) return;
  onBinaryDelta?.(updates);
}

/// Send a pre-framed binary payload (e.g. a TLR2 Loro-delta frame) over the
/// socket when it's OPEN. Returns false — WITHOUT sending — when the socket
/// isn't open, so callers that must not lose the payload (the doc registry's
/// outbound cursor only advances on a real handoff) can retry after reconnect.
export function sendBinary(frame: Uint8Array): boolean {
  if (socket && socket.readyState === WebSocket.OPEN) {
    // Send the exact frame bytes as a fresh ArrayBuffer. `slice` copies just
    // the view's range (correct even if `frame` is a subarray) and yields an
    // ArrayBuffer-backed buffer, which `WebSocket.send` accepts unambiguously
    // (a generic `Uint8Array<ArrayBufferLike>` is not assignable directly).
    socket.send(frame.slice().buffer);
    return true;
  }
  return false;
}
