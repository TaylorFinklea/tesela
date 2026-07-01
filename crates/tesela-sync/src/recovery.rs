//! Recovery-phrase pairing orchestration (`tesela-ra7` P0 step 3a).
//!
//! Bridges the pure crypto in [`crate::crypto::recovery`] (phrase <->
//! key, discovery-handle derivation) and [`crate::crypto::pairing`]
//! (the wire format joiners consume) with the relay's unauthenticated
//! discovery endpoint ([`crate::transport::relay::discover_group`]).
//!
//! **Key architecture decision:** recovery PRODUCES a pairing code —
//! it does NOT adopt group identity directly (that's
//! `crypto::keys::adopt`, used by the existing pairing-code receiver).
//! A phrase-only device (has the 24-word recovery phrase = the
//! `GroupKey`, but neither the `group_id` nor a paired server) resolves
//! its `group_id` via relay discovery, then builds + encodes the same
//! shape of **relay-only** pairing code a QR scan would have handed a
//! joiner. Callers feed the result straight into the existing
//! relay-pairing-code adoption path (iOS: `RelayTicker.cachePairingCode`
//! + `.relay` mode), so recovery needs no new adoption code path.
//!
//! Lives in `tesela-sync` (not `tesela-sync-ffi`) so `tesela-relay`'s
//! conformance/e2e tests can exercise it directly against a spawned
//! relay without going through the FFI boundary.

use crate::crypto::pairing::{decode, encode, PairingCode, PAIRING_CODE_VERSION};
use crate::crypto::recovery::{derive_discovery_handle, key_to_phrase, phrase_to_key};
use crate::device::DeviceId;
use crate::error::{SyncError, SyncResult};
use crate::transport::relay::discover_group;

/// Recover group membership from a 24-word recovery phrase alone.
///
/// `phrase` -> `GroupKey` (via [`phrase_to_key`], which validates word
/// count / wordlist membership / checksum) -> discovery handle (via
/// [`derive_discovery_handle`]) -> `GET {relay_url}/discover/{disc}`
/// (via [`discover_group`]) -> the group's `group_id`.
///
/// On a hit, builds a **relay-only** pairing code (empty `url` — no
/// LAN server — with `relay_url` set to the relay just queried) and
/// base64url-encodes it exactly like [`crate::crypto::pairing::encode`]
/// does for a normal QR/share flow, so downstream adoption code can't
/// tell the two apart.
///
/// On a miss (the phrase's group never registered its discovery
/// handle on this relay — see the relay's `disc_b64` back-compat note)
/// returns a distinct, clearly-worded [`SyncError::Other`] rather than
/// silently treating "not found" as any other failure.
///
/// Never includes phrase words or key bytes in any error message —
/// mirrors [`crate::crypto::recovery::RecoveryError`]'s no-leak
/// posture.
pub async fn recover_pairing_from_phrase(relay_url: &str, phrase: &str) -> SyncResult<String> {
    let key = phrase_to_key(phrase)
        .map_err(|e| SyncError::Other(format!("recovery phrase is invalid: {e}")))?;
    let disc = derive_discovery_handle(&key);
    let group_id = discover_group(relay_url, &disc).await?.ok_or_else(|| {
        SyncError::Other("recovery phrase not found on this relay".to_string())
    })?;
    let code = PairingCode {
        group_id,
        group_key_bytes: *key.as_bytes(),
        device_id: DeviceId::new_random(),
        url: String::new(),
        display_name: "Recovered mosaic".to_string(),
        relay_url: Some(relay_url.to_string()),
        version: PAIRING_CODE_VERSION,
    };
    encode(&code)
}

/// Inverse convenience: given a pairing code, recover the human-
/// readable 24-word recovery phrase for its group key. Powers "Show
/// recovery phrase" style screens that already have a pairing code's
/// group identity on hand.
pub fn recovery_phrase_from_pairing_code(code: &str) -> SyncResult<String> {
    let parsed = decode(code)?;
    let key = parsed.group_identity().group_key;
    Ok(key_to_phrase(&key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::GroupId;

    #[test]
    fn recovery_phrase_from_pairing_code_round_trips() {
        let key = crate::crypto::keys::GroupKey::from_bytes([0x42; 32]);
        let phrase = key_to_phrase(&key);
        let ident = crate::crypto::keys::GroupIdentity {
            group_id: GroupId::from_bytes([0x01; 16]),
            group_key: key,
        };
        let code = PairingCode::from_local(
            &ident,
            DeviceId::from_bytes([0x02; 16]),
            "http://h:1".into(),
            "h".into(),
            None,
        );
        let encoded = encode(&code).unwrap();
        let recovered = recovery_phrase_from_pairing_code(&encoded).unwrap();
        assert_eq!(recovered, phrase);
    }

    #[test]
    fn recovery_phrase_from_pairing_code_rejects_garbage() {
        assert!(recovery_phrase_from_pairing_code("not a valid code $$$").is_err());
    }
}
