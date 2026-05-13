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

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path as AxPath, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tesela_sync::oplog::op::EncodedOp;
use tesela_sync::{
    DeviceId, GroupId, PeerCursor, SqliteEngine, SyncEngine, SyncEnvelope,
};

use crate::state::AppState;

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
    pub ops: Vec<EncodedOp>,
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
    let resp = ProduceResponse {
        ops: batch.ops,
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

pub async fn receive_envelope(
    State(s): State<Arc<AppState>>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let envelope: SyncEnvelope = postcard::from_bytes(&body)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("postcard: {e}")))?;
    let from = envelope.from_device;
    let applied = s
        .sync_engine
        .apply_changes(from, envelope)
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

    if produced.ops.is_empty() {
        return Ok(0);
    }

    // Apply locally. Wrap into a SyncEnvelope so apply_changes sees the
    // same shape regardless of transport.
    let envelope = SyncEnvelope {
        from_device: peer_device,
        to_group: GroupId([0u8; 16]),
        nonce: [0u8; 24],
        ciphertext: postcard::to_allocvec(&produced.ops)
            .map_err(|e| format!("re-encode ops: {e}"))?,
    };
    let applied = s
        .sync_engine
        .apply_changes(peer_device, envelope)
        .await
        .map_err(|e| format!("apply_changes: {e}"))?;

    // Re-broadcast WS events for each touched note so connected clients
    // see the new content immediately. The indexer's file-watcher will
    // also fire, but we want zero latency.
    for note_id_bytes in &applied.note_ids {
        // The note id in the oplog is a UUID, but our notes table uses
        // slugs. Without a mapping, we can't easily map UUID -> Note.
        // For Phase 1.5 the indexer's file-watcher does the heavy
        // lifting; clients will see the WsEvent::NoteUpdated when the
        // indexer fires. Skip the eager broadcast here.
        let _ = note_id_bytes;
    }

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
/// the background sync loop in `main.rs`.
pub async fn sync_with_peer_minimal(
    engine: &SqliteEngine,
    _mosaic_root: &Path,
    peer: &Peer,
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
    if produced.ops.is_empty() {
        return Ok(0);
    }
    let envelope = SyncEnvelope {
        from_device: peer_device,
        to_group: GroupId([0u8; 16]),
        nonce: [0u8; 24],
        ciphertext: postcard::to_allocvec(&produced.ops)
            .map_err(|e| format!("re-encode ops: {e}"))?,
    };
    let applied = engine
        .apply_changes(peer_device, envelope)
        .await
        .map_err(|e| format!("apply_changes: {e}"))?;
    Ok(applied.applied)
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
