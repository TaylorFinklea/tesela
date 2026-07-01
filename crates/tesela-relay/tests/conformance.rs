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
use tesela_sync::crypto::recovery::derive_discovery_handle;
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
    // External-target mode: when `TESELA_RELAY_CONFORMANCE_URL` is set,
    // run this same black-box suite against an already-running relay
    // (e.g. the Cloudflare Worker via `wrangler dev`) instead of an
    // in-process Rust spawn. Every test here is pure HTTP against
    // `base_url`, so only the URL + admin token differ — this is the
    // "one suite, both implementations" gate the file header promised.
    if let Ok(url) = std::env::var("TESELA_RELAY_CONFORMANCE_URL") {
        let base_url = url.trim_end_matches('/').to_string();
        let admin_token = std::env::var("TESELA_RELAY_CONFORMANCE_ADMIN_TOKEN")
            .unwrap_or_else(|_| "test-admin-token-please-rotate".to_string());
        return TestRelay {
            base_url,
            admin_token,
            _tmp: tempfile::tempdir().expect("tmp dir"),
            _server: tokio::spawn(async {}),
        };
    }

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
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
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

/// Build a full set of headers for an authenticated request: the
/// `X-Tesela-*` MAC envelope + `Content-Type: application/json` when
/// the body is non-empty (every body in this protocol is JSON; safer
/// to always set it than to leave it to per-call discipline).
/// Caller supplies path + query + method + body bytes; we build the
/// canonical request, MAC it, and emit the header tuple.
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
    h.insert(
        "X-Tesela-Group",
        hex::encode(group.id.as_bytes()).parse().unwrap(),
    );
    h.insert("X-Tesela-Device", device_id_hex.parse().unwrap());
    h.insert("X-Tesela-Nonce", nonce.parse().unwrap());
    h.insert("X-Tesela-Ts", ts.to_string().parse().unwrap());
    h.insert("X-Tesela-Mac", b64(&mac).parse().unwrap());
    if !body.is_empty() {
        h.insert("Content-Type", "application/json".parse().unwrap());
    }
    h
}

fn random_device_id_hex() -> String {
    // 16 bytes = canonical `tesela_sync::device::DeviceId` size.
    let mut bytes = [0u8; 16];
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
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
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
    assert!(
        put.status().is_success(),
        "PUT /ops expected 2xx, got {}",
        put.status()
    );

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
    assert!(
        r2.status().is_success(),
        "byte-identical re-register should be 200"
    );

    // Different auth_key → 409 with the stored record's PUBLIC fields
    // echoed back (registered_at + intent — never the auth_key itself;
    // see test_16).
    let mut bad = body.clone();
    bad["auth_key_b64"] = json!(b64(&[0u8; 32]));
    let r3 = client.post(&url).json(&bad).send().await.unwrap();
    assert_eq!(r3.status().as_u16(), 409, "conflicting auth_key must 409");
    let echoed: serde_json::Value = r3.json().await.unwrap();
    assert_eq!(echoed["registered_at"], body["registered_at"]);
    assert_eq!(echoed["intent_b64"], body["intent_b64"]);
}

#[tokio::test]
async fn test_03_get_registration_returns_stored_public_fields() {
    let relay = spawn_relay().await;
    let group = fresh_group();
    let now = now_secs();
    let body = register_body(&group, now);
    let path = format!("/groups/{}/register", hex::encode(group.id.as_bytes()));
    let client = reqwest::Client::new();

    let r = client
        .post(format!("{}{}", relay.base_url, path))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());

    let g_path = format!("/groups/{}/registration", hex::encode(group.id.as_bytes()));
    let g = client
        .get(format!("{}{}", relay.base_url, g_path))
        .send()
        .await
        .unwrap();
    assert!(g.status().is_success());
    let echoed: serde_json::Value = g.json().await.unwrap();
    // The joiner-verification fields come back verbatim; the auth_key
    // is deliberately absent (test_16 locks that in).
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
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
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
    assert_eq!(
        r.status().as_u16(),
        401,
        "PUT without MAC must 401, got {}",
        r.status()
    );

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
    assert_eq!(
        r.status().as_u16(),
        401,
        "PUT under wrong auth_key must 401"
    );
}

// ─── Tests 5–7 (stage 3c) ──────────────────────────────────────────

#[tokio::test]
async fn test_05_monotonic_seq() {
    // Three PUTs interleaved across two devices; relay-assigned seqs
    // must be 1, 2, 3 in arrival order, regardless of which device
    // PUT them.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device_a = random_device_id_hex();
    let device_b = random_device_id_hex();
    let client = reqwest::Client::new();
    let now = now_secs();
    // Register first.
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    let mut seqs = Vec::new();
    for (i, device) in [&device_a, &device_b, &device_a].iter().enumerate() {
        let put_body =
            json!({ "from_device": device, "payload_b64": b64(format!("op-{i}").as_bytes()) });
        let body_bytes = serde_json::to_vec(&put_body).unwrap();
        let headers = auth_headers(&group, device, "PUT", &path, "", &body_bytes);
        let r = client
            .put(format!("{}{}", relay.base_url, path))
            .headers(headers)
            .body(body_bytes)
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "PUT {} failed: {}", i, r.status());
        let body: serde_json::Value = r.json().await.unwrap();
        seqs.push(body["seq"].as_i64().expect("seq is integer"));
    }
    assert_eq!(seqs, vec![1, 2, 3], "seqs must be monotonic per group");
}

#[tokio::test]
async fn test_06_since_filter() {
    // GET ?since=1 after three PUTs returns seqs 2 + 3 only.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let client = reqwest::Client::new();
    let now = now_secs();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();
    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    for i in 0..3 {
        let put_body =
            json!({ "from_device": device, "payload_b64": b64(format!("op-{i}").as_bytes()) });
        let body_bytes = serde_json::to_vec(&put_body).unwrap();
        let headers = auth_headers(&group, &device, "PUT", &path, "", &body_bytes);
        client
            .put(format!("{}{}", relay.base_url, path))
            .headers(headers)
            .body(body_bytes)
            .send()
            .await
            .unwrap();
    }
    let headers = auth_headers(&group, &device, "GET", &path, "since=1", &[]);
    let r = client
        .get(format!("{}{}?since=1", relay.base_url, path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let rows: Vec<serde_json::Value> = r.json().await.unwrap();
    let seqs: Vec<i64> = rows.iter().map(|v| v["seq"].as_i64().unwrap()).collect();
    assert_eq!(seqs, vec![2, 3], "since=1 must skip seq 1");
}

#[tokio::test]
async fn test_07_ack_retains_durable_log() {
    // Durable-replica retention (encrypted-replica spine, Phase 1a): PUT 2
    // envelopes; the only known device acks through seq 2; the relay is the
    // off-site encrypted backup, so it RETAINS the ops rather than evicting
    // them on ack — a subsequent GET ?since=0 still returns BOTH (a wiped
    // device restores the whole mosaic from this). (Was test_07_ack_and_gc,
    // which asserted ack-triggered eviction; that GC is removed — compaction
    // is now snapshot-gated, Phase 1b.)
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    for i in 0..2 {
        let put_body =
            json!({ "from_device": device, "payload_b64": b64(format!("op-{i}").as_bytes()) });
        let body_bytes = serde_json::to_vec(&put_body).unwrap();
        let headers = auth_headers(&group, &device, "PUT", &path, "", &body_bytes);
        client
            .put(format!("{}{}", relay.base_url, path))
            .headers(headers)
            .body(body_bytes)
            .send()
            .await
            .unwrap();
    }

    // ACK through seq 2.
    let ack_path = format!("/groups/{}/ack", hex::encode(group.id.as_bytes()));
    let ack_body = json!({ "device": device, "applied_seq": 2 });
    let body_bytes = serde_json::to_vec(&ack_body).unwrap();
    let headers = auth_headers(&group, &device, "POST", &ack_path, "", &body_bytes);
    let ack = client
        .post(format!("{}{}", relay.base_url, ack_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert!(ack.status().is_success(), "ACK failed: {}", ack.status());

    // GET ?since=0 should now be empty.
    let headers = auth_headers(&group, &device, "GET", &path, "since=0", &[]);
    let r = client
        .get(format!("{}{}?since=0", relay.base_url, path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = r.json().await.unwrap();
    assert_eq!(
        rows.len(),
        2,
        "durable retention: after ACK the relay RETAINS both ops (encrypted \
         backup + bootstrap source), got {:?}",
        rows
    );
}

// ─── Snapshot store + snapshot-gated compaction (spine Phase 1b-i) ──

#[tokio::test]
async fn test_snapshot_deposit_compacts_oplog() {
    // PUT 3 ops (seq 1,2,3); deposit a snapshot batch covering seq 2;
    // the relay GCs ops with seq <= 2 (gc == 2) and retains seq 3.
    // GET /snapshots returns the deposited snapshot + compaction_seq 2.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let ops_path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    for i in 0..3 {
        let put_body =
            json!({ "from_device": device, "payload_b64": b64(format!("op-{i}").as_bytes()) });
        let body_bytes = serde_json::to_vec(&put_body).unwrap();
        let headers = auth_headers(&group, &device, "PUT", &ops_path, "", &body_bytes);
        client
            .put(format!("{}{}", relay.base_url, ops_path))
            .headers(headers)
            .body(body_bytes)
            .send()
            .await
            .unwrap();
    }

    // Deposit a snapshot batch covering seq 2.
    let stream_id = b"note-stream-key-A";
    let snap_payload = b"opaque-encrypted-snapshot-A";
    let snap_path = format!("/groups/{}/snapshot", hex::encode(group.id.as_bytes()));
    let snap_body = json!({
        "covers_seq": 2,
        "snapshots": [{ "stream_id_b64": b64(stream_id), "payload_b64": b64(snap_payload) }],
    });
    let body_bytes = serde_json::to_vec(&snap_body).unwrap();
    let headers = auth_headers(&group, &device, "PUT", &snap_path, "", &body_bytes);
    let dep = client
        .put(format!("{}{}", relay.base_url, snap_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert!(
        dep.status().is_success(),
        "snapshot deposit failed: {}",
        dep.status()
    );
    let dep_body: serde_json::Value = dep.json().await.unwrap();
    assert_eq!(dep_body["ok"], true);
    assert_eq!(
        dep_body["gc"].as_u64(),
        Some(2),
        "ops seq 1 + 2 must be GC'd"
    );

    // GET /ops?since=0 must now return ONLY seq 3.
    let headers = auth_headers(&group, &device, "GET", &ops_path, "since=0", &[]);
    let r = client
        .get(format!("{}{}?since=0", relay.base_url, ops_path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = r.json().await.unwrap();
    let seqs: Vec<i64> = rows.iter().map(|v| v["seq"].as_i64().unwrap()).collect();
    assert_eq!(
        seqs,
        vec![3],
        "only the un-superseded op (seq 3) survives compaction"
    );

    // GET /snapshots returns the deposited snapshot + compaction_seq 2.
    let snaps_path = format!("/groups/{}/snapshots", hex::encode(group.id.as_bytes()));
    let headers = auth_headers(&group, &device, "GET", &snaps_path, "", &[]);
    let r = client
        .get(format!("{}{}", relay.base_url, snaps_path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["compaction_seq"].as_i64(), Some(2));
    let snaps = body["snapshots"].as_array().expect("snapshots array");
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0]["stream_id_b64"], b64(stream_id));
    assert_eq!(snaps[0]["payload_b64"], b64(snap_payload));
    assert_eq!(snaps[0]["snapshot_seq"].as_i64(), Some(2));
}

#[tokio::test]
async fn deposit_compact_put_delivers() {
    // Seq-black-hole regression (#195 / L1 CT). When a snapshot GCs the ENTIRE
    // oplog, a subsequent PUT must still get a seq ABOVE the compaction
    // watermark — otherwise a consumer already caught up to the pre-compaction
    // high cursor polls `?since=N` and never sees the new op (`seq <= N` → a
    // permanent inbound black hole). Guards `store.rs`
    // `MAX(MAX(seq), compaction_seq) + 1`; reverting that to `MAX(seq) + 1`
    // (MAX over the now-empty oplog = 0 → new seq 1) makes this test fail.
    // The existing compaction test uses covers_seq=2 (seq 3 survives), which
    // masks the bug; full GC is required to expose it.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let ops_path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    // PUT 3 ops → seq 1,2,3; a consumer conceptually catches up to cursor=3.
    for i in 0..3 {
        let put_body =
            json!({ "from_device": device, "payload_b64": b64(format!("op-{i}").as_bytes()) });
        let body_bytes = serde_json::to_vec(&put_body).unwrap();
        let headers = auth_headers(&group, &device, "PUT", &ops_path, "", &body_bytes);
        client
            .put(format!("{}{}", relay.base_url, ops_path))
            .headers(headers)
            .body(body_bytes)
            .send()
            .await
            .unwrap();
    }

    // Deposit a snapshot covering ALL 3 ops → GC the entire oplog,
    // compaction_seq=3 (MAX(seq) over relay_ops now drops to 0).
    let snap_path = format!("/groups/{}/snapshot", hex::encode(group.id.as_bytes()));
    let snap_body = json!({
        "covers_seq": 3,
        "snapshots": [{ "stream_id_b64": b64(b"note-stream-key-A"), "payload_b64": b64(b"opaque-snapshot") }],
    });
    let body_bytes = serde_json::to_vec(&snap_body).unwrap();
    let headers = auth_headers(&group, &device, "PUT", &snap_path, "", &body_bytes);
    let dep = client
        .put(format!("{}{}", relay.base_url, snap_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    let dep_body: serde_json::Value = dep.json().await.unwrap();
    assert_eq!(dep_body["gc"].as_u64(), Some(3), "all 3 ops must be GC'd");

    // PUT a NEW op AFTER full compaction — it MUST land above the watermark.
    let put_body =
        json!({ "from_device": device, "payload_b64": b64(b"op-after-compaction") });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let headers = auth_headers(&group, &device, "PUT", &ops_path, "", &body_bytes);
    let put_resp = client
        .put(format!("{}{}", relay.base_url, ops_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    let put_resp_body: serde_json::Value = put_resp.json().await.unwrap();
    let new_seq = put_resp_body["seq"].as_i64().expect("seq is integer");
    assert!(
        new_seq > 3,
        "post-compaction op must get seq > compaction watermark (3); got {new_seq} \
         (the black hole: MAX(seq)+1 over the GC'd log = 1)"
    );

    // The caught-up consumer (cursor=3) MUST receive the post-compaction op.
    let headers = auth_headers(&group, &device, "GET", &ops_path, "since=3", &[]);
    let r = client
        .get(format!("{}{}?since=3", relay.base_url, ops_path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = r.json().await.unwrap();
    let seqs: Vec<i64> = rows.iter().map(|v| v["seq"].as_i64().unwrap()).collect();
    assert_eq!(
        seqs,
        vec![new_seq],
        "a consumer caught up to seq 3 must still receive the post-compaction op"
    );
}

#[tokio::test]
async fn test_snapshot_covers_seq_zero_is_inert() {
    // covers_seq = 0 is the chunked-deposit contract (every chunk except
    // the last): the per-stream snapshot upserts MUST apply, but the
    // compaction watermark MUST NOT advance and NO ops may be GC'd —
    // a client crash between chunks leaves the op log fully intact.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let ops_path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    for i in 0..2 {
        let put_body =
            json!({ "from_device": device, "payload_b64": b64(format!("op-{i}").as_bytes()) });
        let body_bytes = serde_json::to_vec(&put_body).unwrap();
        let headers = auth_headers(&group, &device, "PUT", &ops_path, "", &body_bytes);
        client
            .put(format!("{}{}", relay.base_url, ops_path))
            .headers(headers)
            .body(body_bytes)
            .send()
            .await
            .unwrap();
    }

    // Deposit a snapshot with covers_seq = 0 (a non-final chunk).
    let stream_id = b"chunk-stream-key-Z";
    let snap_payload = b"opaque-encrypted-snapshot-Z";
    let snap_path = format!("/groups/{}/snapshot", hex::encode(group.id.as_bytes()));
    let snap_body = json!({
        "covers_seq": 0,
        "snapshots": [{ "stream_id_b64": b64(stream_id), "payload_b64": b64(snap_payload) }],
    });
    let body_bytes = serde_json::to_vec(&snap_body).unwrap();
    let headers = auth_headers(&group, &device, "PUT", &snap_path, "", &body_bytes);
    let dep = client
        .put(format!("{}{}", relay.base_url, snap_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert!(
        dep.status().is_success(),
        "covers_seq=0 deposit must be accepted: {}",
        dep.status()
    );
    let dep_body: serde_json::Value = dep.json().await.unwrap();
    assert_eq!(dep_body["ok"], true);
    assert_eq!(
        dep_body["gc"].as_u64(),
        Some(0),
        "covers_seq=0 must GC nothing"
    );

    // The op log is fully intact.
    let headers = auth_headers(&group, &device, "GET", &ops_path, "since=0", &[]);
    let r = client
        .get(format!("{}{}?since=0", relay.base_url, ops_path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = r.json().await.unwrap();
    let seqs: Vec<i64> = rows.iter().map(|v| v["seq"].as_i64().unwrap()).collect();
    assert_eq!(
        seqs,
        vec![1, 2],
        "no op may be GC'd by a covers_seq=0 deposit"
    );

    // The watermark did NOT move, but the snapshot upsert DID apply.
    let snaps_path = format!("/groups/{}/snapshots", hex::encode(group.id.as_bytes()));
    let headers = auth_headers(&group, &device, "GET", &snaps_path, "", &[]);
    let r = client
        .get(format!("{}{}", relay.base_url, snaps_path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(
        body["compaction_seq"].as_i64(),
        Some(0),
        "covers_seq=0 must not advance the watermark"
    );
    let snaps = body["snapshots"].as_array().expect("snapshots array");
    assert_eq!(snaps.len(), 1, "the snapshot upsert still applies");
    assert_eq!(snaps[0]["stream_id_b64"], b64(stream_id));
    assert_eq!(snaps[0]["payload_b64"], b64(snap_payload));
}

#[tokio::test]
async fn test_snapshots_roundtrip() {
    // Deposit two snapshots with distinct stream_ids; GET /snapshots
    // returns both with byte-identical payloads.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let stream_a = b"stream-A";
    let payload_a = vec![0xABu8; 64];
    let stream_b = b"stream-B-different";
    let payload_b = vec![0xCDu8; 128];

    let snap_path = format!("/groups/{}/snapshot", hex::encode(group.id.as_bytes()));
    let snap_body = json!({
        "covers_seq": 0,
        "snapshots": [
            { "stream_id_b64": b64(stream_a), "payload_b64": b64(&payload_a) },
            { "stream_id_b64": b64(stream_b), "payload_b64": b64(&payload_b) },
        ],
    });
    let body_bytes = serde_json::to_vec(&snap_body).unwrap();
    let headers = auth_headers(&group, &device, "PUT", &snap_path, "", &body_bytes);
    let dep = client
        .put(format!("{}{}", relay.base_url, snap_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert!(
        dep.status().is_success(),
        "deposit failed: {}",
        dep.status()
    );

    let snaps_path = format!("/groups/{}/snapshots", hex::encode(group.id.as_bytes()));
    let headers = auth_headers(&group, &device, "GET", &snaps_path, "", &[]);
    let r = client
        .get(format!("{}{}", relay.base_url, snaps_path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = r.json().await.unwrap();
    let snaps = body["snapshots"].as_array().expect("snapshots array");
    assert_eq!(snaps.len(), 2, "both distinct streams must be present");

    // Index by stream_id to assert byte-identical payloads regardless
    // of ordering.
    let mut by_stream = std::collections::HashMap::new();
    for s in snaps {
        by_stream.insert(
            s["stream_id_b64"].as_str().unwrap().to_string(),
            s["payload_b64"].as_str().unwrap().to_string(),
        );
    }
    assert_eq!(by_stream.get(&b64(stream_a)), Some(&b64(&payload_a)));
    assert_eq!(by_stream.get(&b64(stream_b)), Some(&b64(&payload_b)));
}

#[tokio::test]
async fn test_snapshot_requires_auth() {
    // PUT /snapshot with a missing MAC → 401; with a bogus auth key
    // (MAC mismatch) → 401. Mirrors test_04's auth assertions.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let snap_path = format!("/groups/{}/snapshot", hex::encode(group.id.as_bytes()));
    let snap_body = json!({
        "covers_seq": 0,
        "snapshots": [{ "stream_id_b64": b64(b"s"), "payload_b64": b64(b"p") }],
    });

    // No MAC headers → 401.
    let r = client
        .put(format!("{}{}", relay.base_url, snap_path))
        .json(&snap_body)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        401,
        "PUT /snapshot without MAC must 401, got {}",
        r.status()
    );

    // Bogus auth key → MAC mismatch → 401.
    let mut wrong = group;
    wrong.auth = [0u8; 32];
    let body_bytes = serde_json::to_vec(&snap_body).unwrap();
    let headers = auth_headers(&wrong, &device, "PUT", &snap_path, "", &body_bytes);
    let r = client
        .put(format!("{}{}", relay.base_url, snap_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        401,
        "PUT /snapshot under wrong auth_key must 401"
    );
}

#[tokio::test]
async fn test_seq_allocates_above_compaction_watermark() {
    // The #195 black hole: after a snapshot deposit covering ALL ops
    // (full compaction → relay_ops empty), the next PUT must be
    // assigned a seq ABOVE the compaction watermark — not restart at
    // 1. A seq at-or-below the watermark sits beneath every caught-up
    // consumer's cursor (poll is strictly `seq > since`), so the op is
    // never fetched and the depositor's own next deposit deletes it:
    // the edit becomes permanently undeliverable. The CF Worker gets
    // this for free via AUTOINCREMENT; this case locks the behaviour
    // into the shared suite for both implementations.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    // PUT 3 ops → seqs 1, 2, 3.
    let ops_path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    for i in 0..3 {
        let put_body =
            json!({ "from_device": device, "payload_b64": b64(format!("op-{i}").as_bytes()) });
        let body_bytes = serde_json::to_vec(&put_body).unwrap();
        let headers = auth_headers(&group, &device, "PUT", &ops_path, "", &body_bytes);
        let r = client
            .put(format!("{}{}", relay.base_url, ops_path))
            .headers(headers)
            .body(body_bytes)
            .send()
            .await
            .unwrap();
        assert!(r.status().is_success(), "PUT {} failed: {}", i, r.status());
    }

    // Deposit a snapshot covering seq 3 — FULL compaction, the op log
    // is now empty and the watermark sits at 3.
    let snap_path = format!("/groups/{}/snapshot", hex::encode(group.id.as_bytes()));
    let snap_body = json!({
        "covers_seq": 3,
        "snapshots": [{ "stream_id_b64": b64(b"stream-A"), "payload_b64": b64(b"snap-A") }],
    });
    let body_bytes = serde_json::to_vec(&snap_body).unwrap();
    let headers = auth_headers(&group, &device, "PUT", &snap_path, "", &body_bytes);
    let dep = client
        .put(format!("{}{}", relay.base_url, snap_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert!(
        dep.status().is_success(),
        "snapshot deposit failed: {}",
        dep.status()
    );
    let dep_body: serde_json::Value = dep.json().await.unwrap();
    assert_eq!(dep_body["gc"].as_u64(), Some(3), "all 3 ops must be GC'd");

    // PUT a NEW op. Its seq must land ABOVE the watermark (i.e. 4).
    let put_body = json!({ "from_device": device, "payload_b64": b64(b"post-compaction-edit") });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let headers = auth_headers(&group, &device, "PUT", &ops_path, "", &body_bytes);
    let r = client
        .put(format!("{}{}", relay.base_url, ops_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success(), "post-compaction PUT failed");
    let body: serde_json::Value = r.json().await.unwrap();
    let new_seq = body["seq"].as_i64().expect("seq is integer");
    assert!(
        new_seq > 3,
        "post-compaction op must be assigned a seq above the compaction \
         watermark (3), got {} — a caught-up consumer would never fetch it",
        new_seq
    );

    // A caught-up consumer (cursor == watermark) must receive the new op.
    let headers = auth_headers(&group, &device, "GET", &ops_path, "since=3", &[]);
    let r = client
        .get(format!("{}{}?since=3", relay.base_url, ops_path))
        .headers(headers)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());
    let rows: Vec<serde_json::Value> = r.json().await.unwrap();
    let seqs: Vec<i64> = rows.iter().map(|v| v["seq"].as_i64().unwrap()).collect();
    assert_eq!(
        seqs,
        vec![new_seq],
        "poll(since = watermark) must deliver the post-compaction op"
    );
}

// ─── Tests 8–13 (stage 3d) ─────────────────────────────────────────

#[tokio::test]
async fn test_08_body_size_cap() {
    // PUT > 1 MiB returns 413 (cap is enforced inside the MAC gate
    // via `axum::body::to_bytes` so the cap fires before the handler).
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    // Construct a body > 1 MiB by stuffing payload_b64.
    let big_payload = vec![b'x'; 2 * 1024 * 1024];
    let put_body = json!({ "from_device": device, "payload_b64": b64(&big_payload) });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    let headers = auth_headers(&group, &device, "PUT", &path, "", &body_bytes);
    let r = client
        .put(format!("{}{}", relay.base_url, path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        413,
        "over-cap body must 413, got {}",
        r.status()
    );
}

#[tokio::test]
async fn test_09_rate_limit_per_ip() {
    // Per-IP rate limit. Test with the explicit `--max` set to
    // something tiny so we don't have to fire 1000 requests; this
    // verifies the limiter exists and fires, not the production cap.
    //
    // The default RATE_LIMIT_MAX (1000 / 10s) is too high to exercise
    // in a unit test without burning seconds; rely on the in-memory
    // limiter's existence + a focused asymmetric assertion: fire
    // many requests and confirm at least one 429 surfaces. If the
    // limiter is wired the rate eventually hits 1000; if it isn't
    // every request 2xx/4xx but never 429.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    // Hit /registration (GET, no auth) 1100 times rapidly. The
    // limit is 1000/window — last 100 should 429.
    let mut saw_429 = false;
    for _ in 0..1100 {
        let r = client
            .get(format!(
                "{}/groups/{}/registration",
                relay.base_url,
                hex::encode(group.id.as_bytes())
            ))
            .send()
            .await
            .unwrap();
        if r.status().as_u16() == 429 {
            saw_429 = true;
            break;
        }
    }
    assert!(saw_429, "expected rate limiter to fire 429 after the cap");
}

#[tokio::test]
async fn test_10_cross_group_isolation() {
    // Ops PUT against group A don't appear in GET against group B
    // even when both have valid auth headers per side.
    let relay = spawn_relay().await;
    let a = fresh_group();
    let b = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();

    for g in [&a, &b] {
        client
            .post(format!(
                "{}/groups/{}/register",
                relay.base_url,
                hex::encode(g.id.as_bytes())
            ))
            .json(&register_body(g, now))
            .send()
            .await
            .unwrap();
    }

    // PUT one op into group A.
    let path_a = format!("/groups/{}/ops", hex::encode(a.id.as_bytes()));
    let put_body = json!({ "from_device": device, "payload_b64": b64(b"only-in-a") });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let headers = auth_headers(&a, &device, "PUT", &path_a, "", &body_bytes);
    client
        .put(format!("{}{}", relay.base_url, path_a))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();

    // GET group B — must be empty.
    let path_b = format!("/groups/{}/ops", hex::encode(b.id.as_bytes()));
    let headers = auth_headers(&b, &device, "GET", &path_b, "since=0", &[]);
    let r = client
        .get(format!("{}{}?since=0", relay.base_url, path_b))
        .headers(headers)
        .send()
        .await
        .unwrap();
    let rows: Vec<serde_json::Value> = r.json().await.unwrap();
    assert!(rows.is_empty(), "group B must not see group A's op");
}

#[tokio::test]
async fn test_11_replay_window() {
    // X-Tesela-Ts more than 300s old returns 400.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    // Build auth headers but force a stale timestamp.
    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    let put_body = json!({ "from_device": device, "payload_b64": b64(b"x") });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let stale_ts = now - 600; // 10 minutes ago
    let nonce = random_nonce_b64();
    let canonical = canonical_request(
        "PUT",
        &path,
        "",
        &nonce,
        stale_ts,
        &body_hash_hex(&body_bytes),
    );
    let mac = compute_request_mac(&group.auth, &canonical);
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        "X-Tesela-Group",
        hex::encode(group.id.as_bytes()).parse().unwrap(),
    );
    h.insert("X-Tesela-Device", device.parse().unwrap());
    h.insert("X-Tesela-Nonce", nonce.parse().unwrap());
    h.insert("X-Tesela-Ts", stale_ts.to_string().parse().unwrap());
    h.insert("X-Tesela-Mac", b64(&mac).parse().unwrap());
    h.insert("Content-Type", "application/json".parse().unwrap());
    let r = client
        .put(format!("{}{}", relay.base_url, path))
        .headers(h)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        400,
        "stale ts must 400, got {}",
        r.status()
    );
}

#[tokio::test]
async fn test_12_nonce_dedupe() {
    // Same nonce within window returns 400 on the second use.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    // Build identical headers twice — same nonce + ts. Second should 400.
    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    let put_body = json!({ "from_device": device, "payload_b64": b64(b"x") });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let nonce = random_nonce_b64();
    let ts = now_secs();
    let canonical = canonical_request("PUT", &path, "", &nonce, ts, &body_hash_hex(&body_bytes));
    let mac = compute_request_mac(&group.auth, &canonical);
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(
        "X-Tesela-Group",
        hex::encode(group.id.as_bytes()).parse().unwrap(),
    );
    h.insert("X-Tesela-Device", device.parse().unwrap());
    h.insert("X-Tesela-Nonce", nonce.parse().unwrap());
    h.insert("X-Tesela-Ts", ts.to_string().parse().unwrap());
    h.insert("X-Tesela-Mac", b64(&mac).parse().unwrap());
    h.insert("Content-Type", "application/json".parse().unwrap());

    let r1 = client
        .put(format!("{}{}", relay.base_url, path))
        .headers(h.clone())
        .body(body_bytes.clone())
        .send()
        .await
        .unwrap();
    assert!(r1.status().is_success(), "first use of nonce must succeed");

    let r2 = client
        .put(format!("{}{}", relay.base_url, path))
        .headers(h)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r2.status().as_u16(),
        400,
        "replayed nonce must 400, got {}",
        r2.status()
    );
}

#[tokio::test]
async fn test_13_admin_recovery() {
    // DELETE /admin/groups/{id}/register with bearer token wipes the
    // registration; without the token returns 401.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let admin_path = format!(
        "/admin/groups/{}/register",
        hex::encode(group.id.as_bytes())
    );
    // Without token → 401.
    let r = client
        .delete(format!("{}{}", relay.base_url, admin_path))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 401, "missing admin token must 401");

    // With wrong token → 401.
    let r = client
        .delete(format!("{}{}", relay.base_url, admin_path))
        .bearer_auth("wrong-token")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 401, "wrong admin token must 401");

    // With right token → 204 + registration is gone.
    let r = client
        .delete(format!("{}{}", relay.base_url, admin_path))
        .bearer_auth(&relay.admin_token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 204, "correct admin token must 204");

    let g = client
        .get(format!(
            "{}/groups/{}/registration",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(g.status().as_u16(), 404, "registration must be gone");
}

// ─── Tests 14–15: edge cases both impls must agree on ──────────────
// (Added after an adversarial review of the Cloudflare Worker port
// surfaced a silent-zero-device bug + an un-capped /ack body. Both are
// fixed; these lock the behaviour into the shared suite so neither
// implementation can regress.)

#[tokio::test]
async fn test_14_non_hex_device_id_rejected() {
    // A 32-char-but-non-hex `from_device` must 400 — NOT be silently
    // coerced to the all-zero device (which would misattribute ops), and
    // NOT 500. The MAC is valid over the body; only the field is bad.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let path = format!("/groups/{}/ops", hex::encode(group.id.as_bytes()));
    let bad_device = "z".repeat(32); // even length, passes hex-len, not hex
    let put_body = json!({ "from_device": bad_device, "payload_b64": b64(b"x") });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let headers = auth_headers(&group, &device, "PUT", &path, "", &body_bytes);
    let r = client
        .put(format!("{}{}", relay.base_url, path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        400,
        "non-hex from_device must 400, got {}",
        r.status()
    );
}

#[tokio::test]
async fn test_15_ack_body_size_cap() {
    // The body-size cap applies to ALL MAC-gated endpoints, not just
    // PUT /ops — an over-cap POST /ack must 413 (it's an authenticated
    // DoS surface otherwise).
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let ack_path = format!("/groups/{}/ack", hex::encode(group.id.as_bytes()));
    // Pad the JSON past 1 MiB with an ignored field; the cap fires in
    // the auth gate before the body is even deserialised.
    let pad = vec![b'x'; 2 * 1024 * 1024];
    let ack_body = json!({ "device": device, "applied_seq": 1, "_pad": b64(&pad) });
    let body_bytes = serde_json::to_vec(&ack_body).unwrap();
    let headers = auth_headers(&group, &device, "POST", &ack_path, "", &body_bytes);
    let r = client
        .post(format!("{}{}", relay.base_url, ack_path))
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        413,
        "over-cap ack body must 413, got {}",
        r.status()
    );
}

#[tokio::test]
async fn test_16_registration_endpoints_do_not_leak_auth_key() {
    // The MAC auth_key is HKDF-derived from the group key CLIENT-side —
    // every legitimate member computes it locally and never needs the
    // relay to echo it back. Serving it from the open GET /registration
    // (or the equally open POST /register 409 conflict echo) hands the
    // transport MAC key to anyone who learns the group_id (it's in every
    // URL path → proxy/access logs), collapsing "can deposit/fetch/GC"
    // to "knows the group_id". Joiner hijack verification only needs
    // `registered_at` + `intent_b64` — the signed intent embeds the
    // auth_key, so a mismatched key already fails verification.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let now = now_secs();
    let body = register_body(&group, now);
    let client = reqwest::Client::new();
    let r = client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(r.status().is_success());

    // GET /registration still serves the joiner-verification fields,
    // but NOT the MAC key.
    let g = client
        .get(format!(
            "{}/groups/{}/registration",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .send()
        .await
        .unwrap();
    assert!(g.status().is_success());
    let stored: serde_json::Value = g.json().await.unwrap();
    assert_eq!(stored["registered_at"], body["registered_at"]);
    assert_eq!(stored["intent_b64"], body["intent_b64"]);
    assert!(
        stored.get("auth_key_b64").is_none(),
        "open GET /registration must not serve the group's MAC auth_key, got {stored}"
    );

    // POST /register conflict echo: same open surface, must not leak
    // the stored key either.
    let mut bad = body.clone();
    bad["auth_key_b64"] = json!(b64(&[0u8; 32]));
    let r = client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&bad)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status().as_u16(), 409);
    let echoed: serde_json::Value = r.json().await.unwrap();
    assert!(
        echoed.get("auth_key_b64").is_none(),
        "409 register-conflict echo must not serve the stored auth_key, got {echoed}"
    );
}

#[tokio::test]
async fn test_17_discover_round_trip_and_unknown_404() {
    // Recovery-phrase discovery (ra7 P0 step 2): a phrase-only device
    // has the GroupKey but not the random group_id, so it derives
    // `disc` from the key alone and asks GET /discover/{disc} — no
    // MAC, no server-side group_id input. Pure HTTP against `disc`
    // hex, so this also runs against the CF Worker via
    // TESELA_RELAY_CONFORMANCE_URL once P0.2b lands there.
    let relay = spawn_relay().await;
    let group = fresh_group();
    let now = now_secs();
    let disc = derive_discovery_handle(&group.key);
    let mut body = register_body(&group, now);
    body["disc_b64"] = json!(b64(&disc));

    let client = reqwest::Client::new();
    let r = client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_success(),
        "register with disc_b64 expected 2xx, got {} body={}",
        r.status(),
        r.text().await.unwrap_or_default(),
    );

    // GET /discover/{disc} resolves to this group's group_id, unauthenticated.
    let d = client
        .get(format!(
            "{}/discover/{}",
            relay.base_url,
            hex::encode(disc)
        ))
        .send()
        .await
        .unwrap();
    assert!(
        d.status().is_success(),
        "GET /discover/{{disc}} expected 2xx, got {}",
        d.status()
    );
    let resolved: serde_json::Value = d.json().await.unwrap();
    assert_eq!(resolved["group_id"], hex::encode(group.id.as_bytes()));

    // An unknown/random disc 404s rather than leaking any signal about
    // which handles are registered.
    let mut unknown = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut unknown);
    let miss = client
        .get(format!(
            "{}/discover/{}",
            relay.base_url,
            hex::encode(unknown)
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(miss.status().as_u16(), 404, "unknown disc must 404");

    // A malformed disc (valid hex, but not 32 bytes) is a 400, not a
    // 404/500 — a cross-relay contract point the CF Worker (P0.2b) must
    // match too.
    let bad = client
        .get(format!("{}/discover/abcd", relay.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status().as_u16(), 400, "malformed disc must 400");
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

// ─── /devices — APNs token registry (sync durability P3b) ───────────

/// A MAC'd POST /devices with a valid {device, apns_token} round-trips
/// to 200 {"ok": true} (the request contract shared with the CF Worker);
/// missing MAC → 401; a non-hex apns_token → 400. The APNs PUSH itself
/// is a side-effect to Apple with no relay response, so there's nothing
/// to assert here beyond the registration contract (push is unconfigured
/// in tests — `AppState::open` passes `apns: None`).
#[tokio::test]
async fn test_register_device_contract() {
    let relay = spawn_relay().await;
    let group = fresh_group();
    let device = random_device_id_hex();
    let now = now_secs();
    let client = reqwest::Client::new();
    client
        .post(format!(
            "{}/groups/{}/register",
            relay.base_url,
            hex::encode(group.id.as_bytes())
        ))
        .json(&register_body(&group, now))
        .send()
        .await
        .unwrap();

    let path = format!("/groups/{}/devices", hex::encode(group.id.as_bytes()));
    let url = format!("{}{}", relay.base_url, path);

    // 1) Valid MAC'd registration → 200 {"ok": true}.
    let body = json!({ "device": device, "apns_token": "aabbccddeeff00112233" });
    let body_bytes = serde_json::to_vec(&body).unwrap();
    let headers = auth_headers(&group, &device, "POST", &path, "", &body_bytes);
    let r = client
        .post(&url)
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert!(
        r.status().is_success(),
        "POST /devices should 2xx, got {}",
        r.status()
    );
    let v: serde_json::Value = r.json().await.unwrap();
    assert_eq!(v["ok"], json!(true));

    // 2) Missing MAC headers → 401 (the mac_gate rejects before the handler).
    let body = json!({ "device": device, "apns_token": "aabbccdd" });
    let r = client.post(&url).json(&body).send().await.unwrap();
    assert_eq!(
        r.status().as_u16(),
        401,
        "unauthenticated POST /devices must be 401"
    );

    // 3) Valid MAC but non-hex apns_token → 400.
    let body = json!({ "device": device, "apns_token": "not-hex!!" });
    let body_bytes = serde_json::to_vec(&body).unwrap();
    let headers = auth_headers(&group, &device, "POST", &path, "", &body_bytes);
    let r = client
        .post(&url)
        .headers(headers)
        .body(body_bytes)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status().as_u16(),
        400,
        "non-hex apns_token must be 400"
    );
}
