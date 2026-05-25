//! End-to-end exercise of the `RelayClient` in `tesela-sync` against
//! the in-process `tesela-relay` server. This is the "two desktops in
//! one process, same group, talking through the relay" smoke that
//! validates the full Phase-2 picture: payload is AEAD-sealed by
//! sender, opaque to the relay, opened by recipient, and the round-
//! trip matches the original `SyncEnvelope` byte-for-byte.

use std::net::SocketAddr;

use rand::RngCore;
use reqwest::Url;
use tempfile::TempDir;

use tesela_relay::{router, AppState};
use tesela_sync::crypto::keys::GroupKey;
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
    alice_client.verify_registration().await.expect("alice verify");

    // Bob's join: register_or_recover on the already-registered
    // group must succeed via the idempotent / recovery path.
    let bob_at = bob_client.register_or_recover().await.expect("bob register");
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
    let rows = bob_client.poll(0).await.expect("bob poll");
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

    // Bob acks, GC runs, subsequent poll is empty.
    bob_client.ack(seq).await.expect("bob ack");
    // Alice also acks so the GC has heard from both known members.
    alice_client.ack(seq).await.expect("alice ack");
    let rows = bob_client.poll(0).await.expect("bob poll after gc");
    assert!(rows.is_empty(), "after all known members ack, GC drops the op");
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
    attacker.register_or_recover().await.expect("attacker squat");

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
