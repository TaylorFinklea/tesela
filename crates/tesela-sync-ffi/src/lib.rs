//! UniFFI bridge crate exposing `tesela-sync` to Swift / Kotlin / Python.
//!
//! Phase 4.1 (iOS foundation) — narrow surface chosen to validate the
//! cross-compile + bindings pipeline before we expose the full engine.
//! Once the iPhone app's Settings → Devices screen needs more, we
//! expand the surface. The underlying `tesela-sync` types were written
//! FFI-clean (owned data, no borrows in public signatures, no generics
//! in trait methods), so each expansion is a mechanical wrap.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tesela_sync::{
    decode_loro_relay_payload, decode_pairing_code as decode_pairing_code_inner,
    encode_loro_relay_payload, encode_pairing_code as encode_pairing_code_inner,
    engine::SyncEngine, oplog::op::OpPayload, transport::relay::RelayClient, DeviceId, GroupId,
    GroupKey, Hlc, LoroDocUpdate, LoroEngine, PairingCode as InnerPairingCode, SyncEnvelope,
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

/// Outcome of [`SyncEngineHandle::apply_delta_frame`]. Beyond the count of
/// per-note updates applied, it reports whether ANY update was left PENDING by
/// Loro (a causal gap — the device is on a disjoint lineage / missing deps) and
/// the note id(s) the frame carried, so the caller can trigger an
/// authoritative-snapshot catch-up for exactly those notes.
#[derive(Debug, Clone, uniffi::Record)]
pub struct DeltaApplyOutcome {
    /// Number of per-note updates decoded + applied from the frame.
    pub applied: u32,
    /// `true` when at least one update was left PENDING (missing
    /// dependencies) — the signal to catch up the affected note(s) via
    /// [`SyncEngineHandle::import_note_snapshot`].
    pub needs_catchup: bool,
    /// Hex (32-char) note ids carried by the frame, so the caller knows
    /// which note(s) to request a snapshot for.
    pub note_ids_hex: Vec<String>,
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
    let parsed =
        decode_pairing_code_inner(&code).map_err(|e| FfiSyncError::InvalidPairingCode {
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

/// Parse a block id from EITHER a 32-char dashless hex string OR a 36-char
/// dashed UUID (`019e7a50-4404-...`). Web + iOS block ids are dashed UUIDs,
/// while the engine stores/`hex_id`s them dashless — so the FFI accepts both
/// by stripping dashes before the 16-byte hex parse. `None` on any other
/// shape (wrong length, non-hex chars).
fn parse_block_id_hex(s: &str) -> Option<[u8; 16]> {
    if s.contains('-') {
        let stripped: String = s.chars().filter(|c| *c != '-').collect();
        parse_hex_16(&stripped)
    } else {
        parse_hex_16(s)
    }
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

/// Handle to the authoritative Loro sync engine. Created with
/// [`SyncEngineHandle::open_loro`]; lives behind an `Arc` so multiple Swift
/// callers (UI + background sync task) can hold references concurrently
/// without copying engine state.
///
/// Post-flag-day (2026-05-29) the only constructor is `open_loro`; the
/// legacy SQLite constructors (`open` / `open_with_mosaic`) were removed
/// with the SqliteEngine stack. Sync flows through the Loro relay-update
/// methods, not the retired op-replay path.
#[derive(uniffi::Object)]
pub struct SyncEngineHandle {
    /// The backing engine — the authoritative `LoroEngine`, held behind the
    /// `SyncEngine` trait so the coordinator + write path stay engine-agnostic.
    inner: Arc<dyn SyncEngine>,
    /// The `notes/` directory the engine materializes `<slug>.md` into.
    /// `record_note_diff` reads the prior content from here.
    notes_dir: Option<PathBuf>,
}

#[uniffi::export(async_runtime = "tokio")]
impl SyncEngineHandle {
    /// Open an authoritative **LoroEngine** for iOS (the Loro cutover).
    /// LoroEngine becomes the sole writer: it materializes
    /// `<mosaic_path>/notes/<slug>.md` on every applied change and drives
    /// the relay with the v2 (TLR2) Loro payload. Per-note Loro snapshots
    /// persist under `<mosaic_path>/.tesela/loro/` so cold launches load
    /// from snapshot instead of replaying. The read path (the iOS data
    /// layer reading sandbox `.md` files) is unchanged — Loro just owns
    /// the writes now.
    ///
    /// `mosaic_path` must be absolute (the app sandbox's mosaic dir);
    /// `device_id_hex` is the stable per-device id ([`generate_device_id_hex`]
    /// persisted across launches) — its bytes seed the Loro PeerID, the
    /// prerequisite for clean cross-device merge.
    #[uniffi::constructor]
    pub async fn open_loro(
        mosaic_path: String,
        device_id_hex: String,
    ) -> Result<Arc<Self>, FfiSyncError> {
        let bytes = parse_hex_16(&device_id_hex).ok_or_else(|| FfiSyncError::Other {
            message: "device_id_hex must be 32 hex chars".to_string(),
        })?;
        let device = DeviceId::from_bytes(bytes);
        let mosaic = PathBuf::from(&mosaic_path);
        let notes_dir = mosaic.join("notes");
        let snapshot_dir = mosaic.join(".tesela").join("loro");
        let engine = LoroEngine::with_dirs(
            device,
            Arc::new(Hlc::new(device)),
            snapshot_dir,
            Some(notes_dir.clone()),
        )
        .await
        .map_err(FfiSyncError::from)?;
        Ok(Arc::new(Self {
            inner: Arc::new(engine),
            notes_dir: Some(notes_dir),
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

    /// Block-granular variant of `record_note_upsert_by_slug`. Diffs
    /// the new body against the engine's last-materialized version of
    /// the note (read from `<mosaic>/notes/<slug>.md`) and emits
    /// `BlockUpsert` / `BlockMove` / `BlockDelete` ops for what
    /// actually changed, instead of a single whole-file `NoteUpsert`.
    ///
    /// **Why this matters for sync convergence.** When iOS pushes a
    /// `NoteUpsert(content: full_body)` via the relay and another
    /// peer (web, Mac, second iPhone) has independently edited a
    /// *different* block of the same note between iOS's read and
    /// iOS's push, the wholesale apply on the receiver overwrites
    /// the other peer's block. With block-granular ops, the two
    /// edits target distinct block ids and converge correctly.
    ///
    /// Returns the number of ops emitted (`0` on no-op, `1` for the
    /// first-author NoteUpsert (note never materialized on this device)
    /// or the frontmatter-only fallback NoteUpsert, otherwise one per
    /// block change). The 64-char hex content hash that
    /// `record_note_upsert_by_slug` returned isn't useful here since
    /// multiple ops may be emitted; callers that need de-dup tracking
    /// should hash the new body themselves.
    ///
    /// `title` + `created_at_millis` are only consulted for the
    /// frontmatter-only fallback `NoteUpsert` path (the block ops
    /// don't carry them). The fallback fires when the parsed block
    /// tree is identical but the raw content differs — e.g. a
    /// frontmatter `tags:` change with no block edits. That path is
    /// data-lossy under concurrent edits to the same note, matching
    /// the server-side `record_sync_update` behaviour documented in
    /// `crates/tesela-server/src/routes/notes.rs::record_sync_update`.
    pub async fn record_note_diff(
        &self,
        slug: String,
        new_content: String,
        title: String,
        created_at_millis: i64,
    ) -> Result<u32, FfiSyncError> {
        use tesela_core::note_tree::parse_note;
        use tesela_sync::diff::diff_note_trees;

        let note_id = stable_uuid_from_slug(&slug);

        // Previous content is whatever the engine last materialized
        // for this note. Read it straight from disk — `record_local`
        // updates the file via `materialize` on every accepted op, so
        // disk reflects the engine's view. Missing file → first-author
        // NoteUpsert below (seeds identity + blocks in one op).
        let prev_content = match self.notes_dir.as_ref() {
            Some(notes) => {
                let path = notes.join(format!("{slug}.md"));
                tokio::fs::read_to_string(&path).await.unwrap_or_default()
            }
            None => String::new(),
        };

        // First-author path (2026-06-10): the note was never materialized on
        // this device — either the doc doesn't exist yet, or it exists but
        // has no root identity (a doc created purely from block ops carries
        // no `root.slug`, so the engine logs "cannot materialize — no slug"
        // and the file never appears; refresh then shows nothing and peers
        // receive a slug-less doc). Record ONE full NoteUpsert instead of
        // bare block ops: it seeds slug/title/frontmatter AND the block tree
        // from the content, and its apply is a NON-destructive per-bid
        // reconcile (deleted-wins; absent blocks untouched), so a doc that
        // is already resident with peer content is only ever ADDED to.
        if prev_content.is_empty() {
            let payload = OpPayload::NoteUpsert {
                note_id,
                display_alias: Some(slug),
                title,
                content: new_content,
                created_at_millis,
            };
            self.inner
                .record_local(payload)
                .await
                .map_err(FfiSyncError::from)?;
            return Ok(1);
        }

        let old_tree = parse_note(&prev_content);
        let new_tree = parse_note(&new_content);
        let ops = diff_note_trees(note_id, &old_tree, &new_tree);

        if ops.is_empty() {
            if prev_content == new_content {
                return Ok(0);
            }
            // Parsed tree identical but raw bytes differ (frontmatter
            // change). Fall back to NoteUpsert — same fallback the
            // server side uses.
            let payload = OpPayload::NoteUpsert {
                note_id,
                display_alias: Some(slug),
                title,
                content: new_content,
                created_at_millis,
            };
            self.inner
                .record_local(payload)
                .await
                .map_err(FfiSyncError::from)?;
            return Ok(1);
        }

        let count = ops.len() as u32;
        for op in ops {
            self.inner
                .record_local(op)
                .await
                .map_err(FfiSyncError::from)?;
        }
        Ok(count)
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

    /// Produce the live Loro delta for a just-changed note, framed as a
    /// single TLR2 relay frame ready to push over the instant-multidevice
    /// WebSocket. Computes the note id with the same blake3-truncation
    /// (`stable_uuid_from_slug`) the rest of this bridge uses, exports the
    /// per-doc update via the engine's **cursor-free** `export_doc_update`,
    /// and wraps it in the same TLR2 framing the relay payload uses, so the
    /// WS and relay carry byte-identical frames.
    ///
    /// `since_vv` is a peer's encoded version vector — pass the value a
    /// prior [`Self::note_version`] handed back so we export only the delta
    /// newer than what the peer already has. `since_vv = None` means "full
    /// compact snapshot" (the bootstrap a freshly-joined device needs).
    ///
    /// **Cursor-free by construction.** `export_doc_update` does NOT read or
    /// advance the relay's broadcast cursor (instant-multidevice spec,
    /// finding #3), so driving the WS through this method never contends
    /// with the relay producer (`SyncCoordinator::tick_outbound`) — the
    /// relay path still sees the note as pending. Do NOT route this through
    /// `produce_relay_updates`; that path is cursor-bound.
    ///
    /// Returns `Ok(None)` when the doc isn't resident (nothing to send),
    /// `Ok(Some(frame))` with the TLR2-framed bytes otherwise.
    pub async fn produce_note_delta(
        &self,
        slug: String,
        since_vv: Option<Vec<u8>>,
    ) -> Result<Option<Vec<u8>>, FfiSyncError> {
        let note_id = stable_uuid_from_slug(&slug);
        let Some(update_bytes) = self
            .inner
            .export_doc_update(note_id, since_vv.as_deref())
            .await
        else {
            return Ok(None);
        };
        let frame = encode_loro_relay_payload(&[LoroDocUpdate {
            doc: note_id,
            update_bytes,
        }])
        .map_err(FfiSyncError::from)?;
        Ok(Some(frame))
    }

    /// Apply a TLR2-framed delta frame received over the instant-multidevice
    /// WebSocket. Decodes the TLR2 payload and imports each per-note Loro
    /// update via the engine (which is commutative + idempotent, so
    /// duplicate / out-of-order frames are safe, and materializes the
    /// resulting `<slug>.md` into the iOS sandbox). Returns the number of
    /// per-note updates applied.
    ///
    /// A frame that lacks the TLR2 magic (a legacy v1 payload or foreign
    /// data) decodes to `None`; we return an empty outcome rather than
    /// erroring so the caller can skip it. A genuine decode failure (corrupt
    /// TLR2 body) surfaces as `FfiSyncError`.
    ///
    /// Returns a [`DeltaApplyOutcome`]: the count applied, whether ANY update
    /// was left PENDING by Loro (`needs_catchup` — a causal gap signalling a
    /// disjoint lineage), and the note ids the frame carried, so the caller can
    /// request an authoritative snapshot for exactly those notes when a live
    /// delta couldn't fully integrate.
    pub async fn apply_delta_frame(
        &self,
        frame: Vec<u8>,
    ) -> Result<DeltaApplyOutcome, FfiSyncError> {
        let Some(updates) = decode_loro_relay_payload(&frame).map_err(FfiSyncError::from)? else {
            return Ok(DeltaApplyOutcome {
                applied: 0,
                needs_catchup: false,
                note_ids_hex: Vec::new(),
            });
        };
        let mut applied = 0u32;
        let mut needs_catchup = false;
        let mut note_ids_hex = Vec::with_capacity(updates.len());
        for u in updates {
            note_ids_hex.push(hex_encode(&u.doc));
            let pending = self
                .inner
                .apply_doc_update_status(u.doc, &u.update_bytes)
                .await
                .map_err(FfiSyncError::from)?;
            needs_catchup |= pending;
            applied += 1;
        }
        Ok(DeltaApplyOutcome {
            applied,
            needs_catchup,
            note_ids_hex,
        })
    }

    /// Encoded version vector of a note's current Loro doc, for the
    /// reconnect/catch-up handshake: a peer hands this to the other side's
    /// [`Self::produce_note_delta`] (`since_vv`) so the response carries only
    /// the updates this device is missing. `None` when the doc isn't
    /// resident (nothing to catch up on). Cursor-free — see
    /// [`Self::produce_note_delta`].
    pub async fn note_version(&self, slug: String) -> Option<Vec<u8>> {
        self.inner.doc_version(stable_uuid_from_slug(&slug)).await
    }

    /// Apply a single CHARACTER-LEVEL splice to one block's text — the
    /// outbound foundation for cursor-accurate collaborative editing. Instead
    /// of re-authoring the WHOLE block text via [`Self::record_note_diff`]
    /// (whose Myers-diff turns a concurrent peer's characters into DELETEs →
    /// clobber), a client sends the user's actual keystroke: "delete
    /// `utf16_delete_len` UTF-16 code units at `utf16_offset`, then insert
    /// `insert`" (the two at the same offset = a replace).
    ///
    /// `utf16_offset` / `utf16_delete_len` are **UTF-16 code units**, matching
    /// iOS `NSRange` and JavaScript string indices, so the editor passes its
    /// native offset with no conversion. The splice goes through the block's
    /// `text_seq` LoroText sequence CRDT, so two devices splicing the SAME
    /// block concurrently INTERLEAVE — neither side's characters are lost.
    ///
    /// `slug` → note id with the same `stable_uuid_from_slug` blake3-truncation
    /// the rest of this bridge uses. `block_id_hex` is the block's id as a
    /// 32-char dashless hex string OR a 36-char dashed UUID (both accepted);
    /// an unparseable id is a `FfiSyncError`.
    ///
    /// Returns `1` when the splice applied, `0` when the block isn't found (a
    /// splice is an in-place edit — the block must already exist).
    pub async fn splice_block_text(
        &self,
        slug: String,
        block_id_hex: String,
        utf16_offset: u32,
        utf16_delete_len: u32,
        insert: String,
    ) -> Result<u32, FfiSyncError> {
        let note_id = stable_uuid_from_slug(&slug);
        let block_id = parse_block_id_hex(&block_id_hex).ok_or_else(|| FfiSyncError::Other {
            message: "block_id_hex must be 32 hex chars or a dashed UUID".into(),
        })?;
        self.inner
            .splice_block_text(note_id, block_id, utf16_offset, utf16_delete_len, &insert)
            .await
            .map_err(FfiSyncError::from)
    }

    /// Read a single block's current text — the engine-exact `text_seq`
    /// content — by `slug` + `block_id_hex`. The inbound counterpart of
    /// [`splice_block_text`](Self::splice_block_text): after `apply_delta_frame`
    /// lands a remote splice on the SAME block the user is editing, the iOS
    /// client reads the MERGED text here and reconciles the open `UITextView`
    /// (minimal diff + caret remap). The engine is the source of truth; the
    /// editor matches it. Own-echoes are harmless — the read returns text the
    /// editor already shows, so the reconcile is a no-op.
    ///
    /// `slug` → note id with the same `stable_uuid_from_slug` blake3-truncation
    /// the splice/produce paths use; `block_id_hex` accepts a 32-char dashless
    /// hex string OR a 36-char dashed UUID (both forms the editor may hold).
    /// Returns `None` for an unknown note/block, an empty block, or an
    /// unparseable `block_id_hex`.
    pub async fn read_block_text(
        &self,
        slug: String,
        block_id_hex: String,
    ) -> Result<Option<String>, FfiSyncError> {
        let note_id = stable_uuid_from_slug(&slug);
        let Some(block_id) = parse_block_id_hex(&block_id_hex) else {
            return Ok(None);
        };
        Ok(self.inner.read_block_text(note_id, block_id).await)
    }

    /// Import the server's full Loro snapshot for a note as an **authoritative
    /// re-base**. Used both as a pre-author shared base (a later `recordNoteDiff`
    /// BlockUpsert resolves to the server's tree nodes instead of minting rival
    /// TreeIDs) AND as the disjoint-device catch-up: if the device ALREADY
    /// authored this note on its own lineage, the re-base tombstones the
    /// device's stale same-bid twins and keeps the snapshot's nodes, so the
    /// device truly adopts the server's lineage (server-wins) instead of the
    /// non-authoritative min-`TreeID` dedup keeping its own twin. Computes the
    /// note id with the same `stable_uuid_from_slug` blake3-truncation the rest
    /// of this bridge uses; the engine import is commutative + idempotent, so a
    /// re-import or a snapshot captured mid-edit is safe (no data loss).
    pub async fn import_note_snapshot(
        &self,
        slug: String,
        bytes: Vec<u8>,
    ) -> Result<(), FfiSyncError> {
        let note_id = stable_uuid_from_slug(&slug);
        // AUTHORITATIVE re-base: a disjoint device that already authored this
        // note adopts the server's lineage (its stale same-bid twins are
        // tombstoned, the snapshot's nodes kept) instead of the non-authoritative
        // min-`TreeID` dedup keeping the device's own twin. This is what makes
        // the catch-up actually CONVERGE a disjoint device rather than patching
        // text while leaving it on its own lineage.
        self.inner
            .import_authoritative_snapshot(note_id, &bytes)
            .await
            .map_err(FfiSyncError::from)
    }

    /// Import a relay snapshot keyed by `note_id` (the relay's opaque
    /// `stream_id`), for bootstrap-from-snapshots. `RelayClientHandle::fetch_snapshots`
    /// returns note_id-keyed streams, and the bootstrapping device has the
    /// note_id but NOT the slug (`note_id = stable_uuid_from_slug(slug)` is
    /// one-way), so it can't use the slug-keyed `import_note_snapshot`.
    /// Same authoritative re-base as the slug path.
    pub async fn import_note_snapshot_by_id(
        &self,
        note_id: Vec<u8>,
        bytes: Vec<u8>,
    ) -> Result<(), FfiSyncError> {
        let id: [u8; 16] = note_id
            .as_slice()
            .try_into()
            .map_err(|_| FfiSyncError::Other {
                message: format!("note_id must be 16 bytes, got {}", note_id.len()),
            })?;
        self.inner
            .import_authoritative_snapshot(id, &bytes)
            .await
            .map_err(FfiSyncError::from)
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
    /// Per-seq apply-retry attempts for inbound envelopes whose per-note
    /// apply failed (audit A4). In-memory only — RelayTicker rebuilds the
    /// coordinator on errors and the budget restarting is fine; the bound
    /// exists to unstick the cursor, not to be a durable counter.
    apply_retries: Mutex<HashMap<i64, u32>>,
}

/// How many ticks an inbound envelope whose per-note apply failed is
/// retried (the cursor holds just before it) before giving up: the failed
/// note ids are surfaced via
/// [`TickInboundRecord::needs_catchup_note_ids_hex`] for a snapshot
/// catch-up and the cursor moves past, so one poisoned envelope can't
/// stall every later stream forever (audit A4). Mirrors the server tick's
/// `tesela_server::sync_relay::MAX_APPLY_RETRIES`.
pub const MAX_APPLY_RETRIES: u32 = 5;

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
            apply_retries: Mutex::new(HashMap::new()),
        }))
    }

    /// Drain locally-recorded ops that the relay hasn't seen yet,
    /// postcard-encode them into a `SyncEnvelope`, AEAD-seal via the
    /// relay client, and PUT. Returns a small outcome record the iOS
    /// caller can show in the UI ("sent N ops, seq=…").
    ///
    /// Idempotent on no-op: if there's nothing to send, returns
    /// `ops_sent: 0` / `batches_attempted: 0` without touching the relay.
    ///
    /// Failure honesty (audit A7): a failed encode or relay PUT does NOT
    /// fail the whole tick (the other batches still go out, and the failed
    /// batch's cursors stay uncommitted so it re-produces next tick), but
    /// it IS reported — `batches_failed` > 0 with `last_error` set. The
    /// caller must treat a non-zero `batches_failed` as a sync error
    /// (surface it, back off); `Ok` with `ops_sent: 0` alone is
    /// indistinguishable from "nothing to send".
    pub async fn tick_outbound(&self, max_bytes: u32) -> Result<TickOutboundRecord, FfiSyncError> {
        let _ = max_bytes; // Loro batching uses MAX_RELAY_PLAINTEXT_BYTES, not this.
        let our_device = self.engine.inner.device();

        // Loro v2 outbound: broadcast per-note Loro update bytes behind the
        // TLR2 magic. The cursor lives inside the engine (per-note version
        // vectors), advanced via commit_broadcast_cursors ONLY after a
        // confirmed PUT — so a failed send retries the same delta. Mirrors
        // `tesela_server::sync_relay::tick`.
        let updates = self.engine.inner.produce_relay_updates().await;
        // Chunk into size-bounded batches so each PUT fits the relay body
        // limit (the canonical bootstrap would otherwise 413). Commit each
        // batch's cursors only after its PUT confirms; skip a failed batch so
        // it retries next tick rather than aborting the whole tick.
        let batches =
            tesela_sync::pack_loro_relay_batches(updates, tesela_sync::MAX_RELAY_PLAINTEXT_BYTES);
        let mut ops_count = 0u32;
        let mut last_seq = None;
        let mut attempted = 0u32;
        let mut failed = 0u32;
        let mut last_error: Option<String> = None;
        for (payload, committed) in batches {
            attempted += 1;
            let ciphertext = match encode_loro_relay_payload(&payload) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("tesela-sync-ffi: encode loro payload: {e}");
                    failed += 1;
                    last_error = Some(format!("encode loro payload: {e}"));
                    continue;
                }
            };
            let envelope = SyncEnvelope {
                from_device: our_device,
                to_group: self.group_id,
                nonce: [0u8; 24],
                ciphertext,
            };
            match self.relay.inner.put_envelope(envelope).await {
                Ok((seq, _ts)) => {
                    self.engine.inner.commit_broadcast_cursors(&committed).await;
                    ops_count += payload.len() as u32;
                    last_seq = Some(seq);
                }
                Err(e) => {
                    eprintln!("tesela-sync-ffi: relay put (loro): {e}");
                    failed += 1;
                    last_error = Some(format!("relay put: {e}"));
                }
            }
        }
        Ok(TickOutboundRecord {
            ops_sent: ops_count,
            relay_seq: last_seq,
            new_cursor_ntp: None,
            batches_attempted: attempted,
            batches_failed: failed,
            last_error,
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
    /// Failure modes — honesty rules (audit A4, FFI half):
    /// - Network errors → cursor untouched; next tick retries the same
    ///   batch (relay's idempotent storage + engine's content-hash
    ///   dedupe make this safe).
    /// - An envelope whose per-note apply FAILED is NOT acked past: the
    ///   cursor holds just before it (bounded retry, [`MAX_APPLY_RETRIES`]
    ///   ticks) while later envelopes still apply (idempotent). After the
    ///   budget, the failed note ids are surfaced via
    ///   `needs_catchup_note_ids_hex` (snapshot catch-up is the heal) and
    ///   the cursor moves on. Each failing envelope counts into `errors`.
    /// - An update Loro leaves PENDING (causal gap) advances the cursor
    ///   but its note id is surfaced via `needs_catchup_note_ids_hex` —
    ///   the buffered bytes are in-memory only, so without a snapshot
    ///   catch-up the note silently freezes (and loses the buffer on
    ///   restart).
    pub async fn tick_inbound(&self) -> Result<TickInboundRecord, FfiSyncError> {
        let our_device = self.engine.inner.device();
        let since = *self.inbound_cursor.lock().await;

        let batch = self
            .relay
            .inner
            .poll(since)
            .await
            .map_err(FfiSyncError::from)?;

        let mut applied = 0u32;
        let mut skipped_own = 0u32;
        let mut errors = 0u32;
        let mut max_seq = since;
        // Earliest seq whose apply failed and is still within its retry
        // budget — the cursor is capped just before it below.
        let mut blocked_at: Option<i64> = None;
        let mut needs_catchup: Vec<String> = Vec::new();
        // Rows whose outer payload failed to decode/AEAD-open were
        // skipped inside poll() (deterministic — corrupt payload or a
        // foreign key; the client logged each). Count them as errors
        // and advance past their seqs so one poisoned row can't wedge
        // inbound sync forever.
        for seq in &batch.skipped {
            errors += 1;
            eprintln!("tesela-sync-ffi: relay poll skipped undecryptable seq={seq}");
            if *seq > max_seq {
                max_seq = *seq;
            }
        }
        for (seq, env) in batch.rows {
            if env.from_device == our_device {
                // Our own write echoed back by the relay; advance the
                // cursor but skip the apply.
                if seq > max_seq {
                    max_seq = seq;
                }
                skipped_own += 1;
                continue;
            }
            // Loro v2 inbound: decode the TLR2 payload + import each per-note
            // update (idempotent). A non-v2 payload (legacy / foreign) decodes
            // to None — skip but advance. A decode error is deterministic, so
            // advance past it too rather than re-fetching the same bytes.
            match decode_loro_relay_payload(&env.ciphertext) {
                Ok(Some(updates)) => {
                    let pairs: Vec<([u8; 16], Vec<u8>)> = updates
                        .into_iter()
                        .map(|u| (u.doc, u.update_bytes))
                        .collect();
                    let report = self.engine.inner.apply_relay_updates(&pairs).await;
                    applied += 1;
                    // PENDING imports (causal gap): cursor advances, but the
                    // caller must snapshot-catch-up these notes.
                    for doc in &report.pending {
                        let hex_id = hex_encode(doc);
                        if !needs_catchup.contains(&hex_id) {
                            needs_catchup.push(hex_id);
                        }
                    }
                    if report.failed.is_empty() {
                        self.apply_retries.lock().await.remove(&seq);
                        if seq > max_seq {
                            max_seq = seq;
                        }
                    } else {
                        errors += 1;
                        // Bounded retry: hold the cursor BEFORE this envelope
                        // for up to MAX_APPLY_RETRIES ticks, then give up —
                        // surface the failed notes for snapshot catch-up and
                        // move on so one poisoned envelope can't stall every
                        // later stream forever.
                        let attempts = {
                            let mut retries = self.apply_retries.lock().await;
                            let a = retries.entry(seq).or_insert(0);
                            *a += 1;
                            *a
                        };
                        if attempts >= MAX_APPLY_RETRIES {
                            eprintln!(
                                "tesela-sync-ffi: giving up on envelope seq={seq} after \
                                 {MAX_APPLY_RETRIES} failed apply attempts; notes need \
                                 snapshot catch-up: {:?}",
                                report
                                    .failed
                                    .iter()
                                    .map(|(id, e)| format!("{}: {e}", hex_encode(id)))
                                    .collect::<Vec<_>>()
                            );
                            for (doc, _) in &report.failed {
                                let hex_id = hex_encode(doc);
                                if !needs_catchup.contains(&hex_id) {
                                    needs_catchup.push(hex_id);
                                }
                            }
                            self.apply_retries.lock().await.remove(&seq);
                            if seq > max_seq {
                                max_seq = seq;
                            }
                        } else {
                            eprintln!(
                                "tesela-sync-ffi: apply failed for {}/{} note(s) in envelope \
                                 seq={seq} (attempt {attempts}/{MAX_APPLY_RETRIES}); holding \
                                 the cursor for retry",
                                report.failed.len(),
                                pairs.len()
                            );
                            if blocked_at.map_or(true, |b| seq < b) {
                                blocked_at = Some(seq);
                            }
                        }
                    }
                }
                Ok(None) => {
                    if seq > max_seq {
                        max_seq = seq;
                    }
                }
                Err(e) => {
                    errors += 1;
                    eprintln!("tesela-sync-ffi: relay loro decode seq={seq} err={e} (skipping)");
                    if seq > max_seq {
                        max_seq = seq;
                    }
                }
            }
        }

        // Cap the cursor just before the earliest still-retrying failure so
        // the failed envelope is re-polled next tick. Later envelopes were
        // still applied above (idempotent re-apply next tick is harmless).
        if let Some(b) = blocked_at {
            max_seq = max_seq.min(b - 1);
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
            needs_catchup_note_ids_hex: needs_catchup,
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
    /// Envelopes that were decrypted, decoded, and run through the
    /// engine apply (including ones whose updates landed pending).
    pub applied: u32,
    /// Envelopes the relay echoed back to us (we authored them
    /// originally). Cursor still advances over these but the apply is
    /// skipped.
    pub skipped_own: u32,
    /// Failures this tick: undecryptable/undecodable envelopes PLUS
    /// envelopes with at least one failed per-note apply. Non-zero means
    /// the tick was not fully healthy even though it returned Ok.
    pub errors: u32,
    /// Highest relay-assigned seq acked this tick. Same as the updated
    /// inbound cursor — held BEFORE a failing envelope while its retry
    /// budget lasts (audit A4).
    pub new_cursor_seq: i64,
    /// Hex (32-char) note ids that need an authoritative-snapshot
    /// catch-up: updates Loro left PENDING (causal gap) plus per-note
    /// applies that kept failing past the retry budget. The caller
    /// should fetch the relay snapshots (or the hub's note snapshot)
    /// and import them for exactly these notes, or they silently
    /// freeze (audit A4).
    pub needs_catchup_note_ids_hex: Vec<String>,
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
    /// Batches produced + attempted this tick. `0` ≡ nothing to send.
    pub batches_attempted: u32,
    /// Batches whose encode or relay PUT failed (audit A7). Their
    /// cursors stay uncommitted, so they re-produce next tick. Non-zero
    /// means outbound is NOT healthy even though the call returned Ok —
    /// the caller must surface it (`ops_sent: 0` alone is
    /// indistinguishable from "nothing to send").
    pub batches_failed: u32,
    /// Error message from the most recent failed batch, for the UI.
    pub last_error: Option<String>,
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
        let batch = self
            .inner
            .poll(since_seq)
            .await
            .map_err(FfiSyncError::from)?;
        // Count only decryptable envelopes, but let the watermark cover
        // skipped (undecryptable) rows too — they're permanently
        // unreadable, so a cursor pinned below them would just re-fetch
        // them forever.
        let highest = batch.max_seq().unwrap_or(since_seq);
        Ok(PollProbeRecord {
            count: batch.rows.len() as u32,
            highest_seq: highest,
        })
    }

    /// Fetch the relay's compacted snapshot set + its compaction watermark.
    /// A fresh or long-offline device imports each (note_id-keyed) snapshot,
    /// jumps its inbound cursor to `compaction_seq`, then polls `?since=` for
    /// the tail. This is the ONLY way a device converges past the relay's GC
    /// window when the depositor (the Mac) is offline — the offline-bootstrap
    /// half of the spine. Snapshots are sealed/opened under the GROUP-only AAD
    /// inside the inner client, so the plaintext returned here is ready to
    /// import directly.
    pub async fn fetch_snapshots(&self) -> Result<FetchSnapshotsRecord, FfiSyncError> {
        let (compaction_seq, snaps) = self
            .inner
            .fetch_snapshots()
            .await
            .map_err(FfiSyncError::from)?;
        Ok(FetchSnapshotsRecord {
            compaction_seq,
            snapshots: snaps
                .into_iter()
                .map(|(stream_id, snapshot_seq, payload)| RelaySnapshotRecord {
                    stream_id,
                    snapshot_seq,
                    payload,
                })
                .collect(),
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

/// One decrypted snapshot from the relay — see [`RelayClientHandle::fetch_snapshots`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct RelaySnapshotRecord {
    /// Opaque stream key = the 16-byte `note_id` the snapshot covers. Import
    /// with [`SyncEngineHandle::import_note_snapshot_by_id`].
    pub stream_id: Vec<u8>,
    /// Relay-assigned seq this snapshot covers up to.
    pub snapshot_seq: i64,
    /// Decrypted full-note snapshot bytes (already opened with the group key).
    pub payload: Vec<u8>,
}

/// The relay's compacted snapshot set + watermark — see
/// [`RelayClientHandle::fetch_snapshots`].
#[derive(Debug, Clone, uniffi::Record)]
pub struct FetchSnapshotsRecord {
    /// The relay's compaction watermark. After importing all snapshots, set the
    /// inbound cursor to this, then poll `?since=compaction_seq` for the tail.
    pub compaction_seq: i64,
    pub snapshots: Vec<RelaySnapshotRecord>,
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

    /// Open a `SyncEngineHandle` on a throwaway mosaic dir with the given
    /// device id. Returns the handle so the WS-delta methods can be driven
    /// through the real FFI surface (not the engine directly).
    async fn open_handle(dir: &std::path::Path, device_hex: &str) -> Arc<SyncEngineHandle> {
        SyncEngineHandle::open_loro(dir.to_string_lossy().into_owned(), device_hex.to_string())
            .await
            .expect("open_loro")
    }

    #[tokio::test]
    async fn ws_delta_round_trip_and_concurrent_edits_converge() {
        // PHASE B: the live WS path holds `Arc<dyn SyncEngine>` through this
        // FFI and exchanges Loro deltas via produce_note_delta /
        // apply_delta_frame. Two handles on distinct mosaics + distinct
        // device ids (so their Loro PeerIDs differ — the prerequisite for
        // clean merge) must (1) bootstrap a note A→B, then (2) converge on a
        // concurrent edit exchanged both ways — no flashing.
        let dir_a = tempfile::tempdir().expect("tempdir a");
        let dir_b = tempfile::tempdir().expect("tempdir b");
        // Two FIXED distinct device ids (→ distinct, stable Loro PeerIDs) so
        // the merge is deterministic — mirrors the engine-level convergence
        // tests' `[0xc1;16]` / `[0xd2;16]` devices.
        let dev_a = "c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1";
        let dev_b = "d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2";
        assert_ne!(dev_a, dev_b, "handles need distinct device ids");

        let a = open_handle(dir_a.path(), dev_a).await;
        let b = open_handle(dir_b.path(), dev_b).await;

        let slug = "shared-note".to_string();
        let note_id = stable_uuid_from_slug(&slug);

        // A records the note, then produces a full-snapshot bootstrap frame
        // (since_vv = None). B applies it and must now render A's content.
        a.record_note_upsert_by_slug(
            slug.clone(),
            "Shared".into(),
            "- base <!-- bid:02020202-0202-0202-0202-020202020202 -->\n".into(),
            1,
        )
        .await
        .unwrap();

        let bootstrap = a
            .produce_note_delta(slug.clone(), None)
            .await
            .unwrap()
            .expect("resident doc yields a bootstrap frame");
        assert_eq!(
            &bootstrap[..4],
            &tesela_sync::LORO_RELAY_MAGIC,
            "TLR2 framed"
        );

        let outcome = b.apply_delta_frame(bootstrap).await.unwrap();
        assert_eq!(outcome.applied, 1, "one per-note update applied");
        assert_eq!(
            a.inner.render_note(note_id).await,
            b.inner.render_note(note_id).await,
            "bootstrap via WS-delta methods converges"
        );

        // A doc that isn't resident on B yields no frame (Ok(None)).
        assert!(
            b.produce_note_delta("never-seen".into(), None)
                .await
                .unwrap()
                .is_none(),
            "absent doc → no frame"
        );

        // Concurrent edits: A and B each append a distinct block, then
        // exchange deltas both ways using the peer's version vector as the
        // catch-up cursor (the reconnect handshake shape).
        let ops_a = a.record_note_diff(
            slug.clone(),
            "- base <!-- bid:02020202-0202-0202-0202-020202020202 -->\n- from A <!-- bid:0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a -->\n".into(),
            "Shared".into(),
            1,
        )
        .await
        .unwrap();
        let ops_b = b.record_note_diff(
            slug.clone(),
            "- base <!-- bid:02020202-0202-0202-0202-020202020202 -->\n- from B <!-- bid:0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b -->\n".into(),
            "Shared".into(),
            1,
        )
        .await
        .unwrap();
        assert!(
            ops_a >= 1 && ops_b >= 1,
            "each side emitted a block op (a={ops_a}, b={ops_b})"
        );

        // B → A: A advertises its version vector; B exports the complement
        // (the reconnect/catch-up handshake `note_version` → `produce_note_delta`).
        let a_vv = a.note_version(slug.clone()).await;
        let b_to_a = b
            .produce_note_delta(slug.clone(), a_vv)
            .await
            .unwrap()
            .expect("B has a delta for A");
        a.apply_delta_frame(b_to_a).await.unwrap();

        // A → B: symmetric exchange.
        let b_vv = b.note_version(slug.clone()).await;
        let a_to_b = a
            .produce_note_delta(slug.clone(), b_vv)
            .await
            .unwrap()
            .expect("A has a delta for B");
        b.apply_delta_frame(a_to_b).await.unwrap();

        let ra = a.inner.render_note(note_id).await.unwrap();
        let rb = b.inner.render_note(note_id).await.unwrap();
        assert_eq!(ra, rb, "concurrent edits converge — no flashing");
        assert!(
            ra.contains("base") && ra.contains("from A") && ra.contains("from B"),
            "converged state carries every block: {ra:?}"
        );
    }

    #[tokio::test]
    async fn apply_delta_frame_ignores_non_tlr2_frame() {
        // A frame without the TLR2 magic (legacy/foreign) must be skipped
        // (empty outcome — applied 0, no catch-up), not error.
        let dir = tempfile::tempdir().unwrap();
        let h = open_handle(dir.path(), &generate_device_id_hex()).await;
        let o1 = h
            .apply_delta_frame(b"not a tlr2 frame".to_vec())
            .await
            .unwrap();
        assert_eq!(o1.applied, 0);
        assert!(!o1.needs_catchup);
        assert!(o1.note_ids_hex.is_empty());
        let o2 = h.apply_delta_frame(Vec::new()).await.unwrap();
        assert_eq!(o2.applied, 0);
        assert!(!o2.needs_catchup);
        assert!(o2.note_ids_hex.is_empty());
    }

    #[tokio::test]
    async fn record_note_diff_delete_materializes_locally_and_converges_peer() {
        // The iOS `.relay` delete flow (product test 2026-06-10): a peer
        // (Mac/web) authored the daily; iOS imported it as its shared base;
        // the user deletes one block on iOS → `recordNoteDiff` gets the
        // full fresh content WITHOUT that block. The deletion must:
        //   1. apply locally — the materialized `<slug>.md` loses the block
        //      (otherwise the next refresh resurrects it on the deleting
        //      device), and
        //   2. ride the produced update so a peer converges to X,Z.
        let dir_a = tempfile::tempdir().expect("tempdir a");
        let dir_b = tempfile::tempdir().expect("tempdir b");
        let a = open_handle(dir_a.path(), "c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1").await;
        let b = open_handle(dir_b.path(), "d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2").await;

        let slug = "2026-06-10".to_string();
        let note_id = stable_uuid_from_slug(&slug);
        let bid_x = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
        let bid_y = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";
        let bid_z = "0c0c0c0c-0c0c-0c0c-0c0c-0c0c0c0c0c0c";

        // Peer A authors the daily with blocks X, Y, Z.
        a.record_note_upsert_by_slug(
            slug.clone(),
            slug.clone(),
            format!(
                "---\ntitle: {slug}\n---\n\n- X <!-- bid:{bid_x} -->\n- Y <!-- bid:{bid_y} -->\n- Z <!-- bid:{bid_z} -->\n"
            ),
            1,
        )
        .await
        .unwrap();

        // iOS (B) adopts A's doc as its shared base — the same authoritative
        // snapshot import `bootstrapNoteIfNeeded` performs on-device.
        let snapshot = {
            let docs = a.inner.tracked_note_ids().await;
            assert!(docs.contains(&note_id));
            a.inner
                .export_doc_update(note_id, None)
                .await
                .expect("A exports a full snapshot")
        };
        b.import_note_snapshot(slug.clone(), snapshot).await.unwrap();

        // B's materialized file now carries all three blocks; build the
        // post-delete content the way iOS does — the freshly-authored UI
        // state re-rendered WITHOUT the deleted block (bid markers intact).
        let b_file = dir_b.path().join("notes").join(format!("{slug}.md"));
        let prev = std::fs::read_to_string(&b_file).expect("B materialized the import");
        assert!(prev.contains("- Y"), "precondition: Y present on disk: {prev:?}");
        let without_y: String = prev
            .lines()
            .filter(|l| !l.contains(bid_y))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";

        let ops = b
            .record_note_diff(slug.clone(), without_y.clone(), slug.clone(), 1)
            .await
            .unwrap();
        assert!(ops >= 1, "the delete produced at least one op (got {ops})");

        // (1) Local materialization: the deleting device's own file loses Y.
        let after = std::fs::read_to_string(&b_file).expect("file still present");
        assert!(
            !after.contains(bid_y) && !after.contains("- Y"),
            "deleted block must leave the deleting device's materialized file: {after:?}"
        );
        assert!(
            after.contains("- X") && after.contains("- Z"),
            "surviving blocks stay: {after:?}"
        );

        // (2) Peer convergence: B's produced update converges A to X,Z.
        let a_vv = a.note_version(slug.clone()).await;
        let b_to_a = b
            .produce_note_delta(slug.clone(), a_vv)
            .await
            .unwrap()
            .expect("B has a delta for A");
        a.apply_delta_frame(b_to_a).await.unwrap();
        let ra = a.inner.render_note(note_id).await.unwrap();
        assert!(
            !ra.contains("- Y"),
            "peer must apply the delete: {ra:?}"
        );
        assert!(
            ra.contains("X") && ra.contains("Z"),
            "peer keeps the surviving blocks: {ra:?}"
        );
    }

    #[tokio::test]
    async fn record_note_diff_first_author_materializes_and_carries_slug() {
        // First-author path (2026-06-10): a note authored on this device via
        // `record_note_diff` ONLY (the iOS fresh-day daily) used to create a
        // doc with no root identity — the engine logged "cannot materialize
        // — no slug", the file never appeared (refresh showed nothing), and
        // peers received a slug-less doc they couldn't materialize either.
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        let a = open_handle(dir_a.path(), "c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1").await;
        let b = open_handle(dir_b.path(), "d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2").await;

        let slug = "2026-06-11".to_string();
        let note_id = stable_uuid_from_slug(&slug);
        let ops = a
            .record_note_diff(
                slug.clone(),
                format!(
                    "---\ntitle: {slug}\n---\n\n- first <!-- bid:0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a -->\n"
                ),
                slug.clone(),
                1,
            )
            .await
            .unwrap();
        assert!(ops >= 1);

        // The authoring device materializes its own note.
        let file_a = dir_a.path().join("notes").join(format!("{slug}.md"));
        let on_disk = std::fs::read_to_string(&file_a)
            .expect("first-author record_note_diff must materialize the note");
        assert!(on_disk.contains("first"), "{on_disk:?}");

        // The peer applies A's update and materializes under the same slug.
        let frame = a
            .produce_note_delta(slug.clone(), None)
            .await
            .unwrap()
            .expect("authored doc exports");
        b.apply_delta_frame(frame).await.unwrap();
        let file_b = dir_b.path().join("notes").join(format!("{slug}.md"));
        let peer_disk = std::fs::read_to_string(&file_b)
            .expect("peer materializes the first-authored note (slug travels)");
        assert!(peer_disk.contains("first"), "{peer_disk:?}");
        assert_eq!(
            a.inner.render_note(note_id).await,
            b.inner.render_note(note_id).await
        );
    }

    #[tokio::test]
    async fn stale_inbound_note_upsert_does_not_resurrect_deleted_block() {
        // Anti-clobber guard (data-loss vector #2): an inbound STALE
        // NoteUpsert from a peer that still carries a block the local
        // device deleted must NOT resurrect it — author-intent deletes
        // flow ONLY as explicit BlockDelete ops, never inferred from
        // upsert absence, and a stale upsert's presence must not undo
        // a newer delete.
        let dir = tempfile::tempdir().expect("tempdir");
        let h = open_handle(dir.path(), "c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1").await;
        let slug = "guard-note".to_string();
        let note_id = stable_uuid_from_slug(&slug);
        let bid_x = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
        let bid_y = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";

        h.record_note_upsert_by_slug(
            slug.clone(),
            "Guard".into(),
            format!("- X <!-- bid:{bid_x} -->\n- Y <!-- bid:{bid_y} -->\n"),
            1,
        )
        .await
        .unwrap();

        // Delete Y via the diff path (the iOS author-intent shape).
        let file = dir.path().join("notes").join(format!("{slug}.md"));
        let prev = std::fs::read_to_string(&file).unwrap();
        let without_y: String = prev
            .lines()
            .filter(|l| !l.contains(bid_y))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        h.record_note_diff(slug.clone(), without_y, "Guard".into(), 1)
            .await
            .unwrap();
        let mid = h.inner.render_note(note_id).await.unwrap();
        assert!(!mid.contains("- Y"), "delete applied: {mid:?}");

        // A stale whole-body NoteUpsert arrives (peer with old content
        // that still includes Y). The non-destructive upsert must leave
        // the deleted block deleted.
        h.record_note_upsert_by_slug(
            slug.clone(),
            "Guard".into(),
            format!("- X <!-- bid:{bid_x} -->\n- Y <!-- bid:{bid_y} -->\n"),
            1,
        )
        .await
        .unwrap();
        let after = h.inner.render_note(note_id).await.unwrap();
        assert!(
            !after.contains("- Y"),
            "stale NoteUpsert must NOT resurrect the deleted block: {after:?}"
        );
        assert!(after.contains("- X"), "live block survives: {after:?}");
    }

    #[tokio::test]
    async fn splice_block_text_through_ffi_applies_and_renders() {
        // Drive the character-level splice through the real FFI surface: seed
        // a note with one block, splice an insert via a DASHED-UUID block id
        // (exercising the parser), and assert the rendered note reflects it.
        let dir = tempfile::tempdir().unwrap();
        let h = open_handle(dir.path(), &generate_device_id_hex()).await;
        let slug = "splice-note".to_string();
        let block = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";

        h.record_note_upsert_by_slug(
            slug.clone(),
            "Splice".into(),
            format!("- hello world <!-- bid:{block} -->\n"),
            1,
        )
        .await
        .unwrap();

        // Insert "there " at UTF-16 offset 6 (after "hello ").
        let n = h
            .splice_block_text(slug.clone(), block.to_string(), 6, 0, "there ".into())
            .await
            .unwrap();
        assert_eq!(n, 1, "splice applied");

        let note_id = stable_uuid_from_slug(&slug);
        let rendered = h.inner.render_note(note_id).await.unwrap_or_default();
        assert!(
            rendered.contains("hello there world"),
            "splice landed in the rendered note: {rendered:?}"
        );
    }

    #[tokio::test]
    async fn splice_block_text_unknown_block_returns_zero() {
        // A splice targeting a block id with no live node is a no-op (Ok(0)).
        let dir = tempfile::tempdir().unwrap();
        let h = open_handle(dir.path(), &generate_device_id_hex()).await;
        let slug = "splice-missing".to_string();
        h.record_note_upsert_by_slug(
            slug.clone(),
            "Splice".into(),
            "- present <!-- bid:0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a -->\n".into(),
            1,
        )
        .await
        .unwrap();

        // A block id (dashless this time) that was never created.
        let n = h
            .splice_block_text(
                slug,
                "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b".into(),
                0,
                0,
                "X".into(),
            )
            .await
            .unwrap();
        assert_eq!(n, 0, "missing block → Ok(0)");
    }

    #[tokio::test]
    async fn splice_block_text_rejects_bad_block_id() {
        // An unparseable block id surfaces as an FfiSyncError rather than a
        // silent no-op.
        let dir = tempfile::tempdir().unwrap();
        let h = open_handle(dir.path(), &generate_device_id_hex()).await;
        let err = h
            .splice_block_text("any".into(), "not-a-hex-id".into(), 0, 0, "X".into())
            .await
            .unwrap_err();
        match err {
            FfiSyncError::Other { .. } => {}
            other => panic!("wrong error variant: {other:?}"),
        }
    }

    // ─── A7 / A4 (FFI half): honest tick outcomes ────────────────────────

    /// Spin an in-process relay (mirrors tesela-server's sync_relay tests).
    async fn spawn_relay() -> (String, tempfile::TempDir, tokio::task::JoinHandle<()>) {
        let tmp = tempfile::tempdir().expect("tmp");
        let db = tmp.path().join("relay.sqlite");
        let state = tesela_relay::AppState::open(&db, 4_194_304, Some("admin".into()))
            .await
            .expect("relay state");
        let app = tesela_relay::router(state);
        let listener =
            tokio::net::TcpListener::bind(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))
                .await
                .expect("bind");
        let addr = listener.local_addr().expect("addr");
        let server = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });
        (format!("http://{}/", addr), tmp, server)
    }

    /// Seal + PUT one TLR2 envelope with the given per-note update bytes via
    /// the inner (non-FFI) RelayClient, as a peer device would.
    async fn put_loro_envelope(
        sender: &RelayClient,
        from: DeviceId,
        group: GroupId,
        updates: &[([u8; 16], Vec<u8>)],
    ) -> i64 {
        let payload: Vec<LoroDocUpdate> = updates
            .iter()
            .map(|(doc, bytes)| LoroDocUpdate {
                doc: *doc,
                update_bytes: bytes.clone(),
            })
            .collect();
        let ciphertext = encode_loro_relay_payload(&payload).unwrap();
        let env = SyncEnvelope {
            from_device: from,
            to_group: group,
            nonce: [0u8; 24],
            ciphertext,
        };
        let (seq, _ts) = sender.put_envelope(env).await.expect("put envelope");
        seq
    }

    /// Full device pair (engine + relay client + coordinator) for the
    /// morning-race scenarios — the FFI surfaces iOS actually drives.
    async fn open_device(
        dir: &std::path::Path,
        dev_hex: &str,
        relay_url: &str,
        g: &GroupIdentityRecord,
    ) -> (Arc<SyncEngineHandle>, Arc<SyncCoordinator>) {
        let engine = open_handle(dir, dev_hex).await;
        let relay = RelayClientHandle::new(
            relay_url.into(),
            g.group_id_hex.clone(),
            dev_hex.into(),
            g.group_key_hex.clone(),
        )
        .unwrap();
        relay.register_or_recover().await.expect("register");
        let coord =
            SyncCoordinator::new(engine.clone(), relay, g.group_id_hex.clone()).unwrap();
        (engine, coord)
    }

    #[tokio::test]
    async fn morning_race_disjoint_daily_ios_delete_converges() {
        // The 2026-06-10 product-test bug, device-faithful: BOTH devices
        // author the same fresh daily slug independently before any
        // exchange (the everyday morning race — each device creates
        // "today" on first view, so the two docs start on DISJOINT Loro
        // lineages), converge through real coordinator ticks over a real
        // relay, then iOS deletes blocks via `record_note_diff` (the
        // `.relay` writeback path). The deletes must (1) materialize
        // locally (else the next refresh resurrects them on the deleting
        // device) and (2) reach the peer through the relay.
        let (url, _relay_tmp, _srv) = spawn_relay().await;
        let g = generate_group_identity();
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        // A = desktop (server-style author), B = iOS.
        let (a, coord_a) =
            open_device(dir_a.path(), "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", &url, &g).await;
        let (b, coord_b) =
            open_device(dir_b.path(), "d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2", &url, &g).await;

        let slug = "2026-06-10".to_string();
        let note_id = stable_uuid_from_slug(&slug);
        let bid_ios_empty = "0e0e0e0e-0e0e-0e0e-0e0e-0e0e0e0e0e0e";
        let bid_dude = "0d0d0d0d-0d0d-0d0d-0d0d-0d0d0d0d0d0d";
        let bid_trail = "0f0f0f0f-0f0f-0f0f-0f0f-0f0f0f0f0f0f";

        // iOS authors FIRST — the fresh-pair daily writeback (one empty
        // trailing block), via the same record_note_diff path the app uses.
        b.record_note_diff(
            slug.clone(),
            format!("---\ntitle: {slug}\n---\n\n- <!-- bid:{bid_ios_empty} -->\n"),
            slug.clone(),
            1,
        )
        .await
        .unwrap();

        // Desktop authors ITS daily independently (disjoint lineage).
        a.record_note_upsert_by_slug(
            slug.clone(),
            slug.clone(),
            format!(
                "---\ntitle: {slug}\n---\n\n- dude <!-- bid:{bid_dude} -->\n- <!-- bid:{bid_trail} -->\n"
            ),
            1,
        )
        .await
        .unwrap();

        // Exchange until quiescent (a few rounds of out/in on both sides).
        for _ in 0..4 {
            let _ = coord_a.tick_outbound(1_000_000).await.unwrap();
            let _ = coord_b.tick_outbound(1_000_000).await.unwrap();
            let _ = coord_a.tick_inbound().await.unwrap();
            let _ = coord_b.tick_inbound().await.unwrap();
        }

        let ra = a.inner.render_note(note_id).await.unwrap_or_default();
        let rb = b.inner.render_note(note_id).await.unwrap_or_default();
        assert_eq!(
            ra, rb,
            "after the morning race the two devices' renders must converge"
        );
        assert!(
            rb.contains("dude"),
            "iOS sees the desktop's block after exchange: {rb:?}"
        );

        // The materialized files agree too (what refresh re-reads).
        let file_a = dir_a.path().join("notes").join(format!("{slug}.md"));
        let file_b = dir_b.path().join("notes").join(format!("{slug}.md"));
        let on_disk_a = std::fs::read_to_string(&file_a).expect("desktop materialized");
        let on_disk_b = std::fs::read_to_string(&file_b).expect("iOS materialized");
        assert_eq!(
            on_disk_a, on_disk_b,
            "materialized files identical after convergence"
        );
        eprintln!("=== converged daily ===\n{on_disk_a}\n=== end ===");

        // iOS deletes the desktop-authored 'dude' AND its own empty block
        // — the user's delete gesture re-renders today's blocks without
        // them and pushes the full content through record_note_diff.
        let without: String = on_disk_b
            .lines()
            .filter(|l| !l.contains(bid_dude) && !l.contains(bid_ios_empty))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let ops = b
            .record_note_diff(slug.clone(), without, slug.clone(), 1)
            .await
            .unwrap();
        assert!(ops >= 1, "delete emitted ops (got {ops})");

        // (1) Local materialization — the deleting device's file loses both.
        let after_b = std::fs::read_to_string(&file_b).unwrap();
        assert!(
            !after_b.contains("dude") && !after_b.contains(bid_ios_empty),
            "deleted blocks must leave iOS's own materialized file \
             (refresh resurrects them otherwise): {after_b:?}"
        );

        // (2) Relay propagation — desktop applies the deletes.
        for _ in 0..3 {
            let _ = coord_b.tick_outbound(1_000_000).await.unwrap();
            let _ = coord_a.tick_inbound().await.unwrap();
        }
        let after_a = std::fs::read_to_string(&file_a).unwrap();
        assert!(
            !after_a.contains("dude") && !after_a.contains(bid_ios_empty),
            "the deletes must propagate to the desktop: {after_a:?}"
        );
        assert!(
            after_a.contains(bid_trail),
            "the un-deleted trailing block survives on desktop: {after_a:?}"
        );
    }

    #[tokio::test]
    async fn tick_outbound_reports_put_failures() {
        // A7: a tick whose every relay PUT fails must say so — the old
        // record returned Ok with ops_sent=0, indistinguishable from
        // "nothing to send", so iOS showed healthy sync while outbound
        // was dead.
        let dir = tempfile::tempdir().unwrap();
        let dev = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
        let engine = open_handle(dir.path(), dev).await;
        engine
            .record_note_upsert_by_slug(
                "outbound-note".into(),
                "Outbound".into(),
                "- pending edit\n".into(),
                1,
            )
            .await
            .unwrap();

        // Closed port → every PUT fails fast (connection refused).
        let g = generate_group_identity();
        let relay = RelayClientHandle::new(
            "http://127.0.0.1:9/".into(),
            g.group_id_hex.clone(),
            dev.into(),
            g.group_key_hex.clone(),
        )
        .unwrap();
        let coord = SyncCoordinator::new(engine, relay, g.group_id_hex.clone()).unwrap();

        let rec = coord.tick_outbound(0).await.unwrap();
        assert_eq!(rec.batches_attempted, 1, "one batch was produced + tried");
        assert_eq!(rec.batches_failed, 1, "the failed PUT is reported");
        assert!(
            rec.last_error.is_some(),
            "the PUT error message is surfaced for the UI"
        );
        assert_eq!(rec.ops_sent, 0);
        assert!(rec.relay_seq.is_none());
    }

    #[tokio::test]
    async fn tick_inbound_holds_cursor_at_failed_apply_then_reports_catchup() {
        // A4 (FFI half): an envelope whose per-note apply FAILS must not be
        // acked past — the cursor holds (bounded retry), later envelopes
        // still apply, and after the retry budget the failed note ids are
        // surfaced so Swift can run a snapshot catch-up.
        let (url, _relay_tmp, _srv) = spawn_relay().await;
        let g = generate_group_identity();
        let group_id = GroupId::from_bytes(parse_hex_16(&g.group_id_hex).unwrap());
        let group_key = GroupKey::from_bytes(parse_hex_32(&g.group_key_hex).unwrap());

        // Sender B (inner client — not the surface under test).
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let sender = RelayClient::new(
            reqwest::Url::parse(&url).unwrap(),
            group_id,
            dev_b,
            group_key.clone(),
        );
        sender.register_or_recover().await.expect("b register");

        // Author the good note on a separate handle (same device id as the
        // sender so the envelope's from_device matches its authorship).
        let bdir = tempfile::tempdir().unwrap();
        let author = open_handle(bdir.path(), "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2").await;
        author
            .record_note_upsert_by_slug(
                "good-note".into(),
                "Good".into(),
                "- hello good\n".into(),
                1,
            )
            .await
            .unwrap();
        let note_good = stable_uuid_from_slug("good-note");
        let good_snap = author
            .inner
            .export_doc_update(note_good, None)
            .await
            .expect("good snapshot");

        let note_poison: [u8; 16] = [0x0f; 16];
        let poison_seq = put_loro_envelope(
            &sender,
            dev_b,
            group_id,
            &[(note_poison, b"definitely not a loro update".to_vec())],
        )
        .await;
        let good_seq = put_loro_envelope(&sender, dev_b, group_id, &[(note_good, good_snap)]).await;
        assert!(good_seq > poison_seq);

        // Consumer A through the real FFI coordinator.
        let adir = tempfile::tempdir().unwrap();
        let dev_a = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
        let engine = open_handle(adir.path(), dev_a).await;
        let relay = RelayClientHandle::new(
            url.clone(),
            g.group_id_hex.clone(),
            dev_a.into(),
            g.group_key_hex.clone(),
        )
        .unwrap();
        relay.register_or_recover().await.expect("a register");
        let coord = SyncCoordinator::new(engine.clone(), relay, g.group_id_hex.clone()).unwrap();

        let rec = coord.tick_inbound().await.unwrap();
        assert!(rec.errors >= 1, "the failed apply is counted: {rec:?}");
        assert_eq!(
            rec.new_cursor_seq,
            poison_seq - 1,
            "cursor held before the failed envelope"
        );
        assert!(
            rec.needs_catchup_note_ids_hex.is_empty(),
            "still within the retry budget — not yet handed to catch-up"
        );
        // The good (later) envelope still applied — no stalled streams.
        let rendered = engine
            .inner
            .render_note(note_good)
            .await
            .unwrap_or_default();
        assert!(
            rendered.contains("hello good"),
            "good envelope applies despite the poisoned one: {rendered:?}"
        );

        // Deterministic failure: after the budget, give up — surface the
        // poisoned note for the Swift-side snapshot catch-up + move on.
        let mut last = TickInboundRecord {
            applied: 0,
            skipped_own: 0,
            errors: 0,
            new_cursor_seq: 0,
            needs_catchup_note_ids_hex: Vec::new(),
        };
        for _ in 1..MAX_APPLY_RETRIES {
            last = coord.tick_inbound().await.unwrap();
        }
        assert_eq!(
            last.new_cursor_seq, good_seq,
            "after the retry budget the cursor moves past the poisoned envelope"
        );
        assert!(
            last.needs_catchup_note_ids_hex
                .contains(&hex_encode(&note_poison)),
            "the poisoned note is surfaced for snapshot catch-up: {last:?}"
        );
        assert_eq!(
            coord.inbound_cursor_seq().await,
            good_seq,
            "coordinator cursor matches the record"
        );
    }

    #[tokio::test]
    async fn tick_inbound_reports_pending_as_needs_catchup() {
        // A4 (FFI half): a delta Loro leaves PENDING (causal gap) advances
        // the cursor but must surface the note id so the caller can run an
        // authoritative-snapshot catch-up — silently counting it as applied
        // freezes the note (and the buffer is lost on restart).
        let (url, _relay_tmp, _srv) = spawn_relay().await;
        let g = generate_group_identity();
        let group_id = GroupId::from_bytes(parse_hex_16(&g.group_id_hex).unwrap());
        let group_key = GroupKey::from_bytes(parse_hex_32(&g.group_key_hex).unwrap());

        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let sender = RelayClient::new(
            reqwest::Url::parse(&url).unwrap(),
            group_id,
            dev_b,
            group_key.clone(),
        );
        sender.register_or_recover().await.expect("b register");

        // Base + tail edit; ship ONLY the tail (export since the base VV).
        let bdir = tempfile::tempdir().unwrap();
        let author = open_handle(bdir.path(), "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2").await;
        author
            .record_note_upsert_by_slug(
                "gap-note".into(),
                "Gap".into(),
                "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
                1,
            )
            .await
            .unwrap();
        let note = stable_uuid_from_slug("gap-note");
        let pre_vv = author.inner.doc_version(note).await.expect("pre vv");
        author
            .record_note_upsert_by_slug(
                "gap-note".into(),
                "Gap".into(),
                "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n- tail edit <!-- bid:02020202-0202-0202-0202-020202020202 -->\n".into(),
                1,
            )
            .await
            .unwrap();
        let tail = author
            .inner
            .export_doc_update(note, Some(&pre_vv))
            .await
            .expect("tail delta");
        let tail_seq = put_loro_envelope(&sender, dev_b, group_id, &[(note, tail)]).await;

        let adir = tempfile::tempdir().unwrap();
        let dev_a = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
        let engine = open_handle(adir.path(), dev_a).await;
        let relay = RelayClientHandle::new(
            url.clone(),
            g.group_id_hex.clone(),
            dev_a.into(),
            g.group_key_hex.clone(),
        )
        .unwrap();
        relay.register_or_recover().await.expect("a register");
        let coord = SyncCoordinator::new(engine, relay, g.group_id_hex.clone()).unwrap();

        let rec = coord.tick_inbound().await.unwrap();
        assert_eq!(
            rec.new_cursor_seq, tail_seq,
            "a pending (not failed) envelope still advances the cursor"
        );
        assert!(
            rec.needs_catchup_note_ids_hex.contains(&hex_encode(&note)),
            "the pending note is surfaced for snapshot catch-up: {rec:?}"
        );
        assert_eq!(rec.errors, 0, "pending is a gap, not an error");
    }
}
