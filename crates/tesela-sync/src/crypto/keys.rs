//! Group-key storage adapter (Phase 2.2).
//!
//! For POC, the symmetric group key is persisted as raw 32 bytes inside
//! the mosaic's `.tesela/` dir alongside the device id, paired with a
//! 16-byte `GroupId`. The real plan keeps the key in macOS Keychain /
//! iOS Keychain via `security-framework` — that adapter slots in here
//! by replacing the file-based [`load_or_create`] implementation.
//!
//! Threat model right now: anyone who can read the mosaic directory can
//! read the group key. That's already true for the notes themselves, so
//! the additional exposure is bounded. The keychain adapter closes the
//! gap on shared / multi-user machines.

use crate::error::{SyncError, SyncResult};
use crate::group::GroupId;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 32-byte symmetric key. Used as IKM for the per-envelope HKDF in
/// Phase 2.3. Stored on disk for POC; will move to OS keychain.
#[derive(Clone, Serialize, Deserialize)]
pub struct GroupKey([u8; 32]);

impl GroupKey {
    /// Borrow the raw key bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Wrap raw bytes as a `GroupKey`.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        GroupKey(bytes)
    }

    /// Generate a fresh random key via the OS CSPRNG.
    pub fn random() -> Self {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        GroupKey(bytes)
    }
}

// Manual Debug to avoid leaking secrets in logs. We still want the type
// to participate in `dbg!()` and `Result` formatting without crashing.
impl std::fmt::Debug for GroupKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GroupKey").field("len", &32usize).finish()
    }
}

/// Group identity persisted in a mosaic: the group id + symmetric key.
#[derive(Debug, Clone)]
pub struct GroupIdentity {
    /// The group this device belongs to.
    pub group_id: GroupId,
    /// Shared symmetric key. Empty groups (newly minted single-device
    /// installs) get a fresh random one.
    pub group_key: GroupKey,
}

/// Persist the group identity to two files inside `<mosaic>/.tesela/`.
///
/// Two files rather than one packed struct so we can rotate the key
/// without re-issuing the group id, and so a future keychain-backed
/// storage can swap just the key half without touching the id half.
pub async fn load_or_create(mosaic_root: &Path) -> SyncResult<GroupIdentity> {
    let dir = mosaic_root.join(".tesela");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| SyncError::Other(format!("create .tesela dir: {e}")))?;
    let group_id = load_or_create_group_id(&dir).await?;
    let group_key = load_or_create_group_key(&dir).await?;
    Ok(GroupIdentity {
        group_id,
        group_key,
    })
}

/// Adopt a peer's group identity in place, overwriting the local one.
/// Used by the pairing-code receiver: the joining device throws away its
/// own freshly-minted group and takes the inviter's. Idempotent — a
/// second adopt of the same id+key is a no-op write.
pub async fn adopt(mosaic_root: &Path, ident: &GroupIdentity) -> SyncResult<()> {
    let dir = mosaic_root.join(".tesela");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| SyncError::Other(format!("create .tesela dir: {e}")))?;
    tokio::fs::write(group_id_path(&dir), hex_encode(ident.group_id.as_bytes()))
        .await
        .map_err(|e| SyncError::Other(format!("write group_id: {e}")))?;
    tokio::fs::write(group_key_path(&dir), ident.group_key.as_bytes())
        .await
        .map_err(|e| SyncError::Other(format!("write group_key: {e}")))?;
    Ok(())
}

fn group_id_path(tesela_dir: &Path) -> PathBuf {
    tesela_dir.join("group_id.hex")
}

fn group_key_path(tesela_dir: &Path) -> PathBuf {
    tesela_dir.join("group_key.bin")
}

async fn load_or_create_group_id(tesela_dir: &Path) -> SyncResult<GroupId> {
    let path = group_id_path(tesela_dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(s) => {
            let trimmed = s.trim();
            parse_hex_16(trimmed)
                .map(GroupId::from_bytes)
                .ok_or_else(|| SyncError::Other(format!("bad group_id hex at {}", path.display())))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let id = GroupId::new_random();
            tokio::fs::write(&path, hex_encode(id.as_bytes()))
                .await
                .map_err(|e| SyncError::Other(format!("write group_id: {e}")))?;
            Ok(id)
        }
        Err(e) => Err(SyncError::Other(format!("read group_id: {e}"))),
    }
}

async fn load_or_create_group_key(tesela_dir: &Path) -> SyncResult<GroupKey> {
    let path = group_key_path(tesela_dir);
    match tokio::fs::read(&path).await {
        Ok(bytes) if bytes.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            Ok(GroupKey::from_bytes(arr))
        }
        Ok(_) => Err(SyncError::Other(format!(
            "group_key file at {} has wrong length",
            path.display()
        ))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let k = GroupKey::random();
            tokio::fs::write(&path, k.as_bytes())
                .await
                .map_err(|e| SyncError::Other(format!("write group_key: {e}")))?;
            Ok(k)
        }
        Err(e) => Err(SyncError::Other(format!("read group_key: {e}"))),
    }
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

fn parse_hex_16(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for i in 0..16 {
        let hi = nibble(s.as_bytes()[i * 2])?;
        let lo = nibble(s.as_bytes()[i * 2 + 1])?;
        out[i] = (hi << 4) | lo;
    }
    Some(out)
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_key_random_is_unique() {
        let a = GroupKey::random();
        let b = GroupKey::random();
        assert_ne!(a.as_bytes(), b.as_bytes());
    }

    #[test]
    fn debug_does_not_leak_key_bytes() {
        // Pick a fixture whose bytes don't collide with literals in the
        // Debug output (the field name "len" + the value "32"). 0xab
        // renders as "ab" — not present in the formatted struct.
        let k = GroupKey::from_bytes([0xab; 32]);
        let dbg = format!("{:?}", k);
        assert!(!dbg.contains("ab"), "got dbg = {dbg}");
    }

    #[tokio::test]
    async fn load_or_create_round_trips() {
        let tmp = tempfile::TempDir::new().unwrap();
        let g1 = load_or_create(tmp.path()).await.unwrap();
        let g2 = load_or_create(tmp.path()).await.unwrap();
        assert_eq!(g1.group_id, g2.group_id);
        assert_eq!(g1.group_key.as_bytes(), g2.group_key.as_bytes());
    }

    #[tokio::test]
    async fn adopt_overwrites() {
        let tmp = tempfile::TempDir::new().unwrap();
        let g1 = load_or_create(tmp.path()).await.unwrap();
        let other = GroupIdentity {
            group_id: GroupId::new_random(),
            group_key: GroupKey::random(),
        };
        adopt(tmp.path(), &other).await.unwrap();
        let reloaded = load_or_create(tmp.path()).await.unwrap();
        assert_ne!(reloaded.group_id, g1.group_id);
        assert_eq!(reloaded.group_id, other.group_id);
        assert_eq!(reloaded.group_key.as_bytes(), other.group_key.as_bytes());
    }
}
