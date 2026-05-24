//! WAN sync relay authentication primitives.
//!
//! See `.docs/ai/phases/2026-05-24-relay-protocol-design.md` for the
//! protocol. This module exposes the deterministic per-group auth-key
//! derivation, the registration intent payload that ONLY group-key
//! holders can produce, and the canonical per-request MAC. Both the
//! reference Rust relay (`tesela-relay`), the eventual Cloudflare
//! Worker port, and every device client (`RelayTransport`) compute
//! these the same way so registrations and request signatures
//! interoperate by construction.

use base64::Engine;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::crypto::keys::GroupKey;
use crate::group::GroupId;

type HmacSha256 = Hmac<Sha256>;

/// Domain-separator string for the relay-auth-key derivation. Bumping
/// the version invalidates every existing group's stored relay
/// registration on every relay (auth keys diverge), forcing
/// re-registration end-to-end. Treat as immutable post-v1.
pub const RELAY_AUTH_INFO_V1: &[u8] = b"tesela-relay-auth-v1";

/// Domain-separator string for the registration intent payload.
/// Distinct prefix from `RELAY_AUTH_INFO_V1` so leaking one derivation
/// doesn't trivially compromise the other (defense in depth).
pub const RELAY_INTENT_PREFIX_V1: &str = "tesela-relay-register-v1";

/// Derive the 32-byte per-group relay auth key from the group key +
/// group id. Deterministic — every device in the group computes the
/// same value independently from the same `group_key`, so no
/// out-of-band key exchange to the relay is needed beyond first-write
/// registration.
///
/// Properties:
/// - One-way: holding the auth key doesn't recover the group key
///   (HKDF is a PRF). The relay can authenticate requests but cannot
///   decrypt content.
/// - Per-group: an auth-key compromise for one group doesn't touch
///   any other group (each group's `group_id` is the HKDF salt).
pub fn derive_relay_auth_key(group_key: &GroupKey, group_id: &GroupId) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(group_id.as_bytes()), group_key.as_bytes());
    let mut out = [0u8; 32];
    // HKDF-Expand can produce up to 255 * Sha256::OutputSize bytes
    // (~8160 bytes); 32 is trivially safe so the `expect` here can
    // never fire.
    hk.expand(RELAY_AUTH_INFO_V1, &mut out)
        .expect("32-byte HKDF output is well below the max");
    out
}

/// Build the canonical bytes that get signed during registration. The
/// relay stores this verbatim and serves it back on `GET /registration`;
/// joiners recompute it locally + verify the signed intent matches.
/// Any byte-level discrepancy here (between client, relay, and joiner)
/// breaks the joiner verification path, so the format MUST stay stable.
///
/// Format: `tesela-relay-register-v1|{group_id_hex}|{auth_key_b64}|{ts}`
pub fn intent_msg(group_id: &GroupId, auth_key: &[u8; 32], registered_at: i64) -> String {
    format!(
        "{prefix}|{gid}|{auth_b64}|{ts}",
        prefix = RELAY_INTENT_PREFIX_V1,
        gid = hex::encode(group_id.as_bytes()),
        auth_b64 = base64::engine::general_purpose::STANDARD.encode(auth_key),
        ts = registered_at,
    )
}

/// HMAC-SHA256 the intent payload with the **group key**. Only
/// group-key holders can produce a valid signature — the relay
/// cannot, by design (zero-knowledge).
pub fn sign_intent(group_key: &GroupKey, intent: &str) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(group_key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(intent.as_bytes());
    mac.finalize().into_bytes().into()
}

/// Verify a signed intent. Used by joiners on first connection to a
/// relay to check that the registration was made by a legitimate
/// group-key holder. Returns `true` on match.
pub fn verify_intent(
    group_key: &GroupKey,
    intent: &str,
    candidate_signature: &[u8],
) -> bool {
    let mut mac = HmacSha256::new_from_slice(group_key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(intent.as_bytes());
    // Hmac's `verify_slice` is constant-time.
    mac.verify_slice(candidate_signature).is_ok()
}

/// Build the canonical request bytes that get HMACed for the
/// `X-Tesela-Mac` header. Identical formatting on client + relay; any
/// drift breaks MAC verification.
///
/// Layout: `METHOD\nPATH\nQUERY\nNONCE\nTS\nBODY_HASH_HEX`
///
/// - `query` excludes the leading `?` and may be empty.
/// - `body_hash_hex` is empty for GETs (no body); SHA-256 hex for
///   request bodies. See [`body_hash_hex`].
pub fn canonical_request(
    method: &str,
    path: &str,
    query: &str,
    nonce_b64: &str,
    ts: i64,
    body_hash_hex_value: &str,
) -> String {
    format!("{method}\n{path}\n{query}\n{nonce_b64}\n{ts}\n{body_hash_hex_value}")
}

/// HMAC-SHA256 the canonical request with the **auth key** (not the
/// group key — auth key is what the relay knows + verifies against).
pub fn compute_request_mac(auth_key: &[u8; 32], canonical: &str) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(auth_key)
        .expect("HMAC accepts 32-byte keys");
    mac.update(canonical.as_bytes());
    mac.finalize().into_bytes().into()
}

/// Verify a per-request MAC. Constant-time comparison via `hmac::Mac`.
pub fn verify_request_mac(
    auth_key: &[u8; 32],
    canonical: &str,
    candidate_signature: &[u8],
) -> bool {
    let mut mac = HmacSha256::new_from_slice(auth_key)
        .expect("HMAC accepts 32-byte keys");
    mac.update(canonical.as_bytes());
    mac.verify_slice(candidate_signature).is_ok()
}

/// SHA-256 a request body, hex-encoded. Empty body → empty string
/// (so GETs don't waste cycles hashing nothing — the canonical
/// request still has a stable shape with an empty trailing field).
pub fn body_hash_hex(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }
    let digest = Sha256::digest(body);
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keys::GroupKey;
    use crate::group::GroupId;

    fn fixture_key() -> GroupKey {
        // 32 deterministic bytes for reproducible test vectors.
        let bytes: [u8; 32] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
            0x1d, 0x1e, 0x1f, 0x20,
        ];
        GroupKey::from_bytes(bytes)
    }

    fn fixture_group_id() -> GroupId {
        let bytes: [u8; 16] = [
            0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad,
            0xae, 0xaf,
        ];
        GroupId::from_bytes(bytes)
    }

    /// The two derivations must produce DIFFERENT outputs for the
    /// same key (domain separation works). If this ever flips, the
    /// auth key would be the same as some other derivation and we'd
    /// be back to the open-deposit problem.
    #[test]
    fn auth_key_is_deterministic_and_unique_per_group() {
        let k = fixture_key();
        let g1 = fixture_group_id();
        let g2 = GroupId::from_bytes([0u8; 16]);

        // Same inputs → same output (deterministic).
        assert_eq!(
            derive_relay_auth_key(&k, &g1),
            derive_relay_auth_key(&k, &g1),
        );
        // Different group_id → different auth key.
        assert_ne!(
            derive_relay_auth_key(&k, &g1),
            derive_relay_auth_key(&k, &g2),
        );
    }

    #[test]
    fn intent_round_trips_through_sign_and_verify() {
        let k = fixture_key();
        let g = fixture_group_id();
        let auth = derive_relay_auth_key(&k, &g);
        let msg = intent_msg(&g, &auth, 1_748_182_600);
        let sig = sign_intent(&k, &msg);
        assert!(verify_intent(&k, &msg, &sig));
    }

    /// A signature produced under a different group key must fail
    /// verification — that's the load-bearing hijack-detection
    /// property.
    #[test]
    fn intent_verify_fails_under_wrong_group_key() {
        let real = fixture_key();
        let wrong = GroupKey::from_bytes([0xff; 32]);
        let g = fixture_group_id();
        let auth = derive_relay_auth_key(&real, &g);
        let msg = intent_msg(&g, &auth, 1_748_182_600);
        let bogus_sig = sign_intent(&wrong, &msg);
        assert!(!verify_intent(&real, &msg, &bogus_sig));
    }

    #[test]
    fn request_mac_round_trips() {
        let auth = [0xab; 32];
        let canonical = canonical_request(
            "PUT",
            "/groups/abc/ops",
            "",
            "AAECAwQFBgcICQoLDA0ODw==",
            1_748_182_600,
            &body_hash_hex(b"hello"),
        );
        let mac = compute_request_mac(&auth, &canonical);
        assert!(verify_request_mac(&auth, &canonical, &mac));
    }

    #[test]
    fn body_hash_empty_for_no_body() {
        assert_eq!(body_hash_hex(&[]), "");
        assert_ne!(body_hash_hex(b"x"), "");
    }
}
