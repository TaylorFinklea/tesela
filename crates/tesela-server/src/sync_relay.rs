//! Desktop-side WAN relay sync.
//!
//! When the mosaic config carries `[sync.relay] url = "…"`, the
//! server brings up a [`RelayClient`] on startup, runs the
//! registration + joiner-verification handshake, and then ticks a
//! poll/produce loop alongside the existing per-peer LAN sync. This
//! module is the glue: cursor persistence, the per-tick function,
//! and the JSON status response the web settings page reads.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use tesela_sync::transport::relay::RelayClient;
use tesela_sync::{GroupIdentity, SyncEnvelope};

/// Per-mosaic relay sync state, persisted to `.tesela/relay_state.json`
/// so cursors survive restart. Schema is intentionally tiny — the
/// real state-of-record is the relay itself (server-side ops table)
/// plus the engine's per-note Loro snapshots + broadcast cursors.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelayState {
    /// Highest relay-assigned `seq` we've applied + acked. Drives the
    /// `?since=N` query parameter on poll.
    pub inbound_cursor: i64,
    /// Wall-clock seconds of the last successful poll. Surfaces in
    /// the web settings page so the user can see "last fetch 12s ago".
    pub last_poll_at: Option<i64>,
    /// Wall-clock seconds of the last successful PUT. Same idea.
    pub last_put_at: Option<i64>,
    /// Wall-clock seconds we became registered on the relay.
    /// Persisted from `register_or_recover()`'s return so the joiner
    /// verification path can recover even after the file is rebuilt.
    pub registered_at: Option<i64>,
    /// Most recent error string (if any) from poll/put/register, for
    /// the web UI to surface. Cleared after the next successful tick.
    pub last_error: Option<String>,
    /// Wall-clock seconds of the last snapshot deposit. Drives the
    /// snapshot-cadence gate so we deposit per-note snapshots (and let the
    /// relay compact its retained op log) periodically, not every tick.
    #[serde(default)]
    pub last_snapshot_at: Option<i64>,
}

impl RelayState {
    fn path(mosaic_root: &Path) -> PathBuf {
        mosaic_root.join(".tesela").join("relay_state.json")
    }

    /// Best-effort load. Missing file → default state (fresh run).
    pub async fn load(mosaic_root: &Path) -> Self {
        let path = Self::path(mosaic_root);
        match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Atomic-ish write: write to a tmp file then rename. SQLite-WAL-
    /// style durability isn't needed — losing a few seconds of cursor
    /// progress just means we re-poll a few seqs we've already applied
    /// (idempotent at the engine layer).
    pub async fn save(&self, mosaic_root: &Path) -> Result<()> {
        let path = Self::path(mosaic_root);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("create dir {}", parent.display()))?;
        }
        let bytes = serde_json::to_vec_pretty(self).context("serialize relay state")?;
        let tmp = path.with_extension("json.tmp");
        tokio::fs::write(&tmp, &bytes)
            .await
            .with_context(|| format!("write tmp {}", tmp.display()))?;
        tokio::fs::rename(&tmp, &path)
            .await
            .with_context(|| format!("rename {}", path.display()))?;
        Ok(())
    }
}

/// Runtime handle held by `AppState`. Cloneable so the status endpoint
/// + the background daemon both see the same view.
#[derive(Clone)]
pub struct RelayHandle {
    pub url: String,
    pub client: Arc<RelayClient>,
    pub state: Arc<RwLock<RelayState>>,
    pub mosaic_root: PathBuf,
}

/// JSON shape returned by `GET /sync/relay/status`. Surfaced verbatim
/// to the web settings page so the user sees what's happening.
#[derive(Debug, Clone, Serialize)]
pub struct RelayStatus {
    /// Whether the desktop is configured to talk to a relay at all.
    pub configured: bool,
    /// The relay URL we're talking to (omitted when `configured` is false).
    pub url: Option<String>,
    /// Last persisted cursors + timestamps.
    pub inbound_cursor: i64,
    pub last_poll_at: Option<i64>,
    pub last_put_at: Option<i64>,
    pub registered_at: Option<i64>,
    pub last_error: Option<String>,
}

impl RelayStatus {
    pub fn from_handle(h: &RelayHandle, state: &RelayState) -> Self {
        Self {
            configured: true,
            url: Some(h.url.clone()),
            inbound_cursor: state.inbound_cursor,
            last_poll_at: state.last_poll_at,
            last_put_at: state.last_put_at,
            registered_at: state.registered_at,
            last_error: state.last_error.clone(),
        }
    }

    pub fn disabled() -> Self {
        Self {
            configured: false,
            url: None,
            inbound_cursor: 0,
            last_poll_at: None,
            last_put_at: None,
            registered_at: None,
            last_error: None,
        }
    }
}

/// Outcome of one relay [`tick`]: observability counts plus the set of
/// note ids whose docs were mutated by an inbound apply this tick. The
/// caller uses `applied_note_ids` to drive the live-WS fan-out for
/// relay-originated edits — emit a `WsEvent::NoteUpdated` (so web
/// invalidates) and re-broadcast the applied delta on `ws_delta_tx` (spec
/// finding #4). Empty when nothing inbound applied.
#[derive(Debug, Clone, Default)]
pub struct TickOutcome {
    /// Count of inbound per-note updates applied this tick.
    pub applied: u32,
    /// Count of outbound envelopes PUT this tick.
    pub sent: u32,
    /// Note ids touched by an inbound apply, deduped, in first-seen order.
    pub applied_note_ids: Vec<[u8; 16]>,
}

/// One iteration of the relay sync loop: inbound (poll + apply +
/// ack) followed by outbound (produce + put). Logs at debug level on
/// each step; surfaces hard errors through `RelayState.last_error`
/// for the web UI.
///
/// Returns a [`TickOutcome`] (counts + applied note ids) for observability
/// and the live-WS fan-out of relay-originated edits.
pub async fn tick(
    engine: &dyn tesela_sync::SyncEngine,
    ident: &GroupIdentity,
    handle: &RelayHandle,
) -> Result<TickOutcome> {
    let mut state = handle.state.write().await;
    let mut applied_total = 0u32;
    let mut sent_total = 0u32;
    let mut applied_note_ids: Vec<[u8; 16]> = Vec::new();

    // ─── Inbound ─────────────────────────────────────────────────────
    match handle.client.poll(state.inbound_cursor).await {
        Ok(batch) => {
            let mut max_seq = state.inbound_cursor;
            // Rows whose outer payload failed to decode/AEAD-open were
            // skipped inside poll() (deterministic failures — foreign
            // key, corrupt payload; RelayClient already logged each).
            // Advance past their seqs too so one poisoned row can't
            // wedge inbound sync for this group forever.
            for seq in &batch.skipped {
                if *seq > max_seq {
                    max_seq = *seq;
                }
            }
            for (seq, env) in batch.rows {
                // Drop our own echoes — the relay sees everyone's PUTs,
                // including our own. Applying them would no-op at the
                // engine but burns cycles.
                if env.from_device == engine.device() {
                    if seq > max_seq {
                        max_seq = seq;
                    }
                    continue;
                }
                let peer = env.from_device;
                // The envelope plaintext is the `TLR2` magic +
                // postcard(Vec<LoroDocUpdate>). Import each per-note update
                // (idempotent + commutative). A non-v2 payload (legacy /
                // foreign) decodes to None; we skip it but still advance the
                // relay cursor so we don't re-fetch it forever.
                match tesela_sync::decode_loro_relay_payload(&env.ciphertext) {
                    Ok(Some(updates)) => {
                        let pairs: Vec<([u8; 16], Vec<u8>)> = updates
                            .into_iter()
                            .map(|u| (u.doc, u.update_bytes))
                            .collect();
                        let n = engine.apply_relay_updates(&pairs).await;
                        applied_total += n as u32;
                        if n > 0 {
                            // Loro apply is idempotent, so the caller emitting a
                            // WsEvent for a no-op merge is harmless; record each
                            // doc in this applied batch (deduped) so the live-WS
                            // fan-out can notify web + re-broadcast the delta.
                            for (doc, _) in &pairs {
                                if !applied_note_ids.contains(doc) {
                                    applied_note_ids.push(*doc);
                                }
                            }
                        }
                        if seq > max_seq {
                            max_seq = seq;
                        }
                    }
                    Ok(None) => {
                        tracing::debug!(
                            "relay: skip non-v2 payload seq={} from={}",
                            seq,
                            hex::encode(peer.as_bytes())
                        );
                        if seq > max_seq {
                            max_seq = seq;
                        }
                    }
                    Err(e) => {
                        // A decode error is deterministic — re-fetching the
                        // same bytes will fail identically. Advance past it so
                        // one malformed envelope can't stall inbound sync
                        // forever (the AEAD layer already authenticated the
                        // sender, so this is a sender bug, not tampering;
                        // skipping is safe).
                        tracing::warn!(
                            "relay loro decode seq={} from={}: {} (skipping)",
                            seq,
                            hex::encode(peer.as_bytes()),
                            e
                        );
                        if seq > max_seq {
                            max_seq = seq;
                        }
                    }
                }
            }
            if max_seq > state.inbound_cursor {
                state.inbound_cursor = max_seq;
                if let Err(e) = handle.client.ack(max_seq).await {
                    tracing::debug!("relay ack({}): {}", max_seq, e);
                }
            }
            state.last_poll_at = Some(now_secs_i64());
            state.last_error = None;
        }
        Err(e) => {
            let msg = format!("relay poll: {e}");
            tracing::warn!("{msg}");
            state.last_error = Some(msg);
        }
    }

    // ─── Outbound ────────────────────────────────────────────────────
    let our_device = engine.device();

    // Broadcast per-note Loro updates accrued since each note's
    // last-broadcast version vector (the engine tracks + persists those
    // cursors internally). One envelope carries the whole batch; receivers
    // import idempotently. There is no HLC outbound cursor — the engine's
    // per-note VV cursors are the watermark.
    //
    // produce_relay_updates returns (note_id, update_bytes, captured_vv).
    // The cursor is NOT yet advanced — we commit it only after the PUT is
    // confirmed, so a failed send is retried next tick rather than dropped.
    let updates = engine.produce_relay_updates().await;
    // Chunk into size-bounded batches so each PUT fits the relay body limit —
    // the canonical bootstrap broadcasts every note's full state and would
    // otherwise 413. Commit each batch's cursors only after its PUT confirms;
    // skip (don't stop) on failure so one over-limit batch can't block the
    // others, and uncommitted batches re-produce next tick.
    let batches = tesela_sync::pack_loro_relay_batches(
        updates,
        tesela_sync::MAX_RELAY_PLAINTEXT_BYTES,
    );
    for (payload, committed) in batches {
        let ciphertext = match tesela_sync::encode_loro_relay_payload(&payload) {
            Ok(c) => c,
            Err(e) => {
                state.last_error = Some(format!("encode loro payload: {e}"));
                continue;
            }
        };
        let envelope = SyncEnvelope {
            from_device: our_device,
            to_group: ident.group_id,
            nonce: [0u8; 24],
            ciphertext,
        };
        match handle.client.put_envelope(envelope).await {
            Ok((_seq, _ts)) => {
                engine.commit_broadcast_cursors(&committed).await;
                sent_total += 1;
                state.last_put_at = Some(now_secs_i64());
                state.last_error = None;
            }
            Err(e) => {
                let msg = format!("relay put (loro): {e}");
                tracing::warn!("{msg}");
                state.last_error = Some(msg);
                continue;
            }
        }
    }

    // ─── Snapshot-gated compaction cadence ───────────────────────────
    // Periodically deposit a full per-note snapshot set covering everything
    // we've applied (`inbound_cursor`), so the relay can GC the encrypted op
    // log it retains (it stays a durable backup via the snapshots). This is
    // the live wiring of the Phase-1 mechanism; one depositor (this server)
    // is enough — deposits are idempotent. Gated by a (test-tunable) interval
    // so a busy tick loop doesn't re-upload every note's snapshot constantly.
    let now = now_secs_i64();
    let due = state
        .last_snapshot_at
        .map_or(true, |t| now - t >= snapshot_interval_secs());
    if due && state.inbound_cursor > 0 {
        match deposit_snapshots(engine, &handle.client, state.inbound_cursor).await {
            Ok(gc) => {
                state.last_snapshot_at = Some(now);
                if gc > 0 {
                    tracing::debug!(
                        "relay snapshot deposit: covers seq {}, relay GC'd {} ops",
                        state.inbound_cursor,
                        gc
                    );
                }
            }
            Err(e) => {
                let msg = format!("relay snapshot deposit: {e}");
                tracing::warn!("{msg}");
                state.last_error = Some(msg);
            }
        }
    }

    // Persist whatever progress we made this tick.
    if let Err(e) = state.save(&handle.mosaic_root).await {
        tracing::warn!("relay state save: {e}");
    }
    Ok(TickOutcome {
        applied: applied_total,
        sent: sent_total,
        applied_note_ids,
    })
}

/// Deposit a full per-note snapshot set covering relay-seq `covers_seq`
/// (every tracked note's full Loro snapshot, keyed by note_id = stream_id).
/// Returns the number of ops the relay compacted as a result. Idempotent.
async fn deposit_snapshots(
    engine: &dyn tesela_sync::SyncEngine,
    client: &RelayClient,
    covers_seq: i64,
) -> Result<u64, String> {
    let note_ids = engine.tracked_note_ids().await;
    let mut snapshots: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(note_ids.len());
    for id in note_ids {
        // `export_doc_update(id, None)` = the note's full compact snapshot.
        if let Some(bytes) = engine.export_doc_update(id, None).await {
            snapshots.push((id.to_vec(), bytes));
        }
    }
    if snapshots.is_empty() {
        return Ok(0);
    }
    client
        .put_snapshots(covers_seq, snapshots)
        .await
        .map_err(|e| e.to_string())
}

/// Bootstrap a fresh / long-offline device from the relay's compacted
/// snapshots: if the relay's compaction watermark is ahead of our inbound
/// cursor, the ops we'd need are already GC'd, so we import the per-note
/// snapshots and jump the cursor to the watermark. The subsequent `?since=`
/// poll then collects only the un-compacted tail. Idempotent (Loro merge);
/// a no-op when we're already caught up past the watermark.
pub async fn bootstrap_from_snapshots(
    engine: &dyn tesela_sync::SyncEngine,
    handle: &RelayHandle,
) {
    let (compaction_seq, snaps) = match handle.client.fetch_snapshots().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("relay snapshot bootstrap fetch: {e}");
            return;
        }
    };
    let mut state = handle.state.write().await;
    if compaction_seq <= state.inbound_cursor {
        return; // already past the watermark — nothing compacted out from under us
    }
    let mut imported = 0u32;
    for (stream_id, _snapshot_seq, plaintext) in snaps {
        let Ok(note_id) = <[u8; 16]>::try_from(stream_id.as_slice()) else {
            continue; // v1 stream_id is the 16-byte note_id; skip anything else
        };
        if let Err(e) = engine.import_doc_update(note_id, &plaintext).await {
            tracing::warn!(
                "relay snapshot bootstrap import {}: {e}",
                hex::encode(note_id)
            );
            continue;
        }
        imported += 1;
    }
    state.inbound_cursor = compaction_seq;
    tracing::info!(
        "relay snapshot bootstrap: imported {} note(s), cursor → {}",
        imported,
        compaction_seq
    );
    if let Err(e) = state.save(&handle.mosaic_root).await {
        tracing::warn!("relay state save (post-bootstrap): {e}");
    }
}

/// Snapshot-deposit cadence in seconds. Env-tunable
/// (`TESELA_RELAY_SNAPSHOT_INTERVAL_SECS`) so tests can force every-tick
/// deposits; defaults to 5 minutes in production.
fn snapshot_interval_secs() -> i64 {
    std::env::var("TESELA_RELAY_SNAPSHOT_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(300)
}

/// One-time bring-up: register on the relay (idempotent / recovery
/// path), verify the stored intent, persist `registered_at`. Returns
/// `Ok` even on failure — the caller wires the error into RelayState
/// + lets the daemon retry on its tick.
pub async fn bring_up(
    handle: &RelayHandle,
) -> Result<(), String> {
    let registered_at = handle
        .client
        .register_or_recover()
        .await
        .map_err(|e| format!("register: {e}"))?;
    handle
        .client
        .verify_registration()
        .await
        .map_err(|e| format!("verify: {e}"))?;
    let mut state = handle.state.write().await;
    state.registered_at = Some(registered_at);
    state.last_error = None;
    if let Err(e) = state.save(&handle.mosaic_root).await {
        tracing::warn!("relay state save (post-bringup): {e}");
    }
    Ok(())
}

fn now_secs_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Default poll interval if config doesn't set one.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use std::net::SocketAddr;
    use tesela_relay::{router, AppState as RelayAppState};
    use tesela_sync::crypto::keys::GroupKey;
    use tesela_sync::device::DeviceId;
    use tesela_sync::group::GroupId;
    use tesela_sync::{Hlc, LoroEngine, OpPayload, SyncEngine};

    async fn spawn_relay() -> (reqwest::Url, tempfile::TempDir, tokio::task::JoinHandle<()>) {
        let tmp = tempfile::tempdir().expect("tmp");
        let db = tmp.path().join("relay.sqlite");
        let state = RelayAppState::open(&db, 4_194_304, Some("admin".into()))
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
        (
            reqwest::Url::parse(&format!("http://{}", addr)).unwrap(),
            tmp,
            server,
        )
    }

    fn fresh_group() -> (GroupId, GroupKey) {
        let mut gid = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut gid);
        let mut gk = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut gk);
        (GroupId::from_bytes(gid), GroupKey::from_bytes(gk))
    }

    async fn engine_in(tmp: &tempfile::TempDir, device: DeviceId) -> LoroEngine {
        LoroEngine::with_dirs(
            device,
            Arc::new(Hlc::new(device)),
            tmp.path().join("loro"),
            Some(tmp.path().join("notes")),
        )
        .await
        .expect("loro engine")
    }

    fn handle_for(
        base_url: &reqwest::Url,
        group: GroupId,
        device: DeviceId,
        key: GroupKey,
        mosaic_root: std::path::PathBuf,
    ) -> RelayHandle {
        RelayHandle {
            url: base_url.to_string(),
            client: Arc::new(RelayClient::new(base_url.clone(), group, device, key)),
            state: Arc::new(RwLock::new(RelayState::default())),
            mosaic_root,
        }
    }

    /// The live `tick` deposits per-note snapshots → the relay compacts its op
    /// log → a fresh device restores PURELY from those snapshots via
    /// `bootstrap_from_snapshots`. Exercises the actual 1b-iii wiring (not the
    /// RelayClient methods in isolation).
    #[tokio::test]
    async fn tick_deposits_snapshots_then_fresh_device_bootstraps() {
        // Force a snapshot deposit on every tick.
        std::env::set_var("TESELA_RELAY_SNAPSHOT_INTERVAL_SECS", "0");

        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };

        const NID_A: [u8; 16] = [0x0a; 16];
        const NID_B: [u8; 16] = [0x0b; 16];

        // Device A authors two notes.
        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        for (nid, slug, body) in [
            (NID_A, "alpha", "- hello alpha\n"),
            (NID_B, "beta", "- hello beta\n"),
        ] {
            engine_a
                .record_local(OpPayload::NoteUpsert {
                    note_id: nid,
                    display_alias: Some(slug.into()),
                    title: slug.into(),
                    content: body.into(),
                    created_at_millis: 1,
                })
                .await
                .unwrap();
        }

        let handle_a = handle_for(
            &base_url,
            group,
            dev_a,
            key.clone(),
            a_tmp.path().to_path_buf(),
        );
        bring_up(&handle_a).await.expect("relay bring-up");

        // Tick 1 PUTs the ops; tick 2 polls its own echoes (advancing the
        // cursor) then deposits a snapshot batch covering them → relay compacts.
        tick(&engine_a, &ident, &handle_a).await.unwrap();
        tick(&engine_a, &ident, &handle_a).await.unwrap();

        // The relay compacted: a fresh probe sees a watermark + both snapshots,
        // and the raw op log is gone.
        let probe = RelayClient::new(
            base_url.clone(),
            group,
            DeviceId::from_bytes([0xcc; 16]),
            key.clone(),
        );
        let (comp_seq, snaps) = probe.fetch_snapshots().await.unwrap();
        assert!(
            comp_seq > 0,
            "tick deposited a snapshot batch + advanced the relay watermark"
        );
        assert_eq!(snaps.len(), 2, "both notes' snapshots are on the relay");
        assert!(
            probe.poll(0).await.unwrap().rows.is_empty(),
            "ops <= watermark compacted out from under since=0"
        );

        // Fresh device C restores PURELY from the snapshots (the raw ops are gone).
        let c_tmp = tempfile::tempdir().unwrap();
        let dev_c = DeviceId::from_bytes([0xc1; 16]);
        let engine_c = engine_in(&c_tmp, dev_c).await;
        let handle_c = handle_for(
            &base_url,
            group,
            dev_c,
            key.clone(),
            c_tmp.path().to_path_buf(),
        );
        let _ = handle_c.client.register_or_recover().await; // idempotent join
        bootstrap_from_snapshots(&engine_c, &handle_c).await;

        assert_eq!(
            engine_c.render_note(NID_A).await,
            engine_a.render_note(NID_A).await,
            "C restored note A byte-identically from the snapshot"
        );
        assert_eq!(
            engine_c.render_note(NID_B).await,
            engine_a.render_note(NID_B).await,
            "C restored note B byte-identically from the snapshot"
        );
        assert_eq!(
            handle_c.state.read().await.inbound_cursor,
            comp_seq,
            "bootstrap jumped C's cursor to the relay watermark"
        );
    }
}
