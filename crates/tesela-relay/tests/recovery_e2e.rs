//! End-to-end test for recovery-phrase pairing (`tesela-ra7` P0 step
//! 3a): spawn a relay, register a group (which publishes its
//! discovery handle automatically via `RelayClient::register`), then
//! recover group membership from the phrase ALONE — no server URL, no
//! reachable inviter — and confirm the produced pairing code carries
//! the right group identity and is relay-only.

use std::net::SocketAddr;

use rand::RngCore;
use reqwest::Url;
use tempfile::TempDir;

use tesela_relay::{router, AppState};
use tesela_sync::crypto::keys::GroupKey;
use tesela_sync::crypto::pairing::decode as decode_pairing_code;
use tesela_sync::crypto::recovery::key_to_phrase;
use tesela_sync::device::DeviceId;
use tesela_sync::group::GroupId;
use tesela_sync::recovery::recover_pairing_from_phrase;
use tesela_sync::transport::relay::RelayClient;

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

#[tokio::test]
async fn recover_from_phrase_round_trips() {
    let ctx = spawn().await;
    let (group_id, group_key) = fresh_group();
    let device = fresh_device();

    // Register the group on the relay. `RelayClient::register` always
    // publishes the discovery handle (`disc_b64`) alongside the normal
    // auth-key registration — exactly what any first-registering
    // device already does, so no special "register with disc" setup
    // is needed here (unlike the lower-level conformance test that
    // hand-builds the register body).
    let client = RelayClient::new(
        ctx.base_url.clone(),
        group_id,
        device,
        group_key.clone(),
    );
    client.register_or_recover().await.expect("register group");

    // Recover using ONLY the relay URL + phrase — no group_id, no
    // device id, no prior local state.
    let phrase = key_to_phrase(&group_key);
    let relay_url = ctx.base_url.as_str();
    let code_str = recover_pairing_from_phrase(relay_url, &phrase)
        .await
        .expect("recover from phrase");

    let code = decode_pairing_code(&code_str).expect("decode recovered pairing code");
    assert_eq!(code.group_id, group_id);
    assert_eq!(code.group_key_bytes, *group_key.as_bytes());
    // Relay-only: empty server url, relay_url set — this is what lets
    // iOS's `RelayTicker.isRelayOnlyPairing` route it into `.relay`
    // mode instead of expecting a reachable LAN server.
    assert!(
        code.url.is_empty(),
        "recovered pairing code must have an empty server url (relay-only), got {:?}",
        code.url
    );
    assert_eq!(code.relay_url.as_deref(), Some(relay_url));
}

#[tokio::test]
async fn recover_unregistered_phrase_errs() {
    let ctx = spawn().await;
    let (_group_id, group_key) = fresh_group();
    // This group's discovery handle was NEVER published to this relay.
    let phrase = key_to_phrase(&group_key);

    let result = recover_pairing_from_phrase(ctx.base_url.as_str(), &phrase).await;
    assert!(
        result.is_err(),
        "recovering a phrase for a group never registered on this relay must error, not \
         silently return Ok"
    );
}
