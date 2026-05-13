//! Pairing-code format (Phase 2.2).
//!
//! A pairing code is a single short-ish string that carries everything a
//! joining device needs to enter another device's sync group in one
//! shot: the inviter's URL + device id, plus the symmetric group id and
//! group key. The wire format is postcard + base64url so the result is
//! URL-safe and copy-paste-friendly.
//!
//! Threat model: the code is the secret. Anyone who can read it can
//! sync into the group. The proper flow is "show on inviter, paste on
//! joiner" out-of-band (in person, encrypted DM, etc.). A future QR
//! version uses the same payload.
//!
//! No identity binding yet: there's no proof the device that issued the
//! code is actually the device the joiner ends up paired with. That
//! arrives when Ed25519 device identity lands. For now we trust that
//! the user pasted the code into the device they meant to pair with.

use crate::crypto::keys::{GroupIdentity, GroupKey};
use crate::device::DeviceId;
use crate::error::{SyncError, SyncResult};
use crate::group::GroupId;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// Wire-form pairing code. Encodes to a base64url string via [`encode`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairingCode {
    /// Group the inviter belongs to. Joiner adopts this.
    pub group_id: GroupId,
    /// Symmetric key for the group. Joiner adopts this.
    pub group_key_bytes: [u8; 32],
    /// Inviter's device id, so the joiner can add them as a peer.
    pub device_id: DeviceId,
    /// Reachable URL of the inviter's tesela-server HTTP API. Includes
    /// scheme + host + port, e.g. `http://10.15.109.184:7474`.
    pub url: String,
    /// User-visible display name of the inviter.
    pub display_name: String,
    /// Protocol version. Bumps when the payload schema changes.
    pub version: u8,
}

/// Current pairing-code format version.
pub const PAIRING_CODE_VERSION: u8 = 1;

impl PairingCode {
    /// Build a pairing code for the local device to share with a joiner.
    pub fn from_local(
        ident: &GroupIdentity,
        device_id: DeviceId,
        url: String,
        display_name: String,
    ) -> Self {
        Self {
            group_id: ident.group_id,
            group_key_bytes: *ident.group_key.as_bytes(),
            device_id,
            url,
            display_name,
            version: PAIRING_CODE_VERSION,
        }
    }

    /// Extract the group identity carried by this code so the joiner can
    /// adopt it locally.
    pub fn group_identity(&self) -> GroupIdentity {
        GroupIdentity {
            group_id: self.group_id,
            group_key: GroupKey::from_bytes(self.group_key_bytes),
        }
    }
}

/// Encode a pairing code as a single base64url-no-pad string.
pub fn encode(code: &PairingCode) -> SyncResult<String> {
    let bytes = postcard::to_allocvec(code)?;
    Ok(URL_SAFE_NO_PAD.encode(&bytes))
}

/// Decode a base64url-no-pad string back into a `PairingCode`. Tolerates
/// surrounding whitespace from a careless paste. Rejects versions higher
/// than the local [`PAIRING_CODE_VERSION`] with a clear message.
pub fn decode(s: &str) -> SyncResult<PairingCode> {
    let trimmed = s.trim();
    let bytes = URL_SAFE_NO_PAD
        .decode(trimmed)
        .map_err(|e| SyncError::Other(format!("pairing code base64 decode: {e}")))?;
    let code: PairingCode = postcard::from_bytes(&bytes)?;
    if code.version > PAIRING_CODE_VERSION {
        return Err(SyncError::Other(format!(
            "pairing code version {} is newer than local v{}",
            code.version, PAIRING_CODE_VERSION
        )));
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::keys::GroupKey;

    fn fixture_code() -> PairingCode {
        PairingCode {
            group_id: GroupId::from_bytes([0xa1; 16]),
            group_key_bytes: [0x55; 32],
            device_id: DeviceId::from_bytes([0xc3; 16]),
            url: "http://192.168.1.10:7474".to_string(),
            display_name: "Tay's Laptop".to_string(),
            version: PAIRING_CODE_VERSION,
        }
    }

    #[test]
    fn encode_decode_round_trip() {
        let code = fixture_code();
        let s = encode(&code).unwrap();
        let back = decode(&s).unwrap();
        assert_eq!(code, back);
    }

    #[test]
    fn decode_tolerates_whitespace() {
        let code = fixture_code();
        let s = encode(&code).unwrap();
        let padded = format!("  \n{s}\t\n ");
        let back = decode(&padded).unwrap();
        assert_eq!(code, back);
    }

    #[test]
    fn decode_rejects_garbage() {
        assert!(decode("not a valid base64 url $$$").is_err());
    }

    #[test]
    fn decode_rejects_future_version() {
        let mut code = fixture_code();
        code.version = PAIRING_CODE_VERSION + 1;
        let s = encode(&code).unwrap();
        let err = decode(&s).unwrap_err();
        assert!(err.to_string().contains("newer than local"));
    }

    #[test]
    fn from_local_uses_provided_identity() {
        let ident = GroupIdentity {
            group_id: GroupId::from_bytes([0x77; 16]),
            group_key: GroupKey::from_bytes([0x88; 32]),
        };
        let code = PairingCode::from_local(
            &ident,
            DeviceId::from_bytes([0xee; 16]),
            "http://h:1".into(),
            "h".into(),
        );
        assert_eq!(code.group_id, ident.group_id);
        assert_eq!(&code.group_key_bytes, ident.group_key.as_bytes());
    }

    #[test]
    fn encoded_length_is_reasonable() {
        let code = fixture_code();
        let s = encode(&code).unwrap();
        // Postcard varints + base64 ≈ 4/3 expansion. For our fields the
        // payload sits comfortably under 200 chars, well inside what a
        // text input can hold without wrapping.
        assert!(s.len() < 200, "got length {}", s.len());
    }
}
