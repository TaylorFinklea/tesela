//! The on-wire envelope wrapping one or more encoded ops.
//!
//! Phase 1 carries `ciphertext` as cleartext (postcard-encoded
//! `Vec<EncodedOp>`); Phase 2 will wrap it with AEAD.

use crate::device::DeviceId;
use crate::group::GroupId;
use serde::{Deserialize, Serialize};

/// Envelope transmitted between devices.
///
/// In Phase 1 the `ciphertext` field is cleartext postcard. The relay sees
/// `from_device`, `to_group`, `ciphertext.len()`, and a timestamp. The
/// field is named `ciphertext` so the Phase 2 transition to AEAD adds
/// encryption without renaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEnvelope {
    /// Device that produced this envelope.
    pub from_device: DeviceId,
    /// Group this envelope is destined for.
    pub to_group: GroupId,
    /// AEAD nonce. Phase 1 = zero-filled placeholder. Phase 2 = real.
    pub nonce: [u8; 24],
    /// Sealed bytes. Phase 1 = cleartext postcard `Vec<EncodedOp>`.
    /// Phase 2 = AEAD ciphertext + auth tag.
    pub ciphertext: Vec<u8>,
}
