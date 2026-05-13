//! Device identity.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 16-byte device identifier.
///
/// Generated once per device on first run (UUIDv7, time-ordered). The same
/// `DeviceId` is reused for the lifetime of the install. Width matches
/// `uhlc::ID` so we can use it directly as the device component of an HLC
/// timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub [u8; 16]);

impl DeviceId {
    /// Generate a new random `DeviceId` using UUIDv7 (time-ordered).
    pub fn new_random() -> Self {
        DeviceId(*Uuid::now_v7().as_bytes())
    }

    /// Construct from raw bytes.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        DeviceId(bytes)
    }

    /// Borrow as a byte slice (for hashing, database storage).
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Render as a lowercase hex string (32 chars).
    pub fn to_hex(&self) -> String {
        hex_encode(&self.0)
    }
}

impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// User-visible metadata about a device. Owned strings, no lifetimes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetadata {
    /// The device's id.
    pub device_id: DeviceId,
    /// User-chosen display name ("Taylor's MacBook", "iPhone").
    pub display_name: String,
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_random_is_unique() {
        let a = DeviceId::new_random();
        let b = DeviceId::new_random();
        assert_ne!(a, b);
    }

    #[test]
    fn device_id_hex_roundtrip_length() {
        let id = DeviceId::new_random();
        let hex = id.to_hex();
        assert_eq!(hex.len(), 32);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn device_id_display_matches_hex() {
        let id = DeviceId::from_bytes([0xab; 16]);
        assert_eq!(format!("{id}"), "abababababababababababababababab");
    }
}
