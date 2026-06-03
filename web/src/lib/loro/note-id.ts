/**
 * Slug → 16-byte Loro doc id, matching the Rust server's
 * `stable_uuid_from_slug`.
 *
 * The server derives every per-note Loro doc id as the first 16 bytes of the
 * blake3 hash of the note's slug (the filename stem):
 *
 *   note_id = blake3(utf8(slug))[0..16]
 *
 * (See `crates/tesela-sync/src/engine/loro_engine.rs` — `blake3::hash(stem)`
 * truncated to `[u8; 16]`.) The web client must compute the SAME id so it can
 * filter inbound TLR2 deltas (`LoroDocUpdate.doc`) to the doc for the note it
 * currently has open.
 *
 * Pure JS via `@noble/hashes` (no DOM/wasm) so this is SSR-safe and usable from
 * plain node scripts (the C2.2 convergence check imports it directly).
 *
 * Confirmed against the live wire: `noteIdHex("2026-06-03")` ===
 * `ebbf433ae7ef5b88aef2944f5d8f6114`, the doc id carried on the daily's TLR2
 * delta frame.
 */
import { blake3 } from "@noble/hashes/blake3.js";
import { bytesToHex, utf8ToBytes } from "@noble/hashes/utils.js";

/** Raw 16-byte note id for a slug: `blake3(utf8(slug))[0..16]`. */
export function noteId(slug: string): Uint8Array {
  return blake3(utf8ToBytes(slug)).slice(0, 16);
}

/** Lowercase dashless hex of {@link noteId} — the form TLR2 `doc` ids and the
 *  `/loro/notes/{slug}/snapshot` route key compare against. */
export function noteIdHex(slug: string): string {
  return bytesToHex(noteId(slug));
}

/** Lowercase hex of a raw 16-byte id (e.g. a decoded TLR2 `doc`), for
 *  comparing against {@link noteIdHex}. */
export function bytesToHex16(bytes: Uint8Array): string {
  return bytesToHex(bytes);
}
