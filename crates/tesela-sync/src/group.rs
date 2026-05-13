//! Group identity. A "group" is the set of devices paired together that
//! share a symmetric key and sync their oplogs. For single-user-many-
//! devices, a Tesela install has exactly one group.

use crate::device::DeviceId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 16-byte group identifier. Generated at group genesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(pub [u8; 16]);

impl GroupId {
    /// Generate a new random `GroupId`.
    pub fn new_random() -> Self {
        GroupId(*Uuid::new_v4().as_bytes())
    }

    /// Construct from raw bytes.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        GroupId(bytes)
    }

    /// Borrow as a byte slice.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// Member of a group. Stored in `group_members` table per device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    /// The group this member belongs to.
    pub group_id: GroupId,
    /// Device id of the member.
    pub device_id: DeviceId,
    /// Ed25519 public key, 32 bytes. Used for handshake and key wrapping.
    /// Empty in Phase 1 (crypto not wired yet); becomes load-bearing in
    /// Phase 2.
    pub ed25519_pubkey: Vec<u8>,
    /// Optional user-set display name for this member.
    pub display_name: Option<String>,
    /// Wall-clock time when this member was added, millis since epoch.
    pub added_at_millis: i64,
}
