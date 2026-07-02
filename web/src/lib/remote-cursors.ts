/**
 * Remote-cursors presence store (Phase 2 desktop multi-device).
 *
 * Holds the latest caret of each OTHER peer, keyed by peer id, with a
 * timeout so a peer that goes quiet (or disconnects) fades. The WS client
 * feeds it decoded {@link PresenceFrame}s; each block's CodeMirror editor
 * subscribes and re-renders the carets that fall in ITS block.
 *
 * Plain module state + an explicit subscriber set (not Svelte runes) because
 * the consumer is CodeMirror, which bridges via dispatch, not reactivity.
 */
import type { PresenceFrame } from "./loro/presence";
import { apiBase } from "./runtime-base.ts";

export type RemoteCursor = PresenceFrame & {
  /** Wall-clock ms of the last update — drives staleness. */
  ts: number;
};

/** A peer that hasn't refreshed within this window is treated as gone. */
const STALE_MS = 10_000;

const PALETTE = [
  "#ef4444", "#f97316", "#eab308", "#22c55e",
  "#06b6d4", "#3b82f6", "#a855f7", "#ec4899",
];

function hashStr(s: string): number {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

let _peerId: string | null = null;

/** This tab/session's stable peer id (generated once). */
export function localPeerId(): string {
  if (_peerId) return _peerId;
  _peerId =
    typeof crypto !== "undefined" && crypto.randomUUID
      ? crypto.randomUUID()
      : `peer-${Math.floor(Math.random() * 1e9)}`;
  return _peerId;
}

/** Deterministic palette color for a peer id. */
export function colorForPeer(peer: string): string {
  return PALETTE[hashStr(peer) % PALETTE.length];
}

/** This peer's color. */
export function localColor(): string {
  return colorForPeer(localPeerId());
}

let _deviceName: string | undefined = undefined;
let _deviceNameFetchStarted = false;

/**
 * This device's human label (server's `device_display_name()`), used to flag
 * our caret on peers. Fetched once, same-origin, from `GET /info`; failures are
 * tolerated (stays `undefined`). The fetch is async, so early calls return
 * `undefined` — presence re-publishes on every caret move, so the name lands on
 * later frames once it resolves.
 */
export function localName(): string | undefined {
  if (!_deviceNameFetchStarted) {
    _deviceNameFetchStarted = true;
    void (async () => {
      try {
        const res = await fetch(`${apiBase()}/info`, {
          headers: { Accept: "application/json" },
        });
        if (!res.ok) return;
        const body = (await res.json()) as { device_name?: unknown };
        if (typeof body.device_name === "string" && body.device_name.trim()) {
          _deviceName = body.device_name.trim();
        }
      } catch {
        // Offline / desktop-embed race / no server — stay undefined.
      }
    })();
  }
  return _deviceName;
}

const cursors = new Map<string, RemoteCursor>();
const listeners = new Set<() => void>();
let pruneTimer: ReturnType<typeof setInterval> | null = null;

function notify(): void {
  for (const cb of listeners) cb();
}

function ensurePruneTimer(): void {
  if (pruneTimer || typeof setInterval === "undefined") return;
  pruneTimer = setInterval(() => {
    if (pruneStale()) notify();
  }, 3000);
  // Don't keep the process alive for this timer (Node/tests).
  (pruneTimer as { unref?: () => void })?.unref?.();
}

/** Subscribe to any change (apply / prune). Returns an unsubscribe fn. */
export function subscribeRemoteCursors(cb: () => void): () => void {
  listeners.add(cb);
  ensurePruneTimer();
  return () => {
    listeners.delete(cb);
  };
}

/** Merge a peer's presence frame. Our OWN peer's frames are ignored (a peer
 * has exactly one cursor — moving blocks relocates it). */
export function applyPresenceFrame(f: PresenceFrame, now: number = Date.now()): void {
  if (f.peer === localPeerId()) return;
  cursors.set(f.peer, { ...f, ts: now });
  notify();
}

/** The live (non-stale) remote cursors that fall in a given note + block. */
export function remoteCursorsForBlock(
  slug: string,
  bid: string,
  now: number = Date.now(),
): RemoteCursor[] {
  const out: RemoteCursor[] = [];
  for (const c of cursors.values()) {
    if (now - c.ts > STALE_MS) continue;
    if (c.slug === slug && c.bid === bid) out.push(c);
  }
  return out;
}

/** Drop cursors past the staleness window. Returns whether anything changed. */
export function pruneStale(now: number = Date.now()): boolean {
  let changed = false;
  for (const [peer, c] of cursors) {
    if (now - c.ts > STALE_MS) {
      cursors.delete(peer);
      changed = true;
    }
  }
  return changed;
}

/** Test seam: reset all module state. */
export function _resetForTest(): void {
  cursors.clear();
  listeners.clear();
  if (pruneTimer) {
    clearInterval(pruneTimer);
    pruneTimer = null;
  }
  _peerId = null;
  _deviceName = undefined;
  _deviceNameFetchStarted = false;
}
