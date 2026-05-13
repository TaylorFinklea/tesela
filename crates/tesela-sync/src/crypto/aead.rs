//! XChaCha20-Poly1305 envelope sealing (Phase 2.3).
//!
//! Used to encrypt the postcard-encoded op batch carried inside each
//! [`crate::wire::envelope::SyncEnvelope`] when sync traverses an
//! untrusted hop. The group key is shared out-of-band via the pairing
//! code (Phase 2.2); this module is the consumer.
//!
//! Authenticated data binds the envelope's routing fields (`from_device`
//! and `to_group`) so a man-in-the-middle can't rewrite who claims to
//! be sending the envelope without invalidating the AEAD tag.
//!
//! Per the architecture plan we'll eventually HKDF a per-envelope key
//! from `(group_key, nonce, info="tesela-env-v1")` — that's the right
//! shape for rotation later. For POC we use the group key directly with
//! XChaCha20-Poly1305's 24-byte nonce; the 24-byte nonce is large
//! enough that random per-envelope nonces are safe without the HKDF
//! salt step. The HKDF wrapper drops in cleanly when we want it.

use crate::crypto::keys::GroupKey;
use crate::error::{SyncError, SyncResult};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::RngCore;

/// AEAD-sealed payload + its nonce. The nonce is non-secret and travels
/// alongside the ciphertext in the [`SyncEnvelope`] wire form.
#[derive(Debug, Clone)]
pub struct SealedPayload {
    /// 24-byte XChaCha20-Poly1305 nonce. Random per seal.
    pub nonce: [u8; 24],
    /// Ciphertext + 16-byte Poly1305 tag, concatenated as
    /// `chacha20poly1305` emits it.
    pub ciphertext: Vec<u8>,
}

/// Seal `plaintext` under `key`, binding `aad`. Returns the nonce + the
/// sealed bytes (ciphertext || tag).
pub fn seal(key: &GroupKey, plaintext: &[u8], aad: &[u8]) -> SyncResult<SealedPayload> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.as_bytes()));
    let mut nonce_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|e| SyncError::Crypto(format!("seal: {e}")))?;
    Ok(SealedPayload {
        nonce: nonce_bytes,
        ciphertext,
    })
}

/// Inverse of [`seal`]. Returns the plaintext on a valid tag + matching
/// AAD; returns [`SyncError::Crypto`] on any failure (wrong key, wrong
/// AAD, truncated ciphertext, flipped bit).
pub fn open(
    key: &GroupKey,
    nonce: &[u8; 24],
    ciphertext: &[u8],
    aad: &[u8],
) -> SyncResult<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key.as_bytes()));
    let nonce = XNonce::from_slice(nonce);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|e| SyncError::Crypto(format!("open: {e}")))
}

/// Helper to compose the AAD bytes that bind an envelope's routing
/// metadata. Keeping the layout in one place so the producer and the
/// receiver can't disagree.
pub fn envelope_aad(from_device: &[u8; 16], to_group: &[u8; 16]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(from_device);
    out[16..].copy_from_slice(to_group);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_key() -> GroupKey {
        GroupKey::from_bytes([0x99; 32])
    }

    #[test]
    fn seal_open_round_trip() {
        let key = fixture_key();
        let plaintext = b"hello world, this is some postcard payload";
        let aad = b"from=A,to=G1";
        let sealed = seal(&key, plaintext, aad).unwrap();
        let opened = open(&key, &sealed.nonce, &sealed.ciphertext, aad).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn seal_produces_random_nonce() {
        let key = fixture_key();
        let s1 = seal(&key, b"x", b"a").unwrap();
        let s2 = seal(&key, b"x", b"a").unwrap();
        assert_ne!(s1.nonce, s2.nonce, "nonce reuse is catastrophic for AEAD");
        assert_ne!(s1.ciphertext, s2.ciphertext);
    }

    #[test]
    fn wrong_aad_fails_open() {
        let key = fixture_key();
        let sealed = seal(&key, b"secret", b"correct-aad").unwrap();
        let err = open(&key, &sealed.nonce, &sealed.ciphertext, b"wrong-aad").unwrap_err();
        assert!(matches!(err, SyncError::Crypto(_)));
    }

    #[test]
    fn wrong_key_fails_open() {
        let alice = fixture_key();
        let bob = GroupKey::from_bytes([0x11; 32]);
        let sealed = seal(&alice, b"secret", b"aad").unwrap();
        let err = open(&bob, &sealed.nonce, &sealed.ciphertext, b"aad").unwrap_err();
        assert!(matches!(err, SyncError::Crypto(_)));
    }

    #[test]
    fn flipped_bit_fails_open() {
        let key = fixture_key();
        let mut sealed = seal(&key, b"secret payload", b"aad").unwrap();
        sealed.ciphertext[0] ^= 0x01;
        let err = open(&key, &sealed.nonce, &sealed.ciphertext, b"aad").unwrap_err();
        assert!(matches!(err, SyncError::Crypto(_)));
    }

    #[test]
    fn envelope_aad_layout() {
        let aad = envelope_aad(&[0x11; 16], &[0x22; 16]);
        assert_eq!(&aad[..16], &[0x11; 16]);
        assert_eq!(&aad[16..], &[0x22; 16]);
    }
}
