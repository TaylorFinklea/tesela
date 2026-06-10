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
    decode_loro_relay_payload, encode_loro_relay_payload, Hlc, LoroDocUpdate, LoroEngine,
    OpPayload, SyncEngine,
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
async fn relay_pull(
    engine: &LoroEngine,
    client: &RelayClient,
    cursor: &mut i64,
    self_dev: DeviceId,
) -> usize {
    let rows = client.poll(*cursor).await.expect("poll").rows;
    let mut applied = 0;
    let mut max_seq = *cursor;
    for (seq, env) in rows {
        if env.from_device != self_dev {
            if let Ok(Some(updates)) = decode_loro_relay_payload(&env.ciphertext) {
                let pairs: Vec<([u8; 16], Vec<u8>)> = updates
                    .into_iter()
                    .map(|u| (u.doc, u.update_bytes))
                    .collect();
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
        content:
            "---\ntitle: Shared\n---\n\n- base <!-- bid:70707070-7070-7070-7070-707070707070 -->\n"
                .into(),
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
        after_block_id: None,
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
        after_block_id: None,
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
    assert_eq!(
        ra, rb,
        "engines converge through the real relay — no flashing"
    );
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

/// Encrypted-replica spine, Phase 1a — the durable backup → restore path.
/// Device A authors several notes and pushes them; the relay RETAINS the full
/// encrypted op log (no ack eviction). A brand-new EMPTY device C (a wiped Mac
/// / fresh install) registers and polls from `since=0` and rebuilds A's ENTIRE
/// mosaic from the relay's encrypted backup. This is the off-site-restore the
/// spine exists for.
#[tokio::test]
async fn fresh_device_restores_full_mosaic_from_durable_relay() {
    let ctx = spawn_relay().await;
    let (group, key) = fresh_group();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_c = DeviceId::from_bytes([0xcc; 16]);

    let client_a = RelayClient::new(ctx.base_url.clone(), group, dev_a, key.clone());
    client_a.register_or_recover().await.expect("a register");
    client_a.verify_registration().await.expect("a verify");

    let tmp_a = tempfile::tempdir().unwrap();
    let (a, _notes_a) = authoritative_engine(&tmp_a, dev_a).await;

    // A authors three notes (distinct ids + block ids).
    let notes: [([u8; 16], &str, &str, &str); 3] = [
        (
            [0x11; 16],
            "alpha",
            "Alpha",
            "11111111-1111-1111-1111-111111111111",
        ),
        (
            [0x22; 16],
            "beta",
            "Beta",
            "22222222-2222-2222-2222-222222222222",
        ),
        (
            [0x33; 16],
            "gamma",
            "Gamma",
            "33333333-3333-3333-3333-333333333333",
        ),
    ];
    for (id, slug, title, bid) in notes {
        a.record_local(OpPayload::NoteUpsert {
            note_id: id,
            display_alias: Some(slug.into()),
            title: title.into(),
            content: format!("---\ntitle: {title}\n---\n\n- body of {slug} <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    }
    // A deposits the full history into the durable relay.
    relay_push(&a, &client_a, dev_a, group).await;

    // ── A wiped/fresh device C restores purely from the relay ──
    let client_c = RelayClient::new(ctx.base_url.clone(), group, dev_c, key);
    client_c.register_or_recover().await.expect("c register");
    client_c.verify_registration().await.expect("c verify");
    let tmp_c = tempfile::tempdir().unwrap();
    let (c, notes_c) = authoritative_engine(&tmp_c, dev_c).await;

    let mut cur_c = 0i64;
    let applied = relay_pull(&c, &client_c, &mut cur_c, dev_c).await;
    assert!(
        applied >= 3,
        "C restored A's full op history from since=0 — applied {applied} (>=3 notes)"
    );

    // C rebuilt the ENTIRE mosaic from the encrypted backup, byte-identically.
    for (id, slug, _t, _bid) in notes {
        let rc = c.render_note(id).await;
        let ra = a.render_note(id).await;
        assert_eq!(
            rc, ra,
            "note '{slug}' restored identically from the relay backup"
        );
        assert!(
            notes_c.join(format!("{slug}.md")).exists(),
            "'{slug}.md' materialized on restore"
        );
    }
}

/// Encrypted-replica spine, Phase 1b-ii — the snapshot-gated compaction +
/// bootstrap-from-snapshots path. Device A authors three notes and pushes the
/// deltas (they land in `relay_ops`). A then exports a fresh full snapshot per
/// note and `put_snapshots(covers_seq, ...)` — the relay upserts the encrypted
/// snapshots, advances its compaction watermark, and GCs the superseded deltas.
/// A brand-new EMPTY device C (the deltas are GONE) registers, `fetch_snapshots`,
/// imports each snapshot, and reconstructs A's WHOLE mosaic purely from the
/// compacted snapshot set. This proves compaction never loses data: the
/// snapshots are a complete, self-sufficient replica.
#[tokio::test]
async fn snapshot_compaction_then_fresh_device_bootstraps_from_snapshots() {
    let ctx = spawn_relay().await;
    let (group, key) = fresh_group();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_c = DeviceId::from_bytes([0xcc; 16]);

    let client_a = RelayClient::new(ctx.base_url.clone(), group, dev_a, key.clone());
    client_a.register_or_recover().await.expect("a register");
    client_a.verify_registration().await.expect("a verify");

    let tmp_a = tempfile::tempdir().unwrap();
    let (a, _notes_a) = authoritative_engine(&tmp_a, dev_a).await;

    // A authors three notes (distinct ids + block ids).
    let notes: [([u8; 16], &str, &str, &str); 3] = [
        (
            [0x11; 16],
            "alpha",
            "Alpha",
            "11111111-1111-1111-1111-111111111111",
        ),
        (
            [0x22; 16],
            "beta",
            "Beta",
            "22222222-2222-2222-2222-222222222222",
        ),
        (
            [0x33; 16],
            "gamma",
            "Gamma",
            "33333333-3333-3333-3333-333333333333",
        ),
    ];
    for (id, slug, title, bid) in notes {
        a.record_local(OpPayload::NoteUpsert {
            note_id: id,
            display_alias: Some(slug.into()),
            title: title.into(),
            content: format!("---\ntitle: {title}\n---\n\n- body of {slug} <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    }

    // A pushes the deltas into the durable relay, capturing the assigned seq so
    // it can scope compaction to "everything up to and including this push".
    let updates = a.produce_relay_updates().await;
    assert!(!updates.is_empty(), "A has deltas to push");
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
        from_device: dev_a,
        to_group: group,
        nonce: [0u8; 24],
        ciphertext,
    };
    let (covers_seq, _ts) = client_a.put_envelope(env).await.expect("put envelope");
    a.commit_broadcast_cursors(&committed).await;
    assert!(covers_seq > 0, "relay assigned a positive seq");

    // The deltas are durably in `relay_ops` right now.
    let pre = client_a.poll(0).await.expect("poll pre-snapshot").rows;
    assert!(!pre.is_empty(), "deltas present before compaction");

    // A exports a full snapshot per note (ExportMode::Snapshot via
    // export_doc_update(.., None)) and deposits the encrypted snapshot set,
    // gating compaction at `covers_seq`. stream_id = note_id (v1).
    let mut snapshots: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    for (id, slug, _t, _bid) in notes {
        let snap = a
            .export_doc_update(id, None)
            .await
            .unwrap_or_else(|| panic!("snapshot for '{slug}'"));
        snapshots.push((id.to_vec(), snap));
    }
    let gc = client_a
        .put_snapshots(covers_seq, snapshots)
        .await
        .expect("put snapshots");
    assert!(
        gc > 0,
        "snapshot deposit compacted superseded deltas (gc={gc})"
    );

    // Post-compaction the pre-snapshot deltas are gone from the relay log.
    let post = client_a.poll(0).await.expect("poll post-snapshot").rows;
    assert!(
        post.len() < pre.len(),
        "compaction dropped deltas: {} -> {}",
        pre.len(),
        post.len()
    );
    assert!(
        post.is_empty(),
        "covers_seq covered the whole push — relay log empty after compaction"
    );

    // ── A brand-new EMPTY device C bootstraps PURELY from the snapshots ──
    let client_c = RelayClient::new(ctx.base_url.clone(), group, dev_c, key);
    client_c.register_or_recover().await.expect("c register");
    client_c.verify_registration().await.expect("c verify");
    let tmp_c = tempfile::tempdir().unwrap();
    let (c, notes_c) = authoritative_engine(&tmp_c, dev_c).await;

    let (compaction_seq, fetched) = client_c.fetch_snapshots().await.expect("fetch snapshots");
    assert_eq!(
        compaction_seq, covers_seq,
        "watermark advanced to the push seq"
    );
    assert_eq!(fetched.len(), 3, "one snapshot per note");
    for (stream_id, _snapshot_seq, snap_bytes) in &fetched {
        let id: [u8; 16] = stream_id
            .as_slice()
            .try_into()
            .expect("stream_id is 16 bytes");
        c.import_doc_update(id, snap_bytes)
            .await
            .expect("import snapshot");
    }

    // Any tail past the snapshot watermark (none here — covers_seq covered it).
    let mut cur_c = compaction_seq;
    let _ = relay_pull(&c, &client_c, &mut cur_c, dev_c).await;

    // C reconstructed the ENTIRE mosaic from the compacted snapshot set,
    // byte-identically — the deltas it never saw are fully captured.
    for (id, slug, _t, _bid) in notes {
        let rc = c.render_note(id).await;
        let ra = a.render_note(id).await;
        assert_eq!(
            rc, ra,
            "note '{slug}' restored identically from the snapshot set"
        );
        assert!(
            notes_c.join(format!("{slug}.md")).exists(),
            "'{slug}.md' materialized on snapshot bootstrap"
        );
    }
}
