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

use tesela_sync::engine::PeerCursor;
use tesela_sync::hlc::HlcTimestamp;
use tesela_sync::transport::relay::RelayClient;
use tesela_sync::{GroupIdentity, SyncEnvelope};

/// Per-mosaic relay sync state, persisted to `.tesela/relay_state.json`
/// so cursors survive restart. Schema is intentionally tiny — the
/// real state-of-record is the relay itself (server-side ops table)
/// plus the engine's oplog.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelayState {
    /// Highest relay-assigned `seq` we've applied + acked. Drives the
    /// `?since=N` query parameter on poll.
    pub inbound_cursor: i64,
    /// HLC ntp64 of the most-recent local op we've PUT to the relay.
    /// `None` means "we've never put anything yet" → next produce
    /// emits from the beginning.
    pub outbound_cursor_ntp: Option<i64>,
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
    pub outbound_cursor_ntp: Option<i64>,
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
            outbound_cursor_ntp: state.outbound_cursor_ntp,
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
            outbound_cursor_ntp: None,
            last_poll_at: None,
            last_put_at: None,
            registered_at: None,
            last_error: None,
        }
    }
}

/// One iteration of the relay sync loop: inbound (poll + apply +
/// ack) followed by outbound (produce + put). Logs at debug level on
/// each step; surfaces hard errors through `RelayState.last_error`
/// for the web UI.
///
/// Returns `(applied, sent)` for observability.
pub async fn tick(
    engine: &dyn tesela_sync::SyncEngine,
    ident: &GroupIdentity,
    handle: &RelayHandle,
) -> Result<(u32, u32)> {
    let mut state = handle.state.write().await;
    let mut applied_total = 0u32;
    let mut sent_total = 0u32;

    // ─── Inbound ─────────────────────────────────────────────────────
    match handle.client.poll(state.inbound_cursor).await {
        Ok(envelopes) => {
            let mut max_seq = state.inbound_cursor;
            for (seq, env) in envelopes {
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
                if engine.uses_loro_relay_payload() {
                    // Loro v2 path: the envelope plaintext is the `TLR2`
                    // magic + postcard(Vec<LoroDocUpdate>). Import each
                    // per-note update (idempotent + commutative). A
                    // non-v2 payload (legacy / foreign) decodes to None;
                    // we skip it but still advance the relay cursor so we
                    // don't re-fetch it forever.
                    match tesela_sync::decode_loro_relay_payload(&env.ciphertext) {
                        Ok(Some(updates)) => {
                            let pairs: Vec<([u8; 16], Vec<u8>)> = updates
                                .into_iter()
                                .map(|u| (u.doc, u.update_bytes))
                                .collect();
                            let n = engine.apply_relay_updates(&pairs).await;
                            applied_total += n as u32;
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
                            tracing::warn!(
                                "relay loro apply seq={} from={}: {}",
                                seq,
                                hex::encode(peer.as_bytes()),
                                e
                            );
                        }
                    }
                } else {
                    match engine.apply_changes(peer, env).await {
                        Ok(_applied) => {
                            applied_total += 1;
                            if seq > max_seq {
                                max_seq = seq;
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "relay apply seq={} from={}: {}",
                                seq,
                                hex::encode(peer.as_bytes()),
                                e
                            );
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

    // Loro v2 outbound: broadcast per-note Loro updates accrued since
    // each note's last-broadcast version vector (the engine tracks +
    // persists those cursors internally). One envelope carries the whole
    // batch; receivers import idempotently. No HLC outbound cursor — the
    // engine's per-note VV cursors are the watermark.
    if engine.uses_loro_relay_payload() {
        let updates = engine.produce_relay_updates().await;
        if !updates.is_empty() {
            let payload: Vec<tesela_sync::LoroDocUpdate> = updates
                .into_iter()
                .map(|(doc, update_bytes)| tesela_sync::LoroDocUpdate { doc, update_bytes })
                .collect();
            match tesela_sync::encode_loro_relay_payload(&payload) {
                Ok(ciphertext) => {
                    let envelope = SyncEnvelope {
                        from_device: our_device,
                        to_group: ident.group_id,
                        nonce: [0u8; 24],
                        ciphertext,
                    };
                    match handle.client.put_envelope(envelope).await {
                        Ok((_seq, _ts)) => {
                            sent_total += 1;
                            state.last_put_at = Some(now_secs_i64());
                            state.last_error = None;
                        }
                        Err(e) => {
                            let msg = format!("relay put (loro): {e}");
                            tracing::warn!("{msg}");
                            state.last_error = Some(msg);
                        }
                    }
                }
                Err(e) => {
                    state.last_error = Some(format!("encode loro payload: {e}"));
                }
            }
        }
        if let Err(e) = state.save(&handle.mosaic_root).await {
            tracing::warn!("relay state save: {e}");
        }
        return Ok((applied_total, sent_total));
    }

    let outbound_cursor = match state.outbound_cursor_ntp {
        Some(ntp) => PeerCursor::At(HlcTimestamp::from_ntp64_i64(ntp, our_device)),
        None => PeerCursor::Earliest,
    };
    // Relay fanout: publish only ops we authored. Transitive ops get
    // to other devices via *their* own relay publish, not via us
    // re-broadcasting them (which would create publish loops). See the
    // docstring on `produce_local_authored_since` for the full reasoning.
    match engine
        .produce_local_authored_since(outbound_cursor, 1_000_000)
        .await
    {
        Ok(batch) => {
            if !batch.ops.is_empty() {
                let envelope = SyncEnvelope {
                    from_device: our_device,
                    to_group: ident.group_id,
                    nonce: [0u8; 24],
                    ciphertext: match postcard::to_allocvec(&batch.ops) {
                        Ok(b) => b,
                        Err(e) => {
                            state.last_error = Some(format!("encode ops: {e}"));
                            return Ok((applied_total, sent_total));
                        }
                    },
                };
                match handle.client.put_envelope(envelope).await {
                    Ok((_seq, _ts)) => {
                        sent_total += 1;
                        if let PeerCursor::At(ts) = batch.new_cursor {
                            state.outbound_cursor_ntp = Some(ts.ntp64_as_i64());
                        }
                        state.last_put_at = Some(now_secs_i64());
                        state.last_error = None;
                    }
                    Err(e) => {
                        let msg = format!("relay put: {e}");
                        tracing::warn!("{msg}");
                        state.last_error = Some(msg);
                    }
                }
            }
        }
        Err(e) => {
            let msg = format!("relay produce: {e}");
            tracing::warn!("{msg}");
            state.last_error = Some(msg);
        }
    }

    // Persist whatever progress we made this tick.
    if let Err(e) = state.save(&handle.mosaic_root).await {
        tracing::warn!("relay state save: {e}");
    }
    Ok((applied_total, sent_total))
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
