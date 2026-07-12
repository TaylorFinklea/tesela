//! Phase 1.5 multi-device sync HTTP endpoints.
//!
//! These ride alongside `routes/sync.rs` (which is the Apple Reminders
//! external sync). Naming: the existing module stays as `sync.rs`; this
//! new module is `peer_sync.rs` to make the distinction obvious.
//!
//! Wire protocol:
//!
//! - `GET  /sync/peer/device`     this device's id (JSON)
//! - `GET  /sync/peer/peers`      list of paired peers (JSON)
//! - `POST /sync/peer/peers`      add a peer (JSON in / JSON out)
//! - `DELETE /sync/peer/peers/{device_id_hex}`  remove a peer
//! - `POST /sync/peer/produce`    "what's new since cursor X?" (JSON in / postcard out)
//! - `POST /sync/peer/envelope`   receive a SyncEnvelope (postcard in / 204 out)
//! - `POST /sync/peer/now`        trigger immediate sync with all peers (JSON out)
//! - `GET  /sync/peer/status`     per-peer last sync info (JSON out)
//! - `GET  /sync/peer/discovered` (Phase 2.1) mDNS-discovered LAN peers (JSON out)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::{
    body::Bytes,
    extract::{Path as AxPath, State},
    http::StatusCode,
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tesela_core::{db::SqliteIndex, storage::filesystem::FsNoteStore};
use tesela_sync::{
    decode_pairing_code, encode_pairing_code, DeviceId, GroupIdentity, PairingCode, PeerCursor,
};
use tokio::sync::broadcast;

use crate::state::{AppState, WsEvent};

const PEERS_FILE: &str = "sync_peers.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub device_id_hex: String,
    pub url: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeviceInfo {
    pub device_id_hex: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProduceRequest {
    /// Hex device id of the requester ("give me your ops not from me").
    pub peer_device: String,
    /// Last NTP64 the requester has seen from this device. None means
    /// "from the beginning."
    pub since_hlc_ntp: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PeerStatus {
    pub device_id_hex: String,
    pub url: String,
    pub peer_cursor_ntp: Option<i64>,
}

pub async fn get_device(State(s): State<Arc<AppState>>) -> Json<DeviceInfo> {
    Json(DeviceInfo {
        device_id_hex: s.sync_engine.device().to_hex(),
    })
}

pub async fn list_peers(State(s): State<Arc<AppState>>) -> Json<Vec<Peer>> {
    let peers = read_peers(&s.mosaic_root).await;
    Json(peers)
}

pub async fn add_peer(
    State(s): State<Arc<AppState>>,
    Json(p): Json<Peer>,
) -> Result<Json<Peer>, (StatusCode, String)> {
    // Validate hex.
    if hex_to_device_id(&p.device_id_hex).is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("invalid device_id_hex: {}", p.device_id_hex),
        ));
    }
    let mut peers = read_peers(&s.mosaic_root).await;
    peers.retain(|x| x.device_id_hex != p.device_id_hex);
    peers.push(p.clone());
    write_peers(&s.mosaic_root, &peers)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tracing::info!(
        "sync_peer: added {} at {} (now {} peer(s))",
        p.device_id_hex,
        p.url,
        peers.len()
    );
    Ok(Json(p))
}

pub async fn remove_peer(
    AxPath(device_id_hex): AxPath<String>,
    State(s): State<Arc<AppState>>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut peers = read_peers(&s.mosaic_root).await;
    let before = peers.len();
    peers.retain(|x| x.device_id_hex != device_id_hex);
    write_peers(&s.mosaic_root, &peers)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if peers.len() < before {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((StatusCode::NOT_FOUND, "no such peer".to_string()))
    }
}

// ─── LAN peer-to-peer data plane — RETIRED at the Loro flag-day ──────────
//
// 2026-05-29: the op-replay pull model below (peer asks "give me your ops
// since cursor X", applies them locally) is fundamentally incompatible with
// the Loro engine — Loro has no per-device op log to replay from an HLC
// cursor; its unit of sync is a per-note version-vector update. Devices
// already converge through the relay spine (the proven web↔iOS path), which
// makes LAN P2P a pure latency optimization that is fully redundant with the
// relay for correctness. So the data-plane endpoints below return 501 and the
// daemon path is a no-op. Pairing + discovery (get_device / add_peer /
// pairing-code / discovered / status) stay live so a future LAN P2P built on
// the Loro relay-update protocol can reuse them.

const PEER_DATA_PLANE_RETIRED: &str =
    "LAN peer op-pull was retired at the Loro cutover; devices sync via the relay";

pub async fn produce(
    State(_s): State<Arc<AppState>>,
    Json(_req): Json<ProduceRequest>,
) -> Result<Response, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        PEER_DATA_PLANE_RETIRED.to_string(),
    ))
}

pub async fn receive_envelope(
    State(_s): State<Arc<AppState>>,
    _body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        PEER_DATA_PLANE_RETIRED.to_string(),
    ))
}

pub async fn sync_now(State(_s): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "peers": {}, "note": PEER_DATA_PLANE_RETIRED }))
}

/// Phase 2.1 — mDNS-discovered LAN peers. These are candidates only;
/// adding them to the paired-peers list is still a separate user action.
#[derive(Debug, Serialize)]
pub struct DiscoveredPeerView {
    pub device_id_hex: String,
    pub display_name: String,
    pub url: String,
    /// Seconds since this peer was last seen via mDNS.
    pub last_seen_secs_ago: u64,
}

/// Phase 2.2 — emit a pairing code carrying the local group identity,
/// device id, URL, and display name. The joining side decodes this and
/// adopts the group identity.
///
/// Phase 2.5 (short-code lookup) — every pairing code is also registered
/// in a short-lived in-memory map keyed by a 6-character human-typable
/// code. The response includes both the long code and the short code,
/// so the desktop UI can render the short code under the QR and the
/// joining device can either scan the QR (zero-typing) or type the
/// 6 digits and look the long code up via
/// `GET /sync/peer/short-code/:code`.
#[derive(Debug, Serialize)]
pub struct PairingCodePayload {
    pub code: String,
    pub display_name: String,
    pub device_id_hex: String,
    pub url: String,
    /// 6-character short code (A-Z 0-9, ambiguous chars stripped) that
    /// resolves to this same payload via the lookup endpoint. Lifetime
    /// is bounded by [`SHORT_CODE_TTL`]; expired entries return 404 and
    /// the caller must regenerate.
    pub short_code: String,
    /// Wall-clock seconds until the short code stops resolving. The UI
    /// can use this to render a countdown.
    pub short_code_expires_in_secs: u64,
}

/// How long a published short code is valid. After this, the in-memory
/// entry is reaped and the lookup returns 404.
const SHORT_CODE_TTL: Duration = Duration::from_secs(10 * 60);

/// Characters used for the 6-char short code. Omits visually ambiguous
/// glyphs (0/O, 1/I/L, etc.) so users mistype less when copying off a
/// QR card.
const SHORT_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const SHORT_CODE_LEN: usize = 6;

/// Process-local map from short-code → (full_pairing_code, inserted_at).
/// Lookup endpoints filter out entries past [`SHORT_CODE_TTL`] on every
/// read so we never need a background reaper.
fn short_code_map() -> &'static Mutex<HashMap<String, (String, Instant)>> {
    static MAP: OnceLock<Mutex<HashMap<String, (String, Instant)>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn make_short_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..SHORT_CODE_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..SHORT_CODE_ALPHABET.len());
            SHORT_CODE_ALPHABET[idx] as char
        })
        .collect()
}

/// Insert `code` and return a short verifier. If the random short code
/// collides with a live entry we retry; bounded loop because collisions
/// are astronomically unlikely with this alphabet + length.
fn register_short_code(full: String) -> String {
    let map = short_code_map();
    let mut g = map.lock().unwrap();
    let now = Instant::now();
    // Reap expired entries opportunistically.
    g.retain(|_, (_, inserted)| now.duration_since(*inserted) < SHORT_CODE_TTL);
    for _ in 0..16 {
        let candidate = make_short_code();
        if !g.contains_key(&candidate) {
            g.insert(candidate.clone(), (full, now));
            return candidate;
        }
    }
    // Vanishingly unlikely with 31^6 ≈ 8.8e8 slots and small live size;
    // if we hit it, the caller can just regenerate.
    String::new()
}

fn lookup_short_code(short: &str) -> Option<String> {
    let map = short_code_map();
    let mut g = map.lock().unwrap();
    let now = Instant::now();
    g.retain(|_, (_, inserted)| now.duration_since(*inserted) < SHORT_CODE_TTL);
    g.get(short).map(|(full, _)| full.clone())
}

pub async fn get_pairing_code(
    State(s): State<Arc<AppState>>,
) -> Result<Json<PairingCodePayload>, (StatusCode, String)> {
    let ident = s.group_identity.read().await.clone();
    let device = s.sync_engine.device();
    let code = PairingCode::from_local(
        &ident,
        device,
        s.public_url.clone(),
        s.display_name.clone(),
        s.relay_url.clone(),
    );
    let encoded = encode_pairing_code(&code)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("encode: {e}")))?;
    let short = register_short_code(encoded.clone());
    Ok(Json(PairingCodePayload {
        code: encoded,
        display_name: s.display_name.clone(),
        device_id_hex: device.to_hex(),
        url: s.public_url.clone(),
        short_code: short,
        short_code_expires_in_secs: SHORT_CODE_TTL.as_secs(),
    }))
}

#[derive(Debug, Serialize)]
pub struct RecoveryPhrasePayload {
    pub phrase: String,
}

/// `GET /sync/recovery-phrase` — the current mosaic's 24-word BIP39
/// recovery phrase (`tesela-ra7` P0.3c). The phrase IS the group key in
/// plaintext, same exposure as `get_pairing_code` above; never log it.
pub async fn get_recovery_phrase(State(s): State<Arc<AppState>>) -> Json<RecoveryPhrasePayload> {
    let ident = s.group_identity.read().await.clone();
    let phrase = tesela_sync::crypto::recovery::key_to_phrase(&ident.group_key);
    Json(RecoveryPhrasePayload { phrase })
}

/// `GET /sync/peer/short-code/:code` — resolve a 6-char short code to the
/// full base64url pairing code it was registered with. 404s on unknown
/// or expired codes so the caller can prompt for a regenerate.
pub async fn lookup_pairing_short_code(
    AxPath(short): AxPath<String>,
) -> Result<Json<PairingCodePayload>, (StatusCode, String)> {
    // Normalise: drop separators a UI might add ("123-456" / "123 456").
    let normalised: String = short
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    tracing::info!(
        "short-code lookup: raw={:?} normalised={:?}",
        short,
        normalised
    );
    let Some(full) = lookup_short_code(&normalised) else {
        tracing::info!("short-code lookup: {:?} -> 404", normalised);
        return Err((
            StatusCode::NOT_FOUND,
            "short code unknown or expired".to_string(),
        ));
    };
    let decoded = decode_pairing_code(&full)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("decode: {e}")))?;
    tracing::info!(
        "short-code lookup: {:?} -> OK (inviter={}, url={})",
        normalised,
        decoded.display_name,
        decoded.url
    );
    Ok(Json(PairingCodePayload {
        code: full,
        display_name: decoded.display_name,
        device_id_hex: decoded.device_id.to_hex(),
        url: decoded.url,
        short_code: normalised,
        short_code_expires_in_secs: SHORT_CODE_TTL.as_secs(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct PairWithCodeReq {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct PairWithCodeResp {
    pub device_id_hex: String,
    pub display_name: String,
    pub url: String,
    /// True when we adopted a different group identity to complete the
    /// pair. False when our local group already matched.
    pub adopted_group: bool,
    /// True when the pairing code carried a relay URL that we persisted into
    /// `config.toml` — the joiner is now configured to join the spine. False
    /// for a LAN-only (relay_url=None) code, leaving config untouched. (L1)
    pub relay_configured: bool,
    /// True when the joiner must restart for the new relay config to take
    /// effect — the relay handle is bound at server boot, not hot-swappable.
    pub restart_required: bool,
}

fn restart_required_after_pair(
    adopted_group: bool,
    relay_configured: bool,
    relay_was_configured: bool,
) -> bool {
    relay_configured || (adopted_group && relay_was_configured)
}

pub async fn pair_with_code(
    State(s): State<Arc<AppState>>,
    Json(req): Json<PairWithCodeReq>,
) -> Result<Json<PairWithCodeResp>, (StatusCode, String)> {
    let parsed = decode_pairing_code(&req.code)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("decode: {e}")))?;
    let own_device = s.sync_engine.device();
    if parsed.device_id == own_device {
        return Err((
            StatusCode::BAD_REQUEST,
            "pairing code is from this device".to_string(),
        ));
    }
    let mut adopted = false;
    {
        let current = s.group_identity.read().await.clone();
        if current.group_id != parsed.group_id
            || current.group_key.as_bytes() != &parsed.group_key_bytes
        {
            let incoming = parsed.group_identity();
            tesela_sync::adopt_group_identity(&s.mosaic_root, &incoming)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("adopt: {e}")))?;
            *s.group_identity.write().await = incoming;
            adopted = true;
        }
    }
    let peer = Peer {
        device_id_hex: parsed.device_id.to_hex(),
        url: parsed.url.clone(),
        display_name: Some(parsed.display_name.clone()),
    };
    let mut peers = read_peers(&s.mosaic_root).await;
    peers.retain(|x| x.device_id_hex != peer.device_id_hex);
    peers.push(peer.clone());
    write_peers(&s.mosaic_root, &peers)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // L1 — join the spine: the v2 pairing code carries the inviter's relay
    // URL. Persist it into config.toml so this joiner's next boot brings up
    // the relay against the same spine (without this the joiner adopts the
    // group but stays LAN-only forever). A None relay_url is the LAN-only
    // path — leave config untouched (today's behavior). The relay handle is
    // boot-time only, so signal restart_required rather than hot-swapping it.
    let relay_configured = match parsed.relay_url.as_deref() {
        Some(url) if !url.trim().is_empty() => {
            persist_relay_url(&s.mosaic_root, url).map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("relay config: {e}"),
                )
            })?;
            true
        }
        _ => false,
    };
    let restart_required =
        restart_required_after_pair(adopted, relay_configured, s.relay_url.is_some());

    tracing::info!(
        "sync_peer: paired via code with {} (adopted_group={}, relay_configured={}, restart_required={})",
        peer.device_id_hex,
        adopted,
        relay_configured,
        restart_required
    );
    Ok(Json(PairWithCodeResp {
        device_id_hex: peer.device_id_hex,
        display_name: peer.display_name.clone().unwrap_or_default(),
        url: peer.url,
        adopted_group: adopted,
        relay_configured,
        restart_required,
    }))
}

/// Persist the spine relay URL into the mosaic's `config.toml`, mirroring the
/// `PUT /sync/relay/config` idiom (`relay.rs::put_config`) — load-or-default,
/// set `[sync.relay]`, save. Takes effect on next server boot. Canonicalizes
/// the trailing slash so the stored URL matches `scope_to_identity`'s
/// relay-url comparison key (a divergent form would spuriously re-bootstrap).
fn persist_relay_url(mosaic_root: &Path, url: &str) -> Result<(), String> {
    use tesela_core::config::{Config, RelayConfig};
    let path = mosaic_root.join(".tesela").join("config.toml");
    let mut cfg = Config::load_or_default(&path);
    cfg.sync.relay = Some(RelayConfig {
        url: url.trim().trim_end_matches('/').to_string(),
        poll_interval_ms: 5_000,
    });
    cfg.save(&path).map_err(|e| e.to_string())
}

pub async fn discovered(State(s): State<Arc<AppState>>) -> Json<Vec<DiscoveredPeerView>> {
    let Some(d) = s.lan_discovery.as_ref() else {
        return Json(Vec::new());
    };
    let snapshot = d.snapshot(std::time::Duration::from_secs(60));
    let now = std::time::Instant::now();
    let out = snapshot
        .into_iter()
        .map(|p| DiscoveredPeerView {
            device_id_hex: p.device_id.to_hex(),
            url: p.http_url(),
            last_seen_secs_ago: now.duration_since(p.last_seen).as_secs(),
            display_name: p.display_name,
        })
        .collect();
    Json(out)
}

pub async fn status(State(s): State<Arc<AppState>>) -> Json<Vec<PeerStatus>> {
    let peers = read_peers(&s.mosaic_root).await;
    let mut out = Vec::with_capacity(peers.len());
    for p in peers {
        let cursor_ntp = if let Some(d) = hex_to_device_id(&p.device_id_hex) {
            match s.sync_engine.peer_cursor(d).await {
                Ok(PeerCursor::Earliest) => None,
                Ok(PeerCursor::At(ts)) => Some(ts.ntp64_as_i64()),
                Err(_) => None,
            }
        } else {
            None
        };
        out.push(PeerStatus {
            device_id_hex: p.device_id_hex,
            url: p.url,
            peer_cursor_ntp: cursor_ntp,
        });
    }
    Json(out)
}

async fn read_peers(mosaic_root: &Path) -> Vec<Peer> {
    let path = peers_path(mosaic_root);
    match tokio::fs::read(&path).await {
        Ok(bytes) => serde_json::from_slice::<Vec<Peer>>(&bytes).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// Daemon-friendly variant that does not require an `AppState`. Used by
/// the background sync loop in `main.rs`. Takes the group identity by
/// reference so the daemon can hold its own `RwLock` and snapshot once
/// per tick. The store / index / ws_tx are used to reindex touched notes
/// and broadcast `WsEvent` so the web UI live-updates without a hard
/// refresh.
pub async fn sync_with_peer_minimal(
    _engine: &dyn tesela_sync::SyncEngine,
    _mosaic_root: &Path,
    _store: &FsNoteStore,
    _index: &SqliteIndex,
    _ws_tx: &broadcast::Sender<WsEvent>,
    _peer: &Peer,
    _ident: &GroupIdentity,
) -> Result<u32, String> {
    // Retired at the Loro flag-day — see the data-plane note above. The
    // background daemon still calls this each tick; it's a no-op (devices sync
    // via the relay). Kept with its signature so the daemon wiring in main.rs
    // and a future Loro-based LAN sync slot in without further plumbing.
    Ok(0)
}

/// After `apply_changes` materializes ops onto disk, walk the set of
/// touched `note_id`s and (a) re-fetch the note via the store so the
/// derived SQL projections (tasks view, search index, link graph) are
/// rebuilt, and (b) emit a `WsEvent::NoteUpdated` so the web UI's
/// `queryClient` invalidates its caches and re-renders without a hard
/// refresh.
///
/// Resolves each `note_id` back to a slug by walking the on-disk `notes/`
/// directory and matching `blake3(stem)[..16]` — the same fallback the
/// engine uses for unknown-slug BlockUpserts. Notes whose slug can't be
/// resolved (file deleted concurrently, e.g.) are skipped silently rather
/// than failing the broader sync round; missing-file is normal under
/// concurrent deletion.
async fn write_peers(mosaic_root: &Path, peers: &[Peer]) -> Result<(), std::io::Error> {
    let tesela_dir = mosaic_root.join(".tesela");
    tokio::fs::create_dir_all(&tesela_dir).await?;
    let path = peers_path(mosaic_root);
    let bytes = serde_json::to_vec_pretty(peers).map_err(std::io::Error::other)?;
    tokio::fs::write(path, bytes).await
}

fn peers_path(mosaic_root: &Path) -> PathBuf {
    mosaic_root.join(".tesela").join(PEERS_FILE)
}

fn hex_to_device_id(hex: &str) -> Option<DeviceId> {
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        let hi = hex.as_bytes()[i * 2];
        let lo = hex.as_bytes()[i * 2 + 1];
        let hi = nibble(hi)?;
        let lo = nibble(lo)?;
        *byte = (hi << 4) | lo;
    }
    Some(DeviceId(out))
}

fn nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tesela_core::config::Config;

    /// L1 PV — a paired joiner persists the inviter's relay URL so its next
    /// boot joins the spine. Also asserts the trailing-slash canonicalization
    /// (so the stored URL matches `RelayState::scope_to_identity`'s key).
    #[test]
    fn pair_with_code_persists_relay_url() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".tesela")).unwrap();

        persist_relay_url(tmp.path(), "https://relay.example.com:8443/").unwrap();

        let cfg = Config::load(&tmp.path().join(".tesela").join("config.toml")).unwrap();
        let relay = cfg.sync.relay.expect("[sync.relay] should be configured");
        assert_eq!(relay.url, "https://relay.example.com:8443"); // trailing slash trimmed
        assert_eq!(relay.poll_interval_ms, 5_000);
    }

    /// load-or-default path: no pre-existing config.toml still produces a
    /// valid one, and a whitespace-padded URL is trimmed.
    #[test]
    fn persist_relay_url_creates_config_when_absent() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".tesela")).unwrap();
        assert!(!tmp.path().join(".tesela").join("config.toml").exists());

        persist_relay_url(tmp.path(), "  http://100.64.0.1:9999  ").unwrap();

        let cfg = Config::load(&tmp.path().join(".tesela").join("config.toml")).unwrap();
        assert_eq!(cfg.sync.relay.unwrap().url, "http://100.64.0.1:9999");
    }

    #[test]
    fn adopting_group_with_existing_relay_requires_restart() {
        assert!(restart_required_after_pair(true, false, true));
        assert!(!restart_required_after_pair(true, false, false));
        assert!(!restart_required_after_pair(false, false, true));
        assert!(restart_required_after_pair(false, true, false));
    }
}
