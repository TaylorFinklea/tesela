//! Cross-process relay convergence harness (audit A12).
//!
//! Drives two-or-more REAL `LoroEngine`s as separate sync participants
//! through a REAL in-process `tesela-relay`, using the real `RelayClient`
//! seal/poll/snapshot paths (the per-tick push/pull mirrors
//! `tesela-server::sync_relay::tick`, same as `loro_cutover_e2e.rs`).
//! Every scenario asserts CONTENT convergence — rendered note text equal
//! on all participants — not just cursor equality. This is the harness
//! that would have caught the relay seq black hole (#195 / audit A1) and
//! the cursor-past-failure family before they shipped.
//!
//! Scenarios deliberately NOT duplicated here (audit-first rule):
//! - A4 failed-apply hold + heal: covered end-to-end against the REAL
//!   `tick` in `tesela-server/src/sync_relay.rs` tests
//!   (`tick_holds_cursor_at_failed_apply_then_gives_up_after_bound`,
//!   `tick_pending_delta_triggers_snapshot_catchup`,
//!   `bootstrap_partial_failure_keeps_cursor_and_heals_via_tick`).
//!   The hold/retry policy lives in `tick`, not in the engine/client
//!   layer this harness drives — re-implementing it in a test helper
//!   would only test the helper.
//! - A5 relay-switch cursor reset: the identity-scoped cursor state is
//!   server-level (`RelayState::scope_to_identity`), covered in
//!   `sync_relay.rs` (`relay_state_scope_to_identity`).
//! - Plain same-note concurrent edits (no compaction): covered in
//!   `loro_cutover_e2e.rs` (`two_authoritative_engines_converge_over_real_relay`).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
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

/// Authoritative engine rooted at `root` (so a "restart" can rebuild
/// from the SAME persisted dirs, the way `tesela-server` does on boot).
async fn engine_at(root: &Path, device: DeviceId) -> (LoroEngine, PathBuf) {
    let notes = root.join("notes");
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        root.join("loro"),
        Some(notes.clone()),
    )
    .await
    .expect("authoritative loro engine");
    (engine, notes)
}

/// Mirror of `tick`'s outbound Loro branch: produce per-note updates,
/// wrap in the v2 payload, deposit ONE envelope, commit broadcast
/// cursors only after the confirmed send. Returns the relay-assigned
/// seq, or None when there was nothing new to send.
async fn relay_push(
    engine: &LoroEngine,
    client: &RelayClient,
    from: DeviceId,
    group: GroupId,
) -> Option<i64> {
    let updates = engine.produce_relay_updates().await;
    if updates.is_empty() {
        return None;
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
    let (seq, _ts) = client.put_envelope(env).await.expect("put envelope");
    engine.commit_broadcast_cursors(&committed).await;
    Some(seq)
}

/// Mirror of `tick`'s inbound Loro branch: poll, skip own echoes,
/// decode the v2 payload, apply, then advance + ack the cursor to the
/// batch watermark — which covers skipped (poisoned) rows too, exactly
/// like the live tick. Returns `(rows_delivered, applied_count)`.
async fn relay_pull(
    engine: &LoroEngine,
    client: &RelayClient,
    cursor: &mut i64,
    self_dev: DeviceId,
) -> (usize, usize) {
    let batch = client.poll(*cursor).await.expect("poll");
    let mut applied = 0;
    for (_seq, env) in &batch.rows {
        if env.from_device != self_dev {
            if let Ok(Some(updates)) = decode_loro_relay_payload(&env.ciphertext) {
                let pairs: Vec<([u8; 16], Vec<u8>)> = updates
                    .into_iter()
                    .map(|u| (u.doc, u.update_bytes))
                    .collect();
                applied += engine.apply_relay_updates(&pairs).await.applied_count();
            }
        }
    }
    let rows = batch.rows.len();
    if let Some(max_seq) = batch.max_seq() {
        if max_seq > *cursor {
            *cursor = max_seq;
            let _ = client.ack(max_seq).await;
        }
    }
    (rows, applied)
}

/// Mirror of `tick`'s snapshot-deposit branch: export a FULL snapshot
/// per resident note and deposit the set, gating relay compaction at
/// `covers_seq`. Returns the number of ops the relay GC'd.
async fn deposit_full_snapshots(engine: &LoroEngine, client: &RelayClient, covers_seq: i64) -> u64 {
    let mut snapshots: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    for id in engine.note_ids().await {
        let snap = engine
            .export_doc_update(id, None)
            .await
            .expect("full note snapshot");
        snapshots.push((id.to_vec(), snap));
    }
    client
        .put_snapshots(covers_seq, snapshots)
        .await
        .expect("put snapshots")
}

/// Deposit a raw payload over HTTP with a valid MAC, bypassing
/// `put_envelope`'s always-well-formed sealing — how a poisoned row
/// reaches the relay in real life (version skew, corruption, garbage
/// from anyone past the MAC gate). Same shape as `client_e2e.rs`.
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

async fn upsert_note(engine: &LoroEngine, note_id: [u8; 16], slug: &str, bid: &str) {
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(slug.into()),
            title: slug.into(),
            content: format!("---\ntitle: {slug}\n---\n\n- base of {slug} <!-- bid:{bid} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();
}

async fn upsert_block(engine: &LoroEngine, note_id: [u8; 16], block_id: [u8; 16], text: &str) {
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id,
            note_id,
            parent_block_id: None,
            order_key: hex::encode(block_id)[..4].to_string(),
            indent_level: 0,
            text: text.into(),
            after_block_id: None,
        })
        .await
        .unwrap();
}

async fn assert_converged(a: &LoroEngine, b: &LoroEngine, note: [u8; 16], label: &str) -> String {
    let ra = a.render_note(note).await;
    let rb = b.render_note(note).await;
    assert!(ra.is_some(), "{label}: note resident on first participant");
    assert_eq!(ra, rb, "{label}: rendered note text equal on both");
    ra.unwrap()
}

const NOTE: [u8; 16] = [0x77; 16];
const BID: &str = "70707070-7070-7070-7070-707070707070";

/// A1 regression (the #195 seq black hole), end to end: A edits → B
/// converges (cursor caught up) → A deposits full snapshots (relay GCs
/// the whole op log) → A makes a NEW edit during the quiet period →
/// the new envelope must land ABOVE the compaction watermark so B's
/// caught-up cursor still receives it. With the bug, the post-
/// compaction seq restarts at 1 — below B's cursor — and the edit is
/// permanently undeliverable.
#[tokio::test]
async fn quiet_period_compaction_then_new_edit_reaches_caught_up_peer() {
    let ctx = spawn_relay().await;
    let (group, key) = fresh_group();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let client_a = RelayClient::new(ctx.base_url.clone(), group, dev_a, key.clone());
    let client_b = RelayClient::new(ctx.base_url.clone(), group, dev_b, key);
    client_a.register_or_recover().await.expect("a register");
    client_b.register_or_recover().await.expect("b register");

    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let (a, _) = engine_at(tmp_a.path(), dev_a).await;
    let (b, _) = engine_at(tmp_b.path(), dev_b).await;
    let (mut cur_a, mut cur_b) = (0i64, 0i64);

    // A authors, B converges — B's cursor is now caught up.
    upsert_note(&a, NOTE, "shared", BID).await;
    let seq1 = relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("a pushed");
    let (_, applied) = relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    assert_eq!(applied, 1, "B applied A's note");
    assert_eq!(cur_b, seq1, "B caught up to the head of the log");
    assert_converged(&a, &b, NOTE, "pre-compaction").await;

    // Quiet period: A deposits snapshots covering EVERYTHING — full
    // compaction, the relay op log is empty.
    let gc = deposit_full_snapshots(&a, &client_a, seq1).await;
    assert!(gc > 0, "compaction GC'd the superseded ops (gc={gc})");
    assert!(
        client_a.poll(0).await.expect("probe").rows.is_empty(),
        "relay op log fully compacted"
    );

    // A makes a NEW edit after the quiet period.
    upsert_block(&a, NOTE, [0x7a; 16], "after the quiet period").await;
    let seq2 = relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("a pushed the new edit");
    assert!(
        seq2 > seq1,
        "new op allocates ABOVE the compaction watermark (seq {seq2} vs covers {seq1}) — \
         a reset-to-1 seq is invisible to every caught-up cursor"
    );

    // B (caught-up cursor) must still receive it.
    let (_, applied) = relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    assert_eq!(applied, 1, "B received the post-compaction edit");
    let rendered = assert_converged(&a, &b, NOTE, "post-compaction").await;
    assert!(
        rendered.contains("after the quiet period"),
        "the new edit is present on both sides: {rendered:?}"
    );
    let _ = relay_pull(&a, &client_a, &mut cur_a, dev_a).await;
}

/// Fresh-device bootstrap past GC: A authors + deposits snapshots
/// (compaction GCs the ops) and keeps editing AFTERWARDS. A brand-new
/// device C joins, restores from the snapshot set, then tail-polls the
/// post-compaction ops from the watermark — full content convergence
/// including the tail edit the snapshots never saw.
#[tokio::test]
async fn fresh_device_bootstraps_past_gc_then_tail_polls_new_edits() {
    let ctx = spawn_relay().await;
    let (group, key) = fresh_group();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_c = DeviceId::from_bytes([0xcc; 16]);
    let client_a = RelayClient::new(ctx.base_url.clone(), group, dev_a, key.clone());
    client_a.register_or_recover().await.expect("a register");

    let tmp_a = tempfile::tempdir().unwrap();
    let (a, _) = engine_at(tmp_a.path(), dev_a).await;

    const NOTE2: [u8; 16] = [0x88; 16];
    upsert_note(&a, NOTE, "alpha", BID).await;
    upsert_note(&a, NOTE2, "beta", "80808080-8080-8080-8080-808080808080").await;
    let covers = relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("a pushed");

    // Full compaction: ops are GONE, only snapshots remain.
    deposit_full_snapshots(&a, &client_a, covers).await;
    assert!(
        client_a.poll(0).await.expect("probe").rows.is_empty(),
        "pre-snapshot ops GC'd"
    );

    // A keeps editing after compaction — the tail C must pick up.
    upsert_block(&a, NOTE, [0x7a; 16], "tail edit past the snapshots").await;
    relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("a pushed the tail");

    // Fresh device C: snapshots first, then tail-poll from the watermark.
    let client_c = RelayClient::new(ctx.base_url.clone(), group, dev_c, key);
    client_c.register_or_recover().await.expect("c register");
    let tmp_c = tempfile::tempdir().unwrap();
    let (c, notes_c) = engine_at(tmp_c.path(), dev_c).await;

    let (watermark, snaps) = client_c.fetch_snapshots().await.expect("fetch snapshots");
    assert_eq!(watermark, covers, "relay watermark = the covered push");
    assert_eq!(snaps.len(), 2, "one snapshot per note");
    for (stream_id, _snapshot_seq, snap_bytes) in &snaps {
        let id: [u8; 16] = stream_id.as_slice().try_into().expect("16-byte stream id");
        c.import_doc_update(id, snap_bytes)
            .await
            .expect("import snapshot");
    }
    let mut cur_c = watermark;
    let (_, tail_applied) = relay_pull(&c, &client_c, &mut cur_c, dev_c).await;
    assert_eq!(tail_applied, 1, "C applied the post-compaction tail op");

    let rendered = assert_converged(&a, &c, NOTE, "bootstrap+tail alpha").await;
    assert!(
        rendered.contains("tail edit past the snapshots"),
        "tail edit present on the fresh device: {rendered:?}"
    );
    assert_converged(&a, &c, NOTE2, "bootstrap beta").await;
    assert!(
        notes_c.join("alpha.md").exists() && notes_c.join("beta.md").exists(),
        "fresh device materialized both notes"
    );
}

/// Restart resume: B converges, persists its cursor (as the server's
/// `RelayState` file does) and its engine state (loro snapshot dir),
/// then shuts down. A edits while B is offline. B rebuilds from the
/// SAME persisted dirs + cursor and resumes: it must receive EXACTLY
/// the missed envelopes (no replay storm — no re-broadcast of its own
/// mosaic, no re-fetch of old rows) and skip nothing.
#[tokio::test]
async fn restart_resume_applies_only_missed_ops_no_storm_no_skip() {
    let ctx = spawn_relay().await;
    let (group, key) = fresh_group();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let client_a = RelayClient::new(ctx.base_url.clone(), group, dev_a, key.clone());
    let client_b = RelayClient::new(ctx.base_url.clone(), group, dev_b, key.clone());
    client_a.register_or_recover().await.expect("a register");
    client_b.register_or_recover().await.expect("b register");

    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let (a, _) = engine_at(tmp_a.path(), dev_a).await;
    let (b, _) = engine_at(tmp_b.path(), dev_b).await;
    let (mut cur_a, mut cur_b) = (0i64, 0i64);

    // Converge both ways: A authors the note, B adds a block.
    upsert_note(&a, NOTE, "shared", BID).await;
    relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("a push");
    relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    upsert_block(&b, NOTE, [0x7b; 16], "from B before restart").await;
    relay_push(&b, &client_b, dev_b, group)
        .await
        .expect("b push");
    relay_pull(&a, &client_a, &mut cur_a, dev_a).await;
    // B advances over its own echo so its persisted cursor is the head.
    relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    let pre_restart = assert_converged(&a, &b, NOTE, "pre-restart").await;

    // ── B shuts down. Cursor persisted (the server writes RelayState
    // to disk; here it's the surviving i64). Engine state persisted in
    // tmp_b's loro dir. ──
    let persisted_cursor = cur_b;
    drop(b);
    drop(client_b);

    // A edits twice while B is down → two envelopes B has not seen.
    upsert_block(&a, NOTE, [0x51; 16], "offline edit one").await;
    relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("a push 1");
    upsert_block(&a, NOTE, [0x52; 16], "offline edit two").await;
    relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("a push 2");

    // ── B resumes from its persisted dirs + cursor. ──
    let client_b2 = RelayClient::new(ctx.base_url.clone(), group, dev_b, key);
    let (b2, _) = engine_at(tmp_b.path(), dev_b).await;
    assert_eq!(
        b2.render_note(NOTE).await.as_deref(),
        Some(pre_restart.as_str()),
        "rebuilt engine restored its persisted state"
    );
    // No outbound replay storm: broadcast cursors persisted, so the
    // rebuilt engine has nothing new to emit.
    assert!(
        b2.produce_relay_updates().await.is_empty(),
        "resume must not re-broadcast the whole mosaic"
    );

    let mut cur_b2 = persisted_cursor;
    let (rows, applied) = relay_pull(&b2, &client_b2, &mut cur_b2, dev_b).await;
    assert_eq!(
        rows, 2,
        "resume polls EXACTLY the two missed envelopes (no replay, no skip)"
    );
    assert_eq!(applied, 2, "both offline edits applied");
    let rendered = assert_converged(&a, &b2, NOTE, "post-resume").await;
    assert!(
        rendered.contains("offline edit one") && rendered.contains("offline edit two"),
        "both offline edits present after resume: {rendered:?}"
    );
    assert!(
        rendered.contains("from B before restart"),
        "B's own pre-restart edit survived the restart: {rendered:?}"
    );
    // Steady state: nothing further to fetch.
    let (rows, _) = relay_pull(&b2, &client_b2, &mut cur_b2, dev_b).await;
    assert_eq!(rows, 0, "no poll loop after resume");
}

/// A3 regression, end to end: a garbage envelope BETWEEN two good ones
/// must not wedge the consumer. Both good envelopes apply, the poison
/// is skipped, the cursor advances past it (no permanent re-fetch),
/// and later edits still converge.
#[tokio::test]
async fn poisoned_envelope_between_good_ones_both_good_apply_and_sync_continues() {
    let ctx = spawn_relay().await;
    let (group, key) = fresh_group();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let client_a = RelayClient::new(ctx.base_url.clone(), group, dev_a, key.clone());
    let client_b = RelayClient::new(ctx.base_url.clone(), group, dev_b, key.clone());
    client_a.register_or_recover().await.expect("a register");
    client_b.register_or_recover().await.expect("b register");

    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let (a, _) = engine_at(tmp_a.path(), dev_a).await;
    let (b, _) = engine_at(tmp_b.path(), dev_b).await;
    let mut cur_b = 0i64;

    // Good envelope 1.
    upsert_note(&a, NOTE, "shared", BID).await;
    relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("good 1");
    // Poison between the good ones: fails the outer postcard decode.
    let poison_seq = deposit_raw(&ctx.base_url, group, &key, dev_a, &[0xde, 0xad, 0xbe]).await;
    // Good envelope 2.
    upsert_block(&a, NOTE, [0x7a; 16], "good edit after the poison").await;
    let good2_seq = relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("good 2");
    assert!(poison_seq < good2_seq);

    // ONE pull: both good envelopes apply, the poison is skipped, the
    // cursor lands past it.
    let (rows, applied) = relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    assert_eq!(rows, 2, "both good envelopes delivered around the poison");
    assert_eq!(applied, 2, "both good envelopes applied");
    assert!(
        cur_b >= good2_seq,
        "cursor advanced past the poisoned row (cursor {cur_b})"
    );
    let rendered = assert_converged(&a, &b, NOTE, "around the poison").await;
    assert!(
        rendered.contains("good edit after the poison"),
        "the later good envelope applied: {rendered:?}"
    );

    // Sync continues normally after the poison.
    upsert_block(&a, NOTE, [0x7c; 16], "life goes on").await;
    relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("good 3");
    let (rows, applied) = relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    assert_eq!((rows, applied), (1, 1), "later edits still flow");
    let rendered = assert_converged(&a, &b, NOTE, "after the poison").await;
    assert!(rendered.contains("life goes on"), "{rendered:?}");
}

/// Concurrent edits ACROSS a compaction boundary: A and B are
/// converged, A compacts the relay (quiet period), then BOTH edit the
/// same note concurrently (different blocks). Both outbound envelopes
/// must allocate above the watermark, exchange, and merge — both
/// blocks present on both sides. (The plain no-compaction concurrent
/// case lives in `loro_cutover_e2e.rs`.)
#[tokio::test]
async fn concurrent_edits_across_compaction_boundary_both_survive() {
    let ctx = spawn_relay().await;
    let (group, key) = fresh_group();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let client_a = RelayClient::new(ctx.base_url.clone(), group, dev_a, key.clone());
    let client_b = RelayClient::new(ctx.base_url.clone(), group, dev_b, key);
    client_a.register_or_recover().await.expect("a register");
    client_b.register_or_recover().await.expect("b register");

    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let (a, _) = engine_at(tmp_a.path(), dev_a).await;
    let (b, _) = engine_at(tmp_b.path(), dev_b).await;
    let (mut cur_a, mut cur_b) = (0i64, 0i64);

    // Converge on the base note.
    upsert_note(&a, NOTE, "shared", BID).await;
    let seq1 = relay_push(&a, &client_a, dev_a, group)
        .await
        .expect("a push");
    relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    assert_converged(&a, &b, NOTE, "base").await;

    // Quiet-period compaction.
    deposit_full_snapshots(&a, &client_a, seq1).await;

    // Concurrent edits on both sides, different blocks.
    upsert_block(&a, NOTE, [0x7a; 16], "concurrent from A").await;
    upsert_block(&b, NOTE, [0x7b; 16], "concurrent from B").await;

    // Exchange over a few rounds, both directions.
    for _ in 0..3 {
        relay_push(&a, &client_a, dev_a, group).await;
        relay_push(&b, &client_b, dev_b, group).await;
        relay_pull(&a, &client_a, &mut cur_a, dev_a).await;
        relay_pull(&b, &client_b, &mut cur_b, dev_b).await;
    }

    let rendered = assert_converged(&a, &b, NOTE, "post-compaction concurrent").await;
    assert!(
        rendered.contains("base of shared")
            && rendered.contains("concurrent from A")
            && rendered.contains("concurrent from B"),
        "base + both concurrent blocks survived the compaction boundary: {rendered:?}"
    );
}
