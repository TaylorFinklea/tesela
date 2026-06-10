//! End-to-end exercise of the `RelayClient` in `tesela-sync` against
//! the in-process `tesela-relay` server. This is the "two desktops in
//! one process, same group, talking through the relay" smoke that
//! validates the full Phase-2 picture: payload is AEAD-sealed by
//! sender, opaque to the relay, opened by recipient, and the round-
//! trip matches the original `SyncEnvelope` byte-for-byte.

use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use rand::RngCore;
use reqwest::Url;
use tempfile::TempDir;

use tesela_relay::{router, AppState};
use tesela_sync::crypto::keys::GroupKey;
use tesela_sync::crypto::relay_auth::{
    body_hash_hex, canonical_request, compute_request_mac, derive_relay_auth_key,
};
use tesela_sync::device::DeviceId;
use tesela_sync::group::GroupId;
use tesela_sync::transport::relay::RelayClient;
use tesela_sync::wire::envelope::SyncEnvelope;

struct Ctx {
    base_url: Url,
    _tmp: TempDir,
    _server: tokio::task::JoinHandle<()>,
}

async fn spawn() -> Ctx {
    let tmp = tempfile::tempdir().expect("tmp dir");
    let db = tmp.path().join("relay.sqlite");
    let state = AppState::open(&db, 1_048_576, Some("admin".into()))
        .await
        .expect("relay state");
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });
    Ctx {
        base_url: Url::parse(&format!("http://{}", addr)).unwrap(),
        _tmp: tmp,
        _server: server,
    }
}

fn fresh_group() -> (GroupId, GroupKey) {
    let mut gid = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut gid);
    let mut gk = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut gk);
    (GroupId::from_bytes(gid), GroupKey::from_bytes(gk))
}

fn fresh_device() -> DeviceId {
    let mut d = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut d);
    DeviceId::from_bytes(d)
}

/// Realistic Phase-1 envelope: the SyncEngine produced a cleartext
/// postcard `Vec<EncodedOp>` in `ciphertext`. The `RelayClient`
/// AEAD-wraps this on send and unwraps on recv.
fn fixture_envelope(from: DeviceId, group: GroupId) -> SyncEnvelope {
    SyncEnvelope {
        from_device: from,
        to_group: group,
        nonce: [0u8; 24], // Phase-1 placeholder; the relay client supplies its own AEAD nonce.
        ciphertext: b"postcard(Vec<EncodedOp>) plaintext goes here".to_vec(),
    }
}

#[tokio::test]
async fn two_clients_round_trip_an_envelope_through_the_relay() {
    let ctx = spawn().await;
    let (group, key) = fresh_group();
    let alice = fresh_device();
    let bob = fresh_device();

    let alice_client = RelayClient::new(ctx.base_url.clone(), group, alice, key.clone());
    let bob_client = RelayClient::new(ctx.base_url.clone(), group, bob, key);

    // Alice registers (first-write); Bob joins later and verifies.
    let registered_at = alice_client
        .register_or_recover()
        .await
        .expect("alice register");
    alice_client
        .verify_registration()
        .await
        .expect("alice verify");

    // Bob's join: register_or_recover on the already-registered
    // group must succeed via the idempotent / recovery path.
    let bob_at = bob_client
        .register_or_recover()
        .await
        .expect("bob register");
    assert_eq!(
        bob_at, registered_at,
        "joining device must end up pinned to the same registered_at"
    );
    bob_client.verify_registration().await.expect("bob verify");

    // Alice deposits an envelope.
    let original = fixture_envelope(alice, group);
    let (seq, _ts) = alice_client
        .put_envelope(original.clone())
        .await
        .expect("alice put");
    assert_eq!(seq, 1);

    // Bob polls and gets it back, AEAD-opened.
    let rows = bob_client.poll(0).await.expect("bob poll").rows;
    assert_eq!(rows.len(), 1);
    let (got_seq, got_env) = &rows[0];
    assert_eq!(*got_seq, 1);
    assert_eq!(got_env.from_device, original.from_device);
    assert_eq!(got_env.to_group, original.to_group);
    // The inner content (the cleartext plaintext Alice handed in) is
    // recovered byte-for-byte after the relay round-trip.
    assert_eq!(
        got_env.ciphertext, original.ciphertext,
        "AEAD round-trip must recover the original plaintext"
    );

    // Durable-replica retention (encrypted-replica spine, Phase 1a): both
    // members ack, but the relay is the off-site encrypted backup, so it
    // RETAINS the op rather than evicting it — a re-poll from 0 still returns
    // it (a wiped device restores from this). Ack-triggered GC is removed;
    // compaction is now snapshot-gated (Phase 1b).
    bob_client.ack(seq).await.expect("bob ack");
    alice_client.ack(seq).await.expect("alice ack");
    let rows = bob_client.poll(0).await.expect("bob poll after ack").rows;
    assert_eq!(
        rows.len(),
        1,
        "durable retention: the op survives ack (encrypted backup + bootstrap)"
    );
}

/// Deposit a raw payload directly over HTTP with a valid MAC,
/// bypassing `RelayClient::put_envelope`'s always-well-formed sealing
/// path. This is how a poisoned row reaches the relay in real life:
/// postcard version skew between clients, payload corruption, or
/// garbage from anyone who passes the MAC gate. Returns the assigned
/// seq.
async fn deposit_raw(
    base_url: &Url,
    group: GroupId,
    key: &GroupKey,
    device: DeviceId,
    payload: &[u8],
) -> i64 {
    let b64 = base64::engine::general_purpose::STANDARD;
    let auth = derive_relay_auth_key(key, &group);
    let put_body = serde_json::json!({
        "from_device": hex::encode(device.as_bytes()),
        "payload_b64": b64.encode(payload),
    });
    let body_bytes = serde_json::to_vec(&put_body).unwrap();
    let path = format!("/groups/{}/ops", hex::encode(group.as_bytes()));
    let mut nonce_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = b64.encode(nonce_bytes);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let canonical = canonical_request("PUT", &path, "", &nonce, ts, &body_hash_hex(&body_bytes));
    let mac = compute_request_mac(&auth, &canonical);
    let resp = reqwest::Client::new()
        .put(base_url.join(&path).unwrap())
        .header("Content-Type", "application/json")
        .header("X-Tesela-Group", hex::encode(group.as_bytes()))
        .header("X-Tesela-Device", hex::encode(device.as_bytes()))
        .header("X-Tesela-Nonce", &nonce)
        .header("X-Tesela-Ts", ts.to_string())
        .header("X-Tesela-Mac", b64.encode(mac))
        .body(body_bytes)
        .send()
        .await
        .expect("raw deposit send");
    assert!(
        resp.status().is_success(),
        "raw deposit must 2xx, got {}",
        resp.status()
    );
    let ack: serde_json::Value = resp.json().await.expect("raw deposit ack json");
    ack["seq"].as_i64().expect("seq")
}

#[tokio::test]
async fn poisoned_envelope_is_skipped_not_wedging_the_batch() {
    // One row whose outer payload fails to decode/decrypt must not
    // abort the whole poll: both surrounding good envelopes apply and
    // the poisoned seqs are surfaced so callers advance their cursor
    // past them (otherwise one bad row blocks every subsequent envelope
    // for every consumer, forever — and compaction never GCs it because
    // the depositor's covers_seq is its own stuck inbound cursor).
    let ctx = spawn().await;
    let (group, key) = fresh_group();
    let alice = fresh_device();
    let bob = fresh_device();
    let alice_client = RelayClient::new(ctx.base_url.clone(), group, alice, key.clone());
    let bob_client = RelayClient::new(ctx.base_url.clone(), group, bob, key.clone());
    alice_client
        .register_or_recover()
        .await
        .expect("alice register");

    // seq 1: good envelope.
    let good1 = fixture_envelope(alice, group);
    let (seq1, _) = alice_client
        .put_envelope(good1.clone())
        .await
        .expect("put good1");

    // seq 2: garbage that fails the OUTER postcard decode (3 bytes —
    // too short for the 24-byte nonce).
    let seq2 = deposit_raw(&ctx.base_url, group, &key, alice, &[0xde, 0xad, 0xbe]).await;

    // seq 3: a well-formed OuterPayload whose ciphertext won't
    // AEAD-open under the group key (≈ sealed under a foreign key).
    // Hand-rolled postcard: [u8; 24] nonce as raw bytes, then
    // varint-length-prefixed Vec<u8> ciphertext.
    let mut foreign = Vec::new();
    foreign.extend_from_slice(&[0x42u8; 24]);
    foreign.push(32);
    foreign.extend_from_slice(&[0x99u8; 32]);
    let seq3 = deposit_raw(&ctx.base_url, group, &key, alice, &foreign).await;

    // seq 4: good envelope.
    let good2 = fixture_envelope(alice, group);
    let (seq4, _) = alice_client
        .put_envelope(good2.clone())
        .await
        .expect("put good2");

    // Bob's poll must NOT error out on the poisoned rows.
    let batch = bob_client
        .poll(0)
        .await
        .expect("poll must skip poisoned rows, not wedge the batch");
    let seqs: Vec<i64> = batch.rows.iter().map(|(s, _)| *s).collect();
    assert_eq!(seqs, vec![seq1, seq4], "both good envelopes delivered");
    assert_eq!(batch.rows[0].1.ciphertext, good1.ciphertext);
    assert_eq!(batch.rows[1].1.ciphertext, good2.ciphertext);
    assert_eq!(
        batch.skipped,
        vec![seq2, seq3],
        "poisoned seqs surfaced so callers advance the cursor past them"
    );
    assert_eq!(
        batch.max_seq(),
        Some(seq4),
        "cursor watermark covers good AND skipped rows"
    );
}

#[tokio::test]
async fn hijacked_relay_is_detected_by_joiner_verification() {
    let ctx = spawn().await;
    let (group, real_key) = fresh_group();
    let attacker_key = GroupKey::from_bytes([0xff; 32]);
    let real_device = fresh_device();
    let joiner_device = fresh_device();

    // Attacker registers first under a different key (squatting the
    // group_id they somehow learned without the group_key).
    let attacker = RelayClient::new(ctx.base_url.clone(), group, real_device, attacker_key);
    attacker
        .register_or_recover()
        .await
        .expect("attacker squat");

    // Legitimate joiner (holds the real group_key) tries to verify.
    let joiner = RelayClient::new(ctx.base_url.clone(), group, joiner_device, real_key);
    let err = joiner
        .verify_registration()
        .await
        .expect_err("hijack must surface as an error");
    let msg = format!("{err}");
    assert!(
        msg.contains("HIJACKED") || msg.contains("hijacked"),
        "error must mention hijack: got {msg}"
    );
}

/// Audit regression: a transient non-OK from POST /register (5xx while the
/// relay host is busy, 429, etc.) must NOT take the conflict-recovery path.
/// It used to be misdiagnosed as a 409, and since the group genuinely
/// doesn't exist, recovery then failed with the hijack-shaped
/// "relay 409 but /registration returned 404".
#[tokio::test]
async fn transient_register_failure_is_not_misdiagnosed_as_conflict() {
    use axum::{http::StatusCode, routing::post, Router};

    let app = Router::new().route(
        "/groups/{id}/register",
        post(|| async { (StatusCode::SERVICE_UNAVAILABLE, "busy") }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = reqwest::Url::parse(&format!("http://{}", listener.local_addr().unwrap()))
        .expect("stub url");
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let (group, key) = fresh_group();
    let client = RelayClient::new(base_url, group, fresh_device(), key);
    let err = client
        .register_or_recover()
        .await
        .expect_err("503 must surface as an error");
    let msg = format!("{err}");
    assert!(
        msg.contains("503") || msg.to_lowercase().contains("service unavailable"),
        "error must carry the real status, got: {msg}"
    );
    assert!(
        !msg.contains("409") && !msg.to_lowercase().contains("hijack"),
        "transient failure must not be conflict/hijack-shaped, got: {msg}"
    );
}
