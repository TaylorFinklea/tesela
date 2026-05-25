//! UniFFI bridge crate exposing `tesela-sync` to Swift / Kotlin / Python.
//!
//! Phase 4.1 (iOS foundation) — narrow surface chosen to validate the
//! cross-compile + bindings pipeline before we expose the full engine.
//! Once the iPhone app's Settings → Devices screen needs more, we
//! expand the surface. The underlying `tesela-sync` types were written
//! FFI-clean (owned data, no borrows in public signatures, no generics
//! in trait methods), so each expansion is a mechanical wrap.

use std::sync::Arc;

use tesela_sync::{
    decode_pairing_code as decode_pairing_code_inner,
    encode_pairing_code as encode_pairing_code_inner,
    engine::SqliteEngine,
    transport::relay::RelayClient,
    DeviceId, GroupId, GroupKey,
    PairingCode as InnerPairingCode,
};

uniffi::setup_scaffolding!();

/// FFI-facing error variants. Stays narrow so consumers can match
/// exhaustively from Swift without surprises. All variants are
/// constructed from inner `SyncError`s via the `From` impl below.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FfiSyncError {
    /// The bytes the host handed us don't decode as a valid pairing
    /// code (bad base64, bad postcard, future version, etc.).
    #[error("invalid pairing code: {message}")]
    InvalidPairingCode {
        /// Free-text reason; safe to surface to users.
        message: String,
    },
    /// A wrapped error we couldn't more usefully classify yet.
    #[error("{message}")]
    Other {
        /// Free-text reason.
        message: String,
    },
}

impl From<tesela_sync::SyncError> for FfiSyncError {
    fn from(e: tesela_sync::SyncError) -> Self {
        FfiSyncError::Other {
            message: e.to_string(),
        }
    }
}

/// `tesela-sync` semantic version this bridge wraps. Cheap probe that
/// proves the FFI round-trip works before the harder calls go through.
#[uniffi::export]
pub fn tesela_sync_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Sync op-format version stamped onto every locally produced op.
/// Mirrors `tesela_sync::SYNC_SCHEMA_VERSION` so the Swift layer can
/// surface "version mismatch with desktop" before the engine does.
#[uniffi::export]
pub fn sync_schema_version() -> u32 {
    tesela_sync::SYNC_SCHEMA_VERSION
}

/// Generate a fresh random device id (UUIDv7, hex-encoded). Used on
/// first run of the iOS app to mint the device's identity.
#[uniffi::export]
pub fn generate_device_id_hex() -> String {
    DeviceId::new_random().to_hex()
}

/// Generate a fresh random group identity (id + 32-byte key). Returned
/// as hex strings so the Swift side stays in `String` territory
/// without binary-blob plumbing.
#[uniffi::export]
pub fn generate_group_identity() -> GroupIdentityRecord {
    let id = GroupId::new_random();
    let key = GroupKey::random();
    GroupIdentityRecord {
        group_id_hex: hex_encode(id.as_bytes()),
        group_key_hex: hex_encode(key.as_bytes()),
    }
}

/// Group identity in a Swift-friendly shape: two hex strings.
#[derive(Debug, Clone, uniffi::Record)]
pub struct GroupIdentityRecord {
    /// 32-char lowercase hex of the 16-byte group id.
    pub group_id_hex: String,
    /// 64-char lowercase hex of the 32-byte group key.
    pub group_key_hex: String,
}

/// Decoded view of a pairing code. Swift-friendly: all strings.
#[derive(Debug, Clone, uniffi::Record)]
pub struct PairingCodeRecord {
    /// 32-char hex group id (16 bytes).
    pub group_id_hex: String,
    /// 64-char hex group key (32 bytes).
    pub group_key_hex: String,
    /// 32-char hex device id of the issuing device.
    pub device_id_hex: String,
    /// Reachable HTTP URL (e.g. `http://10.0.0.5:7474`).
    pub url: String,
    /// User-visible display name from the issuer.
    pub display_name: String,
    /// Wire-format version; checked by `decode_pairing_code` already.
    pub version: u32,
}

/// Decode a base64url pairing code string into its fields. Returns
/// `FfiSyncError::InvalidPairingCode` on any decode failure so the
/// Swift UI can surface a clean "Couldn't read pairing code" message.
#[uniffi::export]
pub fn decode_pairing_code(code: String) -> Result<PairingCodeRecord, FfiSyncError> {
    let parsed = decode_pairing_code_inner(&code).map_err(|e| FfiSyncError::InvalidPairingCode {
        message: e.to_string(),
    })?;
    Ok(PairingCodeRecord {
        group_id_hex: hex_encode(parsed.group_id.as_bytes()),
        group_key_hex: hex_encode(&parsed.group_key_bytes),
        device_id_hex: parsed.device_id.to_hex(),
        url: parsed.url,
        display_name: parsed.display_name,
        version: parsed.version as u32,
    })
}

/// Mirror of `decode_pairing_code` for the producing side. Builds a
/// `PairingCode` from raw fields and returns the encoded string. The
/// Swift caller is responsible for supplying a real reachable URL
/// (the desktop's `build_public_url` logic doesn't apply to iPhone).
#[uniffi::export]
pub fn encode_pairing_code(
    group_id_hex: String,
    group_key_hex: String,
    device_id_hex: String,
    url: String,
    display_name: String,
) -> Result<String, FfiSyncError> {
    let group_id = parse_hex_16(&group_id_hex).ok_or_else(|| FfiSyncError::Other {
        message: format!("group_id_hex must be 32-char hex"),
    })?;
    let group_key = parse_hex_32(&group_key_hex).ok_or_else(|| FfiSyncError::Other {
        message: format!("group_key_hex must be 64-char hex"),
    })?;
    let device_id = parse_hex_16(&device_id_hex).ok_or_else(|| FfiSyncError::Other {
        message: format!("device_id_hex must be 32-char hex"),
    })?;
    let code = InnerPairingCode {
        group_id: GroupId::from_bytes(group_id),
        group_key_bytes: group_key,
        device_id: DeviceId::from_bytes(device_id),
        url,
        display_name,
        // The iOS FFI entry point doesn't yet take a relay URL; the
        // host-side path (`tesela-server` peer_sync handler) populates
        // this from `[sync.relay]` config. iOS UniFFI gains this
        // parameter when iOS becomes a sync peer (deferred multi-week
        // track); for now iOS generates LAN-only pairing codes.
        relay_url: None,
        version: tesela_sync::crypto::pairing::PAIRING_CODE_VERSION,
    };
    encode_pairing_code_inner(&code).map_err(FfiSyncError::from)
}

// --- helpers ---------------------------------------------------------------

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
    let bytes = parse_hex(s)?;
    bytes.try_into().ok()
}

fn parse_hex_32(s: &str) -> Option<[u8; 32]> {
    let bytes = parse_hex(s)?;
    bytes.try_into().ok()
}

fn parse_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for chunk in s.as_bytes().chunks_exact(2) {
        let hi = nibble(chunk[0])?;
        let lo = nibble(chunk[1])?;
        out.push((hi << 4) | lo);
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

// ============================================================================
// B.1.1 — SyncEngineHandle (minimal — open + device_hex)
// ============================================================================

/// Handle to a SQLite-backed sync engine. Created with
/// [`SyncEngineHandle::open`]; lives behind an `Arc` so multiple Swift
/// callers (UI + background sync task) can hold references concurrently
/// without copying engine state.
///
/// What the iOS app gets out of this in B.1: just enough to prove the
/// FFI pipeline carries an opened engine round-trip. The producer +
/// consumer methods (`apply_changes`, `produce_changes_since`) are wired
/// in B.2 / B.3 — exposing them ahead of time would force a serialized
/// `SyncEnvelope` shape across the FFI boundary before we've nailed
/// down how iOS consumes ops.
#[derive(uniffi::Object)]
pub struct SyncEngineHandle {
    inner: Arc<SqliteEngine>,
}

#[uniffi::export(async_runtime = "tokio")]
impl SyncEngineHandle {
    /// Open (or create) a SQLite-backed sync engine at the given URL.
    ///
    /// `sqlite_url` follows the sqlx convention: `sqlite:/path/to/file`
    /// (the leading slash makes it absolute). iOS callers typically
    /// pass `format!("sqlite:{}", url.path())` where `url` is a
    /// `FileManager`-derived path inside the app's sandbox.
    ///
    /// `device_id_hex` must be 32 lowercase hex chars — typically the
    /// output of [`generate_device_id_hex`] persisted across launches.
    /// Reusing a stable device id is what keeps HLC timestamps
    /// monotonic across app restarts.
    #[uniffi::constructor]
    pub async fn open(
        sqlite_url: String,
        device_id_hex: String,
    ) -> Result<Arc<Self>, FfiSyncError> {
        let bytes = parse_hex_16(&device_id_hex).ok_or_else(|| FfiSyncError::Other {
            message: "device_id_hex must be 32 hex chars".to_string(),
        })?;
        let device = DeviceId::from_bytes(bytes);
        let engine = SqliteEngine::open(&sqlite_url, device)
            .await
            .map_err(FfiSyncError::from)?;
        Ok(Arc::new(Self {
            inner: Arc::new(engine),
        }))
    }

    /// 32-char hex of this engine's device id. The Swift coordinator
    /// reads this once at boot for display in Settings → Sync.
    pub fn device_hex(&self) -> String {
        self.inner.device().to_hex()
    }
}

// ============================================================================
// B.1.2 — RelayClientHandle (register + verify + poll-count probe)
// ============================================================================

/// Handle to a [`RelayClient`] over UniFFI. Owns its own `reqwest`
/// HTTP client + the HKDF-derived auth key. Swift constructs one per
/// `(relay_url, group)` pair — typically just one per running app.
///
/// In B.1 we expose register / verify / a poll-count probe. The
/// envelope-bearing methods (`put_envelope`, full `poll` with payload)
/// arrive in B.2/B.3 alongside engine apply.
#[derive(uniffi::Object)]
pub struct RelayClientHandle {
    inner: RelayClient,
}

#[uniffi::export(async_runtime = "tokio")]
impl RelayClientHandle {
    /// Construct a relay client. All four hex strings are validated
    /// before any network traffic is attempted; a malformed input
    /// surfaces as `FfiSyncError::Other` rather than a later opaque
    /// network error.
    #[uniffi::constructor]
    pub fn new(
        relay_url: String,
        group_id_hex: String,
        device_id_hex: String,
        group_key_hex: String,
    ) -> Result<Arc<Self>, FfiSyncError> {
        let url = reqwest::Url::parse(&relay_url).map_err(|e| FfiSyncError::Other {
            message: format!("invalid relay URL: {e}"),
        })?;
        let group_id = GroupId::from_bytes(parse_hex_16(&group_id_hex).ok_or_else(|| {
            FfiSyncError::Other {
                message: "group_id_hex must be 32 hex chars".into(),
            }
        })?);
        let device_id = DeviceId::from_bytes(parse_hex_16(&device_id_hex).ok_or_else(|| {
            FfiSyncError::Other {
                message: "device_id_hex must be 32 hex chars".into(),
            }
        })?);
        let group_key = GroupKey::from_bytes(parse_hex_32(&group_key_hex).ok_or_else(|| {
            FfiSyncError::Other {
                message: "group_key_hex must be 64 hex chars".into(),
            }
        })?);
        Ok(Arc::new(Self {
            inner: RelayClient::new(url, group_id, device_id, group_key),
        }))
    }

    /// Register on the relay, recovering an existing matching record
    /// if one exists. Returns the Unix-seconds timestamp pinned to the
    /// registration — the Swift coordinator persists this so subsequent
    /// `register_or_recover()` calls find the same record on the relay
    /// without us having to chase the clock.
    pub async fn register_or_recover(&self) -> Result<i64, FfiSyncError> {
        self.inner
            .register_or_recover()
            .await
            .map_err(FfiSyncError::from)
    }

    /// Hijack-detection check: read back the relay's stored
    /// registration for this group and verify the signed intent against
    /// our group key. Returns Ok(()) when the registration was authored
    /// by a holder of our group key; Err otherwise (someone squatted
    /// the group id but couldn't produce a valid intent signature).
    pub async fn verify_registration(&self) -> Result<(), FfiSyncError> {
        self.inner
            .verify_registration()
            .await
            .map_err(FfiSyncError::from)
    }

    /// Probe: poll for envelopes since `since_seq` and return how many
    /// are pending plus the highest seq seen. Used by the B.1.4 smoke
    /// probe to confirm two-way traffic without yet doing the apply
    /// work. The full envelope-bearing poll lands in B.2.
    pub async fn poll_count(&self, since_seq: i64) -> Result<PollProbeRecord, FfiSyncError> {
        let envelopes = self
            .inner
            .poll(since_seq)
            .await
            .map_err(FfiSyncError::from)?;
        let highest = envelopes
            .iter()
            .map(|(seq, _)| *seq)
            .max()
            .unwrap_or(since_seq);
        Ok(PollProbeRecord {
            count: envelopes.len() as u32,
            highest_seq: highest,
        })
    }
}

/// Probe-only return shape — see [`RelayClientHandle::poll_count`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct PollProbeRecord {
    /// Number of envelopes the relay returned (≤ relay page size).
    pub count: u32,
    /// Highest seq in the returned batch, or `since_seq` when empty.
    pub highest_seq: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_string_is_non_empty() {
        assert!(!tesela_sync_version().is_empty());
    }

    #[test]
    fn generate_device_id_is_32_hex() {
        let s = generate_device_id_hex();
        assert_eq!(s.len(), 32);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn group_identity_lengths() {
        let g = generate_group_identity();
        assert_eq!(g.group_id_hex.len(), 32);
        assert_eq!(g.group_key_hex.len(), 64);
    }

    #[test]
    fn pairing_code_round_trip() {
        let g = generate_group_identity();
        let device = generate_device_id_hex();
        let code = encode_pairing_code(
            g.group_id_hex.clone(),
            g.group_key_hex.clone(),
            device.clone(),
            "http://10.0.0.1:7474".to_string(),
            "Test iPhone".to_string(),
        )
        .unwrap();
        let back = decode_pairing_code(code).unwrap();
        assert_eq!(back.group_id_hex, g.group_id_hex);
        assert_eq!(back.group_key_hex, g.group_key_hex);
        assert_eq!(back.device_id_hex, device);
        assert_eq!(back.url, "http://10.0.0.1:7474");
        assert_eq!(back.display_name, "Test iPhone");
    }

    #[test]
    fn decode_rejects_garbage() {
        let err = decode_pairing_code("not real $$$".to_string()).unwrap_err();
        match err {
            FfiSyncError::InvalidPairingCode { .. } => {}
            other => panic!("wrong error variant: {other:?}"),
        }
    }
}
