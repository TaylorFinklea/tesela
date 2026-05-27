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
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tesela_core::{
    db::SqliteIndex,
    note::NoteId,
    storage::filesystem::FsNoteStore,
    traits::{note_store::NoteStore, search_index::SearchIndex},
};
use tesela_sync::oplog::op::EncodedOp;
use tesela_sync::{
    aead_open, aead_seal, decode_pairing_code, encode_pairing_code, envelope_aad, DeviceId,
    GroupIdentity, PairingCode, PeerCursor, SyncEnvelope,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ProduceResponse {
    /// Phase 2.3 — AEAD-sealed op batch. Receiver opens the envelope
    /// with the shared group key before applying. An empty envelope
    /// (ciphertext == ops-postcard-of-empty-vec) is a legal response
    /// when there's nothing new to send.
    pub envelope: SyncEnvelope,
    /// New cursor (NTP64) pointing past the last produced op. None if
    /// no ops were produced.
    pub new_cursor_ntp: Option<i64>,
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

pub async fn produce(
    State(s): State<Arc<AppState>>,
    Json(req): Json<ProduceRequest>,
) -> Result<Response, (StatusCode, String)> {
    let peer = hex_to_device_id(&req.peer_device).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid peer_device hex: {}", req.peer_device),
        )
    })?;
    let since = match req.since_hlc_ntp {
        None => PeerCursor::Earliest,
        Some(ntp) => PeerCursor::At(tesela_sync::HlcTimestamp::from_ntp64_i64(ntp, peer)),
    };
    let batch = s
        .sync_engine
        .produce_changes_since(peer, since, 1024 * 1024)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let new_cursor_ntp = match batch.new_cursor {
        PeerCursor::Earliest => None,
        PeerCursor::At(ts) => Some(ts.ntp64_as_i64()),
    };
    let ident = s.group_identity.read().await.clone();
    let envelope = seal_ops_envelope(&s.sync_engine.device(), &ident, &batch.ops)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let resp = ProduceResponse {
        envelope,
        new_cursor_ntp,
    };
    let body = postcard::to_allocvec(&resp)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/octet-stream")],
        body,
    )
        .into_response())
}

/// Bundle a batch of ops into an AEAD-sealed envelope addressed to the
/// local group. AAD binds the routing metadata so a relay can't rewrite
/// either field without invalidating the tag.
fn seal_ops_envelope(
    from: &DeviceId,
    ident: &GroupIdentity,
    ops: &[EncodedOp],
) -> Result<SyncEnvelope, String> {
    let plaintext =
        postcard::to_allocvec(&ops.to_vec()).map_err(|e| format!("encode ops: {e}"))?;
    let aad = envelope_aad(from.as_bytes(), ident.group_id.as_bytes());
    let sealed = aead_seal(&ident.group_key, &plaintext, &aad)
        .map_err(|e| format!("seal envelope: {e}"))?;
    Ok(SyncEnvelope {
        from_device: *from,
        to_group: ident.group_id,
        nonce: sealed.nonce,
        ciphertext: sealed.ciphertext,
    })
}

/// Open an AEAD-sealed envelope back into a `Vec<EncodedOp>`. Rejects
/// envelopes addressed to a group we don't belong to.
fn open_ops_envelope(
    envelope: &SyncEnvelope,
    ident: &GroupIdentity,
) -> Result<Vec<EncodedOp>, String> {
    if envelope.to_group != ident.group_id {
        return Err(format!(
            "envelope group_id {:02x?} doesn't match local {:02x?} (pair via code first?)",
            envelope.to_group.as_bytes(),
            ident.group_id.as_bytes()
        ));
    }
    let aad = envelope_aad(envelope.from_device.as_bytes(), ident.group_id.as_bytes());
    let plaintext = aead_open(
        &ident.group_key,
        &envelope.nonce,
        &envelope.ciphertext,
        &aad,
    )
    .map_err(|e| format!("open envelope: {e}"))?;
    postcard::from_bytes::<Vec<EncodedOp>>(&plaintext)
        .map_err(|e| format!("decode ops: {e}"))
}

pub async fn receive_envelope(
    State(s): State<Arc<AppState>>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let envelope: SyncEnvelope = postcard::from_bytes(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("postcard: {e}")))?;
    let from = envelope.from_device;
    let ident = s.group_identity.read().await.clone();
    let ops = open_ops_envelope(&envelope, &ident).map_err(|e| (StatusCode::UNAUTHORIZED, e))?;
    // The engine still consumes a cleartext-ciphertext envelope. Wrap
    // the decrypted ops back into an internal-only envelope so
    // apply_changes stays unchanged (it'll move into the engine once we
    // refactor the trait to take a GroupKey directly).
    let internal = SyncEnvelope {
        from_device: from,
        to_group: ident.group_id,
        nonce: [0u8; 24],
        ciphertext: postcard::to_allocvec(&ops)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
    };
    let applied = s
        .sync_engine
        .apply_changes(from, internal)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    tracing::info!(
        "sync_peer: applied {} ops from {} (deduped={}, parked={})",
        applied.applied,
        from.to_hex(),
        applied.deduped,
        applied.parked
    );
    Ok(StatusCode::NO_CONTENT)
}

pub async fn sync_now(State(s): State<Arc<AppState>>) -> Json<Value> {
    let peers = read_peers(&s.mosaic_root).await;
    let mut results = serde_json::Map::new();
    for peer in &peers {
        match sync_with_peer(&s, peer).await {
            Ok(applied) => {
                results.insert(peer.device_id_hex.clone(), json!({ "applied": applied }));
            }
            Err(e) => {
                results.insert(peer.device_id_hex.clone(), json!({ "error": e }));
            }
        }
    }
    Json(json!({ "peers": results }))
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
                .map_err(|e| {
                    (StatusCode::INTERNAL_SERVER_ERROR, format!("adopt: {e}"))
                })?;
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
    tracing::info!(
        "sync_peer: paired via code with {} (adopted_group={})",
        peer.device_id_hex,
        adopted
    );
    Ok(Json(PairWithCodeResp {
        device_id_hex: peer.device_id_hex,
        display_name: peer.display_name.clone().unwrap_or_default(),
        url: peer.url,
        adopted_group: adopted,
    }))
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

/// One round of pull-then-push with a single peer.
///
/// 1. Ask peer for ops they have not-from-us since our peer_cursor.
/// 2. Apply locally.
/// 3. Send peer our ops not-from-them since their peer_cursor (which we
///    learn by asking them to ack what they have).
///
/// For Phase 1.5 we only do step 1 (pull). Step 3 is symmetric and runs
/// when the peer pulls from us. Both sides' daemons handle their own
/// pulls so transitively everyone converges.
pub async fn sync_with_peer(s: &AppState, peer: &Peer) -> Result<u32, String> {
    let peer_device = hex_to_device_id(&peer.device_id_hex)
        .ok_or_else(|| format!("invalid device id hex: {}", peer.device_id_hex))?;

    // Our cursor for this peer.
    let our_cursor = s
        .sync_engine
        .peer_cursor(peer_device)
        .await
        .map_err(|e| format!("peer_cursor: {e}"))?;
    let since_ntp = match our_cursor {
        PeerCursor::Earliest => None,
        PeerCursor::At(ts) => Some(ts.ntp64_as_i64()),
    };

    // Build the produce request. We tell the peer "I am `our_device`;
    // give me ops not from me since my cursor."
    let req = ProduceRequest {
        peer_device: s.sync_engine.device().to_hex(),
        since_hlc_ntp: since_ntp,
    };
    let url = format!("{}/sync/peer/produce", peer.url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("POST {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("peer responded {}: {url}", resp.status()));
    }
    let body_bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("read response body: {e}"))?;
    let produced: ProduceResponse = postcard::from_bytes(&body_bytes)
        .map_err(|e| format!("decode ProduceResponse: {e}"))?;

    let ident = s.group_identity.read().await.clone();
    let ops = open_ops_envelope(&produced.envelope, &ident)?;
    if ops.is_empty() {
        return Ok(0);
    }

    // Wrap the decrypted ops into a synthetic cleartext envelope for
    // apply_changes (which still consumes plaintext via the ciphertext
    // field). When the engine grows decrypt-in-apply this conversion
    // collapses.
    let internal = SyncEnvelope {
        from_device: peer_device,
        to_group: ident.group_id,
        nonce: [0u8; 24],
        ciphertext: postcard::to_allocvec(&ops)
            .map_err(|e| format!("re-encode ops: {e}"))?,
    };
    let applied = s
        .sync_engine
        .apply_changes(peer_device, internal)
        .await
        .map_err(|e| format!("apply_changes: {e}"))?;

    rebroadcast_touched_notes(
        &s.mosaic_root,
        &s.store,
        &s.index,
        &s.ws_tx,
        &applied.note_ids,
    )
    .await;

    Ok(applied.applied)
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
    engine: &dyn tesela_sync::SyncEngine,
    mosaic_root: &Path,
    store: &FsNoteStore,
    index: &SqliteIndex,
    ws_tx: &broadcast::Sender<WsEvent>,
    peer: &Peer,
    ident: &GroupIdentity,
) -> Result<u32, String> {
    let peer_device = hex_to_device_id(&peer.device_id_hex)
        .ok_or_else(|| format!("invalid device id hex: {}", peer.device_id_hex))?;

    let our_cursor = engine
        .peer_cursor(peer_device)
        .await
        .map_err(|e| format!("peer_cursor: {e}"))?;
    let since_ntp = match our_cursor {
        PeerCursor::Earliest => None,
        PeerCursor::At(ts) => Some(ts.ntp64_as_i64()),
    };
    let req = ProduceRequest {
        peer_device: engine.device().to_hex(),
        since_hlc_ntp: since_ntp,
    };
    let url = format!("{}/sync/peer/produce", peer.url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("POST {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("peer responded {}: {url}", resp.status()));
    }
    let body_bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("read response body: {e}"))?;
    let produced: ProduceResponse = postcard::from_bytes(&body_bytes)
        .map_err(|e| format!("decode ProduceResponse: {e}"))?;
    let ops = open_ops_envelope(&produced.envelope, ident)?;
    if ops.is_empty() {
        return Ok(0);
    }
    let internal = SyncEnvelope {
        from_device: peer_device,
        to_group: ident.group_id,
        nonce: [0u8; 24],
        ciphertext: postcard::to_allocvec(&ops)
            .map_err(|e| format!("re-encode ops: {e}"))?,
    };
    let applied = engine
        .apply_changes(peer_device, internal)
        .await
        .map_err(|e| format!("apply_changes: {e}"))?;
    rebroadcast_touched_notes(mosaic_root, store, index, ws_tx, &applied.note_ids).await;
    Ok(applied.applied)
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
async fn rebroadcast_touched_notes(
    mosaic_root: &Path,
    store: &FsNoteStore,
    index: &SqliteIndex,
    ws_tx: &broadcast::Sender<WsEvent>,
    note_ids: &[[u8; 16]],
) {
    if note_ids.is_empty() {
        return;
    }
    let slug_index = match build_slug_index(mosaic_root).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("sync_peer: failed to build slug index: {e}");
            return;
        }
    };
    for nid in note_ids {
        let Some(slug) = slug_index.get(nid) else {
            continue;
        };
        let note_id = NoteId::new(slug);
        match store.get(&note_id).await {
            Ok(Some(note)) => {
                if let Err(e) = index.reindex(&note).await {
                    tracing::warn!("sync_peer: reindex {slug} after apply: {e}");
                }
                let _ = ws_tx.send(WsEvent::NoteUpdated { note });
            }
            Ok(None) => {
                let _ = ws_tx.send(WsEvent::NoteDeleted {
                    id: slug.to_string(),
                });
            }
            Err(e) => {
                tracing::warn!("sync_peer: store.get {slug} after apply: {e}");
            }
        }
    }
}

/// One-shot scan of `mosaic_root/notes/` returning a `note_id -> slug`
/// map. The note_id derivation matches `stable_uuid_from_slug` in
/// `routes/notes.rs` (blake3 of slug bytes, truncated to 16 bytes).
async fn build_slug_index(mosaic_root: &Path) -> Result<std::collections::HashMap<[u8; 16], String>, String> {
    let notes_dir = mosaic_root.join("notes");
    let mut entries = match tokio::fs::read_dir(&notes_dir).await {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(std::collections::HashMap::new());
        }
        Err(e) => return Err(format!("read_dir {}: {e}", notes_dir.display())),
    };
    let mut out = std::collections::HashMap::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| format!("read_dir entry: {e}"))?
    {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let hash = blake3::hash(stem.as_bytes());
        let mut key = [0u8; 16];
        key.copy_from_slice(&hash.as_bytes()[..16]);
        out.insert(key, stem.to_string());
    }
    Ok(out)
}

async fn write_peers(mosaic_root: &Path, peers: &[Peer]) -> Result<(), std::io::Error> {
    let tesela_dir = mosaic_root.join(".tesela");
    tokio::fs::create_dir_all(&tesela_dir).await?;
    let path = peers_path(mosaic_root);
    let bytes = serde_json::to_vec_pretty(peers)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
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
