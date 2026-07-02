//! Group-key storage adapter (Phase 2.2 → tesela-tp0.2 Keychain cutover).
//!
//! The symmetric group key is persisted as raw 32 bytes inside the
//! mosaic's `.tesela/` dir alongside the device id, paired with a
//! 16-byte `GroupId`. On macOS the desktop server (`tesela-server`, both
//! the Tauri-wrapped desktop app and a self-hosted binary) now stores the
//! key in the macOS Keychain via [`KeychainGroupKeyStore`] (`keyring`
//! crate, `apple-native`/`security-framework` backend); an existing
//! plaintext `group_key.bin` from a pre-cutover install is migrated in
//! and shredded on first run — see [`load_or_create_group_key`].
//! Headless/self-host Linux boxes (no Keychain) — and any macOS box
//! where the login keychain isn't unlocked, e.g. a launchd daemon with
//! no GUI session — stay on [`FileGroupKeyStore`] by setting
//! `TESELA_GROUP_KEY_FILE_STORE`.
//!
//! iOS does NOT go through this Rust path: the FFI never persists a
//! group identity to a mosaic root, so iOS's own key storage (previously
//! the plaintext cached pairing code in `UserDefaults`) is a Swift-side
//! concern — see
//! `app/Tesela-iOS/Sources/Data/KeychainPairingCache.swift`.
//!
//! Threat model right now: anyone who can read the mosaic directory can
//! read a plaintext group key. That's already true for the notes
//! themselves, so the additional exposure is bounded — but the Keychain
//! adapter closes the gap on shared / multi-user machines and against
//! disk-image/backup exfiltration.

use crate::error::{SyncError, SyncResult};
use crate::group::GroupId;
use async_trait::async_trait;
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
    store_group_key_active(&dir, &ident.group_key).await?;
    Ok(())
}

/// Env var that forces the plaintext [`FileGroupKeyStore`] even on a
/// platform with a native Keychain — headless/self-host boxes (no GUI
/// session to unlock a login keychain) opt in with this rather than
/// silently falling back on a Keychain error, so an operator gets an
/// explicit choice instead of a surprise.
pub const FILE_STORE_ENV: &str = "TESELA_GROUP_KEY_FILE_STORE";

#[cfg(all(target_os = "macos", not(test)))]
fn file_store_forced() -> bool {
    std::env::var(FILE_STORE_ENV)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

/// Persist `key` into whichever store is active for this platform/env
/// (Keychain on macOS unless [`FILE_STORE_ENV`] forces the file store).
/// Used by [`adopt`] (pairing-code receiver overwrite) and by the
/// migrate-on-first-run path in [`load_or_create_group_key`].
///
/// `not(test)`-gated on the Keychain branch: `tesela-sync`'s OWN unit
/// tests must never touch the real OS keychain (mirrors `tesela-backup`'s
/// `encrypt` module, which never exercises the real Keychain either).
/// Downstream crates that depend on `tesela-sync` as an ordinary
/// (non-test) dependency — e.g. `tesela-server`'s integration tests —
/// don't get this `cfg(test)` for free and must opt into
/// [`FILE_STORE_ENV`] themselves for hermetic test mosaics.
async fn store_group_key_active(tesela_dir: &Path, key: &GroupKey) -> SyncResult<()> {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        if !file_store_forced() {
            KeychainGroupKeyStore::new(tesela_dir)
                .store_key(key)
                .await?;
            // A rotated/adopted key must not leave the OLD key readable
            // in a stale plaintext file from before this device switched
            // to the Keychain.
            FileGroupKeyStore::new(tesela_dir).shred().await?;
            return Ok(());
        }
    }
    FileGroupKeyStore::new(tesela_dir).store_key(key).await
}

fn group_id_path(tesela_dir: &Path) -> PathBuf {
    tesela_dir.join("group_id.hex")
}

fn group_key_path(tesela_dir: &Path) -> PathBuf {
    tesela_dir.join("group_key.bin")
}

/// Storage seam for the symmetric group key (Phase 2.2 → L1 → tesela-tp0.2).
/// [`FileGroupKeyStore`] keeps the byte-for-byte on-disk format
/// (`group_key.bin`, 32 raw bytes) and is the headless/self-host Linux
/// fallback; [`KeychainGroupKeyStore`] is the macOS default. Neither
/// touches the `group_id.hex` half — exactly the split the two-file
/// layout (see [`load_or_create`]) was designed to allow. Async so a
/// blocking keychain backend can `spawn_blocking` without changing
/// callers.
#[async_trait]
pub trait GroupKeyStore: Send + Sync {
    /// Load the stored group key, or `None` if none has been written yet.
    async fn load_key(&self) -> SyncResult<Option<GroupKey>>;
    /// Persist the group key, overwriting any existing one.
    async fn store_key(&self, key: &GroupKey) -> SyncResult<()>;
}

/// File-backed group-key store: 32 raw bytes at `<tesela_dir>/group_key.bin`.
/// This is the headless/self-host Linux store (and the macOS fallback
/// under [`FILE_STORE_ENV`]) — never delete `group_key.bin` out from
/// under an install that still relies on it; [`load_or_create_group_key`]
/// only removes it once its bytes have landed safely in the Keychain.
pub struct FileGroupKeyStore {
    path: PathBuf,
}

impl FileGroupKeyStore {
    /// Construct a file store rooted at a mosaic's `.tesela/` directory.
    pub fn new(tesela_dir: &Path) -> Self {
        Self {
            path: group_key_path(tesela_dir),
        }
    }

    /// Best-effort shred: overwrite the file with zeros before removing
    /// it, so a migrated-away plaintext copy doesn't linger on disk
    /// readable by anyone who can browse the mosaic directory. No-op if
    /// the file is already gone (never written, or already shredded).
    pub async fn shred(&self) -> SyncResult<()> {
        let len = match tokio::fs::metadata(&self.path).await {
            Ok(meta) => meta.len() as usize,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(SyncError::Other(format!("shred group_key (stat): {e}"))),
        };
        tokio::fs::write(&self.path, vec![0u8; len])
            .await
            .map_err(|e| SyncError::Other(format!("shred group_key (zero): {e}")))?;
        tokio::fs::remove_file(&self.path)
            .await
            .map_err(|e| SyncError::Other(format!("shred group_key (remove): {e}")))
    }
}

#[async_trait]
impl GroupKeyStore for FileGroupKeyStore {
    async fn load_key(&self) -> SyncResult<Option<GroupKey>> {
        match tokio::fs::read(&self.path).await {
            Ok(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(Some(GroupKey::from_bytes(arr)))
            }
            Ok(_) => Err(SyncError::Other(format!(
                "group_key file at {} has wrong length",
                self.path.display()
            ))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SyncError::Other(format!("read group_key: {e}"))),
        }
    }

    async fn store_key(&self, key: &GroupKey) -> SyncResult<()> {
        tokio::fs::write(&self.path, key.as_bytes())
            .await
            .map_err(|e| SyncError::Other(format!("write group_key: {e}")))
    }
}

/// macOS Keychain-backed group-key store (`keyring` crate,
/// `apple-native`/`security-framework` backend). Scoped per-mosaic by
/// keying the Keychain entry's account on the mosaic's `.tesela/` path,
/// mirroring `tesela-backup::encrypt`'s `keyring_account` convention.
/// Never formats key bytes into an error or log line — Keychain errors
/// carry only the OS-level failure reason.
#[cfg(target_os = "macos")]
pub struct KeychainGroupKeyStore {
    account: String,
}

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "tesela-sync-group-key";

#[cfg(target_os = "macos")]
impl KeychainGroupKeyStore {
    /// Construct a Keychain store scoped to a mosaic's `.tesela/`
    /// directory (the account name), so multiple mosaics on the same
    /// machine get independent Keychain entries.
    pub fn new(tesela_dir: &Path) -> Self {
        Self {
            account: tesela_dir.to_string_lossy().into_owned(),
        }
    }
}

#[cfg(target_os = "macos")]
#[async_trait]
impl GroupKeyStore for KeychainGroupKeyStore {
    async fn load_key(&self) -> SyncResult<Option<GroupKey>> {
        let account = self.account.clone();
        tokio::task::spawn_blocking(move || keychain_load(&account))
            .await
            .map_err(|e| SyncError::Other(format!("keychain load task: {e}")))?
    }

    async fn store_key(&self, key: &GroupKey) -> SyncResult<()> {
        let account = self.account.clone();
        let bytes = *key.as_bytes();
        tokio::task::spawn_blocking(move || keychain_store(&account, &bytes))
            .await
            .map_err(|e| SyncError::Other(format!("keychain store task: {e}")))?
    }
}

#[cfg(target_os = "macos")]
fn keychain_load(account: &str) -> SyncResult<Option<GroupKey>> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, account)
        .map_err(|e| SyncError::Other(format!("keychain entry: {e}")))?;
    match entry.get_secret() {
        Ok(bytes) if bytes.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            Ok(Some(GroupKey::from_bytes(arr)))
        }
        Ok(_) => Err(SyncError::Other(
            "keychain group key has wrong length".to_string(),
        )),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SyncError::Other(format!("keychain load: {e}"))),
    }
}

#[cfg(target_os = "macos")]
fn keychain_store(account: &str, bytes: &[u8; 32]) -> SyncResult<()> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, account)
        .map_err(|e| SyncError::Other(format!("keychain entry: {e}")))?;
    entry
        .set_secret(bytes)
        .map_err(|e| SyncError::Other(format!("keychain store: {e}")))
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

/// Load (or mint) the group key from whichever store is active for this
/// platform/env. On macOS (unless [`FILE_STORE_ENV`] forces the file
/// store) this migrates a legacy plaintext `group_key.bin` into the
/// Keychain on first run — see [`load_or_migrate_key`] — and shreds the
/// file once the migration lands. `not(test)`-gated for the same reason
/// as [`store_group_key_active`] — `tesela-sync`'s own unit tests never
/// touch the real OS keychain.
async fn load_or_create_group_key(tesela_dir: &Path) -> SyncResult<GroupKey> {
    let legacy = FileGroupKeyStore::new(tesela_dir);
    #[cfg(all(target_os = "macos", not(test)))]
    {
        if !file_store_forced() {
            let keychain = KeychainGroupKeyStore::new(tesela_dir);
            return load_or_migrate_key(&keychain, &legacy).await;
        }
    }
    // File-forced (env override) or non-macOS: the legacy file store IS
    // the active store, so there's nothing to migrate — just load-or-mint.
    match legacy.load_key().await? {
        Some(k) => Ok(k),
        None => {
            let k = GroupKey::random();
            legacy.store_key(&k).await?;
            Ok(k)
        }
    }
}

/// Migrate-on-first-run: if `primary` (the platform's preferred store —
/// the Keychain, on macOS) already has a key, use it. Otherwise, if the
/// `legacy` file store has a key from a pre-cutover install, adopt it
/// into `primary` and shred the legacy file so the plaintext copy
/// doesn't linger — this is the "no key bytes survive on disk once the
/// Keychain owns the key" guarantee. Otherwise mint a fresh key into
/// `primary` only (no plaintext file is ever created for a fresh pair).
///
/// Generic over `primary` (rather than hardcoding [`KeychainGroupKeyStore`])
/// so the migration algorithm itself gets full unit-test coverage without
/// touching the real OS keychain — `tesela-backup`'s `encrypt` module
/// deliberately avoids exercising the real Keychain in `cargo test` for
/// the same reason.
///
/// Only called from macOS's `not(test)` Keychain branch above (plus the
/// unit tests below on any platform) — dead on a non-macOS release build
/// (Linux self-host, iOS FFI), hence the `allow(dead_code)` there.
#[cfg_attr(not(any(test, target_os = "macos")), allow(dead_code))]
async fn load_or_migrate_key(
    primary: &dyn GroupKeyStore,
    legacy: &FileGroupKeyStore,
) -> SyncResult<GroupKey> {
    if let Some(k) = primary.load_key().await? {
        return Ok(k);
    }
    if let Some(k) = legacy.load_key().await? {
        primary.store_key(&k).await?;
        legacy.shred().await?;
        return Ok(k);
    }
    let k = GroupKey::random();
    primary.store_key(&k).await?;
    Ok(k)
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
    for (i, out_byte) in out.iter_mut().enumerate() {
        let hi = nibble(s.as_bytes()[i * 2])?;
        let lo = nibble(s.as_bytes()[i * 2 + 1])?;
        *out_byte = (hi << 4) | lo;
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

    /// L1 GKS — the seam round-trips and keeps the byte-for-byte on-disk
    /// format (raw 32 bytes at `group_key.bin`) so the keychain backend can
    /// fall back to it for one release without divergence.
    #[tokio::test]
    async fn file_store_round_trips() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = FileGroupKeyStore::new(tmp.path());

        // No key yet → None.
        assert!(store.load_key().await.unwrap().is_none());

        // Store → load returns identical bytes.
        let k = GroupKey::random();
        store.store_key(&k).await.unwrap();
        let loaded = store
            .load_key()
            .await
            .unwrap()
            .expect("key present after store");
        assert_eq!(loaded.as_bytes(), k.as_bytes());

        // On-disk format is exactly the legacy raw 32 bytes at group_key.bin.
        let raw = std::fs::read(tmp.path().join("group_key.bin")).unwrap();
        assert_eq!(raw.len(), 32);
        assert_eq!(&raw[..], k.as_bytes());
    }

    #[tokio::test]
    async fn file_store_shred_removes_and_zeroes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = FileGroupKeyStore::new(tmp.path());
        store.store_key(&GroupKey::random()).await.unwrap();
        let path = tmp.path().join("group_key.bin");
        assert!(path.exists());

        store.shred().await.unwrap();
        assert!(!path.exists());

        // Shredding an already-gone file is a no-op, not an error.
        store.shred().await.unwrap();
    }

    /// tesela-tp0.2 migrate-on-first-run: a pre-cutover install with a
    /// plaintext `group_key.bin` gets its key adopted into the platform
    /// store on first `load_or_create_group_key` (mirrored here by a
    /// stand-in `FileGroupKeyStore` in place of `KeychainGroupKeyStore`
    /// — the algorithm is store-agnostic, so this exercises the exact
    /// same code path without touching the real OS keychain), and the
    /// legacy file is shredded so the plaintext key doesn't linger.
    #[tokio::test]
    async fn migrate_adopts_legacy_file_and_shreds_it() {
        let legacy_dir = tempfile::TempDir::new().unwrap();
        let legacy = FileGroupKeyStore::new(legacy_dir.path());
        let k = GroupKey::random();
        legacy.store_key(&k).await.unwrap();

        let primary_dir = tempfile::TempDir::new().unwrap();
        let primary = FileGroupKeyStore::new(primary_dir.path());

        let migrated = load_or_migrate_key(&primary, &legacy).await.unwrap();
        assert_eq!(migrated.as_bytes(), k.as_bytes());
        assert_eq!(
            primary.load_key().await.unwrap().unwrap().as_bytes(),
            k.as_bytes()
        );
        assert!(
            !legacy_dir.path().join("group_key.bin").exists(),
            "legacy plaintext file must be shredded after a successful migration"
        );
    }

    /// No legacy file and no existing primary key → mint fresh into the
    /// primary store only; no plaintext file is ever created for a
    /// device that pairs fresh post-cutover.
    #[tokio::test]
    async fn migrate_mints_fresh_when_neither_store_has_a_key() {
        let primary_dir = tempfile::TempDir::new().unwrap();
        let legacy_dir = tempfile::TempDir::new().unwrap();
        let primary = FileGroupKeyStore::new(primary_dir.path());
        let legacy = FileGroupKeyStore::new(legacy_dir.path());

        let k = load_or_migrate_key(&primary, &legacy).await.unwrap();
        assert_eq!(
            primary.load_key().await.unwrap().unwrap().as_bytes(),
            k.as_bytes()
        );
        assert!(legacy.load_key().await.unwrap().is_none());
    }

    /// A primary store that already has a key (already migrated, or
    /// always-Keychain device) short-circuits — the legacy file, if any,
    /// is left untouched (no redundant shred of a file that was never
    /// the source of truth for this run).
    #[tokio::test]
    async fn migrate_prefers_existing_primary_key_over_legacy() {
        let primary_dir = tempfile::TempDir::new().unwrap();
        let legacy_dir = tempfile::TempDir::new().unwrap();
        let primary = FileGroupKeyStore::new(primary_dir.path());
        let legacy = FileGroupKeyStore::new(legacy_dir.path());
        let primary_key = GroupKey::random();
        primary.store_key(&primary_key).await.unwrap();
        let legacy_key = GroupKey::random();
        legacy.store_key(&legacy_key).await.unwrap();

        let k = load_or_migrate_key(&primary, &legacy).await.unwrap();
        assert_eq!(k.as_bytes(), primary_key.as_bytes());
        assert!(legacy.load_key().await.unwrap().is_some());
    }
}
