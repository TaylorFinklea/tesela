//! HTTP handlers.
//!
//! Stage 3b lands `/register`, `/registration`, the MAC middleware
//! that gates every other `/groups/{id}/*` endpoint, plus thin
//! stubs for `/ops` + `/ack` so the MAC gate has something to wrap
//! (real op semantics arrive in stages 3c/3d).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tesela_sync::crypto::relay_auth::{body_hash_hex, canonical_request, verify_request_mac};

use crate::state::AppState;
use crate::store::{RegisterOutcome, Registration};

// ─── Health ────────────────────────────────────────────────────────

/// Lightweight liveness check. Operators wire this into their load
/// balancer / Docker healthcheck.
pub async fn health(State(_state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "tesela-relay" }))
}

// ─── /register + /registration ─────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub auth_key_b64: String,
    pub registered_at: i64,
    pub intent_b64: String,
}

#[derive(Debug, Serialize)]
pub struct RegistrationRecord {
    pub auth_key_b64: String,
    pub registered_at: i64,
    pub intent_b64: String,
}

impl From<Registration> for RegistrationRecord {
    fn from(r: Registration) -> Self {
        let b64 = base64::engine::general_purpose::STANDARD;
        Self {
            auth_key_b64: b64.encode(&r.auth_key),
            registered_at: r.registered_at,
            intent_b64: b64.encode(&r.intent),
        }
    }
}

/// `POST /groups/{group_id}/register`
///
/// First-write trust-on-first-use registration. Body carries the
/// deterministic per-group `auth_key` (derived via HKDF from the
/// group key, see `tesela_sync::crypto::relay_auth`) plus a signed
/// intent that ONLY group-key holders can produce. Relay stores the
/// payload verbatim and from this point uses `auth_key` to verify
/// per-request MACs on every other endpoint.
///
/// Idempotent on byte-identical re-register; returns `409` with the
/// stored payload echoed on conflict so joiners can detect hijack
/// squatting.
pub async fn register(
    State(state): State<AppState>,
    Path(group_id_hex): Path<String>,
    Json(req): Json<RegisterRequest>,
) -> Response {
    let Some(group_id) = parse_group_id(&group_id_hex) else {
        return (StatusCode::BAD_REQUEST, "invalid group_id hex").into_response();
    };
    let b64 = base64::engine::general_purpose::STANDARD;
    let Ok(auth_key_vec) = b64.decode(&req.auth_key_b64) else {
        return (StatusCode::BAD_REQUEST, "auth_key_b64 not base64").into_response();
    };
    let Ok(auth_key_arr): Result<[u8; 32], _> = auth_key_vec.try_into() else {
        return (StatusCode::BAD_REQUEST, "auth_key must be 32 bytes").into_response();
    };
    let Ok(intent_vec) = b64.decode(&req.intent_b64) else {
        return (StatusCode::BAD_REQUEST, "intent_b64 not base64").into_response();
    };

    match state
        .inner
        .store
        .register_group(&group_id, &auth_key_arr, req.registered_at, &intent_vec)
        .await
    {
        Ok(RegisterOutcome::Inserted) | Ok(RegisterOutcome::Idempotent) => {
            (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response()
        }
        Ok(RegisterOutcome::Conflict(existing)) => {
            let echoed: RegistrationRecord = existing.into();
            (StatusCode::CONFLICT, Json(echoed)).into_response()
        }
        Err(e) => internal_err(&e.to_string()),
    }
}

/// `GET /groups/{group_id}/registration`
///
/// Returns the stored registration record verbatim. Joiners call
/// this on first connect, recompute the signed intent locally using
/// their group key, and refuse to use the relay if it doesn't match
/// — that's the Certificate-Transparency-style hijack-detection
/// path the protocol rests on.
pub async fn get_registration(
    State(state): State<AppState>,
    Path(group_id_hex): Path<String>,
) -> Response {
    let Some(group_id) = parse_group_id(&group_id_hex) else {
        return (StatusCode::BAD_REQUEST, "invalid group_id hex").into_response();
    };
    match state.inner.store.get_registration(&group_id).await {
        Ok(Some(r)) => Json::<RegistrationRecord>(r.into()).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "group not registered").into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

// ─── Stubs (real impl lands in 3c/3d) ──────────────────────────────

// ─── /ops (stage 3c) ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PutOpRequest {
    /// Hex-encoded device id (32 bytes / 64 chars). The MAC gate
    /// already verified this matches `X-Tesela-Device`; we re-read it
    /// here so the stored op carries it as canonical metadata.
    pub from_device: String,
    /// Opaque AEAD-sealed payload, base64-encoded. The relay never
    /// looks at the bytes — they're inner-envelope content meant only
    /// for group-key holders.
    pub payload_b64: String,
}

#[derive(Debug, Serialize)]
pub struct OpRecord {
    pub seq: i64,
    pub from_device: String,
    pub ts: f64,
    pub payload_b64: String,
}

/// `PUT /groups/{group_id}/ops`
///
/// Append one envelope to the group's FIFO. The relay assigns the
/// monotonic per-group `seq` and the wall-clock `ts` atomically (inside
/// a transaction; SQLite serialises writes so concurrent PUTs from
/// different HTTP threads can't collide). Returns the assigned `(seq, ts)`
/// so the client can pin its own "last-deposited" cursor without an
/// extra GET round-trip.
pub async fn put_op(
    State(state): State<AppState>,
    Path(group_id_hex): Path<String>,
    Json(req): Json<PutOpRequest>,
) -> Response {
    let Some(group_id) = parse_group_id(&group_id_hex) else {
        return (StatusCode::BAD_REQUEST, "invalid group_id hex").into_response();
    };
    let Ok(from_device_vec) = hex::decode(&req.from_device) else {
        return (StatusCode::BAD_REQUEST, "from_device not hex").into_response();
    };
    let Ok(from_device_arr): Result<[u8; 16], _> = from_device_vec.try_into() else {
        return (StatusCode::BAD_REQUEST, "from_device must be 16 bytes (DeviceId)").into_response();
    };
    let b64 = base64::engine::general_purpose::STANDARD;
    let Ok(payload) = b64.decode(&req.payload_b64) else {
        return (StatusCode::BAD_REQUEST, "payload_b64 not base64").into_response();
    };

    let ts = wall_clock_secs_f64();
    match state
        .inner
        .store
        .insert_op(&group_id, &from_device_arr, ts, &payload)
        .await
    {
        Ok((seq, ts)) => {
            // Best-effort touch so PUTs count toward known-members
            // for the GC pass in stage 3d.
            let _ = state
                .inner
                .store
                .touch_device(&group_id, &from_device_arr, ts as i64)
                .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "seq": seq, "ts": ts })),
            )
                .into_response()
        }
        Err(e) => internal_err(&e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct GetOpsQuery {
    /// Return ops with `seq > since`. `since=0` returns all
    /// non-GC'd ops in the group.
    #[serde(default)]
    pub since: i64,
}

/// `GET /groups/{group_id}/ops?since=N`
///
/// Return ops with `seq > since`, ordered ascending. Empty list when
/// the requester is already caught up. The MAC gate has already
/// confirmed the caller is a legitimate group member; here we just
/// translate `RelayOp` → JSON `OpRecord`.
pub async fn get_ops(
    State(state): State<AppState>,
    Path(group_id_hex): Path<String>,
    axum::extract::Query(query): axum::extract::Query<GetOpsQuery>,
    headers: HeaderMap,
) -> Response {
    let Some(group_id) = parse_group_id(&group_id_hex) else {
        return (StatusCode::BAD_REQUEST, "invalid group_id hex").into_response();
    };
    match state.inner.store.list_ops_since(&group_id, query.since).await {
        Ok(rows) => {
            // Touch device-seen if the device header was present —
            // this is the path consumers take (fetch + ack but never
            // PUT), and they need to count as known members for GC.
            if let Some(device_hex) = header_str(&headers, "x-tesela-device") {
                if let Ok(bytes) = hex::decode(device_hex) {
                    if let Ok(arr) = <[u8; 16]>::try_from(bytes) {
                        let _ = state
                            .inner
                            .store
                            .touch_device(&group_id, &arr, wall_clock_secs_f64() as i64)
                            .await;
                    }
                }
            }
            let b64 = base64::engine::general_purpose::STANDARD;
            let records: Vec<OpRecord> = rows
                .into_iter()
                .map(|r| OpRecord {
                    seq: r.seq,
                    from_device: hex::encode(&r.from_device),
                    ts: r.ts,
                    payload_b64: b64.encode(&r.payload),
                })
                .collect();
            (StatusCode::OK, Json(records)).into_response()
        }
        Err(e) => internal_err(&e.to_string()),
    }
}

// ─── /ack (stage 3d) ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AckRequest {
    /// Hex-encoded device id of the device confirming application.
    pub device: String,
    /// Highest seq this device has applied. The relay marks every op
    /// up to and including this seq as acked by `device`; runs GC if
    /// every known group member has acked.
    pub applied_seq: i64,
}

/// `POST /groups/{group_id}/ack`
///
/// Record that `device` has applied every op with `seq <= applied_seq`
/// in this group. Triggers a GC pass: any op every known group member
/// has acked gets deleted.
///
/// "Known members" = devices that have touched this group within the
/// last 30 days (`KNOWN_MEMBER_TTL_SECS`); see `state::touch_device`
/// + `state::known_members_hex`. This means a phone offline more than
/// 30 days loses its backlog — fine for the deposit-box model, where
/// the authoritative source is the desktop's on-disk state.
pub async fn post_ack(
    State(state): State<AppState>,
    Path(group_id_hex): Path<String>,
    Json(req): Json<AckRequest>,
) -> Response {
    let Some(group_id) = parse_group_id(&group_id_hex) else {
        return (StatusCode::BAD_REQUEST, "invalid group_id hex").into_response();
    };
    let Ok(device_vec) = hex::decode(&req.device) else {
        return (StatusCode::BAD_REQUEST, "device not hex").into_response();
    };
    let Ok(device_arr): Result<[u8; 16], _> = device_vec.try_into() else {
        return (StatusCode::BAD_REQUEST, "device must be 16 bytes (DeviceId)").into_response();
    };

    if let Err(e) = state
        .inner
        .store
        .ack_ops(&group_id, &device_arr, req.applied_seq)
        .await
    {
        return internal_err(&e.to_string());
    }
    let now = wall_clock_secs_f64() as i64;
    let _ = state
        .inner
        .store
        .touch_device(&group_id, &device_arr, now)
        .await;

    // GC pass — every op acked by every known member can be dropped.
    let known = state
        .inner
        .store
        .known_members_hex(&group_id, now, KNOWN_MEMBER_TTL_SECS)
        .await
        .unwrap_or_default();
    let _gc = state
        .inner
        .store
        .gc_fully_acked_ops(&group_id, &known)
        .await
        .unwrap_or(0);

    (StatusCode::OK, Json(serde_json::json!({ "ok": true }))).into_response()
}

/// Retention window for known group members. A device that hasn't
/// been seen for longer is forgotten and its un-acked backlog becomes
/// GC-eligible. 30 days mirrors the spec; tuned together with this
/// retention contract.
pub const KNOWN_MEMBER_TTL_SECS: i64 = 30 * 24 * 60 * 60;

// ─── Admin recovery (stage 3d) ─────────────────────────────────────

/// `DELETE /admin/groups/{group_id}/register`
///
/// Hijack-recovery escape hatch. Wipes the stored registration so the
/// legitimate group owner can re-register from a known-good device.
/// Gated on `Authorization: Bearer <admin_token>`; admin endpoints
/// are disabled (404) when the binary started without `--admin-token`.
pub async fn admin_delete_registration(
    State(state): State<AppState>,
    Path(group_id_hex): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(expected_token) = state.inner.admin_token.as_deref() else {
        return (StatusCode::NOT_FOUND, "admin endpoints disabled").into_response();
    };
    let auth = header_str(&headers, "authorization").unwrap_or("");
    let Some(supplied) = auth.strip_prefix("Bearer ") else {
        return (StatusCode::UNAUTHORIZED, "missing bearer token").into_response();
    };
    if !constant_time_eq(supplied.as_bytes(), expected_token.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, "bad admin token").into_response();
    }
    let Some(group_id) = parse_group_id(&group_id_hex) else {
        return (StatusCode::BAD_REQUEST, "invalid group_id hex").into_response();
    };
    match state.inner.store.delete_registration(&group_id).await {
        Ok(true) => (StatusCode::NO_CONTENT, "").into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "group not registered").into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

/// Constant-time byte-slice comparison so admin-token comparison
/// doesn't leak via wall-clock timing. Implements its own loop
/// instead of pulling in the `subtle` crate for one call site.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ─── MAC middleware ────────────────────────────────────────────────

/// Replay-window tolerance: requests with `X-Tesela-Ts` more than
/// this many seconds in the past or future are rejected.
const REPLAY_WINDOW_SECS: i64 = 300;

/// Verify the per-request MAC for any endpoint that requires it
/// (everything in `/groups/{id}/*` except `/register` and
/// `/registration`). Failure modes: missing headers (401), invalid
/// timestamp / outside replay window (400), replayed nonce (400),
/// group not registered (401), MAC mismatch (401).
pub async fn mac_gate(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    // Pull group_id from the URI path: `/groups/{hex}/...`
    let path = request.uri().path().to_string();
    let Some(group_id_hex) = group_id_from_path(&path) else {
        return (StatusCode::UNAUTHORIZED, "missing group id in path").into_response();
    };
    let Some(group_id) = parse_group_id(group_id_hex) else {
        return (StatusCode::UNAUTHORIZED, "invalid group id hex").into_response();
    };

    let headers = request.headers().clone();
    let Some(mac_b64) = header_str(&headers, "x-tesela-mac") else {
        return (StatusCode::UNAUTHORIZED, "missing X-Tesela-Mac").into_response();
    };
    let Some(nonce) = header_str(&headers, "x-tesela-nonce") else {
        return (StatusCode::UNAUTHORIZED, "missing X-Tesela-Nonce").into_response();
    };
    let Some(ts_str) = header_str(&headers, "x-tesela-ts") else {
        return (StatusCode::UNAUTHORIZED, "missing X-Tesela-Ts").into_response();
    };
    let Ok(ts) = ts_str.parse::<i64>() else {
        return (StatusCode::UNAUTHORIZED, "X-Tesela-Ts not an integer").into_response();
    };

    // Replay-window check. Wall-clock skew tolerance ±300s per spec.
    let now = wall_clock_secs_f64() as i64;
    if (now - ts).abs() > REPLAY_WINDOW_SECS {
        return (StatusCode::BAD_REQUEST, "X-Tesela-Ts outside replay window").into_response();
    }

    // Nonce dedupe — same nonce within `NONCE_TTL` is a replay.
    if !state.record_nonce(&group_id, nonce) {
        return (StatusCode::BAD_REQUEST, "nonce already seen").into_response();
    }

    let registration = match state.inner.store.get_registration(&group_id).await {
        Ok(Some(r)) => r,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "group not registered").into_response(),
        Err(e) => return internal_err(&e.to_string()),
    };
    let Ok(auth_key_arr): Result<[u8; 32], _> = registration.auth_key.clone().try_into() else {
        return internal_err("stored auth_key wrong length");
    };

    // Buffer the body so we can hash it for the MAC + replay it to
    // the downstream handler. Cheap because spec caps PUT bodies at
    // 1 MiB.
    let (parts, body) = request.into_parts();
    let body_bytes = match axum::body::to_bytes(body, state.inner.max_body).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::PAYLOAD_TOO_LARGE, "body too large").into_response(),
    };

    let method = parts.method.as_str();
    let path_only = parts.uri.path();
    let query = parts.uri.query().unwrap_or("");
    let canonical = canonical_request(method, path_only, query, nonce, ts, &body_hash_hex(&body_bytes));

    let b64 = base64::engine::general_purpose::STANDARD;
    let Ok(mac_bytes) = b64.decode(mac_b64) else {
        return (StatusCode::UNAUTHORIZED, "X-Tesela-Mac not base64").into_response();
    };

    if !verify_request_mac(&auth_key_arr, &canonical, &mac_bytes) {
        return (StatusCode::UNAUTHORIZED, "MAC mismatch").into_response();
    }

    // Rebuild the request with the buffered body for the handler.
    let new_req = axum::extract::Request::from_parts(parts, axum::body::Body::from(body_bytes));
    let _ = (mac_b64, header::HeaderMap::new());
    let _state: Arc<()> = Arc::new(());
    next.run(new_req).await
}

/// Per-IP rate limit. Runs before everything else on `/groups/*` so
/// even pre-auth scan traffic gets throttled.
pub async fn rate_gate(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if !state.check_ip_rate(addr.ip()) {
        return (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response();
    }
    next.run(request).await
}

// ─── Helpers ───────────────────────────────────────────────────────

fn parse_group_id(hex_str: &str) -> Option<[u8; 16]> {
    let bytes = hex::decode(hex_str).ok()?;
    bytes.try_into().ok()
}

fn group_id_from_path(path: &str) -> Option<&str> {
    // Match `/groups/{id}/...` or `/admin/groups/{id}/...`.
    let trimmed = path
        .strip_prefix("/admin/groups/")
        .or_else(|| path.strip_prefix("/groups/"))?;
    trimmed.split('/').next()
}

fn header_str<'a>(h: &'a HeaderMap, name: &str) -> Option<&'a str> {
    h.get(name).and_then(|v| v.to_str().ok())
}

fn internal_err(msg: &str) -> Response {
    tracing::error!("internal error: {}", msg);
    (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
}

/// Server-assigned wall-clock timestamp in seconds (f64 to retain
/// sub-second precision in the spec's `ts` field).
fn wall_clock_secs_f64() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
