/**
 * TLR2 — the Tesela Loro relay binary frame codec (protocol v2), in pure TS.
 *
 * Byte-for-byte mirror of the Rust `crates/tesela-sync/src/wire/mod.rs`
 * `encode_loro_relay_payload` / `decode_loro_relay_payload`. A frame is:
 *
 *   b"TLR2" (4 bytes) ++ rawDeflate( postcard( Vec<LoroDocUpdate> ) )
 *
 * where `struct LoroDocUpdate { doc: [u8;16], update_bytes: Vec<u8> }`.
 *
 * Compression is RAW DEFLATE (no zlib/gzip header) — Rust uses
 * `miniz_oxide::deflate::compress_to_vec(body, 6)` / `inflate::decompress_to_vec`,
 * the header-less variants. fflate's `deflateSync`/`inflateSync` are the raw
 * equivalents (`zlibSync`/`gzipSync` would add headers and MUST NOT be used).
 *
 * The postcard layout has no field tags; fields are emitted in declared order:
 *   - `Vec<LoroDocUpdate>` = varint(len) then len × LoroDocUpdate.
 *   - `LoroDocUpdate` = `doc` (exactly 16 raw bytes — a [u8;16] fixed array is
 *     just its 16 elements, no length prefix) then `update_bytes` =
 *     varint(len) then len raw bytes.
 *   - varint = postcard unsigned LEB128: little-endian 7-bit groups, high bit
 *     (0x80) means "more bytes follow".
 *
 * Pure TS, no DOM/wasm. The only dependency is `fflate` for DEFLATE.
 */
import { deflateSync, inflateSync } from "fflate";

/** Magic + version prefix for the Loro relay payload (protocol v2): b"TLR2". */
const TLR2_MAGIC = new Uint8Array([0x54, 0x4c, 0x52, 0x32]);

/** One doc's Loro update bytes — the decoded unit of a TLR2 frame. */
export type LoroDocUpdate = {
  /** 16-byte note id the update belongs to. */
  doc: Uint8Array;
  /** Loro update bytes (a delta export, or full state for bootstrap). */
  updateBytes: Uint8Array;
};

/**
 * Append-only writer over a growable `Uint8Array`, with the two postcard
 * primitives this codec needs: unsigned LEB128 varints and raw byte runs.
 */
class ByteWriter {
  private buf = new Uint8Array(64);
  private len = 0;

  private ensure(extra: number): void {
    const needed = this.len + extra;
    if (needed <= this.buf.length) return;
    let cap = this.buf.length * 2;
    while (cap < needed) cap *= 2;
    const next = new Uint8Array(cap);
    next.set(this.buf.subarray(0, this.len));
    this.buf = next;
  }

  /** Write a single byte. */
  pushByte(b: number): void {
    this.ensure(1);
    this.buf[this.len++] = b & 0xff;
  }

  /** Write raw bytes verbatim (no length prefix). */
  pushBytes(bytes: Uint8Array): void {
    this.ensure(bytes.length);
    this.buf.set(bytes, this.len);
    this.len += bytes.length;
  }

  /**
   * Write a postcard unsigned varint (LEB128): 7 bits per byte, little-endian,
   * high bit set on every byte except the last. JS numbers are exact integers
   * up to 2^53, far beyond any wire length here, so a plain `>>> 7` loop is
   * safe (always non-negative, < 2^53).
   */
  pushVarint(value: number): void {
    let v = value >>> 0 === value ? value : Math.floor(value);
    while (v >= 0x80) {
      this.pushByte((v & 0x7f) | 0x80);
      v = Math.floor(v / 128);
    }
    this.pushByte(v & 0x7f);
  }

  /** Return the written bytes as a tightly-sized view. */
  toUint8Array(): Uint8Array {
    return this.buf.slice(0, this.len);
  }
}

/** Sequential reader over a `Uint8Array` for the postcard decode path. */
class ByteReader {
  private readonly buf: Uint8Array;
  private pos = 0;

  constructor(buf: Uint8Array) {
    this.buf = buf;
  }

  get remaining(): number {
    return this.buf.length - this.pos;
  }

  /** Read a postcard unsigned varint (LEB128). Throws on truncation/overflow. */
  readVarint(): number {
    let result = 0;
    let shift = 1; // multiplier for the current 7-bit group (128**groupIndex)
    for (let i = 0; i < 10; i++) {
      if (this.pos >= this.buf.length) {
        throw new Error("tlr2: varint truncated");
      }
      const byte = this.buf[this.pos++];
      result += (byte & 0x7f) * shift;
      if ((byte & 0x80) === 0) {
        if (!Number.isSafeInteger(result)) {
          throw new Error("tlr2: varint exceeds safe integer range");
        }
        return result;
      }
      shift *= 128;
    }
    throw new Error("tlr2: varint too long");
  }

  /** Read exactly `n` raw bytes (a copy). Throws if fewer remain. */
  readBytes(n: number): Uint8Array {
    if (n < 0 || this.pos + n > this.buf.length) {
      throw new Error("tlr2: byte run out of bounds");
    }
    const out = this.buf.slice(this.pos, this.pos + n);
    this.pos += n;
    return out;
  }
}

/** Postcard-encode `Vec<LoroDocUpdate>` (no DEFLATE, no magic). */
function postcardEncode(updates: LoroDocUpdate[]): Uint8Array {
  const w = new ByteWriter();
  w.pushVarint(updates.length);
  for (const u of updates) {
    if (u.doc.length !== 16) {
      throw new Error(
        `tlr2: doc id must be exactly 16 bytes, got ${u.doc.length}`,
      );
    }
    w.pushBytes(u.doc); // [u8;16] — 16 raw bytes, no length prefix.
    w.pushVarint(u.updateBytes.length);
    w.pushBytes(u.updateBytes);
  }
  return w.toUint8Array();
}

/** Postcard-decode `Vec<LoroDocUpdate>` from a fully-inflated body. */
function postcardDecode(body: Uint8Array): LoroDocUpdate[] {
  const r = new ByteReader(body);
  const count = r.readVarint();
  const updates: LoroDocUpdate[] = [];
  for (let i = 0; i < count; i++) {
    const doc = r.readBytes(16); // [u8;16]
    const updateLen = r.readVarint();
    const updateBytes = r.readBytes(updateLen);
    updates.push({ doc, updateBytes });
  }
  return updates;
}

/**
 * Encode a batch of per-doc Loro updates as a TLR2 frame:
 * `TLR2` ++ rawDeflate(postcard(updates)). Mirrors the Rust
 * `encode_loro_relay_payload`.
 */
export function encodeTlr2(updates: LoroDocUpdate[]): Uint8Array {
  const body = postcardEncode(updates);
  // fflate `deflateSync` = raw DEFLATE (no zlib/gzip header), matching
  // `miniz_oxide::deflate::compress_to_vec`. Level 6 matches the Rust default.
  const compressed = deflateSync(body, { level: 6 });
  const out = new Uint8Array(TLR2_MAGIC.length + compressed.length);
  out.set(TLR2_MAGIC, 0);
  out.set(compressed, TLR2_MAGIC.length);
  return out;
}

/**
 * Decode a TLR2 frame produced by {@link encodeTlr2} (or the Rust server).
 * Returns `null` when the bytes are shorter than the 4-byte magic or don't
 * start with `TLR2` (a v1/foreign frame) — mirrors the Rust
 * `decode_loro_relay_payload` returning `Ok(None)`. Throws if the magic is
 * present but the DEFLATE body or postcard structure is corrupt.
 */
export function decodeTlr2(frame: Uint8Array): LoroDocUpdate[] | null {
  if (frame.length < TLR2_MAGIC.length) return null;
  for (let i = 0; i < TLR2_MAGIC.length; i++) {
    if (frame[i] !== TLR2_MAGIC[i]) return null;
  }
  // `inflateSync` = raw INFLATE (no header), matching
  // `miniz_oxide::inflate::decompress_to_vec`.
  const body = inflateSync(frame.subarray(TLR2_MAGIC.length));
  return postcardDecode(body);
}
