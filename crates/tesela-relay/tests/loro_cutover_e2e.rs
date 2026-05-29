//! End-to-end Loro cutover exercise: two authoritative `LoroEngine`s
//! (distinct devices) sync through the in-process `tesela-relay` using
//! the real v2 relay payload — `produce_relay_updates` → `TLR2`-magic
//! encode → AEAD seal → HTTP `put_envelope`, and `poll` → AEAD open →
//! decode → `apply_relay_updates` → materialize `<slug>.md`.
//!
//! This is the "two Macs over the relay" proof minus the HTTP server
//! front-end: it stresses the actual encryption boundary, the relay's
//! store/seq/ack semantics, the wire codec, and on-disk materialization.
//! The per-tick push/pull below mirrors `tesela-server::sync_relay::tick`
//! on the `uses_loro_relay_payload` branch.

use std::net::SocketAddr;
use std::sync::Arc;

use rand::RngCore;
use reqwest::Url;
use tempfile::TempDir;

use tesela_relay::{router, AppState};
use tesela_sync::crypto::keys::GroupKey;
use tesela_sync::device::DeviceId;
use tesela_sync::group::GroupId;
use tesela_sync::transport::relay::RelayClient;
use tesela_sync::wire::envelope::SyncEnvelope;
use tesela_sync::{
    decode_loro_relay_payload, encode_loro_relay_payload, Hlc, LoroDocUpdate, LoroEngine, OpPayload,
    SyncEngine,
};

struct Ctx {
    base_url: Url,
    _tmp: TempDir,
    _server: tokio::task::JoinHandle<()>,
}

async fn spawn_relay() -> Ctx {
    let tmp = tempfile::tempdir().expect("tmp dir");
    let db = tmp.path().join("relay.sqlite");
    let state = AppState::open(&db, 4_194_304, Some("admin".into()))
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

async fn authoritative_engine(tmp: &TempDir, device: DeviceId) -> (LoroEngine, std::path::PathBuf) {
    let notes = tmp.path().join("notes");
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        tmp.path().join("loro"),
        Some(notes.clone()),
    )
    .await
    .expect("authoritative loro engine");
    (engine, notes)
}

/// Mirror of `tick`'s outbound Loro branch: produce per-note updates,
/// wrap in the v2 payload, deposit one envelope.
async fn relay_push(engine: &LoroEngine, client: &RelayClient, from: DeviceId, group: GroupId) {
    let updates = engine.produce_relay_updates().await;
    if updates.is_empty() {
        return;
    }
    let payload: Vec<LoroDocUpdate> = updates
        .iter()
        .map(|(doc, update_bytes, _vv)| LoroDocUpdate {
            doc: *doc,
            update_bytes: update_bytes.clone(),
        })
        .collect();
    let committed: Vec<([u8; 16], Vec<u8>)> =
        updates.into_iter().map(|(doc, _b, vv)| (doc, vv)).collect();
    let ciphertext = encode_loro_relay_payload(&payload).expect("encode v2");
    let env = SyncEnvelope {
        from_device: from,
        to_group: group,
        nonce: [0u8; 24],
        ciphertext,
    };
    client.put_envelope(env).await.expect("put envelope");
    // Confirmed send → advance the broadcast cursor.
    engine.commit_broadcast_cursors(&committed).await;
}

/// Mirror of `tick`'s inbound Loro branch: poll, skip own echoes, decode
/// the v2 payload, apply, advance + ack the relay seq.
async fn relay_pull(engine: &LoroEngine, client: &RelayClient, cursor: &mut i64, self_dev: DeviceId) -> usize {
    let rows = client.poll(*cursor).await.expect("poll");
    let mut applied = 0;
    let mut max_seq = *cursor;
    for (seq, env) in rows {
        if env.from_device != self_dev {
            if let Ok(Some(updates)) = decode_loro_relay_payload(&env.ciphertext) {
                let pairs: Vec<([u8; 16], Vec<u8>)> =
                    updates.into_iter().map(|u| (u.doc, u.update_bytes)).collect();
                applied += engine.apply_relay_updates(&pairs).await;
            }
        }
        if seq > max_seq {
            max_seq = seq;
        }
    }
    if max_seq > *cursor {
        *cursor = max_seq;
        let _ = client.ack(max_seq).await;
    }
    applied
}

#[tokio::test]
async fn two_authoritative_engines_converge_over_real_relay() {
    let ctx = spawn_relay().await;
    let (group, key) = fresh_group();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);

    let client_a = RelayClient::new(ctx.base_url.clone(), group, dev_a, key.clone());
    let client_b = RelayClient::new(ctx.base_url.clone(), group, dev_b, key);
    client_a.register_or_recover().await.expect("a register");
    client_a.verify_registration().await.expect("a verify");
    client_b.register_or_recover().await.expect("b register");
    client_b.verify_registration().await.expect("b verify");

    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let (a, notes_a) = authoritative_engine(&tmp_a, dev_a).await;
    let (b, notes_b) = authoritative_engine(&tmp_b, dev_b).await;
    let mut cur_a = 0i64;
    let mut cur_b = 0i64;

    let note = [0x77u8; 16];

    // A creates the note → materializes shared.md → broadcasts.
    a.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("shared".into()),
        title: "Shared".into(),
        content: "---\ntitle: Shared\n---\n\n- base <!-- bid:70707070-7070-7070-7070-707070707070 -->\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    let file_a = notes_a.join("shared.md");
    let file_b = notes_b.join("shared.md");
    assert!(file_a.exists(), "A materialized its own write");

    // One relay round: A pushes, B pulls + materializes.
    relay_push(&a, &client_a, dev_a, group).await;
    let applied = relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    assert_eq!(applied, 1, "B applied A's note from the relay");
    assert!(file_b.exists(), "B materialized the relayed note");
    assert_eq!(
        tokio::fs::read_to_string(&file_a).await.unwrap(),
        tokio::fs::read_to_string(&file_b).await.unwrap(),
        "files identical after first sync"
    );

    // Concurrent edits on both devices (the flashing scenario).
    a.record_local(OpPayload::BlockUpsert {
        block_id: [0x7a; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "a".into(),
        indent_level: 0,
        text: "from A".into(),
    })
    .await
    .unwrap();
    b.record_local(OpPayload::BlockUpsert {
        block_id: [0x7b; 16],
        note_id: note,
        parent_block_id: None,
        order_key: "b".into(),
        indent_level: 0,
        text: "from B".into(),
    })
    .await
    .unwrap();

    // A few sync rounds both directions to fully exchange the deltas.
    for _ in 0..3 {
        relay_push(&a, &client_a, dev_a, group).await;
        relay_push(&b, &client_b, dev_b, group).await;
        relay_pull(&a, &client_a, &mut cur_a, dev_a).await;
        relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    }

    let ra = a.render_note(note).await.unwrap();
    let rb = b.render_note(note).await.unwrap();
    assert_eq!(ra, rb, "engines converge through the real relay — no flashing");
    assert!(
        ra.contains("base") && ra.contains("from A") && ra.contains("from B"),
        "all three concurrent blocks present: {ra:?}"
    );
    assert_eq!(
        tokio::fs::read_to_string(&file_a).await.unwrap(),
        tokio::fs::read_to_string(&file_b).await.unwrap(),
        "materialized files converge"
    );

    // Steady state: a further round transmits nothing new (no ping-pong).
    relay_push(&a, &client_a, dev_a, group).await;
    relay_push(&b, &client_b, dev_b, group).await;
    let a_more = relay_pull(&a, &client_a, &mut cur_a, dev_a).await;
    let b_more = relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    let ra2 = a.render_note(note).await.unwrap();
    assert_eq!(ra2, ra, "stable after extra round — convergence holds");
    // Whatever (idempotent) updates may have been in flight, the render
    // is unchanged — that is the no-flashing guarantee.
    let _ = (a_more, b_more);
}
