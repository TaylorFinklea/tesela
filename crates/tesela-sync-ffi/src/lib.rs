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
    engine::{PeerCursor, SqliteEngine, SyncEngine},
    oplog::op::OpPayload,
    transport::relay::RelayClient,
    DeviceId, GroupId, GroupKey, SyncEnvelope,
    PairingCode as InnerPairingCode,
};
use tokio::sync::Mutex;

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
    /// Reachable HTTP URL of the issuing tesela-server (e.g.
    /// `http://10.0.0.5:7474`).
    pub url: String,
    /// User-visible display name from the issuer.
    pub display_name: String,
    /// Wire-format version; checked by `decode_pairing_code` already.
    pub version: u32,
    /// WAN relay URL the issuer is configured against, if any.
    /// `None` ≡ the issuer is LAN-only. When set, the joining device
    /// should auto-configure the same relay so cross-network sync
    /// works without an extra copy-paste. Populated since pairing
    /// code v2 (2026-05-24).
    pub relay_url: Option<String>,
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
        relay_url: parsed.relay_url,
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

/// Mirror of `tesela-server::routes::notes::stable_uuid_from_slug`:
/// blake3-hash the slug, take the first 16 bytes as the note's stable
/// 128-bit id. iOS uses this so its NoteUpsert ops land on the same
/// note id Mac would have minted from the same slug, instead of
/// creating an orphan.
fn stable_uuid_from_slug(slug: &str) -> [u8; 16] {
    let hash = blake3::hash(slug.as_bytes());
    let bytes = hash.as_bytes();
    let mut out = [0u8; 16];
    out.copy_from_slice(&bytes[..16]);
    out
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
    /// Open (or create) a SQLite-backed sync engine at the given URL,
    /// without filesystem materialization. Applied ops land in the
    /// oplog but no `.md` files are written. Useful for headless
    /// engines or tests; **iOS should normally use
    /// [`Self::open_with_mosaic`] instead** so apply ticks
    /// materialize into the iOS app sandbox.
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

    /// Like [`Self::open`] but ALSO knows about a mosaic root directory,
    /// so applied ops materialize into `<mosaic_path>/notes/<slug>.md`.
    /// This is the iOS production shape: pass the app sandbox's
    /// Documents/<mosaic-name>/ as `mosaic_path` and the engine takes
    /// care of writing the on-disk notes for the indexer (and the iOS
    /// data layer) to read.
    ///
    /// `mosaic_path` must be an absolute filesystem path; the engine
    /// creates `mosaic_path/notes/` if missing.
    #[uniffi::constructor]
    pub async fn open_with_mosaic(
        sqlite_url: String,
        mosaic_path: String,
        device_id_hex: String,
    ) -> Result<Arc<Self>, FfiSyncError> {
        let bytes = parse_hex_16(&device_id_hex).ok_or_else(|| FfiSyncError::Other {
            message: "device_id_hex must be 32 hex chars".to_string(),
        })?;
        let device = DeviceId::from_bytes(bytes);
        let engine = SqliteEngine::open_with_mosaic(
            &sqlite_url,
            Some(std::path::PathBuf::from(&mosaic_path)),
            device,
        )
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

    /// Slug-flavoured variant of [`Self::record_note_upsert`]. Computes
    /// the note id with the same blake3-truncation Mac's server uses
    /// (`stable_uuid_from_slug` in `tesela-server::routes::notes`), so
    /// iOS-authored ops land on the same note id Mac would have
    /// assigned. Keeps the Swift caller from having to mirror that
    /// hash logic.
    ///
    /// `created_at_millis` should be a stable timestamp from the
    /// note's first creation. Reusing the same value on every edit
    /// keeps the engine's HLC ordering monotonic.
    pub async fn record_note_upsert_by_slug(
        &self,
        slug: String,
        title: String,
        content: String,
        created_at_millis: i64,
    ) -> Result<String, FfiSyncError> {
        let note_id = stable_uuid_from_slug(&slug);
        let payload = OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(slug.clone()),
            title,
            content,
            created_at_millis,
        };
        let hash = self
            .inner
            .record_local(payload)
            .await
            .map_err(FfiSyncError::from)?;
        Ok(hex_encode(&hash.0))
    }

    /// Record a "create or update a note" op locally. Returns the
    /// resulting 64-char hex content hash that the engine assigned —
    /// callers can use it to dedupe their UI's optimistic write against
    /// the eventual relay-replayed op.
    ///
    /// `note_id_hex` must be 32 hex chars (16 bytes — UUID). The Swift
    /// caller mints one (e.g. `UUID().uuidString` stripped of dashes)
    /// on first save and reuses it for subsequent edits to the same note.
    /// `created_at_millis` is Unix millis at first creation; reused on
    /// updates so the engine's HLC stays monotonic across both edits.
    pub async fn record_note_upsert(
        &self,
        note_id_hex: String,
        display_alias: Option<String>,
        title: String,
        content: String,
        created_at_millis: i64,
    ) -> Result<String, FfiSyncError> {
        let note_id = parse_hex_16(&note_id_hex).ok_or_else(|| FfiSyncError::Other {
            message: "note_id_hex must be 32 hex chars".into(),
        })?;
        let payload = OpPayload::NoteUpsert {
            note_id,
            display_alias,
            title,
            content,
            created_at_millis,
        };
        let hash = self
            .inner
            .record_local(payload)
            .await
            .map_err(FfiSyncError::from)?;
        Ok(hex_encode(&hash.0))
    }
}

// ============================================================================
// B.2.1 — SyncCoordinator (engine + relay + cursor; outbound tick)
// ============================================================================

/// Coordinator that owns a (SyncEngine, RelayClient, group identity) tuple
/// and ticks the outbound half of the sync loop. Mirrors the
/// `tesela_server::sync_relay::tick` outbound branch — but exposed over
/// UniFFI so iOS Swift can drive it directly without re-implementing the
/// produce → postcard → wrap → put choreography.
///
/// In B.2 we only expose the outbound tick (iPhone-side writes flow to
/// the Mac). Inbound (Mac writes flow to iPhone) lands in B.3 alongside
/// the apply path + materialization.
///
/// The outbound cursor is held in-memory only for B.2 — restart starts
/// from `Earliest`, which re-sends already-acked ops harmlessly thanks
/// to the relay's nonce dedupe + receiver-side idempotent apply. B.3
/// will persist it through `RelayState`-equivalent storage.
#[derive(uniffi::Object)]
pub struct SyncCoordinator {
    engine: Arc<SyncEngineHandle>,
    relay: Arc<RelayClientHandle>,
    group_id: GroupId,
    /// Outbound cursor (HLC ntp64 of last sent op), `None` ≡ Earliest.
    outbound_cursor: Mutex<Option<i64>>,
    /// Inbound cursor (highest relay-assigned `seq` we've applied + acked).
    /// `0` ≡ nothing applied yet (the relay's first seq is `1`).
    inbound_cursor: Mutex<i64>,
}

#[uniffi::export(async_runtime = "tokio")]
impl SyncCoordinator {
    /// Wire an engine + relay client + group identity together. The
    /// engine + relay handles are reference-counted so Swift can keep
    /// holding them for direct calls (e.g. `engine.device_hex()`,
    /// `relay.poll_count(...)`) without surrendering ownership.
    ///
    /// `group_id_hex` must match the group the relay was registered
    /// against. Mismatch surfaces at first `tick_outbound` as a relay
    /// MAC-verify failure.
    #[uniffi::constructor]
    pub fn new(
        engine: Arc<SyncEngineHandle>,
        relay: Arc<RelayClientHandle>,
        group_id_hex: String,
    ) -> Result<Arc<Self>, FfiSyncError> {
        let group_id = GroupId::from_bytes(parse_hex_16(&group_id_hex).ok_or_else(|| {
            FfiSyncError::Other {
                message: "group_id_hex must be 32 hex chars".into(),
            }
        })?);
        Ok(Arc::new(Self {
            engine,
            relay,
            group_id,
            outbound_cursor: Mutex::new(None),
            inbound_cursor: Mutex::new(0),
        }))
    }

    /// Drain locally-recorded ops that the relay hasn't seen yet,
    /// postcard-encode them into a `SyncEnvelope`, AEAD-seal via the
    /// relay client, and PUT. Returns a small outcome record the iOS
    /// caller can show in the UI ("sent N ops, seq=…").
    ///
    /// Idempotent on no-op: if there's nothing to send, returns
    /// `ops_sent: 0` without touching the relay.
    ///
    /// Errors fall into two buckets and the caller should treat them
    /// differently:
    /// - Network/relay errors (timeouts, 4xx, MAC fail) → don't advance
    ///   the cursor; the next tick will retry the same batch.
    /// - Engine errors (db corruption, encoding) → the cursor stays put
    ///   but Swift may need to surface them to the user as a sync halt.
    pub async fn tick_outbound(
        &self,
        max_bytes: u32,
    ) -> Result<TickOutboundRecord, FfiSyncError> {
        let our_device = self.engine.inner.device();
        let cursor_guard = self.outbound_cursor.lock().await;
        let cursor = match *cursor_guard {
            Some(ntp) => PeerCursor::At(
                tesela_sync::hlc::HlcTimestamp::from_ntp64_i64(ntp, our_device),
            ),
            None => PeerCursor::Earliest,
        };
        drop(cursor_guard);

        // Relay fanout: publish only ops we authored. See the docstring
        // on `produce_local_authored_since` for why transitive
        // republishing is wrong here.
        let _ = our_device; // kept for cursor construction above
        let batch = self
            .engine
            .inner
            .produce_local_authored_since(cursor, max_bytes as usize)
            .await
            .map_err(FfiSyncError::from)?;
        if batch.ops.is_empty() {
            return Ok(TickOutboundRecord {
                ops_sent: 0,
                relay_seq: None,
                new_cursor_ntp: None,
            });
        }

        let ciphertext = postcard::to_allocvec(&batch.ops).map_err(|e| FfiSyncError::Other {
            message: format!("postcard encode: {e}"),
        })?;
        let envelope = SyncEnvelope {
            from_device: our_device,
            to_group: self.group_id,
            // Unused on this layer — RelayClient.put_envelope mints its
            // own outer nonce for the AEAD seal. Kept zero to match the
            // Mac's tick.
            nonce: [0u8; 24],
            ciphertext,
        };

        let ops_count = batch.ops.len() as u32;
        let (seq, _ts) = self
            .relay
            .inner
            .put_envelope(envelope)
            .await
            .map_err(FfiSyncError::from)?;

        // Advance cursor only on full success.
        let new_cursor_ntp = if let PeerCursor::At(ts) = batch.new_cursor {
            let ntp = ts.ntp64_as_i64();
            *self.outbound_cursor.lock().await = Some(ntp);
            Some(ntp)
        } else {
            None
        };

        Ok(TickOutboundRecord {
            ops_sent: ops_count,
            relay_seq: Some(seq),
            new_cursor_ntp,
        })
    }

    /// Current outbound cursor (ntp64) — `None` means nothing has been
    /// sent yet this run. Surfaced for the dev smoke UI.
    pub async fn outbound_cursor_ntp(&self) -> Option<i64> {
        *self.outbound_cursor.lock().await
    }

    /// Drain incoming envelopes from the relay since the last applied
    /// `seq`, decrypt + decode each, apply via the engine (which
    /// materializes the resulting NoteUpsert/etc into the iOS sandbox
    /// when the engine was opened via [`SyncEngineHandle::open_with_mosaic`]),
    /// then ack the highest applied seq back to the relay.
    ///
    /// Self-echo handling: envelopes whose `from_device == our_device`
    /// are skipped at the apply step but still advance the cursor — the
    /// relay broadcasts to all members including the author, and
    /// re-applying our own writes would burn cycles for no effect.
    ///
    /// Failure modes — same shape as `tick_outbound`:
    /// - Network errors → cursor untouched; next tick retries the same
    ///   batch (relay's idempotent storage + engine's content-hash
    ///   dedupe make this safe).
    /// - Per-envelope apply errors are logged + counted but do NOT
    ///   abort the whole tick; bad envelopes from one device shouldn't
    ///   stop us from applying good envelopes from another.
    pub async fn tick_inbound(&self) -> Result<TickInboundRecord, FfiSyncError> {
        let our_device = self.engine.inner.device();
        let since = *self.inbound_cursor.lock().await;

        let envelopes = self
            .relay
            .inner
            .poll(since)
            .await
            .map_err(FfiSyncError::from)?;

        let mut applied = 0u32;
        let mut skipped_own = 0u32;
        let mut errors = 0u32;
        let mut max_seq = since;
        for (seq, env) in envelopes {
            if env.from_device == our_device {
                // Our own write echoed back by the relay; advance the
                // cursor but skip the apply.
                if seq > max_seq {
                    max_seq = seq;
                }
                skipped_own += 1;
                continue;
            }
            let peer = env.from_device;
            match SyncEngine::apply_changes(self.engine.inner.as_ref(), peer, env).await {
                Ok(_applied_set) => {
                    applied += 1;
                    if seq > max_seq {
                        max_seq = seq;
                    }
                }
                Err(e) => {
                    errors += 1;
                    eprintln!("tesela-sync-ffi: relay apply seq={seq} err={e}");
                }
            }
        }

        if max_seq > since {
            // Ack first, then advance our cursor. If ack fails the next
            // tick re-polls the same range — harmless thanks to engine
            // content-hash dedupe.
            if let Err(e) = self.relay.inner.ack(max_seq).await {
                eprintln!("tesela-sync-ffi: relay ack({max_seq}) failed: {e}");
            }
            *self.inbound_cursor.lock().await = max_seq;
        }

        Ok(TickInboundRecord {
            applied,
            skipped_own,
            errors,
            new_cursor_seq: max_seq,
        })
    }

    /// Current inbound cursor (relay seq) — `0` means nothing has been
    /// applied yet this run.
    pub async fn inbound_cursor_seq(&self) -> i64 {
        *self.inbound_cursor.lock().await
    }

    /// Restore the inbound cursor from prior-session persistence.
    /// Idempotent and clamping: a request to move BACKWARDS is
    /// ignored (the engine has already applied past that point and
    /// re-applying is a waste of bandwidth, even though it's safe
    /// thanks to content-hash dedupe).
    pub async fn set_inbound_cursor_seq(&self, seq: i64) {
        let mut guard = self.inbound_cursor.lock().await;
        if seq > *guard {
            *guard = seq;
        }
    }

    /// Restore the outbound cursor (HLC ntp64) from prior-session
    /// persistence. Same clamping rule as the inbound setter — won't
    /// move backwards.
    pub async fn set_outbound_cursor_ntp(&self, ntp: i64) {
        let mut guard = self.outbound_cursor.lock().await;
        if guard.map(|cur| ntp > cur).unwrap_or(true) {
            *guard = Some(ntp);
        }
    }
}

/// Outcome of [`SyncCoordinator::tick_inbound`]. Designed to be small
/// enough to render in a one-line status string.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TickInboundRecord {
    /// Envelopes that were decrypted, decoded, and successfully
    /// applied via the engine.
    pub applied: u32,
    /// Envelopes the relay echoed back to us (we authored them
    /// originally). Cursor still advances over these but the apply is
    /// skipped.
    pub skipped_own: u32,
    /// Envelopes whose apply failed. Logged but don't abort the tick.
    pub errors: u32,
    /// Highest relay-assigned seq seen in this batch. Same as the
    /// updated inbound cursor on a successful tick.
    pub new_cursor_seq: i64,
}

/// Outcome of [`SyncCoordinator::tick_outbound`]. Designed to be small
/// enough to render in a one-line status string.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TickOutboundRecord {
    /// Number of ops included in the envelope. `0` ≡ nothing to send.
    pub ops_sent: u32,
    /// Relay-assigned seq of the envelope, or `None` when nothing was
    /// sent.
    pub relay_seq: Option<i64>,
    /// HLC ntp64 of the new outbound cursor, or `None` when nothing
    /// was sent (or the produced batch wasn't `At`-cursor-shaped).
    pub new_cursor_ntp: Option<i64>,
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
