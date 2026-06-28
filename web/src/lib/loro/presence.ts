/**
 * PRES — the ephemeral presence frame codec (Phase 2 desktop multi-device).
 *
 * A frame is: b"PRES" (4 bytes) ++ utf8( JSON(PresenceFrame) ).
 *
 * Distinct from the {@link ../loro/tlr2 TLR2} Loro-delta magic so both the
 * server (`route_inbound_binary` in `crates/tesela-server/src/routes/ws.rs`)
 * and the web (`ws-client`) can tell a transient cursor update apart from a
 * persisted document delta and route it to the remote-cursor store WITHOUT
 * touching the CRDT. Presence rides the same WS binary fan-out as deltas.
 *
 * JSON (not postcard) because the body is tiny, human-debuggable, and the
 * server forwards it opaquely — only the 4-byte magic matters to the relay.
 */

/** Magic prefix for a presence frame: b"PRES". Keep in sync with the Rust
 * `WS_PRESENCE_MAGIC`. */
const PRES_MAGIC = new Uint8Array([0x50, 0x52, 0x45, 0x53]);

/** One peer's live caret/selection in a note. Plain `{bid, offset}` (NOT a loro
 * Cursor) — CodeMirror's decoration mapping auto-shifts a remote caret through
 * local edits, and peers re-publish on every move, so an op-anchored cursor
 * isn't needed on the web (it is on iOS, where UITextView doesn't auto-remap). */
export type PresenceFrame = {
  /** Stable per-tab/session peer id (the sender). */
  peer: string;
  /** Display color, `#RRGGBB`. */
  color: string;
  /** Optional short label for the peer. */
  name?: string;
  /** Slug of the note the caret is in. */
  slug: string;
  /** Block id (the `<!-- bid:… -->` uuid) the caret is in. */
  bid: string;
  /** utf16 caret offset within that block's text. */
  offset: number;
};

function hasMagic(frame: Uint8Array): boolean {
  if (frame.length < PRES_MAGIC.length) return false;
  for (let i = 0; i < PRES_MAGIC.length; i++) {
    if (frame[i] !== PRES_MAGIC[i]) return false;
  }
  return true;
}

/** `true` if `frame` carries the PRES magic (a presence frame, not a delta). */
export function isPresenceFrame(frame: Uint8Array): boolean {
  return hasMagic(frame);
}

/** Encode a presence frame: `PRES` ++ utf8(JSON). */
export function encodePresence(f: PresenceFrame): Uint8Array {
  const json = new TextEncoder().encode(JSON.stringify(f));
  const out = new Uint8Array(PRES_MAGIC.length + json.length);
  out.set(PRES_MAGIC, 0);
  out.set(json, PRES_MAGIC.length);
  return out;
}

/**
 * Decode a presence frame. Returns `null` for a non-PRES frame (e.g. a TLR2
 * delta), malformed JSON, or a payload missing the required fields — so the
 * caller can fall through to the delta path safely.
 */
export function decodePresence(frame: Uint8Array): PresenceFrame | null {
  if (!hasMagic(frame)) return null;
  try {
    const json = new TextDecoder("utf-8", { fatal: true }).decode(
      frame.subarray(PRES_MAGIC.length),
    );
    const f = JSON.parse(json) as Partial<PresenceFrame>;
    if (
      typeof f.peer !== "string" ||
      typeof f.color !== "string" ||
      typeof f.slug !== "string" ||
      typeof f.bid !== "string" ||
      typeof f.offset !== "number"
    ) {
      return null;
    }
    return f as PresenceFrame;
  } catch {
    return null;
  }
}
