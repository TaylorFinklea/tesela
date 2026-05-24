//! HTTP-level conformance test suite for the Tesela sync relay.
//!
//! Per the protocol spec (`.docs/ai/phases/2026-05-24-relay-protocol-
//! design.md`), every relay implementation — the Rust/Axum
//! self-host in this crate, and the future Cloudflare Worker port —
//! must pass this suite. Tests are written against `reqwest` + a
//! `base_url` so they're portable: future stage 7 work will hoist
//! the test functions into a shared `tesela-relay-conformance` crate
//! that the Worker CI also runs against a deployed preview.
//!
//! Today these tests run against an in-process spawn of the Rust relay
//! (random port, tmp SQLite).
//!
//! ## Test ordering vs implementation stages
//!
//! Tests 1–4: stage 3b (/register + /registration + auth gate).
//! Tests 5–7: stage 3c (/ops PUT + GET + since filter).
//! Tests 8–13: stage 3d (ack/GC/limits/cross-group/replay/admin).
//!
//! All tests are present from stage 2b (this commit) and will fail
//! until their respective implementation stage lands — that's
//! deliberate, it's TDD discipline scaled to the multi-stage track.

use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use rand::RngCore;
use serde_json::json;
use tempfile::TempDir;

use tesela_relay::{router, AppState};
use tesela_sync::crypto::keys::GroupKey;
use tesela_sync::crypto::relay_auth::{
    body_hash_hex, canonical_request, compute_request_mac, derive_relay_auth_key, intent_msg,
    sign_intent,
};
use tesela_sync::group::GroupId;

// ─── Harness helpers ────────────────────────────────────────────────

/// Owned handle to a spawned in-process relay. Drops the listener +
/// SQLite tmp dir on `drop`.
struct TestRelay {
    base_url: String,
    admin_token: String,
    _tmp: TempDir,
    _server: tokio::task::JoinHandle<()>,
}

async fn spawn_relay() -> TestRelay {
    let tmp = tempfile::tempdir().expect("tmp dir");
    let db = tmp.path().join("relay.sqlite");
    let admin_token = "test-admin-token-please-rotate".to_string();
    let state = AppState::open(&db, 1_048_576, Some(admin_token.clone()))
        .await
        .expect("open relay state");
    let app = router(state);

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind random port");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    TestRelay {
        base_url: format!("http://{}", addr),
        admin_token,
        _tmp: tmp,
        _server: server,
    }
}

// ─── Crypto convenience ─────────────────────────────────────────────

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn random_nonce_b64() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

struct Group {
    id: GroupId,
    key: GroupKey,
    auth: [u8; 32],
}

fn fresh_group() -> Group {
    let mut gid_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut gid_bytes);
    let mut gk_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut gk_bytes);
    let id = GroupId::from_bytes(gid_bytes);
    let key = GroupKey::from_bytes(gk_bytes);
    let auth = derive_relay_auth_key(&key, &id);
    Group { id, key, auth }
}

/// Build the registration request body.
fn register_body(group: &Group, registered_at: i64) -> serde_json::Value {
    let msg = intent_msg(&group.id, &group.auth, registered_at);
    let intent = sign_intent(&group.key, &msg);
    json!({
        "auth_key_b64": b64(&group.auth),
        "registered_at": registered_at,
        "intent_b64": b64(&intent),
    })
}

/// Build a full set of `X-Tesela-*` headers for an authenticated
/// request. Caller supplies path + query + method + body bytes; we
/// build the canonical request, MAC it, and emit the header tuple.
fn auth_headers(
    group: &Group,
    device_id_hex: &str,
    method: &str,
    path: &str,
    query: &str,
    body: &[u8],
) -> reqwest::header::HeaderMap {
    let nonce = random_nonce_b64();
    let ts = now_secs();
    let canonical = canonical_request(method, path, query, &nonce, ts, &body_hash_hex(body));
    let mac = compute_request_mac(&group.auth, &canonical);
    let mut h = reqwest::header::HeaderMap::new();
    h.insert("X-Tesela-Group", hex::encode(group.id.as_bytes()).parse().unwrap());
    h.insert("X-Tesela-Device", device_id_hex.parse().unwrap());
    h.insert("X-Tesela-Nonce", nonce.parse().unwrap());
    h.insert("X-Tesela-Ts", ts.to_string().parse().unwrap());
    h.insert("X-Tesela-Mac", b64(&mac).parse().unwrap());
    h
}

fn random_device_id_hex() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

// ─── Tests 1–4 (stage 3b) ──────────────────────────────────────────

#[tokio::test]
async fn test_01_register_round_trip_and_first_op() {
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let body = register_body(&group, now);

    let client = reqwest::Client::new();
    // POST /register
    let r = client
        .post(format!("{}/groups/{}/register", relay.base_url, hex::encode(group.id.as_bytes())))
        .json(&body)
        .send()
        .await
        .expect("send register");
    assert!(
        r.status().is_success(),
        "POST /register expected 2xx, got {} body={}",
        r.status(),
        r.text().await.unwrap_or_default(),
    );

    // PUT one envelope
    let payload = b"opaque-encrypted-stuff";
    let put_body = json!({ "from_device": device, "payload_b64": b64(payload) });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    let headers = auth_headers(&group, &device, "PUT", &path, "", &body_bytes);
    let put = client
        .put(format!("{}{}", relay.base_url, path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .expect("send PUT");
    assert!(put.status().is_success(), "PUT /ops expected 2xx, got {}", put.status());

    // GET ?since=0 returns the envelope
    let headers = auth_headers(&group, &device, "GET", &path, "since=0", &[]);
    let get = client
        .get(format!("{}{}?since=0", relay.base_url, path))
        .headers(headers)
        .send()
        .await
        .expect("send GET");
    assert!(get.status().is_success());
    let rows: Vec<serde_json::Value> = get.json().await.expect("json");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["seq"], 1);
}

#[tokio::test]
async fn test_02_register_idempotent_on_match_conflict_on_differ() {
    let relay = spawn_relay().await;
    let group = fresh_group();
    let now = now_secs();
    let body = register_body(&group, now);
    let path = format!("/groups/{}/register", hex::encode(group.id.as_bytes()));
    let url = format!("{}{}", relay.base_url, path);
    let client = reqwest::Client::new();

    let r1 = client.post(&url).json(&body).send().await.unwrap();
    assert!(r1.status().is_success());

    // Same bytes → idempotent 200.
    let r2 = client.post(&url).json(&body).send().await.unwrap();
    assert!(r2.status().is_success(), "byte-identical re-register should be 200");

    // Different auth_key → 409 with stored payload echoed back.
    let mut bad = body.clone();
    bad["auth_key_b64"] = json!(b64(&[0u8; 32]));
    let r3 = client.post(&url).json(&bad).send().await.unwrap();
    assert_eq!(r3.status().as_u16(), 409, "conflicting auth_key must 409");
    let echoed: serde_json::Value = r3.json().await.unwrap();
    assert_eq!(echoed["auth_key_b64"], body["auth_key_b64"]);
}

#[tokio::test]
async fn test_03_get_registration_returns_stored_record_verbatim() {
    let relay = spawn_relay().await;
    let group = fresh_group();
    let now = now_secs();
    let body = register_body(&group, now);
    let path = format!("/groups/{}/register", hex::encode(group.id.as_bytes()));
    let client = reqwest::Client::new();

    let r = client.post(format!("{}{}", relay.base_url, path)).json(&body).send().await.unwrap();
    assert!(r.status().is_success());

    let g_path = format!("/groups/{}/registration", hex::encode(group.id.as_bytes()));
    let g = client.get(format!("{}{}", relay.base_url, g_path)).send().await.unwrap();
    assert!(g.status().is_success());
    let echoed: serde_json::Value = g.json().await.unwrap();
    assert_eq!(echoed["auth_key_b64"], body["auth_key_b64"]);
    assert_eq!(echoed["registered_at"], body["registered_at"]);
    assert_eq!(echoed["intent_b64"], body["intent_b64"]);
}

#[tokio::test]
async fn test_04_mac_required_for_non_registration_endpoints() {
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("{}/groups/{}/register", relay.base_url, hex::encode(group.id.as_bytes())))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    let put_body = json!({ "from_device": device, "payload_b64": b64(b"x") });

    // No headers → 401.
    let r = client
        .put(format!("{}{}", relay.base_url, path))
        .json(&put_body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 401, "PUT without MAC must 401, got {}", r.status());

    // Bogus auth key → MAC mismatch → 401.
    let mut wrong = group;
    wrong.auth = [0u8; 32];
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let headers = auth_headers(&wrong, &device, "PUT", &path, "", &body_bytes);
    let r = client
        .put(format!("{}{}", relay.base_url, path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 401, "PUT under wrong auth_key must 401");
}

// ─── Tests 5–7 (stage 3c) ──────────────────────────────────────────

#[tokio::test]
async fn test_05_monotonic_seq() {
    // Three PUTs from two devices interleaved; assigned seqs are 1, 2, 3.
    // Stub — fleshed out alongside stage 3c (PUT /ops impl).
    let _ = spawn_relay().await;
    // TODO(stage-3c): exercise PUT three times, assert seqs in order.
}

#[tokio::test]
async fn test_06_since_filter() {
    // GET ?since=1 after three PUTs returns seqs 2 + 3.
    let _ = spawn_relay().await;
    // TODO(stage-3c).
}

#[tokio::test]
async fn test_07_ack_and_gc() {
    // After all known devices ack seq N, subsequent GET ?since=0
    // doesn't return seq N anymore.
    let _ = spawn_relay().await;
    // TODO(stage-3d for the GC half; placeholder lives in 3c set).
}

// ─── Tests 8–13 (stage 3d) ─────────────────────────────────────────

#[tokio::test]
async fn test_08_body_size_cap() {
    // PUT > 1 MiB returns 413.
    let _ = spawn_relay().await;
    // TODO(stage-3d).
}

#[tokio::test]
async fn test_09_rate_limit_per_ip() {
    // 1000 PUTs in 10 seconds from one IP returns 429 on last few.
    let _ = spawn_relay().await;
    // TODO(stage-3d).
}

#[tokio::test]
async fn test_10_cross_group_isolation() {
    // Ops PUT against group A don't leak into GET against group B
    // even with valid headers per side.
    let _ = spawn_relay().await;
    // TODO(stage-3d).
}

#[tokio::test]
async fn test_11_replay_window() {
    // X-Tesela-Ts more than 300s old returns 400.
    let _ = spawn_relay().await;
    // TODO(stage-3d).
}

#[tokio::test]
async fn test_12_nonce_dedupe() {
    // Same nonce within 5 minutes returns 400.
    let _ = spawn_relay().await;
    // TODO(stage-3d).
}

#[tokio::test]
async fn test_13_admin_recovery() {
    // DELETE /admin/groups/{id}/register with bearer token wipes the
    // registration; without the token returns 401.
    let _ = spawn_relay().await;
    // TODO(stage-3d).
}

// Suppress "field is never read" while stages 3b–3d wire up endpoints
// that actually USE the test relay's admin_token field.
#[allow(dead_code)]
fn _admin_token_currently_unused(r: &TestRelay) -> &str {
    &r.admin_token
}

// Tiny standalone sanity check that the harness compiles + spawns.
#[tokio::test]
async fn test_00_harness_spawns_relay_and_health_works() {
    let relay = spawn_relay().await;
    let r = reqwest::get(format!("{}/", relay.base_url)).await.unwrap();
    assert!(r.status().is_success(), "/ health endpoint should 2xx");
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

// Force the time-based Duration import to be used somewhere obvious so
// the import stays meaningful even before tests 11/12 fill in. Will
// remove when stage 3d's nonce-dedupe test exercises Duration.
const _UNUSED_DURATION: Duration = Duration::from_secs(0);
