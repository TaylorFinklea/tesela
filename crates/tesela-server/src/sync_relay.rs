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
    /// For each hex note id currently in `catchup_notes`, the relay-seq at
    /// (or just below) which it became undeposited — used by the periodic
    /// gated-compaction deposit to bound `covers_seq` STRICTLY below the
    /// earliest such seq, so the relay's group-wide
    /// `DELETE relay_ops WHERE seq <= covers_seq` GC can never claim
    /// coverage of ops belonging to a note we don't yet hold genuinely
    /// (tesela-sclr.4). Two flavors of entry:
    ///   - Apply-failure / pending (causal-gap) notes: the seq of the
    ///     envelope that triggered it — its raw op may be the ONLY path to
    ///     recover the note if no peer has independently deposited it yet,
    ///     so it must be protected from GC.
    ///   - Snapshot-bootstrap partial failures: `i64::MAX` (no bound
    ///     needed) — bootstrap only queues a note here when the relay's
    ///     compaction watermark is ALREADY ahead of our cursor, i.e. its
    ///     raw ops are already gone; there is nothing left to protect, and
    ///     bounding covers_seq for it would only block GC pointlessly.
    /// A catch-up note with NO entry here (state predates this field, or a
    /// hand-edited state file) defaults to the maximally conservative
    /// bound of seq 0 — i.e. fully blocks compaction, same as before this
    /// feature, rather than risk destroying an unprotected note's ops.
    /// Persisted alongside `catchup_notes` so a restart doesn't forget the
    /// bound and unsafely widen it.
    #[serde(default)]
    pub catchup_since_seq: HashMap<String, i64>,
    /// Per-seq apply-retry attempts for envelopes whose per-note apply
    /// failed. In-memory only — a restart restarts the budget, which is
    /// fine (the retry bound exists to unstick the cursor, not to be a
    /// durable counter).
    #[serde(skip)]
    pub apply_retries: HashMap<i64, u32>,
    /// Per-note hash of the LAST snapshot we deposited to the relay —
    /// shared by BOTH the broadcast heal-deposit and the periodic gated-
    /// compaction deposit — so an identical re-export (same
    /// `export_doc_update` bytes) skips a redundant upload from EITHER
    /// path. Throttles ONLY identical churn — any new/changed content
    /// hashes differently and ALWAYS deposits immediately (never re-strands
    /// a diverged peer, never lets a stale periodic pass skip a real
    /// change). Per-content, NOT per-cadence or per-watermark: the relay's
    /// compaction watermark (`relay_group_meta.compaction_seq`) is a
    /// SEPARATE group-scalar decoupled from each note's `relay_snapshots`
    /// row, so an already-deposited, still-byte-identical row stays valid
    /// no matter how far the watermark later advances — watermark movement
    /// alone is never a reason to re-deposit an unchanged note (see the
    /// hash-skip in `deposit_snapshots` for the checkpoint-only path that
    /// still advances the watermark with zero re-uploads). Gating the heal
    /// on the compaction cadence / catch-up / inbound_cursor caused the
    /// past-day-stuck bug. In-memory only — a restart re-deposits once per
    /// note, which is harmless (idempotent upsert).
    #[serde(skip)]
    pub deposit_hashes: HashMap<[u8; 16], u64>,
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
    // Set when this poll's `X-Tesela-Compaction-Seq` header is ahead of our
    // inbound cursor: the ops we still need were GC'd off the op log, so we
    // must bootstrap from the relay's deposited snapshots instead of polling
    // (a silent permanent stall otherwise). The actual bootstrap runs AFTER
    // the inbound write-lock is dropped — `bootstrap_from_snapshots`
    // re-acquires `handle.state` (deadlock guard).
    let mut needs_bootstrap = false;

    // ─── Inbound ─────────────────────────────────────────────────────
    match handle.client.poll(state.inbound_cursor).await {
        Ok(batch) => {
            let batch_compaction = batch.compaction_seq;
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
                            // This envelope's raw op may be the note's ONLY
                            // recovery path — bound future compaction below it
                            // (tesela-sclr.4).
                            state.catchup_since_seq.entry(hex_id.clone()).or_insert(seq);
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
                                    // Same rationale as the pending case above —
                                    // this envelope's raw op is the recovery
                                    // path of last resort (tesela-sclr.4).
                                    state.catchup_since_seq.entry(hex_id.clone()).or_insert(seq);
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
            // If the relay's compaction watermark is ahead of our (now-capped)
            // inbound cursor, the ops we still need have been GC'd — flag a
            // snapshot bootstrap. Runs below, after this write lock is dropped.
            if let Some(cs) = batch_compaction {
                if cs > state.inbound_cursor {
                    needs_bootstrap = true;
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

    // ─── Snapshot bootstrap when behind the compaction watermark ─────
    // DEADLOCK GUARD: `bootstrap_from_snapshots` re-acquires `handle.state`,
    // so the inbound write lock MUST be released first. Re-acquire afterward
    // for the catch-up + outbound phases. The bootstrap keeps the convergence
    // clobber guards (non-destructive import_doc_update merge; ALL-OR-NOTHING
    // cursor jump only when every per-note import succeeds, else the failed
    // notes are queued for catch-up and the cursor holds; touches ONLY the
    // inbound cursor, never the outbound broadcast cursors).
    drop(state);
    if needs_bootstrap {
        bootstrap_from_snapshots(engine, handle).await;
    }
    let mut state = handle.state.write().await;

    // ─── Causal-gap ledger auto-issue (tesela-c7s item 2) ────────────
    // The engine records EVERY note whose inbound update Loro left pending
    // (causal gap) in a durable ledger. Drain the notes that have stayed
    // pending past one full apply pass into the catch-up queue: a same-session
    // missing-base delta is given one pass to self-heal, and anything still
    // stuck is escalated to an authoritative-snapshot catch-up. Additive to
    // the immediate `report.pending` queueing above — it also covers a note
    // still stuck across ticks / restart (the ledger is persisted), so the
    // gap can never be forgotten as a bare log line.
    let ledger_catchup = engine.notes_needing_snapshot_catchup().await;
    if !ledger_catchup.is_empty() {
        // These heal via the relay's authoritative snapshot, not their raw
        // ops, so protecting the op log from GC below them isn't needed — bound
        // at the current inbound cursor (never 0, which would freeze group
        // compaction) so an already-`report.pending`-queued note keeps its
        // tighter seq (`or_insert` won't overwrite).
        let floor = state.inbound_cursor.max(1);
        for id in ledger_catchup {
            let hex_id = hex::encode(id);
            state.catchup_since_seq.entry(hex_id.clone()).or_insert(floor);
            if !state.catchup_notes.contains(&hex_id) {
                state.catchup_notes.push(hex_id);
            }
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
                state.catchup_since_seq.remove(h);
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
    let strand_alarms_before = engine.outbound_strand_alarm_count().await;
    let updates = engine.produce_relay_updates().await;
    // Surface the deposit-strand class at tick granularity (tesela-c7s item
    // 3): if producing this tick's broadcast had to fall back from an
    // incremental delta to a full snapshot for any dirty note (stale-ahead /
    // undecodable cursor), shout it here too — this is the live signature of
    // the wedge ("fresh edits, ZERO incremental PUT /ops"). The confirmed
    // snapshot deposit above then re-anchors the cursor (item 4).
    let strand_alarms_after = engine.outbound_strand_alarm_count().await;
    if strand_alarms_after > strand_alarms_before {
        tracing::warn!(
            "relay: {} outbound strand alarm(s) this tick — dirty note(s) shipped a \
             snapshot fallback instead of an incremental delta (deposit-strand class, \
             tesela-c7s)",
            strand_alarms_after - strand_alarms_before
        );
    }
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
    //
    // NO CURSOR REPAIR HERE (tesela-c7s F1, round 2). Every id in
    // `broadcast_note_ids` was just PUT as a broadcast op and its cursor
    // advanced by `commit_broadcast_cursors` above — for a note that was
    // stranded (stale-ahead / undecodable cursor), `produce_relay_updates`
    // already shipped a full-snapshot FALLBACK as that broadcast op (so peers
    // polling `GET ops?since=N` DO receive the content — the real convergence
    // fix), and the commit re-anchored the cursor to the vv captured at export
    // time. So by the time this heal-deposit runs, the strand is ALREADY
    // healed with the exact "snapshot-time vv, never swallow a concurrent edit"
    // semantics the repair primitive provides — a `repair_broadcast_cursors_
    // after_snapshot` call here would find every cursor already at the
    // identical version and change nothing (the round-1 no-op). The single,
    // shared cursor-repair mechanism instead lives at the SNAPSHOT-DEPOSIT
    // sites that can land a note's content WITHOUT a prior broadcast+commit —
    // `deposit_snapshots` below (server) and `RelayTicker.depositSnapshotsIfDue`
    // (iOS) — where a stranded cursor is NOT otherwise re-anchored.
    if !broadcast_note_ids.is_empty() {
        let mut heal: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        // (note_id, content hash) pairs to record AFTER a successful deposit,
        // so a failed deposit re-tries next tick instead of recording the hash.
        let mut deposited_hashes: Vec<([u8; 16], u64)> = Vec::new();
        for id in &broadcast_note_ids {
            if not_genuine_this_tick.contains(id) {
                continue;
            }
            if state.catchup_notes.contains(&hex::encode(id)) {
                continue;
            }
            if let Some(bytes) = engine.export_doc_update(*id, None).await {
                // Per-content throttle (layered ON TOP of the genuine /
                // catch-up skips above): skip ONLY when this exact snapshot
                // was already deposited (identical re-broadcast churn). Any
                // new/changed content hashes differently and deposits now.
                let h = export_snapshot_hash(&bytes);
                if state.deposit_hashes.get(id) == Some(&h) {
                    continue;
                }
                deposited_hashes.push((*id, h));
                heal.push((id.to_vec(), bytes));
            }
        }
        if !heal.is_empty() {
            match handle
                .client
                .put_snapshots_chunked(0, heal, deposit_chunk_budget_bytes())
                .await
            {
                Ok(_) => {
                    // Record only on success — a failed deposit must re-try.
                    for (id, h) in deposited_hashes {
                        state.deposit_hashes.insert(id, h);
                    }
                }
                Err(e) => {
                    tracing::warn!("relay: heal-snapshot deposit (broadcast notes): {e}");
                }
            }
        }
    }

    // ─── Snapshot-gated compaction cadence ───────────────────────────
    // Periodically deposit a per-note snapshot set covering everything
    // we've SAFELY applied, so the relay can GC the encrypted op log it
    // retains (it stays a durable backup via the snapshots). This is the
    // live wiring of the Phase-1 mechanism; one depositor (this server) is
    // enough — deposits are idempotent. Gated by a (test-tunable) interval
    // so a busy tick loop doesn't re-upload every note's snapshot constantly.
    //
    // Per-note compaction scoping (tesela-sclr.4): a stuck note (queued in
    // `catchup_notes`) must NEVER let `covers_seq` claim coverage of ops it
    // needs — the relay's GC (`DELETE relay_ops WHERE seq <= covers_seq`)
    // is group-wide, note-blind (see `deposit_snapshots` doc), so the only
    // lever available here is bounding the numeric watermark itself STRICTLY
    // below the earliest still-undeposited note's seq (`catchup_since_seq`).
    // This still lets compaction advance — and GC — everything OLDER than
    // that boundary (e.g. another note's earlier, already-healthy edits),
    // instead of the previous all-or-nothing gate that froze compaction for
    // the WHOLE group the moment any one note got stuck.
    let now = now_secs_i64();
    let due = state
        .last_snapshot_at
        .is_none_or(|t| now - t >= snapshot_interval_secs());
    if due {
        let earliest_stuck_seq: Option<i64> = if state.catchup_notes.is_empty() {
            None
        } else {
            Some(
                state
                    .catchup_notes
                    .iter()
                    .map(|h| state.catchup_since_seq.get(h).copied().unwrap_or(0))
                    .min()
                    .unwrap_or(0),
            )
        };
        let safe_covers_seq = match earliest_stuck_seq {
            None => state.inbound_cursor,
            Some(s) => state.inbound_cursor.min(s.saturating_sub(1)),
        };
        if !state.catchup_notes.is_empty() {
            if safe_covers_seq > 0 {
                tracing::warn!(
                    "relay: snapshot deposit SCOPED to seq {} — {} note(s) awaiting \
                     catch-up excluded + their ops protected from GC: {:?}",
                    safe_covers_seq,
                    state.catchup_notes.len(),
                    state.catchup_notes
                );
            } else {
                // The earliest stuck note sits at (or near) the very start of
                // the still-live op log — no boundary exists that both
                // protects it and advances anything. Conservative: the op
                // log grows until catch-up heals the queue (loud either way).
                tracing::warn!(
                    "relay: snapshot deposit SKIPPED — {} note(s) awaiting catch-up \
                     block compaction entirely: {:?}",
                    state.catchup_notes.len(),
                    state.catchup_notes
                );
            }
        }
        if safe_covers_seq > 0 {
            // Exclude the same two sets the heal-deposit excludes: notes
            // already given up on (`catchup_notes`) AND notes whose apply
            // is still mid-retry THIS tick (`not_genuine_this_tick`) — a
            // note isn't safely deposit-able the instant it fails, only
            // once it heals; the retry window is real and the periodic
            // path must not deposit a vivified/partial doc for it just
            // because it hasn't been officially queued for catch-up yet.
            let mut excluded = state.catchup_notes.clone();
            for id in &not_genuine_this_tick {
                let h = hex::encode(id);
                if !excluded.contains(&h) {
                    excluded.push(h);
                }
            }
            match deposit_snapshots(
                engine,
                &handle.client,
                safe_covers_seq,
                &excluded,
                &mut state.deposit_hashes,
            )
            .await
            {
                Ok(report) => {
                    // Stamp the cadence even on a partial (skipped-notes)
                    // deposit: retrying every tick can't shrink an oversize
                    // snapshot, it would just re-upload the mosaic in a loop.
                    state.last_snapshot_at = Some(now);
                    if report.complete() {
                        if report.gc > 0 {
                            tracing::debug!(
                                "relay snapshot deposit: covers seq {} in {} chunk(s), relay GC'd {} ops",
                                safe_covers_seq,
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

/// Deposit a per-note snapshot set covering relay-seq `covers_seq`, so the
/// relay can GC the encrypted op log it retains. Idempotent.
///
/// Change-driven (2026-07-02): only notes whose full export hashes
/// DIFFERENT from `deposit_hashes`' last-recorded value are actually
/// re-uploaded — an idle mosaic's cadence tick uploads ~zero bytes instead
/// of re-exporting + re-sealing every tracked note every interval. This is
/// safe because the relay decouples the two things a deposit does:
///   1. Per-stream `relay_snapshots` rows (upserted only for streams present
///      in THIS request's body).
///   2. The group-scalar `relay_group_meta.compaction_seq` watermark + the
///      `DELETE relay_ops WHERE seq <= covers_seq` GC, which applies to
///      EVERY stream regardless of which ones this request's body touched
///      (`crates/tesela-relay/src/store.rs` `deposit_snapshot_batch`).
/// So watermark movement alone is NEVER a reason to re-deposit an unchanged
/// note — the note's existing row (from a prior periodic OR heal deposit)
/// stays byte-valid at any later, higher `covers_seq`. What DOES require a
/// re-deposit is content change: even a content-neutral edit (e.g. an
/// edit immediately reverted) advances the note's underlying Loro oplog, so
/// its exported snapshot bytes — and hash — differ from the last deposit,
/// and it is included again. A note with NO recorded hash (never deposited
/// by either path) always deposits, so first-time coverage is never skipped.
///
/// Because relay-side GC is watermark-only (not conditioned on the request
/// body), the watermark must still advance even when every note is
/// unchanged — otherwise an otherwise-idle mosaic would never let the relay
/// GC ops it has already fully applied (e.g. other peers' envelopes). When
/// nothing needs re-upload this deposits a zero-entry checkpoint PUT
/// (`covers_seq` with an empty snapshot list) instead of skipping the
/// relay call entirely.
///
/// Chunked (non-empty case) under [`deposit_chunk_budget_bytes`] so a
/// whole-mosaic deposit (hundreds of notes) can't 413 against the relay's
/// request-body cap as one giant PUT. Only the FINAL chunk carries the real
/// `covers_seq` (= relay GC + watermark advance); intermediate chunks
/// deposit with `covers_seq = 0`, which both relay impls treat as inert, so
/// a crash mid-deposit leaves the op log intact and the next deposit heals
/// it.
///
/// `excluded_notes` (hex note ids — notes queued for catch-up AND notes
/// still mid-retry this tick) are EXCLUDED from the upload entirely — same
/// clobber guard as the broadcast heal-deposit: a note we don't genuinely
/// hold must never overwrite the relay's `relay_snapshots` row with
/// vivified/partial content. The caller is responsible for bounding
/// `covers_seq` so it never claims coverage of ops those excluded notes
/// still need (tesela-sclr.4).
async fn deposit_snapshots(
    engine: &dyn tesela_sync::SyncEngine,
    client: &RelayClient,
    covers_seq: i64,
    excluded_notes: &[String],
    deposit_hashes: &mut HashMap<[u8; 16], u64>,
) -> Result<tesela_sync::transport::relay::SnapshotDepositReport, String> {
    let note_ids = engine.tracked_note_ids().await;
    if note_ids.is_empty() {
        return Ok(Default::default());
    }
    let mut snapshots: Vec<(Vec<u8>, Vec<u8>)> = Vec::with_capacity(note_ids.len());
    // (note_id, content hash) pairs to record AFTER a successful deposit —
    // mirrors the heal-deposit's crash-safety pattern (a failed PUT must
    // re-try, not be silently marked as covered).
    let mut pending_hashes: Vec<([u8; 16], u64)> = Vec::new();
    // (note_id, vv AT SNAPSHOT-EXPORT TIME) — the SINGLE, shared cursor-repair
    // mechanism (tesela-c7s F1, round 2). This is the deposit path that can
    // land a note's content WITHOUT a prior broadcast+commit re-anchoring its
    // cursor (e.g. a note whose broadcast PUT FAILED but whose chunked deposit
    // here succeeds — commit never ran, so the strand persists), so it is the
    // load-bearing home of `repair_broadcast_cursors_after_snapshot`, not the
    // broadcast heal-deposit above (where commit already healed). The vv is
    // captured BEFORE the export so it is never AHEAD of the deposited content
    // — repairing to it can only rewind a genuinely stale-ahead / undecodable
    // cursor, never swallow a concurrent edit that landed after the cut, and it
    // leaves a healthy or behind cursor untouched (`broadcast_cursor_needs_repair`).
    let mut repair_pairs: Vec<([u8; 16], Vec<u8>)> = Vec::new();
    for id in note_ids {
        if excluded_notes.contains(&hex::encode(id)) {
            continue; // clobber guard — content not genuinely held
        }
        let snap_vv = engine.doc_version(id).await;
        // `export_doc_update(id, None)` = the note's full compact snapshot.
        if let Some(bytes) = engine.export_doc_update(id, None).await {
            let h = export_snapshot_hash(&bytes);
            if deposit_hashes.get(&id) == Some(&h) {
                continue; // unchanged since the last deposit (either path)
            }
            pending_hashes.push((id, h));
            if let Some(vv) = snap_vv {
                repair_pairs.push((id, vv));
            }
            snapshots.push((id.to_vec(), bytes));
        }
    }
    if snapshots.is_empty() {
        // Every genuinely-held tracked note already has a valid relay-side
        // row — just checkpoint the watermark so GC can still run.
        return client
            .put_snapshots(covers_seq, Vec::new())
            .await
            .map(|gc| tesela_sync::transport::relay::SnapshotDepositReport {
                gc,
                chunks_sent: 1,
                skipped_streams: Vec::new(),
            })
            .map_err(|e| e.to_string());
    }
    let report = client
        .put_snapshots_chunked(covers_seq, snapshots, deposit_chunk_budget_bytes())
        .await
        .map_err(|e| e.to_string())?;
    // Record a hash only for notes the relay actually accepted — an
    // oversize single-entry SKIP (see `SnapshotDepositReport::skipped_streams`)
    // never landed, so it must stay eligible for re-attempt next cadence
    // rather than being marked as covered.
    for (id, h) in pending_hashes {
        if !report.skipped_streams.iter().any(|s| s.as_slice() == id) {
            deposit_hashes.insert(id, h);
        }
    }
    // Re-anchor any stranded outbound cursor to its snapshot-time version for
    // every note the relay ACCEPTED (skip the oversize-skipped ones — their
    // content never landed, so a peer catch-up can't reach it and the cursor
    // must keep re-producing). A no-op for healthy / behind cursors. This is
    // the load-bearing cursor-repair call site the F2 integration test drives.
    repair_pairs.retain(|(id, _)| !report.skipped_streams.iter().any(|s| s.as_slice() == id));
    engine
        .repair_broadcast_cursors_after_snapshot(&repair_pairs)
        .await;
    Ok(report)
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
            // The relay's compaction watermark is ALREADY ahead of our
            // cursor (that's why we bootstrapped) — this note's raw ops are
            // already gone, so there's nothing left to protect: no seq
            // bound needed (tesela-sclr.4).
            state
                .catchup_since_seq
                .entry(hex_id.clone())
                .or_insert(i64::MAX);
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

/// Hash a note's exported snapshot bytes for the per-note heal-deposit
/// throttle. Only stability WITHIN a process run matters (the map is
/// `#[serde(skip)]`), so the std hasher is sufficient — it distinguishes
/// changed content (always re-deposit) from identical re-broadcast churn
/// (skip).
fn export_snapshot_hash(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
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

    /// tesela-sclr.3: the periodic gated-compaction deposit must be
    /// change-driven, not a blind full re-upload every cadence tick. An
    /// unchanged note's relay-side snapshot row must stay byte-identical
    /// (proving it was never re-sealed/re-sent — AEAD sealing is randomized,
    /// so any re-upload would produce different ciphertext even for the
    /// same plaintext) while a changed note's edit lands within one
    /// interval AND the compaction watermark still advances (GC'ing the new
    /// op) even though the unchanged note contributed zero upload bytes.
    #[tokio::test]
    async fn periodic_deposit_skips_unchanged_notes_but_still_compacts() {
        std::env::set_var("TESELA_RELAY_SNAPSHOT_INTERVAL_SECS", "0");

        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };

        const NID_A: [u8; 16] = [0x0d; 16];
        const NID_B: [u8; 16] = [0x0e; 16];

        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa3; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        for (nid, slug, body) in [
            (NID_A, "delta", "- hello delta\n"),
            (NID_B, "epsilon", "- hello epsilon\n"),
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

        // Tick 1 PUTs the ops; tick 2 polls the echo + deposits + compacts.
        tick(&engine_a, &ident, &handle_a).await.unwrap();
        tick(&engine_a, &ident, &handle_a).await.unwrap();

        let probe = RelayClient::new(
            base_url.clone(),
            group,
            DeviceId::from_bytes([0xcd; 16]),
            key.clone(),
        );
        let (comp_seq_1, snaps_1) = probe.fetch_snapshots().await.unwrap();
        assert!(comp_seq_1 > 0, "first periodic deposit compacted");
        assert!(
            probe.poll(0).await.unwrap().rows.is_empty(),
            "op log compacted after the first deposit"
        );
        let baseline_b_payload = snaps_1
            .iter()
            .find(|(id, _, _)| id == &NID_B.to_vec())
            .map(|(_, _, p)| p.clone())
            .expect("B deposited in the first pass");

        // Edit ONLY note A.
        engine_a
            .record_local(OpPayload::NoteUpsert {
                note_id: NID_A,
                display_alias: Some("delta".into()),
                title: "delta".into(),
                content: "- hello delta EDITED\n".into(),
                created_at_millis: 2,
            })
            .await
            .unwrap();

        // Tick 3 broadcasts + heal-deposits A's edit (covers_seq=0, inert);
        // tick 4 polls A's own echo (advancing inbound_cursor past it) and
        // the periodic pass checkpoints the NEW covers_seq — B never
        // re-uploads (already covered), yet the relay still compacts A's
        // fresh edit envelope out of the op log.
        tick(&engine_a, &ident, &handle_a).await.unwrap();
        tick(&engine_a, &ident, &handle_a).await.unwrap();

        let (comp_seq_2, snaps_2) = probe.fetch_snapshots().await.unwrap();
        assert!(
            comp_seq_2 > comp_seq_1,
            "compaction watermark advanced again in one more interval \
             (before {comp_seq_1}, after {comp_seq_2})"
        );
        assert!(
            probe.poll(0).await.unwrap().rows.is_empty(),
            "op log compacted again despite B contributing zero re-upload bytes"
        );

        let a_payload = snaps_2
            .iter()
            .find(|(id, _, _)| id == &NID_A.to_vec())
            .map(|(_, _, p)| p.clone())
            .expect("A re-deposited after its edit");
        assert_ne!(
            a_payload,
            snaps_1
                .iter()
                .find(|(id, _, _)| id == &NID_A.to_vec())
                .map(|(_, _, p)| p.clone())
                .unwrap(),
            "A's edit landed on the relay within one interval"
        );

        let b_payload_2 = snaps_2
            .iter()
            .find(|(id, _, _)| id == &NID_B.to_vec())
            .map(|(_, _, p)| p.clone())
            .expect("B's row still present (never re-uploaded, not dropped)");
        assert_eq!(
            b_payload_2, baseline_b_payload,
            "unchanged B was never re-sealed/re-sent — byte-identical AEAD ciphertext \
             proves the periodic deposit skipped it (a re-upload would randomize the nonce)"
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

    /// tesela-sclr.4: one permanently-stuck note must not block whole-group
    /// GC forever. Note B's earlier, healthy edit predates note A's
    /// poisoning — once A gives up (queued for catch-up), the scoped
    /// periodic deposit must still compact B's op (seq below A's), while
    /// STRICTLY retaining A's own op (the relay's GC is seq-only, group-wide,
    /// so the only lever is bounding `covers_seq` below A's seq).
    #[tokio::test]
    async fn periodic_deposit_scopes_covers_seq_below_stuck_note() {
        // NOTE: deliberately does NOT touch `TESELA_RELAY_SNAPSHOT_INTERVAL_SECS`
        // — it's process-global and shared with sibling tests that force it
        // to "0" (always due), so mutating it to a different value here would
        // race across parallel test threads. Instead the cadence is kept off
        // during setup by priming `last_snapshot_at` directly below (works
        // regardless of whatever interval a concurrent thread has set), and
        // forced due for the one tick under test the SAME way every other
        // test in this module already does (interval "0", the only value any
        // test here ever sets, so no cross-test conflict).
        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };

        const NID_GOOD: [u8; 16] = [0x2a; 16]; // "B" — healthy, edits BEFORE A gets stuck
        const NID_POISON: [u8; 16] = [0x2f; 16]; // "A" — permanently stuck

        let b_tmp = tempfile::tempdir().unwrap();
        let dev_b = DeviceId::from_bytes([0xb4; 16]);
        let engine_b = engine_in(&b_tmp, dev_b).await;
        engine_b
            .record_local(OpPayload::NoteUpsert {
                note_id: NID_GOOD,
                display_alias: Some("goodb".into()),
                title: "goodb".into(),
                content: "- hello goodb\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let good_snap = engine_b.export_doc_update(NID_GOOD, None).await.unwrap();
        let client_b = RelayClient::new(base_url.clone(), group, dev_b, key.clone());
        client_b.register_or_recover().await.expect("b register");

        // GOOD arrives FIRST (lower seq) — POISON arrives SECOND (higher
        // seq) and is the one that gets stuck.
        let good_seq = put_loro_envelope(&client_b, dev_b, group, &[(NID_GOOD, good_snap)]).await;
        let poison_seq = put_loro_envelope(
            &client_b,
            dev_b,
            group,
            &[(NID_POISON, b"definitely not a loro update".to_vec())],
        )
        .await;
        assert!(poison_seq > good_seq);

        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa4; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        let handle_a = handle_for(
            &base_url,
            group,
            dev_a,
            key.clone(),
            a_tmp.path().to_path_buf(),
        );
        bring_up(&handle_a).await.expect("relay bring-up");
        // `due` treats a never-deposited `last_snapshot_at` as due
        // immediately regardless of the interval — prime it so the cadence
        // stays off while POISON gets stuck (a stray early deposit of GOOD,
        // e.g. from a concurrent test racing the shared interval env var to
        // "0", would be harmless — it can never touch POISON's seq either
        // way — so this is a best-effort quieting, not a correctness
        // requirement of the assertions below).
        handle_a.state.write().await.last_snapshot_at = Some(now_secs_i64());

        // Drive ticks until POISON exhausts its retry budget and is queued
        // for catch-up (mirrors `tick_holds_cursor_at_failed_apply_then_gives_up_after_bound`).
        tick(&engine_a, &ident, &handle_a).await.unwrap();
        for _ in 1..MAX_APPLY_RETRIES {
            tick(&engine_a, &ident, &handle_a).await.unwrap();
        }
        {
            let state = handle_a.state.read().await;
            assert!(
                state.catchup_notes.contains(&hex::encode(NID_POISON)),
                "poison note queued for catch-up: {:?}",
                state.catchup_notes
            );
            assert_eq!(
                state.catchup_since_seq.get(&hex::encode(NID_POISON)).copied(),
                Some(poison_seq),
                "the poisoned envelope's own seq is tracked as the protection bound"
            );
        }

        let probe = RelayClient::new(
            base_url.clone(),
            group,
            DeviceId::from_bytes([0xce; 16]),
            key.clone(),
        );

        // Force the cadence due (same env var value "0" every test in this
        // module already uses — no new conflicting value) and run one more
        // tick: the SCOPED deposit must compact B's earlier op while
        // retaining A's, regardless of whether an earlier tick already
        // happened to deposit GOOD.
        std::env::set_var("TESELA_RELAY_SNAPSHOT_INTERVAL_SECS", "0");
        tick(&engine_a, &ident, &handle_a).await.unwrap();

        let (comp_seq_after, snaps_after) = probe.fetch_snapshots().await.unwrap();
        assert!(
            comp_seq_after > 0 && comp_seq_after < poison_seq,
            "watermark advanced but stayed STRICTLY below the poisoned note's seq \
             (watermark {comp_seq_after}, poison seq {poison_seq})"
        );
        assert!(
            snaps_after
                .iter()
                .any(|(id, _, _)| id == &NID_GOOD.to_vec()),
            "B's healthy content was deposited"
        );
        assert!(
            !snaps_after
                .iter()
                .any(|(id, _, _)| id == &NID_POISON.to_vec()),
            "A's garbage content was never deposited (clobber guard)"
        );

        let raw_after = probe.poll(0).await.unwrap();
        assert!(
            !raw_after.rows.iter().any(|(seq, _)| *seq == good_seq),
            "B's op was GC'd — one stuck note no longer blocks whole-group GC"
        );
        assert!(
            raw_after.rows.iter().any(|(seq, _)| *seq == poison_seq),
            "A's op is RETAINED — covers_seq never claimed coverage of it"
        );

        assert!(
            handle_a
                .state
                .read()
                .await
                .catchup_notes
                .contains(&hex::encode(NID_POISON)),
            "A stays queued for catch-up (still genuinely stuck)"
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

    /// CONVERGENCE-CRITICAL (desktop, 2026-06-28): a consumer whose inbound
    /// cursor has fallen BEHIND the relay's compaction watermark — the ops it
    /// still needs were GC'd off the op log — must detect that (via the
    /// `X-Tesela-Compaction-Seq` poll header) and bootstrap from the deposited
    /// snapshots in a normal tick, NOT silently stall on empty polls. The
    /// bootstrap must converge it to content it never polled, touch only the
    /// inbound cursor, and never clobber a local un-broadcast edit.
    #[tokio::test]
    async fn tick_bootstraps_when_behind_compaction_watermark() {
        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };

        const NID_ALPHA: [u8; 16] = [0xa0; 16];
        const NID_BETA: [u8; 16] = [0xbe; 16];
        const NID_GAMMA: [u8; 16] = [0x6a; 16];

        // ── Engine A authors ALPHA + broadcasts it ──
        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa1; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        engine_a
            .record_local(OpPayload::NoteUpsert {
                note_id: NID_ALPHA,
                display_alias: Some("alpha".into()),
                title: "alpha".into(),
                content: "- hello alpha\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let handle_a = handle_for(&base_url, group, dev_a, key.clone(), a_tmp.path().to_path_buf());
        bring_up(&handle_a).await.expect("A bring-up");
        tick(&engine_a, &ident, &handle_a).await.unwrap(); // PUT ALPHA op

        // ── Engine B ticks to pick up ALPHA → inbound_cursor = N ──
        let b_tmp = tempfile::tempdir().unwrap();
        let dev_b = DeviceId::from_bytes([0xb1; 16]);
        let engine_b = engine_in(&b_tmp, dev_b).await;
        let handle_b = handle_for(&base_url, group, dev_b, key.clone(), b_tmp.path().to_path_buf());
        bring_up(&handle_b).await.expect("B bring-up");
        tick(&engine_b, &ident, &handle_b).await.unwrap();
        let n = handle_b.state.read().await.inbound_cursor;
        assert!(n > 0, "B advanced its cursor polling A's broadcast");
        assert!(
            engine_b
                .render_note(NID_ALPHA)
                .await
                .unwrap_or_default()
                .contains("hello alpha"),
            "B has ALPHA after the first tick"
        );

        // ── A authors BETA (NEVER broadcast as an op) and a snapshot batch is
        //    deposited covering a seq far above N: this advances the relay
        //    compaction watermark and GC-DELETEs every op <= covers_seq, so the
        //    only path to BETA is the deposited snapshot. ──
        engine_a
            .record_local(OpPayload::NoteUpsert {
                note_id: NID_BETA,
                display_alias: Some("beta".into()),
                title: "beta".into(),
                content: "- hello beta\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let snap_alpha = engine_a.export_doc_update(NID_ALPHA, None).await.unwrap();
        let snap_beta = engine_a.export_doc_update(NID_BETA, None).await.unwrap();
        let covers = n + 1000;
        handle_a
            .client
            .put_snapshots(
                covers,
                vec![
                    (NID_ALPHA.to_vec(), snap_alpha),
                    (NID_BETA.to_vec(), snap_beta),
                ],
            )
            .await
            .expect("deposit snapshot batch + advance watermark");

        // (c) B.poll(N) now returns EMPTY (ops GC'd) and the header reports a
        //     compaction watermark ahead of B's cursor.
        let probe_batch = handle_b.client.poll(n).await.unwrap();
        assert!(
            probe_batch.rows.is_empty(),
            "ops <= watermark were GC'd; poll since=N empty"
        );
        assert_eq!(
            probe_batch.compaction_seq,
            Some(covers),
            "poll header surfaces the relay compaction watermark"
        );
        assert!(covers > n, "watermark is ahead of B's cursor");

        // (f) CLOBBER setup: B makes a LOCAL un-broadcast edit BEFORE the
        //     bootstrapping tick. It must survive the bootstrap merge and still
        //     be pending-outbound (the bootstrap touches only the inbound cursor).
        engine_b
            .record_local(OpPayload::NoteUpsert {
                note_id: NID_GAMMA,
                display_alias: Some("gamma".into()),
                title: "gamma".into(),
                content: "- local gamma edit\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // (d) ONE B tick: poll is empty but the header says B is behind, so the
        //     tick bootstraps from snapshots — jumping the cursor to the
        //     watermark and converging B to BETA (a note B NEVER polled).
        let outcome = tick(&engine_b, &ident, &handle_b).await.unwrap();
        assert_eq!(
            handle_b.state.read().await.inbound_cursor,
            covers,
            "tick bootstrap jumped B's inbound cursor to the watermark"
        );
        assert_eq!(
            engine_b.render_note(NID_BETA).await,
            engine_a.render_note(NID_BETA).await,
            "B converged to BETA purely from the deposited snapshot (never polled)"
        );
        // (f) the local edit SURVIVED the bootstrap merge …
        assert!(
            engine_b
                .render_note(NID_GAMMA)
                .await
                .unwrap_or_default()
                .contains("local gamma edit"),
            "B's local un-broadcast edit survived the bootstrap (non-destructive merge)"
        );
        // … and was still pending-outbound: the same tick broadcast it to the
        // relay (the bootstrap never touched the outbound cursors).
        assert!(
            outcome.sent >= 1,
            "B flushed its pending-outbound edit this tick"
        );
        let probe = RelayClient::new(
            base_url.clone(),
            group,
            DeviceId::from_bytes([0xcc; 16]),
            key.clone(),
        );
        let tail = probe.poll(covers).await.unwrap();
        let mut saw_gamma = false;
        for (_seq, env) in &tail.rows {
            if let Ok(Some(updates)) = tesela_sync::decode_loro_relay_payload(&env.ciphertext) {
                if updates.iter().any(|u| u.doc == NID_GAMMA) {
                    saw_gamma = true;
                }
            }
        }
        assert!(
            saw_gamma,
            "B's pending-outbound local edit was broadcast (not clobbered) by the bootstrap"
        );

        // (d/e) SECOND tick = stable no-op for convergence + the NEGATIVE
        //       bootstrap case: poll now only returns B's own echoes, the cursor
        //       is already at/above the watermark, so compaction_seq <= cursor
        //       and NO bootstrap fires. Content is unchanged.
        let beta_before = engine_b.render_note(NID_BETA).await;
        let gamma_before = engine_b.render_note(NID_GAMMA).await;
        tick(&engine_b, &ident, &handle_b).await.unwrap();
        assert!(
            handle_b.state.read().await.inbound_cursor >= covers,
            "second tick never regresses the cursor"
        );
        assert_eq!(
            engine_b.render_note(NID_BETA).await,
            beta_before,
            "BETA stable on the no-op tick (no spurious re-bootstrap)"
        );
        assert_eq!(
            engine_b.render_note(NID_GAMMA).await,
            gamma_before,
            "GAMMA stable on the no-op tick"
        );
    }

    /// Strand a note's broadcast cursor with an UNDECODABLE value — the
    /// loro-free strand class (`outbound_cursor_stranded` / `export_doc_update`
    /// both take their `Err(_)` arm), net-equivalent to the stale-ahead class
    /// for the cursor-repair guard. The note is left dirty: `produce` ships a
    /// full-snapshot fallback until the cursor is re-anchored.
    async fn strand_cursor_undecodable(engine: &LoroEngine, note: [u8; 16]) {
        engine
            .commit_broadcast_cursors(&[(note, vec![0xff, 0xff, 0xff, 0xff])])
            .await;
    }

    /// tesela-c7s F1 (round 2): the SINGLE, shared cursor-repair mechanism is
    /// wired at the SNAPSHOT-DEPOSIT production call site (`deposit_snapshots`),
    /// NOT the broadcast heal-deposit (where `commit_broadcast_cursors` already
    /// healed). This exercises that exact production call site: a stranded note
    /// deposited WITHOUT a prior broadcast+commit must have its cursor
    /// re-anchored by the repair INSIDE `deposit_snapshots`, so it stops looping
    /// the snapshot fallback and resumes incremental deltas.
    ///
    /// REVERT-DISCRIMINATING: neutralize the
    /// `engine.repair_broadcast_cursors_after_snapshot(&repair_pairs)` call at
    /// the end of `deposit_snapshots` (or make `broadcast_cursor_needs_repair`
    /// always return false) and the post-deposit `produce` still finds the note
    /// dirty (stranded) — the "produce is now EMPTY" assertion fails.
    #[tokio::test]
    async fn deposit_snapshots_repairs_stranded_cursor_at_production_site() {
        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        const NID: [u8; 16] = [0x5a; 16];

        let a_tmp = tempfile::tempdir().unwrap();
        let dev_a = DeviceId::from_bytes([0xa5; 16]);
        let engine_a = engine_in(&a_tmp, dev_a).await;
        engine_a
            .record_local(OpPayload::NoteUpsert {
                note_id: NID,
                display_alias: Some("strand".into()),
                title: "strand".into(),
                content: "- base body\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();

        // Strand WITHOUT ever broadcasting: the note's cursor is garbage but no
        // op ever hit the relay, so nothing (not commit, not the heal-deposit)
        // has re-anchored it. `deposit_snapshots` is the only path that can.
        strand_cursor_undecodable(&engine_a, NID).await;
        assert_eq!(
            engine_a.produce_relay_updates().await.len(),
            1,
            "stranded: the dirty note re-exports (snapshot fallback), never nothing"
        );

        let client_a = RelayClient::new(base_url.clone(), group, dev_a, key.clone());
        client_a.register_or_recover().await.expect("a register");

        // Direct call to the production `deposit_snapshots` fn (inert deposit).
        let mut hashes = std::collections::HashMap::new();
        deposit_snapshots(&engine_a, &client_a, 0, &[], &mut hashes)
            .await
            .expect("deposit");

        // HEALED at the production call site: cursor re-anchored to the
        // snapshot-time version, so with no new edits `produce` ships NOTHING.
        assert!(
            engine_a.produce_relay_updates().await.is_empty(),
            "deposit_snapshots' repair re-anchored the stranded cursor → strand healed"
        );

        // And the next edit ships a real INCREMENTAL delta a base-less peer
        // cannot apply cleanly (proving it is NOT a full snapshot).
        engine_a
            .record_local(OpPayload::BlockUpsert {
                block_id: [0x5b; 16],
                note_id: NID,
                parent_block_id: None,
                order_key: "z".into(),
                indent_level: 0,
                text: "after-heal".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let out = engine_a.produce_relay_updates().await;
        assert_eq!(out.len(), 1, "the post-heal edit ships");
        let (_id, delta, _vv) = &out[0];
        let fresh = engine_in(&tempfile::tempdir().unwrap(), DeviceId::from_bytes([0x5c; 16])).await;
        let report = fresh.apply_relay_updates(&[(NID, delta.clone())]).await;
        assert_eq!(
            report.pending,
            vec![NID],
            "the post-heal broadcast is an INCREMENTAL delta (base-less peer leaves it pending), \
             not a self-contained snapshot — the strand is truly gone"
        );
    }

    /// tesela-c7s F2 (round 2), CONVERGENCE-CRITICAL, integration repro at the
    /// production call sites: a sender with a dirty, STRANDED note (its content
    /// reaches the relay only as a deposited snapshot, never as a pollable op)
    /// deposits + REPAIRS its cursor (`deposit_snapshots`), resumes INCREMENTAL
    /// ops, and a fresh peer converges WITHOUT any restart / bootstrap-from-
    /// scratch: it polls the incremental resume, hits the causal gap, and the
    /// pending-ledger auto snapshot catch-up (inside the receiver's real `tick`)
    /// heals it from the deposited snapshot.
    ///
    /// REVERT-DISCRIMINATING against BOTH new mechanisms:
    ///  - CURSOR REPAIR (F1): neutralize the repair in `deposit_snapshots` and
    ///    the sender's resume ships a full SNAPSHOT instead of an incremental
    ///    delta — the "base-less peer leaves the resume pending" assertion fails
    ///    (the receiver would then converge directly, no gap, no catch-up).
    ///  - AUTO CATCH-UP (item 2): neutralize `catchup_from_snapshots` (or the
    ///    `report.pending` → `catchup_notes` queueing) and the receiver's tail
    ///    stays pending forever — it never gets the base, so the convergence
    ///    assertion fails.
    #[tokio::test]
    async fn strand_deposit_repair_resume_incremental_converges_peer_via_catchup() {
        let (base_url, _relay_tmp, _relay_srv) = spawn_relay().await;
        let (group, key) = fresh_group();
        let ident = GroupIdentity {
            group_id: group,
            group_key: key.clone(),
        };
        const NID: [u8; 16] = [0x6c; 16];

        // ── Sender S authors a base, then its cursor is stranded WITHOUT the
        //    base ever being broadcast as an op (the post-authoritative-import
        //    strand shape: the note's content lives only as a deposited
        //    snapshot, the op log has nothing for it). ──
        let s_tmp = tempfile::tempdir().unwrap();
        let dev_s = DeviceId::from_bytes([0x51; 16]);
        let engine_s = engine_in(&s_tmp, dev_s).await;
        engine_s
            .record_local(OpPayload::NoteUpsert {
                note_id: NID,
                display_alias: Some("conv".into()),
                title: "conv".into(),
                content: "- alpha <!-- bid:11111111-1111-1111-1111-111111111111 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        strand_cursor_undecodable(&engine_s, NID).await;

        let client_s = RelayClient::new(base_url.clone(), group, dev_s, key.clone());
        client_s.register_or_recover().await.expect("s register");

        // S DEPOSITS its snapshot (inert) + REPAIRS the stranded cursor — the
        // production call site under test. After this, S's cursor sits at the
        // base version, so the NEXT edit ships incrementally.
        let mut hashes = std::collections::HashMap::new();
        deposit_snapshots(&engine_s, &client_s, 0, &[], &mut hashes)
            .await
            .expect("deposit base snapshot + repair cursor");

        // S resumes: a small tail edit → produce a genuine INCREMENTAL delta.
        engine_s
            .record_local(OpPayload::BlockUpsert {
                block_id: [0x62; 16],
                note_id: NID,
                parent_block_id: None,
                order_key: "z".into(),
                indent_level: 0,
                text: "beta-resume".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let out = engine_s.produce_relay_updates().await;
        assert_eq!(out.len(), 1, "S ships its resumed edit");
        let (_id, resume_delta, _vv) = out.into_iter().next().unwrap();

        // REPAIR DISCRIMINATOR: the resume is an INCREMENTAL delta — a base-less
        // peer cannot apply it cleanly (Loro leaves it PENDING behind the base).
        // Without the F1 repair this would be a self-contained snapshot instead.
        let probe = engine_in(&tempfile::tempdir().unwrap(), DeviceId::from_bytes([0x63; 16])).await;
        let probe_report = probe.apply_relay_updates(&[(NID, resume_delta.clone())]).await;
        assert_eq!(
            probe_report.pending,
            vec![NID],
            "S's resume is incremental (references the base only on the relay as a snapshot) — \
             the F1 cursor repair is load-bearing here"
        );

        // S broadcasts the incremental resume as a relay op.
        put_loro_envelope(&client_s, dev_s, group, &[(NID, resume_delta)]).await;

        // ── Receiver R (fresh) runs its REAL tick: polls the incremental resume,
        //    hits the causal gap, and the auto snapshot catch-up heals it from
        //    S's deposited snapshot — converging without any bootstrap-from-
        //    scratch. ──
        let r_tmp = tempfile::tempdir().unwrap();
        let dev_r = DeviceId::from_bytes([0x71; 16]);
        let engine_r = engine_in(&r_tmp, dev_r).await;
        let handle_r = handle_for(&base_url, group, dev_r, key.clone(), r_tmp.path().to_path_buf());
        bring_up(&handle_r).await.expect("R bring-up");

        tick(&engine_r, &ident, &handle_r).await.unwrap();

        // CATCH-UP DISCRIMINATOR + convergence: R has BOTH the base (from the
        // deposited-snapshot catch-up) and the resumed tail (the polled
        // incremental delta, integrated once its base arrived).
        let rendered = engine_r.render_note(NID).await.unwrap_or_default();
        assert!(
            rendered.contains("alpha"),
            "R recovered the base from the deposited snapshot (auto catch-up): {rendered:?}"
        );
        assert!(
            rendered.contains("beta-resume"),
            "R integrated S's incremental resume once the base arrived: {rendered:?}"
        );
        assert_eq!(
            engine_r.render_note(NID).await,
            engine_s.render_note(NID).await,
            "peer converged to the sender's content with no restart / bootstrap-from-scratch"
        );
        assert!(
            handle_r.state.read().await.catchup_notes.is_empty(),
            "the healed note left the catch-up queue"
        );
    }
}
