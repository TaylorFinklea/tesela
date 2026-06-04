/**
 * Crypto helpers — Web Crypto wrappers used by the relay's MAC
 * verification path. All match the Rust side bit-for-bit:
 *
 *   - HMAC-SHA256 keyed with the per-group `auth_key` (32 bytes)
 *   - SHA-256 over the request body for the canonical_request hash
 *
 * The relay NEVER touches the `group_key` itself — that's the
 * content-encryption key, held only on devices. The relay only
 * stores + uses `auth_key`, which is HKDF-derived from `group_key`
 * client-side (see crates/tesela-sync/src/crypto/relay_auth.rs).
 */

/** SHA-256 → lowercase hex string. Mirrors Rust's `hex::encode(SHA256(bytes))`. */
export async function sha256Hex(bytes: ArrayBuffer | Uint8Array): Promise<string> {
  const data = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  const digest = await crypto.subtle.digest("SHA-256", data);
  return toHex(new Uint8Array(digest));
}

/** Empty-body hash: convenience so handlers don't all repeat it. */
export const EMPTY_BODY_HASH = "" as const;

/**
 * HMAC-SHA256(key, message) → raw 32 bytes.
 * Matches Rust's `hmac::Hmac::<Sha256>::new_from_slice(&key).update(msg).finalize()`.
 */
export async function hmacSha256(
  key: Uint8Array,
  message: string,
): Promise<Uint8Array> {
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    key,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign(
    "HMAC",
    cryptoKey,
    new TextEncoder().encode(message),
  );
  return new Uint8Array(sig);
}

/**
 * Constant-time compare of two byte arrays. Same length required.
 * Matches Rust's `ring::constant_time::verify_slices_are_equal`.
 *
 * Web Crypto doesn't expose a generic timing-safe compare; we
 * implement one here. Branch-prediction-resistant XOR fold.
 */
export function constantTimeEq(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  let diff = 0;
  for (let i = 0; i < a.length; i++) {
    diff |= a[i]! ^ b[i]!;
  }
  return diff === 0;
}

// ─── Encoding helpers ──────────────────────────────────────────────

const HEX_CHARS = "0123456789abcdef";

export function toHex(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i++) {
    const b = bytes[i]!;
    out += HEX_CHARS[b >> 4]! + HEX_CHARS[b & 0x0f]!;
  }
  return out;
}

export function fromHex(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) throw new Error("hex string must be even length");
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    const byte = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
    // `parseInt("zz", 16)` → NaN, which a Uint8Array would silently
    // coerce to 0 — corrupting an invalid device id into the all-zero
    // device. Reject instead, matching Rust's `hex::decode` erroring.
    if (Number.isNaN(byte)) throw new Error("invalid hex character");
    out[i] = byte;
  }
  return out;
}

/** True iff `s` is exactly `byteLen` bytes of lowercase-or-uppercase hex.
 *  Used to return a clean 400 (not a thrown 500) for malformed id headers. */
export function isHex(s: string, byteLen: number): boolean {
  return s.length === byteLen * 2 && /^[0-9a-fA-F]+$/.test(s);
}

/** Standard base64 (with padding). Matches Rust's `base64::engine::general_purpose::STANDARD`. */
export function toB64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!);
  return btoa(bin);
}

export function fromB64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/**
 * Reconstruct the canonical request string that every MAC is signed
 * against. Exact mirror of Rust's `tesela-sync::crypto::relay_auth::canonical_request`.
 *
 * Format:
 *   `<method>\n<path>\n<query>\n<nonce>\n<ts>\n<body_hash_hex>`
 *
 * For empty bodies (GETs, /ack noop), `body_hash_hex` is the empty
 * string (NOT `SHA256("")` — matches Rust's `body_hash_hex(&[])` →
 * `String::new()`).
 */
export function canonicalRequest(
  method: string,
  path: string,
  query: string,
  nonce: string,
  ts: string,
  bodyHashHex: string,
): string {
  return [method, path, query, nonce, ts, bodyHashHex].join("\n");
}
