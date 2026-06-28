//! Desktop-side WAN relay sync.
//!
//! When the mosaic config carries `[sync.relay] url = "…"`, the
//! server brings up a [`RelayClient`] on startup, runs the
//! registration + joiner-verification handshake, and then ticks a
//! poll/produce loop alongside the existing per-peer LAN sync. This
//! module is the glue: cursor persistence, the per-tick function,
//! and the JSON status response the web settings page reads.

use std::collections::HashMap;
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
    /// Hex note ids whose inbound apply / snapshot import FAILED (or whose
    /// snapshot bootstrap partially failed) and that need a targeted
    /// snapshot catch-up. Retried every tick via `fetch_snapshots` →
    /// `import_authoritative_snapshot` (idempotent); removed on success.
    /// Persisted so a restart keeps retrying instead of forgetting the
    /// failure (audit A4).
    #[serde(default)]
    pub catchup_notes: Vec<String>,
    /// Per-seq apply-retry attempts for envelopes whose per-note apply
    /// failed. In-memory only — a restart restarts the budget, which is
    /// fine (the retry bound exists to unstick the cursor, not to be a
    /// durable counter).
    #[serde(skip)]
    pub apply_retries: HashMap<i64, u32>,
    /// Relay URL the cursors were earned against. Together with
    /// `group_id_hex` this scopes the persisted state to ONE
    /// (relay, group) identity: relay seqs are a per-relay, per-group
    /// namespace, so replaying a cursor against a different relay/group
    /// silently skips every op below it (audit A5). `None` = legacy state
    /// file written before identity scoping.
    #[serde(default)]
    pub relay_url: Option<String>,
    /// 32-char hex group id the cursors were earned against. See
    /// `relay_url`.
    #[serde(default)]
    pub group_id_hex: Option<String>,
}

/// How many ticks an envelope whose per-note apply failed is retried (the
/// cursor holds just before it) before we give up: queue the failed notes
/// for snapshot catch-up, log loudly, and move the cursor past so one
/// poisoned envelope can't stall every later stream forever (audit A4).
pub const MAX_APPLY_RETRIES: u32 = 5;

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

    /// Reconcile the persisted state with the CURRENT (relay_url, group_id)
    /// identity (audit A5). Relay seqs are a per-relay, per-group namespace
    /// that restarts at 1 on a fresh relay, so a cursor earned against a
    /// different identity must not be replayed — `poll(?since=stale_high)`
    /// would return empty forever while the snapshot-bootstrap guard
    /// (`compaction_seq > cursor`) also never fires: a silent, permanent
    /// inbound stall. This is the exact shape of the planned HA→Cloudflare
    /// relay migration and of re-pairing into a new group.
    ///
    /// - Identity matches (or was never recorded — legacy file): stamp the
    ///   current identity, keep the cursors. Returns `false`.
    /// - Identity differs: reset to a fresh state carrying only the new
    ///   identity, so bring-up re-registers and re-bootstraps from the new
    ///   relay's snapshots. Returns `true`.
    pub fn scope_to_identity(&mut self, relay_url: &str, group_id_hex: &str) -> bool {
        let matches = match (&self.relay_url, &self.group_id_hex) {
            (Some(u), Some(g)) => u == relay_url && g == group_id_hex,
            // Legacy state (pre-identity): adopt the current identity
            // without resetting — the common in-place upgrade keeps its
            // cursor; the NEXT relay/group switch is then detected.
            _ => true,
        };
        if matches {
            self.relay_url = Some(relay_url.to_string());
            self.group_id_hex = Some(group_id_hex.to_string());
            return false;
        }
        *self = RelayState {
            relay_url: Some(relay_url.to_string()),
            group_id_hex: Some(group_id_hex.to_string()),
            ..Default::default()
        };
        true
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
    /// The exact per-note delta bytes that applied this tick. The caller
    /// re-broadcasts these verbatim on `ws_delta_tx` so live web clients'
    /// Loro docs converge — the same "re-broadcast the applied bytes"
    /// approach the WS inbound handler uses. (A post-apply `export_doc_update`
    /// can't recover them: the engine's export cursor already consumed them.)
    pub applied_updates: Vec<tesela_sync::LoroDocUpdate>,
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
    let mut applied_updates: Vec<tesela_sync::LoroDocUpdate> = Vec::new();
    // Notes whose inbound apply FAILED or landed PENDING this tick — we do NOT
    // genuinely hold their content (the doc is empty/partial/vivified), so they
    // must be EXCLUDED from the heal-snapshot deposit below: depositing such a
    // snapshot as authoritative would let a peer's catch-up overwrite real
    // content (clobber). Guarded by `tick_holds_cursor…`.
    let mut not_genuine_this_tick: Vec<[u8; 16]> = Vec::new();

    // ─── Inbound ─────────────────────────────────────────────────────
    match handle.client.poll(state.inbound_cursor).await {
        Ok(batch) => {
            let mut max_seq = state.inbound_cursor;
            // Earliest seq in this batch whose apply FAILED and is still
            // within its retry budget — the cursor is capped just before it
            // below, so the envelope is re-polled + retried next tick
            // instead of being silently acked past (audit A4).
            let mut blocked_at: Option<i64> = None;
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
                        let report = engine.apply_relay_updates(&pairs).await;
                        applied_total += report.applied_count() as u32;
                        // Record notes we do NOT genuinely hold this tick (failed
                        // or pending apply) so the heal-deposit skips them.
                        for doc in &report.pending {
                            not_genuine_this_tick.push(*doc);
                        }
                        for (doc, _) in &report.failed {
                            not_genuine_this_tick.push(*doc);
                        }
                        // Loro apply is idempotent, so the caller emitting a
                        // WsEvent for a no-op merge is harmless; record each
                        // cleanly-applied doc (deduped) so the live-WS
                        // fan-out can notify web + re-broadcast the delta.
                        for doc in &report.applied {
                            if !applied_note_ids.contains(doc) {
                                applied_note_ids.push(*doc);
                            }
                        }
                        // Carry the exact bytes that applied so the caller can
                        // re-broadcast them on ws_delta_tx (live web clients'
                        // Loro docs converge without a hard refresh).
                        for (doc, bytes) in &pairs {
                            if report.applied.contains(doc) {
                                applied_updates.push(tesela_sync::LoroDocUpdate {
                                    doc: *doc,
                                    update_bytes: bytes.clone(),
                                });
                            }
                        }
                        // A PENDING import (causal gap — the base was
                        // compacted away or never delivered) advances the
                        // cursor but queues the note for the targeted
                        // snapshot catch-up below: the buffered bytes live
                        // in-memory only and the note is frozen until its
                        // base arrives (audit A4).
                        for doc in &report.pending {
                            let hex_id = hex::encode(doc);
                            if !state.catchup_notes.contains(&hex_id) {
                                state.catchup_notes.push(hex_id);
                            }
                        }
                        if report.failed.is_empty() {
                            state.apply_retries.remove(&seq);
                            if seq > max_seq {
                                max_seq = seq;
                            }
                        } else {
                            // Bounded retry: hold the cursor BEFORE this
                            // envelope for up to MAX_APPLY_RETRIES ticks (a
                            // transient engine failure heals on a retry),
                            // then give up — queue the failed notes for
                            // snapshot catch-up and move on, so one
                            // poisoned note can't stall every later stream
                            // forever.
                            let attempts = {
                                let a = state.apply_retries.entry(seq).or_insert(0);
                                *a += 1;
                                *a
                            };
                            if attempts >= MAX_APPLY_RETRIES {
                                tracing::error!(
                                    "relay: giving up on envelope seq={} from={} after {} failed apply attempts; \
                                     queueing {} note(s) for snapshot catch-up: {:?}",
                                    seq,
                                    hex::encode(peer.as_bytes()),
                                    MAX_APPLY_RETRIES,
                                    report.failed.len(),
                                    report
                                        .failed
                                        .iter()
                                        .map(|(id, e)| format!("{}: {e}", hex::encode(id)))
                                        .collect::<Vec<_>>()
                                );
                                for (doc, _) in &report.failed {
                                    let hex_id = hex::encode(doc);
                                    if !state.catchup_notes.contains(&hex_id) {
                                        state.catchup_notes.push(hex_id);
                                    }
                                }
                                state.apply_retries.remove(&seq);
                                if seq > max_seq {
                                    max_seq = seq;
                                }
                            } else {
                                tracing::warn!(
                                    "relay: apply failed for {}/{} note(s) in envelope seq={} \
                                     (attempt {}/{}); holding the cursor for retry",
                                    report.failed.len(),
                                    pairs.len(),
                                    seq,
                                    attempts,
                                    MAX_APPLY_RETRIES
                                );
                                if blocked_at.is_none_or(|b| seq < b) {
                                    blocked_at = Some(seq);
                                }
                            }
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
            // Cap the cursor just before the earliest still-retrying
            // failure so the failed envelope is re-polled next tick. Later
            // envelopes were still applied above (idempotent) — they just
            // get re-applied harmlessly until the failure resolves.
            if let Some(b) = blocked_at {
                max_seq = max_seq.min(b - 1);
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

    // ─── Targeted snapshot catch-up ──────────────────────────────────
    // Heal notes whose inbound apply failed or landed PENDING (and notes a
    // partially-failed bootstrap queued): authoritatively re-import each
    // from the relay's deposited snapshot (idempotent). Healed notes leave
    // the queue and join the fan-out set; notes without a deposited
    // snapshot stay queued for a later tick.
    if !state.catchup_notes.is_empty() {
        let healed = catchup_from_snapshots(engine, &handle.client, &state.catchup_notes).await;
        if !healed.is_empty() {
            state.catchup_notes.retain(|h| !healed.contains(h));
            for h in &healed {
                if let Some(id) = parse_hex_note_id(h) {
                    if !applied_note_ids.contains(&id) {
                        applied_note_ids.push(id);
                    }
                }
            }
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
    let batches =
        tesela_sync::pack_loro_relay_batches(updates, tesela_sync::MAX_RELAY_PLAINTEXT_BYTES);
    // Note ids successfully broadcast this tick — their heal-snapshots are
    // deposited right after the loop (past-day convergence, 2026-06-28).
    let mut broadcast_note_ids: Vec<[u8; 16]> = Vec::new();
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
                for (nid, _vv) in &committed {
                    broadcast_note_ids.push(*nid);
                }
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

    // ─── Heal-snapshot deposit (past-day convergence, 2026-06-28) ─────
    // For every note we just broadcast AND genuinely hold, deposit its current
    // full snapshot to the relay as an INERT deposit (covers_seq = 0 → available
    // for a peer's catch-up, never advances the GC watermark). The gated
    // compaction below only deposits on a 5-min cadence, with inbound_cursor > 0,
    // and never while a catch-up is queued — so a cold PAST-DAY note's fresh
    // snapshot would otherwise reach the relay minutes late or not at all,
    // leaving a DIVERGED peer stuck ("editing on desktop doesn't kick it").
    //
    // CLOBBER GUARD: skip notes we don't genuinely hold this tick — a note whose
    // apply FAILED/PENDED is auto-vivified as an empty/partial doc; depositing
    // that as authoritative would let a peer heal FROM it and lose real content.
    // Excluding `not_genuine_this_tick` + `catchup_notes` keeps only real
    // local-authored / cleanly-applied notes. Being independent of the gated
    // compaction, healthy notes also heal even when another note is stuck (the
    // deadlock). Additive — never touches the merge/apply path; best-effort.
    if !broadcast_note_ids.is_empty() {
        let mut heal: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for id in &broadcast_note_ids {
            if not_genuine_this_tick.contains(id) {
                continue;
            }
            if state.catchup_notes.contains(&hex::encode(id)) {
                continue;
            }
            if let Some(bytes) = engine.export_doc_update(*id, None).await {
                heal.push((id.to_vec(), bytes));
            }
        }
        if !heal.is_empty() {
            if let Err(e) = handle
                .client
                .put_snapshots_chunked(0, heal, deposit_chunk_budget_bytes())
                .await
            {
                tracing::warn!("relay: heal-snapshot deposit (broadcast notes): {e}");
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
        .is_none_or(|t| now - t >= snapshot_interval_secs());
    if due && state.inbound_cursor > 0 && !state.catchup_notes.is_empty() {
        // Don't deposit (→ relay GC) while notes we failed to integrate are
        // still queued: covers_seq = our cursor, and the deposited snapshots
        // would LACK those notes' content — the GC would destroy their ops
        // group-wide. Conservative: the op log grows until the catch-up
        // heals the queue (loud in the log either way).
        tracing::warn!(
            "relay: snapshot deposit SKIPPED — {} note(s) awaiting catch-up: {:?}",
            state.catchup_notes.len(),
            state.catchup_notes
        );
    }
    if due && state.inbound_cursor > 0 && state.catchup_notes.is_empty() {
        match deposit_snapshots(engine, &handle.client, state.inbound_cursor).await {
            Ok(report) => {
                // Stamp the cadence even on a partial (skipped-notes)
                // deposit: retrying every tick can't shrink an oversize
                // snapshot, it would just re-upload the mosaic in a loop.
                state.last_snapshot_at = Some(now);
                if report.complete() {
                    if report.gc > 0 {
                        tracing::debug!(
                            "relay snapshot deposit: covers seq {} in {} chunk(s), relay GC'd {} ops",
                            state.inbound_cursor,
                            report.chunks_sent,
                            report.gc
                        );
                    }
                } else {
                    let skipped: Vec<String> =
                        report.skipped_streams.iter().map(hex::encode).collect();
                    let msg = format!(
                        "relay snapshot deposit: {} note snapshot(s) exceed the relay body cap \
                         and were SKIPPED ({:?}) — compaction watermark NOT advanced",
                        skipped.len(),
                        skipped
                    );
                    tracing::warn!("{msg}");
                    state.last_error = Some(msg);
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
        applied_updates,
    })
}

/// Deposit a full per-note snapshot set covering relay-seq `covers_seq`
/// (every tracked note's full Loro snapshot, keyed by note_id = stream_id).
/// Idempotent.
///
/// Chunked under [`deposit_chunk_budget_bytes`] so a whole-mosaic deposit
/// (hundreds of notes) can't 413 against the relay's request-body cap as
/// one giant PUT. Only the FINAL chunk carries the real `covers_seq` (=
/// relay GC + watermark advance); intermediate chunks deposit with
/// `covers_seq = 0`, which both relay impls treat as inert, so a crash
/// mid-deposit leaves the op log intact and the next deposit heals it.
async fn deposit_snapshots(
    engine: &dyn tesela_sync::SyncEngine,
    client: &RelayClient,
    covers_seq: i64,
) -> Result<tesela_sync::transport::relay::SnapshotDepositReport, String> {
    let note_ids = engine.tracked_note_ids().await;
    let mut snapshots: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(note_ids.len());
    for id in note_ids {
        // `export_doc_update(id, None)` = the note's full compact snapshot.
        if let Some(bytes) = engine.export_doc_update(id, None).await {
            snapshots.push((id.to_vec(), bytes));
        }
    }
    if snapshots.is_empty() {
        return Ok(Default::default());
    }
    client
        .put_snapshots_chunked(covers_seq, snapshots, deposit_chunk_budget_bytes())
        .await
        .map_err(|e| e.to_string())
}

/// Bootstrap a fresh / long-offline device from the relay's compacted
/// snapshots: if the relay's compaction watermark is ahead of our inbound
/// cursor, the ops we'd need are already GC'd, so we import the per-note
/// snapshots and jump the cursor to the watermark. The subsequent `?since=`
/// poll then collects only the un-compacted tail. Idempotent (Loro merge);
/// a no-op when we're already caught up past the watermark.
///
/// The cursor jumps to the watermark ONLY when every per-note import
/// succeeded: the ops a snapshot covers are already GC'd on the relay, so
/// jumping past a failed import would permanently skip that note (audit
/// A4). On partial failure the failed note ids are queued in
/// [`RelayState::catchup_notes`] and healed by the per-tick targeted
/// snapshot catch-up (`fetch_snapshots` is idempotent); a later bootstrap
/// retries too, since the cursor stays below the watermark.
pub async fn bootstrap_from_snapshots(engine: &dyn tesela_sync::SyncEngine, handle: &RelayHandle) {
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
    let mut failed: Vec<String> = Vec::new();
    for (stream_id, _snapshot_seq, plaintext) in snaps {
        let Ok(note_id) = <[u8; 16]>::try_from(stream_id.as_slice()) else {
            continue; // v1 stream_id is the 16-byte note_id; skip anything else
        };
        if let Err(e) = engine.import_doc_update(note_id, &plaintext).await {
            tracing::warn!(
                "relay snapshot bootstrap import {}: {e}",
                hex::encode(note_id)
            );
            failed.push(hex::encode(note_id));
            continue;
        }
        imported += 1;
    }
    if failed.is_empty() {
        state.inbound_cursor = compaction_seq;
        tracing::info!(
            "relay snapshot bootstrap: imported {} note(s), cursor → {}",
            imported,
            compaction_seq
        );
    } else {
        tracing::error!(
            "relay snapshot bootstrap: {}/{} imports FAILED — cursor held at {} \
             (not jumped to watermark {}); failed notes queued for snapshot catch-up: {:?}",
            failed.len(),
            failed.len() + imported as usize,
            state.inbound_cursor,
            compaction_seq,
            failed
        );
        for hex_id in failed {
            if !state.catchup_notes.contains(&hex_id) {
                state.catchup_notes.push(hex_id);
            }
        }
    }
    if let Err(e) = state.save(&handle.mosaic_root).await {
        tracing::warn!("relay state save (post-bootstrap): {e}");
    }
}

/// Targeted snapshot catch-up for notes whose inbound apply failed or was
/// left PENDING by Loro: fetch the relay's deposited snapshots and
/// authoritatively re-import the ones matching `targets_hex` (32-char hex
/// note ids). Returns the hex ids that healed; targets with no deposited
/// snapshot (or whose import failed again) are left for the caller to
/// retry later. Idempotent — `fetch_snapshots` + the authoritative import
/// are both safe to repeat.
pub(crate) async fn catchup_from_snapshots(
    engine: &dyn tesela_sync::SyncEngine,
    client: &RelayClient,
    targets_hex: &[String],
) -> Vec<String> {
    let (_watermark, snaps) = match client.fetch_snapshots().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("relay snapshot catch-up fetch: {e}");
            return Vec::new();
        }
    };
    let mut healed = Vec::new();
    for (stream_id, _snapshot_seq, plaintext) in snaps {
        let Ok(note_id) = <[u8; 16]>::try_from(stream_id.as_slice()) else {
            continue;
        };
        let hex_id = hex::encode(note_id);
        if !targets_hex.contains(&hex_id) {
            continue;
        }
        match engine
            .import_authoritative_snapshot(note_id, &plaintext)
            .await
        {
            Ok(()) => {
                tracing::info!("relay snapshot catch-up healed note {hex_id}");
                healed.push(hex_id);
            }
            Err(e) => {
                tracing::warn!("relay snapshot catch-up import {hex_id}: {e}");
            }
        }
    }
    healed
}

/// Parse a 32-char hex note id back to its 16 raw bytes.
pub(crate) fn parse_hex_note_id(hex_id: &str) -> Option<[u8; 16]> {
    let bytes = hex::decode(hex_id).ok()?;
    <[u8; 16]>::try_from(bytes.as_slice()).ok()
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

/// Per-request body budget for chunked snapshot deposits. Env-tunable
/// (`TESELA_RELAY_DEPOSIT_CHUNK_BYTES`); defaults to 4 MiB of serialized
/// payload — comfortable headroom under the HA relay's 16 MiB cap while
/// the 413-adaptive halving in `put_snapshots_chunked` degrades to fit
/// tighter caps (e.g. the CF Worker's 1 MiB default) automatically.
fn deposit_chunk_budget_bytes() -> usize {
    std::env::var("TESELA_RELAY_DEPOSIT_CHUNK_BYTES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4 * 1024 * 1024)
}

/// One-time bring-up: register on the relay (idempotent / recovery
/// path), verify the stored intent, persist `registered_at`. Returns
/// `Ok` even on failure — the caller wires the error into RelayState
/// + lets the daemon retry on its tick.
pub async fn bring_up(handle: &RelayHandle) -> Result<(), String> {
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

    /// Past-day convergence (Taylor 2026-06-28): editing a note must promptly
    /// deposit its heal-snapshot to the relay (not wait out the 5-min compaction
    /// cadence), so a peer whose copy diverged can catch it up. A genuinely-held
    /// note (locally authored here) IS deposited on broadcast. The clobber guard
    /// — a FAILED/pending (vivified) note must NOT be deposited — is covered by
    /// `tick_holds_cursor_at_failed_apply_then_gives_up_after_bound` (the poison
    /// note stays queued, i.e. was never spuriously healed from a bad snapshot).
    #[tokio::test]
    async fn broadcast_deposits_heal_snapshot_for_genuine_note() {
        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };
        const NID: [u8; 16] = [0x0c; 16];
        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa2; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        engine_a
            .record_local(OpPayload::NoteUpsert {
                note_id: NID,
                display_alias: Some("gamma".into()),
                title: "Gamma".into(),
                content: "- hello gamma\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let handle_a = handle_for(&base_url, group, dev_a, key.clone(), a_tmp.path().to_path_buf());
        // GET /snapshots is MAC-gated + needs the group registered (pairing does
        // this in production).
        handle_a.client.register(1).await.unwrap();

        // One tick broadcasts the local edit. inbound_cursor stays 0, so the
        // gated compaction is SKIPPED — the heal-deposit must still land it.
        tick(&engine_a, &ident, &handle_a).await.unwrap();

        let (_seq, snaps) = handle_a.client.fetch_snapshots().await.unwrap();
        let ids: Vec<Vec<u8>> = snaps.iter().map(|(id, _, _)| id.clone()).collect();
        assert!(
            ids.contains(&NID.to_vec()),
            "a genuine local edit must deposit its heal-snapshot on broadcast \
             (got {} snapshot(s))",
            snaps.len()
        );
    }

    /// A5: persisted cursors are only valid against the (relay_url, group_id)
    /// identity they were earned from. A mismatch (relay migration, re-pair
    /// into a different group) must reset the state so bring-up re-bootstraps
    /// from the new relay's snapshots instead of replaying a stale high
    /// cursor against a fresh seq namespace (silent permanent inbound stall).
    #[test]
    fn relay_state_scope_to_identity() {
        // Legacy state file (no identity recorded): adopt the current
        // identity without resetting — the in-place upgrade keeps the cursor.
        let mut s = RelayState {
            inbound_cursor: 42,
            registered_at: Some(1),
            ..Default::default()
        };
        assert!(!s.scope_to_identity("https://relay-a/", "aa11"));
        assert_eq!(s.inbound_cursor, 42, "legacy stamp keeps the cursor");
        assert_eq!(s.relay_url.as_deref(), Some("https://relay-a/"));
        assert_eq!(s.group_id_hex.as_deref(), Some("aa11"));

        // Same identity: no reset.
        assert!(!s.scope_to_identity("https://relay-a/", "aa11"));
        assert_eq!(s.inbound_cursor, 42);

        // Different relay URL: full reset (cursor, catch-up queue,
        // registration timestamp) + re-stamp.
        s.catchup_notes = vec!["ff".into()];
        assert!(s.scope_to_identity("https://relay-b/", "aa11"));
        assert_eq!(s.inbound_cursor, 0, "stale cursor reset for the new relay");
        assert!(s.catchup_notes.is_empty());
        assert_eq!(s.registered_at, None);
        assert_eq!(s.relay_url.as_deref(), Some("https://relay-b/"));
        assert_eq!(s.group_id_hex.as_deref(), Some("aa11"));

        // Different group on the same relay: reset too (per-group seq
        // namespaces restart at 1).
        let mut s2 = RelayState {
            inbound_cursor: 7,
            relay_url: Some("https://relay-b/".into()),
            group_id_hex: Some("aa11".into()),
            ..Default::default()
        };
        assert!(s2.scope_to_identity("https://relay-b/", "bb22"));
        assert_eq!(s2.inbound_cursor, 0);
        assert_eq!(s2.group_id_hex.as_deref(), Some("bb22"));
    }

    /// Seal + PUT one TLR2 relay envelope carrying the given per-note update
    /// bytes, as a peer device would. Returns the relay-assigned seq.
    async fn put_loro_envelope(
        client: &RelayClient,
        from: DeviceId,
        group: GroupId,
        updates: &[([u8; 16], Vec<u8>)],
    ) -> i64 {
        let payload: Vec<tesela_sync::LoroDocUpdate> = updates
            .iter()
            .map(|(doc, bytes)| tesela_sync::LoroDocUpdate {
                doc: *doc,
                update_bytes: bytes.clone(),
            })
            .collect();
        let ciphertext = tesela_sync::encode_loro_relay_payload(&payload).unwrap();
        let env = SyncEnvelope {
            from_device: from,
            to_group: group,
            nonce: [0u8; 24],
            ciphertext,
        };
        let (seq, _ts) = client.put_envelope(env).await.expect("put envelope");
        seq
    }

    /// A4: an envelope whose per-note apply FAILS must not be acked past.
    /// The cursor holds at the seq before the failure (bounded retry), good
    /// envelopes beyond it still apply, and after the retry budget the
    /// poisoned note is queued for snapshot catch-up and the cursor moves on.
    #[tokio::test]
    async fn tick_holds_cursor_at_failed_apply_then_gives_up_after_bound() {
        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };

        const NID_POISON: [u8; 16] = [0x0f; 16];
        const NID_GOOD: [u8; 16] = [0x1a; 16];

        // Sender B: one poisoned update (valid TLR2 wire, garbage Loro
        // bytes → deterministic apply failure) then one good full snapshot.
        let b_tmp = tempfile::tempdir().unwrap();
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let engine_b = engine_in(&b_tmp, dev_b).await;
        engine_b
            .record_local(OpPayload::NoteUpsert {
                note_id: NID_GOOD,
                display_alias: Some("good".into()),
                title: "good".into(),
                content: "- hello good\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let good_snap = engine_b.export_doc_update(NID_GOOD, None).await.unwrap();
        let client_b = RelayClient::new(base_url.clone(), group, dev_b, key.clone());
        client_b.register_or_recover().await.expect("b register");
        let poison_seq = put_loro_envelope(
            &client_b,
            dev_b,
            group,
            &[(NID_POISON, b"definitely not a loro update".to_vec())],
        )
        .await;
        let good_seq = put_loro_envelope(&client_b, dev_b, group, &[(NID_GOOD, good_snap)]).await;
        assert!(good_seq > poison_seq);

        // Consumer A.
        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        let handle_a = handle_for(
            &base_url,
            group,
            dev_a,
            key.clone(),
            a_tmp.path().to_path_buf(),
        );
        bring_up(&handle_a).await.expect("relay bring-up");

        tick(&engine_a, &ident, &handle_a).await.unwrap();

        // The good envelope (later seq) still applied — one poisoned note
        // doesn't stall other streams …
        let rendered = engine_a.render_note(NID_GOOD).await.unwrap_or_default();
        assert!(
            rendered.contains("hello good"),
            "good envelope applies despite the poisoned one: {rendered:?}"
        );
        // … but the cursor must NOT advance past the failed envelope.
        assert_eq!(
            handle_a.state.read().await.inbound_cursor,
            poison_seq - 1,
            "cursor holds before the failed apply (bounded retry)"
        );

        // Exhaust the retry budget: the failure is deterministic, so after
        // MAX_APPLY_RETRIES ticks the note is queued for snapshot catch-up
        // and the cursor moves past the poisoned envelope.
        for _ in 1..MAX_APPLY_RETRIES {
            tick(&engine_a, &ident, &handle_a).await.unwrap();
        }
        let state = handle_a.state.read().await;
        // `>=` not `==`: A's own outbound re-broadcast of the applied good
        // note lands on the relay too, and A advances over its own echo.
        assert!(
            state.inbound_cursor >= good_seq,
            "after the retry budget the cursor moves past the poisoned envelope \
             (cursor {}, poison seq {poison_seq})",
            state.inbound_cursor
        );
        assert!(
            state.catchup_notes.contains(&hex::encode(NID_POISON)),
            "the poisoned note is queued for snapshot catch-up: {:?}",
            state.catchup_notes
        );
    }

    /// A4: a delta that Loro leaves PENDING (causal gap — its base was
    /// compacted away / never delivered) must trigger a targeted snapshot
    /// catch-up in the same tick, not silently freeze the note.
    #[tokio::test]
    async fn tick_pending_delta_triggers_snapshot_catchup() {
        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };

        const NID: [u8; 16] = [0x2b; 16];

        // Sender B: base state, then a tail edit exported SINCE the base —
        // the tail alone cannot integrate on a device that lacks the base.
        let b_tmp = tempfile::tempdir().unwrap();
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let engine_b = engine_in(&b_tmp, dev_b).await;
        engine_b
            .record_local(OpPayload::NoteUpsert {
                note_id: NID,
                display_alias: Some("gap".into()),
                title: "gap".into(),
                content: "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let base_snap = engine_b.export_doc_update(NID, None).await.unwrap();
        let pre_vv = engine_b.doc_version(NID).await.unwrap();
        engine_b
            .record_local(OpPayload::BlockUpsert {
                block_id: [0x77; 16],
                note_id: NID,
                parent_block_id: None,
                order_key: "z".into(),
                indent_level: 0,
                text: "TAIL EDIT".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let tail = engine_b
            .export_doc_update(NID, Some(&pre_vv))
            .await
            .unwrap();

        // The base lives ONLY as a relay snapshot (as after compaction);
        // the live op log carries only the tail delta.
        let client_b = RelayClient::new(base_url.clone(), group, dev_b, key.clone());
        client_b.register_or_recover().await.expect("b register");
        client_b
            .put_snapshots(0, vec![(NID.to_vec(), base_snap)])
            .await
            .expect("deposit base snapshot");
        let tail_seq = put_loro_envelope(&client_b, dev_b, group, &[(NID, tail)]).await;

        // Consumer A: one tick must apply the tail (pending), notice the
        // causal gap, and heal it from the relay snapshot — converging.
        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        let handle_a = handle_for(
            &base_url,
            group,
            dev_a,
            key.clone(),
            a_tmp.path().to_path_buf(),
        );
        bring_up(&handle_a).await.expect("relay bring-up");
        let outcome = tick(&engine_a, &ident, &handle_a).await.unwrap();

        let rendered = engine_a.render_note(NID).await.unwrap_or_default();
        assert!(
            rendered.contains("TAIL EDIT"),
            "pending tail delta healed via snapshot catch-up in the same tick: {rendered:?}"
        );
        assert!(
            rendered.contains("alpha"),
            "base content restored from the snapshot: {rendered:?}"
        );
        assert_eq!(
            handle_a.state.read().await.inbound_cursor,
            tail_seq,
            "a pending (not failed) envelope still advances the cursor"
        );
        assert!(
            handle_a.state.read().await.catchup_notes.is_empty(),
            "healed note removed from the catch-up queue"
        );
        assert!(
            outcome.applied_note_ids.contains(&NID),
            "healed note included in the fan-out set"
        );
    }

    /// A4: snapshot bootstrap must NOT jump the cursor to the watermark when
    /// per-note imports failed — the failed notes are queued and healed on a
    /// later tick (fetch_snapshots is idempotent), and a later bootstrap can
    /// still advance the cursor once everything imports.
    #[tokio::test]
    async fn bootstrap_partial_failure_keeps_cursor_and_heals_via_tick() {
        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };

        const NID_OK: [u8; 16] = [0x3c; 16];
        const NID_BAD: [u8; 16] = [0x4d; 16];

        let b_tmp = tempfile::tempdir().unwrap();
        let dev_b = DeviceId::from_bytes([0xb2; 16]);
        let engine_b = engine_in(&b_tmp, dev_b).await;
        for (nid, slug) in [(NID_OK, "ok-note"), (NID_BAD, "bad-note")] {
            engine_b
                .record_local(OpPayload::NoteUpsert {
                    note_id: nid,
                    display_alias: Some(slug.into()),
                    title: slug.into(),
                    content: format!("- body of {slug}\n"),
                    created_at_millis: 1,
                })
                .await
                .unwrap();
        }
        let snap_ok = engine_b.export_doc_update(NID_OK, None).await.unwrap();
        let snap_bad_valid = engine_b.export_doc_update(NID_BAD, None).await.unwrap();

        let client_b = RelayClient::new(base_url.clone(), group, dev_b, key.clone());
        client_b.register_or_recover().await.expect("b register");
        // Deposit covering seq 7: one valid snapshot + one corrupt one.
        client_b
            .put_snapshots(
                7,
                vec![
                    (NID_OK.to_vec(), snap_ok),
                    (NID_BAD.to_vec(), b"corrupt snapshot bytes".to_vec()),
                ],
            )
            .await
            .expect("deposit snapshots");

        // Fresh consumer A bootstraps: the valid note imports, the corrupt
        // one fails — the cursor must NOT jump to the watermark.
        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        let handle_a = handle_for(
            &base_url,
            group,
            dev_a,
            key.clone(),
            a_tmp.path().to_path_buf(),
        );
        bring_up(&handle_a).await.expect("relay bring-up");
        bootstrap_from_snapshots(&engine_a, &handle_a).await;

        let rendered_ok = engine_a.render_note(NID_OK).await.unwrap_or_default();
        assert!(
            rendered_ok.contains("body of ok-note"),
            "valid snapshot imported despite the corrupt sibling: {rendered_ok:?}"
        );
        {
            let state = handle_a.state.read().await;
            assert_eq!(
                state.inbound_cursor, 0,
                "cursor must NOT jump to the watermark past a failed import"
            );
            assert!(
                state.catchup_notes.contains(&hex::encode(NID_BAD)),
                "failed note queued for catch-up: {:?}",
                state.catchup_notes
            );
        }

        // The depositor replaces the corrupt snapshot with a valid one; the
        // next tick's targeted catch-up heals the note.
        client_b
            .put_snapshots(7, vec![(NID_BAD.to_vec(), snap_bad_valid)])
            .await
            .expect("re-deposit valid snapshot");
        tick(&engine_a, &ident, &handle_a).await.unwrap();

        let rendered_bad = engine_a.render_note(NID_BAD).await.unwrap_or_default();
        assert!(
            rendered_bad.contains("body of bad-note"),
            "failed note healed by the per-tick snapshot catch-up: {rendered_bad:?}"
        );
        assert!(
            handle_a.state.read().await.catchup_notes.is_empty(),
            "healed note removed from the catch-up queue"
        );

        // And a re-bootstrap is NOT permanently skipped: with every import
        // now succeeding, the cursor advances to the watermark.
        bootstrap_from_snapshots(&engine_a, &handle_a).await;
        assert_eq!(
            handle_a.state.read().await.inbound_cursor,
            7,
            "bootstrap advances the cursor once ALL imports succeed"
        );
    }
}
