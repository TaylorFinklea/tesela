//! Desktop ↔ Cloudflare-relay presence bridge (Phase 3b, Stage 2).
//!
//! Maintains a long-lived WebSocket to the CF relay's
//! `GET /groups/{id}/presence/ws` endpoint and bridges it with this
//! server's local `/ws` PRES fan-out:
//!
//! - **OUTBOUND** — locally-originated `PRES` frames (cursor/selection from
//!   web clients on this server's `/ws`) are observed on `ws_delta_tx`
//!   (origin = `Some(conn)`), AEAD-sealed, and sent to the relay. A
//!   CF-injected frame is re-published with `origin = None`, so the
//!   `origin.is_some()` filter is the loop guard — we never re-send what we
//!   received from the relay.
//! - **INBOUND** — frames the relay broadcasts from OTHER devices are
//!   AEAD-opened and fanned out on `ws_delta_tx` with `origin = None`
//!   (mirroring the `sync_relay` tick fan-out), so every local socket
//!   converges. The relay never echoes our own frames back (it excludes this
//!   device's socket), so no inbound self-echo guard is needed here.
//!
//! ## Zero-knowledge
//!
//! The relay only ever sees `postcard(OuterPayload{nonce, ciphertext})` —
//! opaque XChaCha20-Poly1305 ciphertext sealed under the group key with the
//! group-only [`presence_aad`]. Plaintext presence never leaves this process
//! unencrypted and is NEVER logged.

use std::time::Duration;

use base64::Engine as _;
use futures::{SinkExt, StreamExt};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};

use tesela_sync::crypto::aead::{open as aead_open, presence_aad, seal as aead_seal};
use tesela_sync::crypto::keys::GroupKey;
use tesela_sync::crypto::relay_auth::{
    canonical_request, compute_request_mac, derive_relay_auth_key,
};
use tesela_sync::device::DeviceId;
use tesela_sync::group::GroupId;

use crate::routes::ws::is_presence_frame;
use crate::state::WsDelta;

/// Min/max reconnect backoff. Doubles from `MIN` to `MAX` on consecutive
/// connect/session failures; resets to `MIN` after a successful connect.
const BACKOFF_MIN: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(32);

/// WS keep-alive: ping the relay every 30s so a NAT/edge-dropped idle socket
/// is detected and reconnected (CF DO idle eviction + Stage-1 residual #3).
const HEARTBEAT: Duration = Duration::from_secs(30);

/// The opaque wire payload the relay stores + broadcasts verbatim. Same shape
/// as the `OuterPayload` `RelayClient` uses for `/ops` + `/snapshots`
/// (postcard-structural — defined locally so this module owns no relay
/// internals). `ciphertext` is the AEAD seal of the inner `b"PRES" ++ json`
/// frame; the relay never opens it.
#[derive(Debug, Serialize, Deserialize)]
struct OuterPayload {
    nonce: [u8; 24],
    ciphertext: Vec<u8>,
}

/// Build the presence WebSocket URL from the relay base URL by scheme-swapping
/// (`http` → `ws`, `https` → `wss`) and joining `/groups/{hex}/presence/ws`.
/// Returns `None` for a base URL whose scheme isn't http/https.
fn presence_ws_url(relay_base: &reqwest::Url, group_id: &GroupId) -> Option<String> {
    let ws_scheme = match relay_base.scheme() {
        "https" => "wss",
        "http" => "ws",
        // Already a ws/wss base, or something unexpected — pass through ws/wss.
        "wss" => "wss",
        "ws" => "ws",
        other => {
            tracing::warn!("presence relay: unsupported relay scheme `{other}` — no presence");
            return None;
        }
    };
    let authority = relay_base.authority();
    if authority.is_empty() {
        tracing::warn!("presence relay: relay URL has no host — no presence");
        return None;
    }
    Some(format!(
        "{ws_scheme}://{authority}/groups/{}/presence/ws",
        hex::encode(group_id.as_bytes())
    ))
}

/// Spawn the presence bridge task. Non-blocking: returns immediately and the
/// task runs for the life of the process, reconnecting with backoff. A no-op
/// (logs + returns) if the relay URL can't be turned into a ws/wss URL.
pub fn spawn(
    relay_base: &reqwest::Url,
    group_id: GroupId,
    device_id: DeviceId,
    group_key: GroupKey,
    ws_delta_tx: broadcast::Sender<WsDelta>,
) {
    let Some(ws_url) = presence_ws_url(relay_base, &group_id) else {
        return;
    };
    // Same MAC key the rest of the relay client uses (cached, allocation-free
    // request signing). AEAD key (group_key) and MAC key (auth_key) stay
    // distinct — never crossed.
    let auth_key = derive_relay_auth_key(&group_key, &group_id);
    let mut delta_rx = ws_delta_tx.subscribe();

    tokio::spawn(async move {
        let mut backoff = BACKOFF_MIN;
        loop {
            match connect(&ws_url, &group_id, &device_id, &auth_key).await {
                Ok(ws_stream) => {
                    tracing::info!("presence relay: connected to {ws_url}");
                    backoff = BACKOFF_MIN;
                    run_session(
                        ws_stream,
                        &mut delta_rx,
                        &ws_delta_tx,
                        &group_key,
                        &group_id,
                    )
                    .await;
                    tracing::debug!("presence relay: session ended; reconnecting");
                }
                Err(e) => {
                    tracing::warn!(
                        "presence relay: connect failed: {e}; retrying in {:?}",
                        backoff
                    );
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(BACKOFF_MAX);
        }
    });
}

/// Type alias for the connected presence socket.
type PresenceSocket = tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
>;

/// Open the presence WebSocket, authenticating the upgrade GET with the same
/// MAC scheme as the other relay calls. The signed canonical path MUST be
/// `/groups/{hex}/presence/ws` (CF rebuilds the canonical from
/// `x-tesela-original-path`).
async fn connect(
    ws_url: &str,
    group_id: &GroupId,
    device_id: &DeviceId,
    auth_key: &[u8; 32],
) -> Result<PresenceSocket, String> {
    let mut nonce = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce);
    let nonce_b64 = base64::engine::general_purpose::STANDARD.encode(nonce);
    let ts = now_secs_i64();
    let path = format!("/groups/{}/presence/ws", hex::encode(group_id.as_bytes()));
    // Empty query, empty body hash — mirror `GET /ops`.
    let canonical = canonical_request("GET", &path, "", &nonce_b64, ts, "");
    let mac = compute_request_mac(auth_key, &canonical);
    let mac_b64 = base64::engine::general_purpose::STANDARD.encode(mac);

    let mut request = ws_url
        .into_client_request()
        .map_err(|e| format!("build presence request: {e}"))?;
    let headers = request.headers_mut();
    let set = |headers: &mut tokio_tungstenite::tungstenite::http::HeaderMap,
               name: &'static str,
               value: &str|
     -> Result<(), String> {
        let v = HeaderValue::from_str(value).map_err(|e| format!("header {name}: {e}"))?;
        headers.insert(name, v);
        Ok(())
    };
    set(headers, "x-tesela-group", &hex::encode(group_id.as_bytes()))?;
    set(headers, "x-tesela-device", &hex::encode(device_id.as_bytes()))?;
    set(headers, "x-tesela-nonce", &nonce_b64)?;
    set(headers, "x-tesela-ts", &ts.to_string())?;
    set(headers, "x-tesela-mac", &mac_b64)?;

    let (ws_stream, _resp) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("presence ws connect: {e}"))?;
    Ok(ws_stream)
}

/// Drive one connected session until the socket closes or errors. Bridges
/// outbound (local presence → relay) and inbound (relay → local fan-out),
/// with a 30s heartbeat ping.
async fn run_session(
    ws_stream: PresenceSocket,
    delta_rx: &mut broadcast::Receiver<WsDelta>,
    ws_delta_tx: &broadcast::Sender<WsDelta>,
    group_key: &GroupKey,
    group_id: &GroupId,
) {
    let (mut sink, mut stream) = ws_stream.split();
    let mut heartbeat = tokio::time::interval(HEARTBEAT);
    // The first immediate tick would fire a ping before any idle period; skip it.
    heartbeat.tick().await;

    loop {
        tokio::select! {
            // OUTBOUND: a locally-originated presence frame on the WS fan-out.
            delta = delta_rx.recv() => match delta {
                Ok(WsDelta { origin, frame }) => {
                    // Loop guard: only forward LOCAL presence (origin set). A
                    // CF-injected frame is re-published with origin = None.
                    if origin.is_none() || !is_presence_frame(&frame) {
                        continue;
                    }
                    let outer = match seal_frame(group_key, group_id, &frame) {
                        Ok(o) => o,
                        Err(e) => {
                            tracing::warn!("presence relay: seal outbound: {e}");
                            continue;
                        }
                    };
                    if sink.send(Message::Binary(outer.into())).await.is_err() {
                        break;
                    }
                }
                // Ephemeral: a lagged fan-out just means we dropped some cursor
                // moves while busy — peers re-publish on the next move. Resync.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
            // INBOUND: a frame the relay broadcast from another device.
            msg = stream.next() => match msg {
                Some(Ok(Message::Binary(bytes))) => {
                    match open_frame(group_key, group_id, &bytes) {
                        Ok(inner) => {
                            // The relay only broadcasts to OTHER devices' sockets
                            // (it excludes ours), so an inbound frame is never our
                            // own echo — fan it out to every local socket.
                            let _ = ws_delta_tx.send(WsDelta { origin: None, frame: inner });
                        }
                        Err(e) => {
                            tracing::warn!("presence relay: open inbound (skipping): {e}");
                        }
                    }
                }
                Some(Ok(Message::Ping(payload))) => {
                    if sink.send(Message::Pong(payload)).await.is_err() {
                        break;
                    }
                }
                // Pong / Text / Frame are not part of the protocol; ignore.
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    tracing::debug!("presence relay: ws error: {e}");
                    break;
                }
                None => break,
            },
            // HEARTBEAT: keep the idle socket alive / detect a dead peer.
            _ = heartbeat.tick() => {
                if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }
        }
    }
}

/// Seal a local `b"PRES" ++ json` frame into the postcard outer-payload bytes
/// the relay forwards opaquely. AEAD under the group key + group-only AAD.
fn seal_frame(group_key: &GroupKey, group_id: &GroupId, frame: &[u8]) -> Result<Vec<u8>, String> {
    let aad = presence_aad(group_id.as_bytes());
    let sealed = aead_seal(group_key, frame, &aad).map_err(|e| e.to_string())?;
    let outer = OuterPayload {
        nonce: sealed.nonce,
        ciphertext: sealed.ciphertext,
    };
    postcard::to_allocvec(&outer).map_err(|e| format!("postcard outer: {e}"))
}

/// Open an inbound postcard outer payload back to the inner `b"PRES" ++ json`
/// frame. Errors on a malformed outer, a foreign/rotated key, or an AAD
/// mismatch.
fn open_frame(group_key: &GroupKey, group_id: &GroupId, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let outer: OuterPayload =
        postcard::from_bytes(bytes).map_err(|e| format!("postcard outer: {e}"))?;
    let aad = presence_aad(group_id.as_bytes());
    let inner = aead_open(group_key, &outer.nonce, &outer.ciphertext, &aad)
        .map_err(|e| e.to_string())?;
    if !is_presence_frame(&inner) {
        return Err("opened payload is not a PRES frame".into());
    }
    Ok(inner)
}

fn now_secs_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (GroupKey, GroupId) {
        (GroupKey::from_bytes([0x77; 32]), GroupId::from_bytes([0x42; 16]))
    }

    #[test]
    fn presence_ws_url_scheme_swaps() {
        let g = GroupId::from_bytes([0xab; 16]);
        let hex = hex::encode(g.as_bytes());
        assert_eq!(
            presence_ws_url(&reqwest::Url::parse("https://relay.example.com").unwrap(), &g),
            Some(format!("wss://relay.example.com/groups/{hex}/presence/ws"))
        );
        assert_eq!(
            presence_ws_url(&reqwest::Url::parse("http://127.0.0.1:8787").unwrap(), &g),
            Some(format!("ws://127.0.0.1:8787/groups/{hex}/presence/ws"))
        );
    }

    #[test]
    fn seal_then_open_round_trips_the_frame() {
        let (key, group) = fixture();
        let frame = b"PRES{\"peer\":\"aa\",\"color\":\"#fff\",\"slug\":\"d\",\"bid\":\"b\",\"offset\":3}";
        let sealed = seal_frame(&key, &group, frame).unwrap();
        let opened = open_frame(&key, &group, &sealed).unwrap();
        assert_eq!(opened, frame);
    }

    #[test]
    fn open_fails_under_a_different_group() {
        let (key, group) = fixture();
        let other = GroupId::from_bytes([0x43; 16]);
        let frame = b"PRES{\"peer\":\"aa\",\"color\":\"#fff\",\"slug\":\"d\",\"bid\":\"b\",\"offset\":3}";
        let sealed = seal_frame(&key, &group, frame).unwrap();
        assert!(
            open_frame(&key, &other, &sealed).is_err(),
            "a presence frame sealed for one group must not open under another"
        );
    }
}
