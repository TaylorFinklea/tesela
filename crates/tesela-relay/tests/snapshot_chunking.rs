//! Chunked snapshot-deposit integration tests (the live whole-mosaic
//! 413 fix): `RelayClient::put_snapshots_chunked` against the in-process
//! Rust relay with configurable body caps.
//!
//! The contract under test:
//! - a chunked deposit is equivalent to a single deposit (a fresh device
//!   bootstraps everything; the watermark lands on `covers_seq`);
//! - every chunk except the last deposits with `covers_seq = 0`, so a
//!   crash between chunks leaves the op log un-GC'd and the watermark
//!   unmoved, and the next full deposit heals;
//! - a 413 halves the chunk budget adaptively until the relay accepts;
//! - a single snapshot that alone exceeds the relay cap is skipped with
//!   the watermark withheld (advancing it would GC ops whose content the
//!   deposited set lacks).
//!
//! The cross-impl `covers_seq = 0` wire contract itself is locked in the
//! shared conformance suite (`test_snapshot_covers_seq_zero_is_inert`),
//! which also runs against the Cloudflare Worker via `wrangler dev`.

use std::collections::HashMap;
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

async fn spawn_relay(max_body: usize) -> Ctx {
    let tmp = tempfile::tempdir().expect("tmp dir");
    let db = tmp.path().join("relay.sqlite");
    let state = AppState::open(&db, max_body, Some("admin".into()))
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

fn random_bytes(len: usize) -> Vec<u8> {
    let mut v = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut v);
    v
}

fn stream_id(tag: u8) -> Vec<u8> {
    vec![tag; 16]
}

/// Register a client for `(group, device)` and PUT `n` small ops so the
/// relay has a live op log + a seq watermark target. Returns the highest
/// assigned seq.
async fn registered_client_with_ops(
    ctx: &Ctx,
    group: GroupId,
    key: &GroupKey,
    device_tag: u8,
    n_ops: usize,
) -> (RelayClient, i64) {
    let device = DeviceId::from_bytes([device_tag; 16]);
    let client = RelayClient::new(ctx.base_url.clone(), group, device, key.clone());
    client.register_or_recover().await.expect("register");
    let mut max_seq = 0;
    for i in 0..n_ops {
        let env = SyncEnvelope {
            from_device: device,
            to_group: group,
            nonce: [0u8; 24],
            ciphertext: format!("op-{i}").into_bytes(),
        };
        let (seq, _ts) = client.put_envelope(env).await.expect("put op");
        max_seq = max_seq.max(seq);
    }
    (client, max_seq)
}

/// Fetch the relay's snapshots as a `stream_id → plaintext` map plus the
/// compaction watermark, through a FRESH probe client (distinct device) —
/// the fresh-device bootstrap view.
async fn probe_snapshots(
    ctx: &Ctx,
    group: GroupId,
    key: &GroupKey,
) -> (i64, HashMap<Vec<u8>, Vec<u8>>) {
    let probe = RelayClient::new(
        ctx.base_url.clone(),
        group,
        DeviceId::from_bytes([0xee; 16]),
        key.clone(),
    );
    let (watermark, snaps) = probe.fetch_snapshots().await.expect("fetch snapshots");
    let map = snaps
        .into_iter()
        .map(|(sid, _seq, plain)| (sid, plain))
        .collect();
    (watermark, map)
}

/// Chunked deposit ≡ single deposit: a fresh device bootstraps the exact
/// same `(stream_id → plaintext)` set with the same watermark, and the op
/// log is compacted identically.
#[tokio::test]
async fn chunked_deposit_equals_single_deposit() {
    let snapshots: Vec<(Vec<u8>, Vec<u8>)> = (0u8..10)
        .map(|i| (stream_id(i + 1), random_bytes(50 * 1024)))
        .collect();

    // Reference: one un-chunked deposit on its own relay/group.
    let ctx_single = spawn_relay(16 * 1024 * 1024).await;
    let (group_s, key_s) = fresh_group();
    let (client_s, covers_s) =
        registered_client_with_ops(&ctx_single, group_s, &key_s, 0xa1, 3).await;
    let gc_single = client_s
        .put_snapshots(covers_s, snapshots.clone())
        .await
        .expect("single deposit");
    let (wm_single, map_single) = probe_snapshots(&ctx_single, group_s, &key_s).await;

    // Chunked: tiny budget forces one entry per chunk (10 PUTs).
    let ctx_chunked = spawn_relay(16 * 1024 * 1024).await;
    let (group_c, key_c) = fresh_group();
    let (client_c, covers_c) =
        registered_client_with_ops(&ctx_chunked, group_c, &key_c, 0xa2, 3).await;
    assert_eq!(covers_c, covers_s, "same op count on both relays");
    let report = client_c
        .put_snapshots_chunked(covers_c, snapshots.clone(), 64 * 1024)
        .await
        .expect("chunked deposit");
    assert!(report.complete(), "nothing skipped: {report:?}");
    assert!(
        report.chunks_sent >= 2,
        "the budget must actually have split the deposit (sent {} chunks)",
        report.chunks_sent
    );
    assert_eq!(report.gc, gc_single, "same ops compacted as the single PUT");

    let (wm_chunked, map_chunked) = probe_snapshots(&ctx_chunked, group_c, &key_c).await;
    assert_eq!(wm_chunked, wm_single, "watermark lands on covers_seq");
    assert_eq!(wm_chunked, covers_c);
    assert_eq!(
        map_chunked, map_single,
        "fresh-device bootstrap sees the identical snapshot set"
    );
    // Sanity against the source of truth too.
    for (sid, plain) in &snapshots {
        assert_eq!(map_chunked.get(sid), Some(plain), "byte-identical payload");
    }
    // Ops at/below covers_seq are gone on both.
    assert!(client_c.poll(0).await.unwrap().rows.is_empty());
}

/// Crash between chunks: depositing chunks 1..n-1 (which the chunked path
/// sends with `covers_seq = 0`) must leave the op log un-GC'd and the
/// watermark unmoved; the next FULL deposit heals everything.
#[tokio::test]
async fn crash_between_chunks_leaves_ops_ungcd_and_watermark_unmoved() {
    let ctx = spawn_relay(16 * 1024 * 1024).await;
    let (group, key) = fresh_group();
    let (client, covers) = registered_client_with_ops(&ctx, group, &key, 0xb1, 3).await;

    let snapshots: Vec<(Vec<u8>, Vec<u8>)> = (0u8..6)
        .map(|i| (stream_id(i + 1), random_bytes(8 * 1024)))
        .collect();

    // Simulate the crash: only the non-final chunks landed. The chunked
    // path sends every non-final chunk with covers_seq = 0; depositing
    // the first half that way IS the post-crash relay state.
    let gc_partial = client
        .put_snapshots(0, snapshots[..3].to_vec())
        .await
        .expect("partial covers_seq=0 deposit");
    assert_eq!(gc_partial, 0, "covers_seq=0 must GC nothing");

    let polled = client.poll(0).await.expect("poll");
    assert_eq!(
        polled.rows.len() + polled.skipped.len(),
        3,
        "all ops still present after the partial deposit"
    );
    let (watermark, map) = probe_snapshots(&ctx, group, &key).await;
    assert_eq!(watermark, 0, "watermark unmoved by covers_seq=0 chunks");
    assert_eq!(map.len(), 3, "partial upserts are stored (harmless)");

    // Recovery: the next full chunked deposit heals — watermark advances,
    // ops compact, the full set is present.
    let report = client
        .put_snapshots_chunked(covers, snapshots.clone(), 16 * 1024)
        .await
        .expect("healing deposit");
    assert!(report.complete());
    assert_eq!(report.gc, 3, "the healing deposit compacts the op log");
    let (watermark, map) = probe_snapshots(&ctx, group, &key).await;
    assert_eq!(watermark, covers);
    assert_eq!(map.len(), 6);
    for (sid, plain) in &snapshots {
        assert_eq!(map.get(sid), Some(plain));
    }
}

/// A relay with a small body cap 413s the configured budget; the client
/// halves and retries until the relay accepts — no relay-side change, no
/// persisted state.
#[tokio::test]
async fn deposit_413_halves_budget_until_accepted() {
    // Cap well below the configured 4 MiB budget but above single entries.
    let ctx = spawn_relay(600 * 1024).await;
    let (group, key) = fresh_group();
    let (client, covers) = registered_client_with_ops(&ctx, group, &key, 0xc1, 2).await;

    // ~641 KiB of serialized entries in one configured-budget chunk →
    // 413 → halve (4 MiB → … → 512 KiB) → accepted in two chunks.
    let snapshots: Vec<(Vec<u8>, Vec<u8>)> = (0u8..12)
        .map(|i| (stream_id(i + 1), random_bytes(40 * 1024)))
        .collect();

    let report = client
        .put_snapshots_chunked(covers, snapshots.clone(), 4 * 1024 * 1024)
        .await
        .expect("adaptive deposit");
    assert!(report.complete(), "nothing skipped: {report:?}");
    assert!(
        report.chunks_sent >= 2,
        "the halved budget split the deposit (sent {})",
        report.chunks_sent
    );
    assert_eq!(report.gc, 2, "final chunk carried the real covers_seq");

    let (watermark, map) = probe_snapshots(&ctx, group, &key).await;
    assert_eq!(watermark, covers);
    assert_eq!(map.len(), 12);
    for (sid, plain) in &snapshots {
        assert_eq!(map.get(sid), Some(plain));
    }
}

/// One snapshot alone exceeds even the relay's cap: it is skipped (loud
/// warn) instead of wedging the whole deposit, the rest still upserts,
/// and the watermark is withheld — advancing it would GC ops whose
/// content the deposited set lacks.
#[tokio::test]
async fn oversize_single_snapshot_skipped_without_advancing_watermark() {
    let ctx = spawn_relay(96 * 1024).await;
    let (group, key) = fresh_group();
    let (client, covers) = registered_client_with_ops(&ctx, group, &key, 0xd1, 3).await;

    let oversize_id = stream_id(0x66);
    let snapshots: Vec<(Vec<u8>, Vec<u8>)> = vec![
        (oversize_id.clone(), random_bytes(200 * 1024)), // > cap even alone
        (stream_id(0x01), random_bytes(8 * 1024)),
        (stream_id(0x02), random_bytes(8 * 1024)),
    ];

    let report = client
        .put_snapshots_chunked(covers, snapshots.clone(), 4 * 1024 * 1024)
        .await
        .expect("deposit must not wedge on the oversize note");
    assert_eq!(
        report.skipped_streams,
        vec![oversize_id.clone()],
        "exactly the oversize snapshot was skipped"
    );
    assert!(!report.complete());
    assert_eq!(report.gc, 0, "watermark withheld → nothing GC'd");

    let polled = client.poll(0).await.expect("poll");
    assert_eq!(
        polled.rows.len() + polled.skipped.len(),
        3,
        "op log intact — the skipped note's ops survive"
    );
    let (watermark, map) = probe_snapshots(&ctx, group, &key).await;
    assert_eq!(watermark, 0, "compaction watermark NOT advanced");
    assert_eq!(map.len(), 2, "the fitting snapshots still deposited");
    assert!(!map.contains_key(&oversize_id));
}
