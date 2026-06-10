//! Long-period soak test against a REAL deployed relay.
//!
//! `#[ignore]`d + env-gated: this is NOT part of the normal test run.
//! It drives two real `LoroEngine` participants (same helpers/tick
//! semantics as `convergence_harness.rs`) against an EXTERNAL relay
//! URL — the deployed image, end to end over the real network — and
//! repeatedly crosses the snapshot-compaction quiet period that
//! produced the seq-reset black hole (#195 / audit A1, fixed in
//! `61506af`). Each round:
//!
//! 1. A edits (new block carrying a round marker) → push.
//! 2. B polls until content-converged (timeout-bounded).
//! 3. B edits → push; A polls until content-converged (both directions).
//! 4. A deposits full per-note snapshots covering its inbound cursor
//!    (mirror of `tick`'s snapshot branch: `covers_seq = inbound
//!    cursor`) — the relay compacts its op log.
//! 5. Sleep the quiet period (default 420s — past the production
//!    5-minute snapshot cadence, so EVERY round's next edit lands on a
//!    freshly-compacted log; with the old bug the post-compaction seq
//!    reset below B's cursor and the edit was permanently invisible).
//!
//! Transient network errors (the relay typically lives on home
//! hardware reached over Tailscale) are retried inside the round; a
//! direction only fails after its convergence timeout. The final
//! assert requires every round to have converged both ways, with a
//! full per-round transcript printed.
//!
//! Config (env):
//! - `TESELA_SOAK_RELAY_URL`  — REQUIRED. Base URL of the deployed relay.
//! - `TESELA_SOAK_ROUNDS`     — default 12.
//! - `TESELA_SOAK_QUIET_SECS` — default 420.
//! - `TESELA_SOAK_CONVERGE_TIMEOUT_SECS` — default 60.
//!
//! Launch (long run, ~12 rounds x 7 min ≈ 1.5h):
//! ```sh
//! TESELA_SOAK_RELAY_URL=http://100.85.144.53:8484 \
//!   cargo test -p tesela-relay --test soak soak_real_relay -- --ignored --nocapture
//! ```
//!
//! A FRESH random group id/key is generated per run (the relay is
//! multi-tenant; existing groups are never touched). The group id is
//! printed so it can be admin-deleted later if desired.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::RngCore;
use reqwest::Url;

use tesela_sync::crypto::keys::GroupKey;
use tesela_sync::device::DeviceId;
use tesela_sync::group::GroupId;
use tesela_sync::transport::relay::RelayClient;
use tesela_sync::wire::envelope::SyncEnvelope;
use tesela_sync::{
    decode_loro_relay_payload, encode_loro_relay_payload, Hlc, LoroDocUpdate, LoroEngine,
    OpPayload, SyncEngine,
};

// ─── Config ─────────────────────────────────────────────────────────

const DEFAULT_ROUNDS: usize = 12;
const DEFAULT_QUIET_SECS: u64 = 420;
const DEFAULT_CONVERGE_TIMEOUT_SECS: u64 = 60;
/// Sleep between convergence-wait poll attempts.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Bounded retries for one-shot relay calls (push / snapshot deposit).
const OP_RETRIES: usize = 10;
const OP_RETRY_DELAY: Duration = Duration::from_secs(3);

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn now_log() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

// ─── Helpers (mirrors of convergence_harness.rs, retry-hardened) ────

fn fresh_group() -> (GroupId, GroupKey) {
    let mut gid = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut gid);
    let mut gk = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut gk);
    (GroupId::from_bytes(gid), GroupKey::from_bytes(gk))
}

fn random_id16() -> [u8; 16] {
    let mut id = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut id);
    id
}

fn uuid_like(bytes: &[u8; 16]) -> String {
    let h = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

async fn engine_at(root: &Path, device: DeviceId) -> LoroEngine {
    LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        root.join("loro"),
        Some(root.join("notes")),
    )
    .await
    .expect("authoritative loro engine")
}

/// Mirror of `tick`'s outbound Loro branch (`relay_push` in the
/// harness), with bounded retries on the network call. Broadcast
/// cursors commit only after a confirmed send; a retry after a
/// lost-response success deposits a duplicate envelope, which is
/// harmless (Loro update application is idempotent).
/// Register with transient-failure retries. A `Crypto` error is a true
/// conflict/hijack signal — fail fast. Anything else (5xx while the HA
/// box is busy serving the real group, network blip) retries.
async fn register_with_retry(client: &RelayClient, who: &str) {
    let mut last = String::new();
    for attempt in 1..=5u32 {
        match client.register_or_recover().await {
            Ok(_) => return,
            Err(e @ tesela_sync::error::SyncError::Crypto(_)) => {
                panic!("{who} registers: non-transient: {e}")
            }
            Err(e) => {
                last = e.to_string();
                println!(
                    "[{}] {who} register attempt {attempt}/5 failed (transient): {last}",
                    now_log()
                );
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
    panic!("{who} registers: all attempts failed: {last}");
}

async fn push_with_retry(
    engine: &LoroEngine,
    client: &RelayClient,
    from: DeviceId,
    group: GroupId,
    label: &str,
) -> Result<Option<i64>, String> {
    let updates = engine.produce_relay_updates().await;
    if updates.is_empty() {
        return Ok(None);
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
    let ciphertext = encode_loro_relay_payload(&payload).map_err(|e| format!("encode v2: {e}"))?;
    let env = SyncEnvelope {
        from_device: from,
        to_group: group,
        nonce: [0u8; 24],
        ciphertext,
    };
    let mut last_err = String::new();
    for attempt in 1..=OP_RETRIES {
        match client.put_envelope(env.clone()).await {
            Ok((seq, _ts)) => {
                engine.commit_broadcast_cursors(&committed).await;
                return Ok(Some(seq));
            }
            Err(e) => {
                last_err = e.to_string();
                println!(
                    "[{}] {label}: push attempt {attempt}/{OP_RETRIES} failed (retrying): {last_err}",
                    now_log()
                );
                tokio::time::sleep(OP_RETRY_DELAY).await;
            }
        }
    }
    Err(format!("push failed after {OP_RETRIES} attempts: {last_err}"))
}

/// Mirror of `tick`'s inbound Loro branch (`relay_pull` in the
/// harness): poll once, skip own echoes, apply, advance + ack the
/// cursor to the batch watermark. Network errors bubble up so the
/// convergence wait can treat them as transient.
async fn pull_once(
    engine: &LoroEngine,
    client: &RelayClient,
    cursor: &mut i64,
    self_dev: DeviceId,
) -> Result<(usize, usize), String> {
    let batch = client.poll(*cursor).await.map_err(|e| e.to_string())?;
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
    Ok((rows, applied))
}

/// Mirror of `tick`'s snapshot-deposit branch: full per-note
/// snapshots, compaction gated at `covers_seq` (= depositor's inbound
/// cursor, exactly like the live tick). Returns ops GC'd by the relay.
async fn deposit_snapshots_with_retry(
    engine: &LoroEngine,
    client: &RelayClient,
    covers_seq: i64,
) -> Result<u64, String> {
    let mut snapshots: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    for id in engine.note_ids().await {
        let snap = engine
            .export_doc_update(id, None)
            .await
            .ok_or_else(|| format!("export snapshot: note {} not resident", hex::encode(id)))?;
        snapshots.push((id.to_vec(), snap));
    }
    let mut last_err = String::new();
    for attempt in 1..=OP_RETRIES {
        match client.put_snapshots(covers_seq, snapshots.clone()).await {
            Ok(gc) => return Ok(gc),
            Err(e) => {
                last_err = e.to_string();
                println!(
                    "[{}] snapshot deposit attempt {attempt}/{OP_RETRIES} failed (retrying): {last_err}",
                    now_log()
                );
                tokio::time::sleep(OP_RETRY_DELAY).await;
            }
        }
    }
    Err(format!(
        "snapshot deposit failed after {OP_RETRIES} attempts: {last_err}"
    ))
}

/// Poll `dst` until its rendered note equals `src`'s AND contains
/// `marker`, or the timeout elapses. Poll errors are transient (logged,
/// retried). On timeout, returns a debug dump: both cursors, both
/// renders, and the relay's compaction watermark.
#[allow(clippy::too_many_arguments)]
async fn wait_converged(
    src: &LoroEngine,
    dst: &LoroEngine,
    dst_client: &RelayClient,
    dst_cursor: &mut i64,
    dst_dev: DeviceId,
    note: [u8; 16],
    marker: &str,
    timeout: Duration,
    label: &str,
) -> Result<Duration, String> {
    let start = Instant::now();
    loop {
        if let Err(e) = pull_once(dst, dst_client, dst_cursor, dst_dev).await {
            println!("[{}] {label}: transient poll error (retrying): {e}", now_log());
        }
        let rs = src.render_note(note).await;
        let rd = dst.render_note(note).await;
        if let (Some(rs), Some(rd)) = (&rs, &rd) {
            if rs == rd && rd.contains(marker) {
                return Ok(start.elapsed());
            }
        }
        if start.elapsed() >= timeout {
            let watermark = match dst_client.fetch_snapshots().await {
                Ok((w, snaps)) => format!("{w} ({} snapshot(s))", snaps.len()),
                Err(e) => format!("unavailable: {e}"),
            };
            return Err(format!(
                "{label}: NOT converged after {:.1}s — marker {marker:?}; \
                 dst cursor {dst_cursor}; relay compaction watermark {watermark}; \
                 src render: {rs:?}; dst render: {rd:?}",
                start.elapsed().as_secs_f64()
            ));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

// ─── The soak ───────────────────────────────────────────────────────

struct RoundOutcome {
    round: usize,
    /// A-edit → B-converged latency, or failure detail.
    a_to_b: Result<Duration, String>,
    /// B-edit → A-converged latency, or failure detail.
    b_to_a: Result<Duration, String>,
    /// relay ops GC'd by this round's snapshot deposit.
    gc: Result<u64, String>,
}

impl RoundOutcome {
    fn ok(&self) -> bool {
        self.a_to_b.is_ok() && self.b_to_a.is_ok() && self.gc.is_ok()
    }
    fn line(&self) -> String {
        fn dir(r: &Result<Duration, String>) -> String {
            match r {
                Ok(d) => format!("ok in {:.3}s", d.as_secs_f64()),
                Err(e) => format!("FAILED — {e}"),
            }
        }
        let gc = match &self.gc {
            Ok(n) => format!("gc={n}"),
            Err(e) => format!("snapshot FAILED — {e}"),
        };
        format!(
            "round {:>2}: A→B {} | B→A {} | {}",
            self.round,
            dir(&self.a_to_b),
            dir(&self.b_to_a),
            gc
        )
    }
}

#[tokio::test]
#[ignore = "long-running soak against a real deployed relay; set TESELA_SOAK_RELAY_URL and pass --ignored"]
async fn soak_real_relay() {
    let url = std::env::var("TESELA_SOAK_RELAY_URL").expect(
        "TESELA_SOAK_RELAY_URL must point at a deployed relay, \
         e.g. TESELA_SOAK_RELAY_URL=http://100.85.144.53:8484",
    );
    let base_url = Url::parse(&url).expect("TESELA_SOAK_RELAY_URL parses as a URL");
    let rounds = env_u64("TESELA_SOAK_ROUNDS", DEFAULT_ROUNDS as u64) as usize;
    let quiet = Duration::from_secs(env_u64("TESELA_SOAK_QUIET_SECS", DEFAULT_QUIET_SECS));
    let converge_timeout = Duration::from_secs(env_u64(
        "TESELA_SOAK_CONVERGE_TIMEOUT_SECS",
        DEFAULT_CONVERGE_TIMEOUT_SECS,
    ));

    // ── Record WHICH deployment this soak ran against. ──
    let identity = match reqwest::Client::new()
        .get(base_url.clone())
        .timeout(Duration::from_secs(15))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            format!("GET / -> {status}: {body}")
        }
        Err(e) => panic!("relay at {url} unreachable: {e}"),
    };
    println!("[{}] soak target: {url}", now_log());
    println!("[{}] relay identity: {identity}", now_log());
    println!(
        "[{}] config: rounds={rounds} quiet={}s converge_timeout={}s",
        now_log(),
        quiet.as_secs(),
        converge_timeout.as_secs()
    );

    // ── Fresh group + two participants (the relay is multi-tenant;
    // a random group id/key can never collide with real data). ──
    let (group, key) = fresh_group();
    println!(
        "[{}] fresh group: {} (admin-deletable later if desired)",
        now_log(),
        hex::encode(group.as_bytes())
    );
    let dev_a = DeviceId::from_bytes(random_id16());
    let dev_b = DeviceId::from_bytes(random_id16());
    let client_a = RelayClient::new(base_url.clone(), group, dev_a, key.clone());
    let client_b = RelayClient::new(base_url.clone(), group, dev_b, key);
    // Registration retries: the relay lives on home hardware that's also
    // serving the user's real group every 5s — a transient 5xx (SQLite
    // busy, HA restart) must not kill a multi-hour soak before round 1.
    // Crypto errors (true conflict/hijack) still fail fast.
    register_with_retry(&client_a, "A").await;
    register_with_retry(&client_b, "B").await;

    let tmp_a = tempfile::tempdir().expect("tmp A");
    let tmp_b = tempfile::tempdir().expect("tmp B");
    let a = engine_at(tmp_a.path(), dev_a).await;
    let b = engine_at(tmp_b.path(), dev_b).await;
    let (mut cur_a, mut cur_b) = (0i64, 0i64);

    // ── Base note: A authors, pushes, B converges once before the
    // rounds begin. ──
    let note = random_id16();
    let bid = uuid_like(&random_id16());
    let slug = format!("soak-{}", &hex::encode(note)[..8]);
    a.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some(slug.clone()),
        title: slug.clone(),
        content: format!("---\ntitle: {slug}\n---\n\n- soak base <!-- bid:{bid} -->\n"),
        created_at_millis: 1,
    })
    .await
    .expect("base note");
    push_with_retry(&a, &client_a, dev_a, group, "setup")
        .await
        .expect("setup push")
        .expect("setup produced an update");
    wait_converged(
        &a,
        &b,
        &client_b,
        &mut cur_b,
        dev_b,
        note,
        "soak base",
        converge_timeout,
        "setup A→B",
    )
    .await
    .expect("setup convergence");
    println!("[{}] setup: base note converged on B", now_log());

    // ── Rounds. ──
    let mut outcomes: Vec<RoundOutcome> = Vec::with_capacity(rounds);
    for round in 1..=rounds {
        let round_started = Instant::now();
        println!("[{}] ── round {round}/{rounds} ──", now_log());

        // 1. A edits with a round marker, pushes.
        let marker_a = format!("round {round} from A");
        a.record_local(OpPayload::BlockUpsert {
            block_id: random_id16(),
            note_id: note,
            parent_block_id: None,
            order_key: format!("a{round:04}"),
            indent_level: 0,
            text: marker_a.clone(),
            after_block_id: None,
        })
        .await
        .expect("A block upsert");
        let a_to_b = match push_with_retry(&a, &client_a, dev_a, group, "A push").await {
            Ok(seq) => {
                println!("[{}] A pushed seq {seq:?}", now_log());
                // 2. B polls until converged.
                wait_converged(
                    &a,
                    &b,
                    &client_b,
                    &mut cur_b,
                    dev_b,
                    note,
                    &marker_a,
                    converge_timeout,
                    "A→B",
                )
                .await
            }
            Err(e) => Err(e),
        };

        // 3. B edits, A converges (the reverse direction).
        let marker_b = format!("round {round} from B");
        b.record_local(OpPayload::BlockUpsert {
            block_id: random_id16(),
            note_id: note,
            parent_block_id: None,
            order_key: format!("b{round:04}"),
            indent_level: 0,
            text: marker_b.clone(),
            after_block_id: None,
        })
        .await
        .expect("B block upsert");
        let b_to_a = match push_with_retry(&b, &client_b, dev_b, group, "B push").await {
            Ok(seq) => {
                println!("[{}] B pushed seq {seq:?}", now_log());
                wait_converged(
                    &b,
                    &a,
                    &client_a,
                    &mut cur_a,
                    dev_a,
                    note,
                    &marker_b,
                    converge_timeout,
                    "B→A",
                )
                .await
            }
            Err(e) => Err(e),
        };

        // 4. Snapshot deposit + compaction, mirroring tick: covers_seq
        //    = A's inbound cursor (A is caught up to the head after its
        //    convergence pull). This compacts the relay's op log, so
        //    NEXT round's edits must allocate above the watermark — the
        //    exact #195 black-hole condition, every round.
        let gc = deposit_snapshots_with_retry(&a, &client_a, cur_a).await;
        if let Ok(n) = &gc {
            println!(
                "[{}] snapshot deposit covers seq {cur_a}, relay GC'd {n} ops",
                now_log()
            );
        }

        let outcome = RoundOutcome {
            round,
            a_to_b,
            b_to_a,
            gc,
        };
        println!(
            "[{}] {} (round wall time {:.1}s)",
            now_log(),
            outcome.line(),
            round_started.elapsed().as_secs_f64()
        );
        outcomes.push(outcome);

        // 5. Quiet period (skip after the final round).
        if round < rounds {
            println!(
                "[{}] quiet period: sleeping {}s…",
                now_log(),
                quiet.as_secs()
            );
            tokio::time::sleep(quiet).await;
        }
    }

    // ── Transcript + final assert. ──
    println!("\n══ soak transcript ({url}) ══");
    println!("relay identity: {identity}");
    for o in &outcomes {
        println!("{}", o.line());
    }
    let failures: Vec<&RoundOutcome> = outcomes.iter().filter(|o| !o.ok()).collect();
    assert!(
        failures.is_empty(),
        "{} of {} soak round(s) FAILED — see transcript above",
        failures.len(),
        outcomes.len()
    );
    println!(
        "All {} round(s) converged both directions across compaction quiet periods.",
        outcomes.len()
    );
}
