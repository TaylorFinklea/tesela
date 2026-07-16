//! WAN relay client. Talks the protocol defined in
//! `.docs/ai/phases/2026-05-24-relay-protocol-design.md` against
//! either the Rust/Axum `tesela-relay` self-host or the future
//! Cloudflare Worker port (same wire format).
//!
//! Unlike the LAN [`Transport`](super::Transport) trait, the relay
//! isn't session-oriented — it's an async deposit box. We expose a
//! direct surface (`register`, `verify_registration`, `put_envelope`,
//! `poll`, `ack`) and let the desktop orchestrator drive it on a
//! poll timer. Forcing it into the streaming `Transport` shape would
//! mask what it actually is.
//!
//! ## Zero-knowledge guarantee
//!
//! Every payload sent to the relay is AEAD-sealed with the group key
//! using `crypto::aead::seal`. The relay never sees plaintext.
//! `poll()` opens incoming payloads with the same key, so callers
//! get back the original `SyncEnvelope` they would have gotten from
//! the LAN transport — relay vs. LAN is invisible above this layer.

use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::crypto::aead::{
    envelope_aad, open as aead_open, seal as aead_seal,
    seal_with_nonce_prefix as aead_seal_with_nonce_prefix,
};
use crate::crypto::keys::GroupKey;
use crate::crypto::relay_auth::{
    body_hash_hex, canonical_request, compute_request_mac, derive_relay_auth_key, intent_msg,
    sign_intent, verify_intent,
};
use crate::device::DeviceId;
use crate::error::{SyncError, SyncResult};
use crate::group::GroupId;
use crate::wire::envelope::SyncEnvelope;

/// Bridge between a `SyncEngine` and a deployed relay. One instance
/// per (group, device) — typically the desktop creates one on
/// startup if `[sync.relay]` is configured.
pub struct RelayClient {
    base_url: Url,
    group_id: GroupId,
    device_id: DeviceId,
    group_key: GroupKey,
    /// Cached HKDF derivation — same on every device, but we hold
    /// it locally so request-time MAC computation is allocation-free.
    auth_key: [u8; 32],
    http: Client,
}

/// What the relay returned for a registration record. Carries the
/// fields the joiner-verification path needs. Deliberately does NOT
/// carry the auth_key: the relay no longer serves it (it's the MAC
/// key, and `/registration` is an open endpoint) — every member
/// derives it locally via HKDF from the group key instead.
#[derive(Debug, Clone)]
pub struct StoredRegistration {
    /// Wall-clock seconds at first-write registration. Stable across
    /// idempotent re-registrations; used to reconstruct the signed
    /// intent locally during joiner verification.
    pub registered_at: i64,
    /// `HMAC-SHA256(group_key, intent_msg(group_id, auth_key,
    /// registered_at))`. Joiners recompute this with their local
    /// `group_key` + locally-derived auth_key and verify match —
    /// that's the hijack-detection invariant the design rests on.
    pub intent: Vec<u8>,
}

impl RelayClient {
    /// Build a new client. Does no I/O — call `register()` /
    /// `verify_registration()` explicitly so callers control error
    /// reporting + retry policy.
    pub fn new(base_url: Url, group_id: GroupId, device_id: DeviceId, group_key: GroupKey) -> Self {
        let auth_key = derive_relay_auth_key(&group_key, &group_id);
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .expect("reqwest client construction is infallible with default config");
        Self {
            base_url,
            group_id,
            device_id,
            group_key,
            auth_key,
            http,
        }
    }

    /// `POST /groups/{id}/register`. Idempotent on byte-identical
    /// re-register (the relay returns 200 if our `(auth_key,
    /// registered_at, intent)` tuple matches what's stored).
    pub async fn register(&self, registered_at: i64) -> SyncResult<()> {
        let intent_text = intent_msg(&self.group_id, &self.auth_key, registered_at);
        let intent = sign_intent(&self.group_key, &intent_text);
        // Recovery-phrase discovery handle (ra7 P0 step 2): a one-way
        // PRF of the group key alone, independent of `group_id`.
        // Published on every registration so a future phrase-only
        // device (has the key, not the group_id) can resolve this
        // group via `GET /discover/{disc}`. NOT part of the signed
        // intent — see `intent_msg`.
        let disc = crate::crypto::recovery::derive_discovery_handle(&self.group_key);
        let body = serde_json::json!({
            "auth_key_b64": base64_std(&self.auth_key),
            "registered_at": registered_at,
            "intent_b64": base64_std(&intent),
            "disc_b64": base64_std(&disc),
        });
        let url = self.group_url("/register");
        let resp = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(net_err("register"))?;
        match resp.status() {
            StatusCode::OK => Ok(()),
            StatusCode::CONFLICT => Err(SyncError::Crypto(
                "relay registration conflict: a different (auth_key, registered_at, intent) is \
                 already stored. Caller should fetch /registration and recover."
                    .into(),
            )),
            // Any other status is a transient/server failure, NOT a conflict —
            // it must not take the conflict-recovery path (a 500/503 here used
            // to be misdiagnosed as "relay 409 but /registration returned 404",
            // turning a blip into a scary hijack-shaped error). `Other` lets
            // callers treat it as retryable.
            s => {
                let body = resp.text().await.unwrap_or_default();
                Err(SyncError::Other(format!(
                    "register: relay returned {s}: {}",
                    body.chars().take(200).collect::<String>()
                )))
            }
        }
    }

    /// Higher-level register: try with `now()`; on conflict, fetch
    /// the stored record + verify intent + retry register with the
    /// stored timestamp so the idempotent path succeeds. Lets a
    /// fresh client join a group that's already registered without
    /// any persisted state. Returns the timestamp actually pinned on
    /// the relay so callers can store it for future runs.
    pub async fn register_or_recover(&self) -> SyncResult<i64> {
        let now = now_secs_i64();
        match self.register(now).await {
            Ok(()) => Ok(now),
            Err(SyncError::Crypto(_)) => {
                let stored = self.fetch_registration().await?.ok_or_else(|| {
                    SyncError::Crypto("relay 409 but /registration returned 404".into())
                })?;
                let intent_text = intent_msg(&self.group_id, &self.auth_key, stored.registered_at);
                if !verify_intent(&self.group_key, &intent_text, &stored.intent) {
                    return Err(SyncError::Crypto(
                        "relay registration is hijacked: stored intent does not verify under \
                         our group key. Use admin recovery to delete the bogus registration."
                            .into(),
                    ));
                }
                // Idempotent re-register with the stored timestamp.
                self.register(stored.registered_at).await?;
                Ok(stored.registered_at)
            }
            Err(e) => Err(e),
        }
    }

    /// `GET /groups/{id}/registration`. Returns `None` on 404.
    pub async fn fetch_registration(&self) -> SyncResult<Option<StoredRegistration>> {
        let url = self.group_url("/registration");
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(net_err("fetch_registration"))?;
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(SyncError::Crypto(format!(
                "fetch_registration: relay returned {}",
                resp.status()
            )));
        }
        let r = resp
            .json::<RegistrationWire>()
            .await
            .map_err(net_err("fetch_registration body"))?;
        Ok(Some(r.into()))
    }

    /// Joiner check: the load-bearing security gate. Fetches the
    /// stored registration record and verifies the signed intent
    /// against our local group_key. MUST be called on first connect
    /// (or after `register_or_recover`) before trusting the relay
    /// with traffic. Returns Ok(()) on match, error containing the
    /// hijack signal otherwise.
    pub async fn verify_registration(&self) -> SyncResult<()> {
        let stored = self
            .fetch_registration()
            .await?
            .ok_or_else(|| SyncError::Crypto("relay has no registration for this group".into()))?;
        // `intent_text` embeds OUR locally-derived auth_key, so a
        // registration stored under a different auth_key (wrong
        // group_key, wrong group_id, or a squatter) already fails this
        // verify — no need for the relay to echo the key back.
        let intent_text = intent_msg(&self.group_id, &self.auth_key, stored.registered_at);
        if !verify_intent(&self.group_key, &intent_text, &stored.intent) {
            return Err(SyncError::Crypto(
                "relay registration intent does not verify under our group_key — HIJACKED. \
                 Use admin recovery to delete the bogus registration and re-pair."
                    .into(),
            ));
        }
        Ok(())
    }

    /// Deposit one envelope. AEAD-seals the caller's payload first so
    /// the relay only ever sees ciphertext. Returns the assigned
    /// `(seq, ts)` so callers can pin their last-deposited cursor.
    pub async fn put_envelope(&self, envelope: SyncEnvelope) -> SyncResult<(i64, f64)> {
        let aad = envelope_aad(self.device_id.as_bytes(), self.group_id.as_bytes());
        let sealed = aead_seal(&self.group_key, &envelope.ciphertext, &aad)?;
        let outer = OuterPayload {
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
        };
        let outer_bytes = postcard::to_allocvec(&outer)
            .map_err(|e| SyncError::Other(format!("postcard serialize outer: {e}")))?;
        let body = serde_json::json!({
            "from_device": hex::encode(self.device_id.as_bytes()),
            "payload_b64": base64_std(&outer_bytes),
        });
        let body_bytes =
            serde_json::to_vec(&body).map_err(|e| SyncError::Other(format!("json body: {e}")))?;
        let path = format!("/groups/{}/ops", hex::encode(self.group_id.as_bytes()));
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
        let nonce_b64 = self.fresh_nonce_b64();
        let ts = now_secs_i64();
        let canonical = canonical_request(
            "PUT",
            &path,
            "",
            &nonce_b64,
            ts,
            &body_hash_hex(&body_bytes),
        );
        let mac = compute_request_mac(&self.auth_key, &canonical);
        let resp = self
            .http
            .put(url)
            .header("Content-Type", "application/json")
            .header("X-Tesela-Group", hex::encode(self.group_id.as_bytes()))
            .header("X-Tesela-Device", hex::encode(self.device_id.as_bytes()))
            .header("X-Tesela-Nonce", &nonce_b64)
            .header("X-Tesela-Ts", ts.to_string())
            .header("X-Tesela-Mac", base64_std(&mac))
            .body(body_bytes)
            .send()
            .await
            .map_err(net_err("put_envelope"))?;
        if !resp.status().is_success() {
            return Err(SyncError::Crypto(format!(
                "put_envelope: relay returned {}",
                resp.status()
            )));
        }
        let ack: PutResponse = resp
            .json()
            .await
            .map_err(net_err("put_envelope response body"))?;
        Ok((ack.seq, ack.ts))
    }

    /// Fetch envelopes the relay has buffered for this group since
    /// the caller's cursor. Each row in [`PollBatch::rows`] is
    /// `(seq, envelope)` — the caller advances its cursor to
    /// [`PollBatch::max_seq`] (which also covers skipped rows) + calls
    /// `ack` once the SyncEngine has applied them.
    ///
    /// A row whose outer payload fails to decode or AEAD-open does NOT
    /// fail the batch: the failure is deterministic (corrupt payload,
    /// postcard version skew, or a foreign/rotated key), so re-fetching
    /// the same bytes can never succeed — aborting would wedge every
    /// subsequent envelope for this consumer forever. Such rows are
    /// logged + collected in [`PollBatch::skipped`] so callers advance
    /// past them, mirroring how the post-decrypt apply path already
    /// skips undecodable inner payloads.
    pub async fn poll(&self, since: i64) -> SyncResult<PollBatch> {
        let path = format!("/groups/{}/ops", hex::encode(self.group_id.as_bytes()));
        let query = format!("since={since}");
        let url = self
            .base_url
            .join(&format!("{path}?{query}"))
            .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
        let nonce_b64 = self.fresh_nonce_b64();
        let ts = now_secs_i64();
        let canonical = canonical_request("GET", &path, &query, &nonce_b64, ts, "");
        let mac = compute_request_mac(&self.auth_key, &canonical);
        let resp = self
            .http
            .get(url)
            .header("X-Tesela-Group", hex::encode(self.group_id.as_bytes()))
            .header("X-Tesela-Device", hex::encode(self.device_id.as_bytes()))
            .header("X-Tesela-Nonce", &nonce_b64)
            .header("X-Tesela-Ts", ts.to_string())
            .header("X-Tesela-Mac", base64_std(&mac))
            .send()
            .await
            .map_err(net_err("poll"))?;
        if !resp.status().is_success() {
            return Err(SyncError::Crypto(format!(
                "poll: relay returned {}",
                resp.status()
            )));
        }
        let compaction_seq = resp
            .headers()
            .get("X-Tesela-Compaction-Seq")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<i64>().ok());
        let rows: Vec<RelayOpWire> = resp.json().await.map_err(net_err("poll response body"))?;
        let mut out = PollBatch {
            rows: Vec::with_capacity(rows.len()),
            skipped: Vec::new(),
            compaction_seq,
        };
        for row in rows {
            match self.open_relay_row(&row) {
                Ok(envelope) => out.rows.push((row.seq, envelope)),
                Err(e) => {
                    tracing::warn!(
                        seq = row.seq,
                        from_device = %row.from_device,
                        "relay poll: skipping undecryptable envelope: {e}"
                    );
                    out.skipped.push(row.seq);
                }
            }
        }
        Ok(out)
    }

    /// Decode + AEAD-open one relay row. Failures here are
    /// deterministic per-row conditions, isolated so `poll` can skip
    /// the row instead of failing the whole batch.
    fn open_relay_row(&self, row: &RelayOpWire) -> SyncResult<SyncEnvelope> {
        let from_device_bytes = hex::decode(&row.from_device)
            .map_err(|e| SyncError::Other(format!("from_device hex: {e}")))?;
        let from_device_arr: [u8; 16] = from_device_bytes
            .try_into()
            .map_err(|_| SyncError::Other("from_device wrong length".into()))?;
        let outer_bytes = base64::engine::general_purpose::STANDARD
            .decode(&row.payload_b64)
            .map_err(|e| SyncError::Other(format!("payload base64: {e}")))?;
        let outer: OuterPayload = postcard::from_bytes(&outer_bytes)
            .map_err(|e| SyncError::Other(format!("postcard outer: {e}")))?;
        let aad = envelope_aad(&from_device_arr, self.group_id.as_bytes());
        let plaintext = aead_open(&self.group_key, &outer.nonce, &outer.ciphertext, &aad)?;
        Ok(SyncEnvelope {
            from_device: DeviceId::from_bytes(from_device_arr),
            to_group: self.group_id,
            nonce: outer.nonce,
            ciphertext: plaintext,
        })
    }

    /// Deposit a full set of per-stream encrypted snapshots covering
    /// relay-seq `covers_seq`, snapshot-gating compaction on the relay.
    ///
    /// Each entry is `(stream_id, plaintext_snapshot_bytes)`. The
    /// plaintext is AEAD-sealed with the group key (SAME scheme as
    /// [`put_envelope`](Self::put_envelope)) so the relay only ever sees
    /// ciphertext; the `stream_id` is sent verbatim (opaque to the
    /// relay). The relay upserts each snapshot, advances its compaction
    /// watermark to `covers_seq`, and GCs `relay_ops` rows with
    /// `seq <= covers_seq`. Returns the number of ops the relay
    /// compacted away.
    ///
    /// v1 uses `stream_id = note_id` (16 bytes). The relay treats it as
    /// opaque bytes, so a later privacy hardening can swap in an
    /// HMAC-derived opaque stream key without touching the relay.
    // TODO(privacy): derive `stream_id` as an HMAC of `note_id` under a
    // per-group stream key so the relay can't correlate snapshots to
    // stable note identifiers across pushes. Out of scope for Phase
    // 1b-ii (v1 ships `stream_id = note_id`).
    pub async fn put_snapshots(
        &self,
        covers_seq: i64,
        snapshots: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> SyncResult<u64> {
        self.put_snapshots_at_seq(covers_seq, covers_seq, snapshots)
            .await
    }

    /// Deposit snapshots whose per-entry recency is `snapshot_seq` while the
    /// batch independently advances compaction only through `covers_seq`.
    /// Keeping these values separate lets a non-final chunk use
    /// `covers_seq = 0` without making its rows stale-at-zero.
    pub async fn put_snapshots_at_seq(
        &self,
        snapshot_seq: i64,
        covers_seq: i64,
        snapshots: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> SyncResult<u64> {
        let entries = self.seal_snapshot_entries(snapshot_seq, &snapshots)?;
        let wire: Vec<serde_json::Value> = entries.into_iter().map(|e| e.wire).collect();
        match self.send_snapshot_batch(covers_seq, &wire).await? {
            SnapshotSendStatus::Accepted(gc) => Ok(gc),
            SnapshotSendStatus::TooLarge => Err(SyncError::Crypto(format!(
                "put_snapshots: relay returned {}",
                StatusCode::PAYLOAD_TOO_LARGE
            ))),
        }
    }

    /// Chunked variant of [`put_snapshots`](Self::put_snapshots) for full-
    /// mosaic deposits that would blow past a relay's request-body cap as
    /// one PUT (the live 413 on ~250 resident notes).
    ///
    /// Packs the sealed snapshot entries into size-bounded chunks (at most
    /// `budget_bytes` of serialized request body each, base64 inflation
    /// included) and deposits them as a sequence of `PUT /snapshot` calls.
    ///
    /// ## GC safety invariant
    ///
    /// Every chunk EXCEPT the last is deposited with `covers_seq = 0`,
    /// which both relay implementations treat as inert: the per-stream
    /// snapshot upserts apply, but the compaction watermark only moves
    /// forward (`MAX(existing, 0)` = no advance) and `DELETE … seq <= 0`
    /// GCs nothing (seqs start at 1). ONLY the final chunk carries the
    /// real `covers_seq`. A crash between chunks therefore leaves the
    /// relay's op log un-GC'd and the watermark unmoved — the partial
    /// upserts are harmless and the next full deposit heals everything.
    ///
    /// ## Adaptive degrade on 413
    ///
    /// The effective relay cap isn't known client-side (HA self-host
    /// defaults differ from the CF Worker's 1 MiB), so on a 413 the chunk
    /// budget is HALVED and the chunk re-packed + retried — down to a
    /// floor of [`SNAPSHOT_CHUNK_FLOOR_BYTES`], after which entries are
    /// sent one at a time. Nothing about the degraded budget is persisted;
    /// the next deposit starts from the configured budget again.
    ///
    /// A SINGLE entry whose request alone still 413s can never succeed by
    /// retrying (halving the budget doesn't shrink it), so it is skipped
    /// with a loud warn and reported in
    /// [`SnapshotDepositReport::skipped_streams`]. When anything was
    /// skipped the final chunk is ALSO sent with `covers_seq = 0`:
    /// advancing the watermark would GC ops whose content the deposited
    /// snapshot set lacks (the skipped note), destroying them group-wide.
    pub async fn put_snapshots_chunked(
        &self,
        covers_seq: i64,
        snapshots: Vec<(Vec<u8>, Vec<u8>)>,
        budget_bytes: usize,
    ) -> SyncResult<SnapshotDepositReport> {
        self.put_snapshots_chunked_at_seq(covers_seq, covers_seq, snapshots, budget_bytes)
            .await
    }

    /// Chunked snapshot deposit with independent per-entry recency and batch
    /// compaction sequences. See [`Self::put_snapshots_at_seq`].
    pub async fn put_snapshots_chunked_at_seq(
        &self,
        snapshot_seq: i64,
        covers_seq: i64,
        snapshots: Vec<(Vec<u8>, Vec<u8>)>,
        budget_bytes: usize,
    ) -> SyncResult<SnapshotDepositReport> {
        let mut report = SnapshotDepositReport::default();
        if snapshots.is_empty() {
            return Ok(report);
        }
        let mut queue: std::collections::VecDeque<SealedSnapshotEntry> =
            self.seal_snapshot_entries(snapshot_seq, &snapshots)?.into();
        // Headroom for the `{"covers_seq":…,"snapshots":[…]}` wrapper.
        const ENVELOPE_OVERHEAD: usize = 64;
        let mut budget = budget_bytes.max(1);
        while let Some(first) = queue.pop_front() {
            // Greedy pack: always at least one entry, then more while the
            // serialized body stays under the current budget.
            let mut size = ENVELOPE_OVERHEAD + first.size;
            let mut chunk = vec![first];
            while let Some(next) = queue.front() {
                if size + next.size > budget {
                    break;
                }
                size += next.size;
                chunk.push(queue.pop_front().expect("front exists"));
            }
            // The real covers_seq rides ONLY on the final chunk, and only
            // when no entry was skipped (see GC safety invariant above).
            let covers = if queue.is_empty() && report.skipped_streams.is_empty() {
                covers_seq
            } else {
                0
            };
            let wire: Vec<serde_json::Value> = chunk.iter().map(|e| e.wire.clone()).collect();
            match self.send_snapshot_batch(covers, &wire).await? {
                SnapshotSendStatus::Accepted(gc) => {
                    report.gc += gc;
                    report.chunks_sent += 1;
                }
                SnapshotSendStatus::TooLarge if chunk.len() == 1 => {
                    // Retrying an identical single-entry request can never
                    // succeed — the entry itself exceeds the relay's cap.
                    let entry = chunk.into_iter().next().expect("one entry");
                    tracing::warn!(
                        stream_id = %hex::encode(&entry.stream_id),
                        size_bytes = entry.size,
                        "relay snapshot deposit: single snapshot exceeds the relay body cap — \
                         SKIPPED (compaction watermark will NOT advance this deposit)"
                    );
                    report.skipped_streams.push(entry.stream_id);
                }
                SnapshotSendStatus::TooLarge => {
                    // Adaptive degrade: halve the budget and re-pack this
                    // chunk's entries (front of the queue keeps order). At
                    // the floor, fall back to one-entry-per-request so a
                    // tiny relay cap can't loop a same-size chunk forever.
                    budget = if budget > SNAPSHOT_CHUNK_FLOOR_BYTES {
                        (budget / 2).max(SNAPSHOT_CHUNK_FLOOR_BYTES)
                    } else {
                        1
                    };
                    tracing::warn!(
                        new_budget_bytes = budget,
                        chunk_entries = chunk.len(),
                        "relay snapshot deposit: 413 on chunk — halving budget and retrying"
                    );
                    for entry in chunk.into_iter().rev() {
                        queue.push_front(entry);
                    }
                }
            }
        }
        Ok(report)
    }

    /// AEAD-seal each `(stream_id, plaintext)` snapshot into its wire-ready
    /// JSON entry, with the serialized size pre-measured for chunk packing.
    fn seal_snapshot_entries(
        &self,
        snapshot_seq: i64,
        snapshots: &[(Vec<u8>, Vec<u8>)],
    ) -> SyncResult<Vec<SealedSnapshotEntry>> {
        let mut entries = Vec::with_capacity(snapshots.len());
        for (stream_id, plaintext) in snapshots {
            // Preserve the legacy OuterPayload + group-only AEAD so old
            // clients can still read rows written by a new client. The nonce
            // marker makes the appended routing record mandatory for new
            // clients, preventing a relay from stripping it as a downgrade.
            let sealed = aead_seal_with_nonce_prefix(
                &self.group_key,
                plaintext,
                &snapshot_aad(self.group_id.as_bytes()),
                SNAPSHOT_V2_NONCE_PREFIX,
            )?;
            let outer = OuterPayload {
                nonce: sealed.nonce,
                ciphertext: sealed.ciphertext,
            };
            let mut outer_bytes = postcard::to_allocvec(&outer)
                .map_err(|e| SyncError::Other(format!("postcard serialize outer: {e}")))?;
            append_snapshot_route_record(
                &mut outer_bytes,
                &self.group_key,
                self.group_id.as_bytes(),
                stream_id,
                snapshot_seq,
                &outer,
            )?;
            let wire = serde_json::json!({
                "stream_id_b64": base64_std(stream_id),
                "snapshot_seq": snapshot_seq,
                "payload_b64": base64_std(&outer_bytes),
            });
            // Exact serialized footprint of this entry in the request body
            // (+1 for the separating comma) — base64 inflation included.
            let size = serde_json::to_vec(&wire)
                .map_err(|e| SyncError::Other(format!("json entry: {e}")))?
                .len()
                + 1;
            entries.push(SealedSnapshotEntry {
                stream_id: stream_id.clone(),
                wire,
                size,
            });
        }
        Ok(entries)
    }

    /// One authenticated `PUT /groups/{id}/snapshot` carrying pre-sealed
    /// wire entries. Distinguishes 413 (so the chunked deposit can adapt)
    /// from other failures (propagated as errors).
    async fn send_snapshot_batch(
        &self,
        covers_seq: i64,
        entries: &[serde_json::Value],
    ) -> SyncResult<SnapshotSendStatus> {
        let body = serde_json::json!({
            "snapshot_seq_version": 1,
            "covers_seq": covers_seq,
            "snapshots": entries,
        });
        let body_bytes =
            serde_json::to_vec(&body).map_err(|e| SyncError::Other(format!("json body: {e}")))?;
        let path = format!("/groups/{}/snapshot", hex::encode(self.group_id.as_bytes()));
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
        let nonce_b64 = self.fresh_nonce_b64();
        let ts = now_secs_i64();
        let canonical = canonical_request(
            "PUT",
            &path,
            "",
            &nonce_b64,
            ts,
            &body_hash_hex(&body_bytes),
        );
        let mac = compute_request_mac(&self.auth_key, &canonical);
        let resp = self
            .http
            .put(url)
            .header("Content-Type", "application/json")
            .header("X-Tesela-Group", hex::encode(self.group_id.as_bytes()))
            .header("X-Tesela-Device", hex::encode(self.device_id.as_bytes()))
            .header("X-Tesela-Nonce", &nonce_b64)
            .header("X-Tesela-Ts", ts.to_string())
            .header("X-Tesela-Mac", base64_std(&mac))
            .body(body_bytes)
            .send()
            .await
            .map_err(net_err("put_snapshots"))?;
        if resp.status() == StatusCode::PAYLOAD_TOO_LARGE {
            return Ok(SnapshotSendStatus::TooLarge);
        }
        if !resp.status().is_success() {
            return Err(SyncError::Crypto(format!(
                "put_snapshots: relay returned {}",
                resp.status()
            )));
        }
        let ack: PutSnapshotResponse = resp
            .json()
            .await
            .map_err(net_err("put_snapshots response body"))?;
        Ok(SnapshotSendStatus::Accepted(ack.gc))
    }

    /// Fetch the latest encrypted snapshot per opaque stream plus the
    /// relay's compaction watermark. A fresh/recovered device bootstraps
    /// from these (open + import each), then polls `?since=` for the
    /// tail. Each returned tuple is `(stream_id, snapshot_seq,
    /// plaintext_snapshot_bytes)` — the payload is `aead_open`-ed back to
    /// the original snapshot plaintext with the group key.
    pub async fn fetch_snapshots(&self) -> SyncResult<(i64, Vec<(Vec<u8>, i64, Vec<u8>)>)> {
        let path = format!(
            "/groups/{}/snapshots",
            hex::encode(self.group_id.as_bytes())
        );
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
        let nonce_b64 = self.fresh_nonce_b64();
        let ts = now_secs_i64();
        let canonical = canonical_request("GET", &path, "", &nonce_b64, ts, "");
        let mac = compute_request_mac(&self.auth_key, &canonical);
        let resp = self
            .http
            .get(url)
            .header("X-Tesela-Group", hex::encode(self.group_id.as_bytes()))
            .header("X-Tesela-Device", hex::encode(self.device_id.as_bytes()))
            .header("X-Tesela-Nonce", &nonce_b64)
            .header("X-Tesela-Ts", ts.to_string())
            .header("X-Tesela-Mac", base64_std(&mac))
            .send()
            .await
            .map_err(net_err("fetch_snapshots"))?;
        if !resp.status().is_success() {
            return Err(SyncError::Crypto(format!(
                "fetch_snapshots: relay returned {}",
                resp.status()
            )));
        }
        let wire: SnapshotsWire = resp
            .json()
            .await
            .map_err(net_err("fetch_snapshots response body"))?;
        let b64 = base64::engine::general_purpose::STANDARD;
        let mut out = Vec::with_capacity(wire.snapshots.len());
        for entry in wire.snapshots {
            let stream_id = b64
                .decode(&entry.stream_id_b64)
                .map_err(|e| SyncError::Other(format!("stream_id base64: {e}")))?;
            let outer_bytes = b64
                .decode(&entry.payload_b64)
                .map_err(|e| SyncError::Other(format!("payload base64: {e}")))?;
            let plaintext = open_snapshot_payload(
                &self.group_key,
                self.group_id.as_bytes(),
                &stream_id,
                entry.snapshot_seq,
                &outer_bytes,
            )?;
            out.push((stream_id, entry.snapshot_seq, plaintext));
        }
        Ok((wire.compaction_seq, out))
    }

    /// Tell the relay "this device has applied every op up to and
    /// including `applied_seq`". Drives server-side GC.
    pub async fn ack(&self, applied_seq: i64) -> SyncResult<()> {
        let body = serde_json::json!({
            "device": hex::encode(self.device_id.as_bytes()),
            "applied_seq": applied_seq,
        });
        let body_bytes =
            serde_json::to_vec(&body).map_err(|e| SyncError::Other(format!("json body: {e}")))?;
        let path = format!("/groups/{}/ack", hex::encode(self.group_id.as_bytes()));
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
        let nonce_b64 = self.fresh_nonce_b64();
        let ts = now_secs_i64();
        let canonical = canonical_request(
            "POST",
            &path,
            "",
            &nonce_b64,
            ts,
            &body_hash_hex(&body_bytes),
        );
        let mac = compute_request_mac(&self.auth_key, &canonical);
        let resp = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .header("X-Tesela-Group", hex::encode(self.group_id.as_bytes()))
            .header("X-Tesela-Device", hex::encode(self.device_id.as_bytes()))
            .header("X-Tesela-Nonce", &nonce_b64)
            .header("X-Tesela-Ts", ts.to_string())
            .header("X-Tesela-Mac", base64_std(&mac))
            .body(body_bytes)
            .send()
            .await
            .map_err(net_err("ack"))?;
        if !resp.status().is_success() {
            return Err(SyncError::Crypto(format!(
                "ack: relay returned {}",
                resp.status()
            )));
        }
        Ok(())
    }

    /// Register this device's APNs push token so the relay can send a
    /// content-available silent push to our other devices on every op
    /// deposit (sync durability P3b). MAC-authed exactly like `ack`. The
    /// token is a routing identifier, not note content — the relay stays
    /// zero-knowledge. Idempotent (relay upserts by device id).
    pub async fn register_device(&self, apns_token_hex: &str) -> SyncResult<()> {
        let body = serde_json::json!({
            "device": hex::encode(self.device_id.as_bytes()),
            "apns_token": apns_token_hex,
        });
        let body_bytes =
            serde_json::to_vec(&body).map_err(|e| SyncError::Other(format!("json body: {e}")))?;
        let path = format!("/groups/{}/devices", hex::encode(self.group_id.as_bytes()));
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
        let nonce_b64 = self.fresh_nonce_b64();
        let ts = now_secs_i64();
        let canonical = canonical_request(
            "POST",
            &path,
            "",
            &nonce_b64,
            ts,
            &body_hash_hex(&body_bytes),
        );
        let mac = compute_request_mac(&self.auth_key, &canonical);
        let resp = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .header("X-Tesela-Group", hex::encode(self.group_id.as_bytes()))
            .header("X-Tesela-Device", hex::encode(self.device_id.as_bytes()))
            .header("X-Tesela-Nonce", &nonce_b64)
            .header("X-Tesela-Ts", ts.to_string())
            .header("X-Tesela-Mac", base64_std(&mac))
            .body(body_bytes)
            .send()
            .await
            .map_err(net_err("register_device"))?;
        if !resp.status().is_success() {
            return Err(SyncError::Crypto(format!(
                "register_device: relay returned {}",
                resp.status()
            )));
        }
        Ok(())
    }

    // ── Helpers ────────────────────────────────────────────────────

    fn group_url(&self, suffix: &str) -> Url {
        let path = format!(
            "/groups/{}{}",
            hex::encode(self.group_id.as_bytes()),
            suffix
        );
        self.base_url
            .join(&path)
            .expect("group_url is always a valid path append")
    }

    fn fresh_nonce_b64(&self) -> String {
        let mut bytes = [0u8; 16];
        use rand::RngCore;
        rand::thread_rng().fill_bytes(&mut bytes);
        base64_std(&bytes)
    }
}

/// `GET /discover/{disc}` — resolve a recovery-phrase discovery handle
/// to its `group_id` (`tesela-ra7` P0 step 3a).
///
/// Deliberately a MODULE-LEVEL function, not a [`RelayClient`] method:
/// a phrase-only device has the `GroupKey` but neither the `group_id`
/// nor an `auth_key` yet (both need `group_id` as HKDF salt), so there
/// is no `RelayClient` to construct. This is the unauthenticated
/// bootstrap step that produces the `group_id` a normal `RelayClient`
/// can then be built from — mirrors the relay's `GET /discover/{disc}`
/// handler, which is intentionally NOT behind the MAC gate.
///
/// Returns `Ok(None)` on a 404 (no group has published this discovery
/// handle — never registered, or registered before ra7 P0 step 2
/// added `disc_b64`). Any other non-2xx status is an `Err`.
pub async fn discover_group(relay_url: &str, disc: &[u8; 32]) -> SyncResult<Option<GroupId>> {
    let base = Url::parse(relay_url)
        .map_err(|e| SyncError::Other(format!("discover_group: invalid relay url: {e}")))?;
    let path = format!("/discover/{}", hex::encode(disc));
    let url = base
        .join(&path)
        .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
    let http = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("reqwest client construction is infallible with default config");
    let resp = http.get(url).send().await.map_err(net_err("discover_group"))?;
    if resp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(SyncError::Other(format!(
            "discover_group: relay returned {}",
            resp.status()
        )));
    }
    let wire: DiscoverWire = resp
        .json()
        .await
        .map_err(net_err("discover_group response body"))?;
    let bytes = hex::decode(&wire.group_id)
        .map_err(|e| SyncError::Other(format!("discover_group: group_id hex: {e}")))?;
    let arr: [u8; 16] = bytes
        .try_into()
        .map_err(|_| SyncError::Other("discover_group: group_id wrong length".into()))?;
    Ok(Some(GroupId::from_bytes(arr)))
}

/// `GET /discover/{disc}` response body.
#[derive(Debug, Deserialize)]
struct DiscoverWire {
    group_id: String,
}

/// Floor for the adaptive 413-halving in
/// [`RelayClient::put_snapshots_chunked`]. Once the budget reaches this,
/// the next 413 degrades to one-entry-per-request rather than re-packing
/// a same-size chunk forever against a relay cap below the floor.
pub const SNAPSHOT_CHUNK_FLOOR_BYTES: usize = 256 * 1024;

/// Outcome of one [`RelayClient::put_snapshots_chunked`] deposit.
#[derive(Debug, Default)]
pub struct SnapshotDepositReport {
    /// Total ops the relay GC'd (non-zero only when the final chunk's
    /// real `covers_seq` landed — covers_seq=0 chunks never GC).
    pub gc: u64,
    /// Number of `PUT /snapshot` requests that succeeded.
    pub chunks_sent: u32,
    /// Stream ids (note ids) whose snapshot alone exceeded the relay's
    /// body cap and were SKIPPED. Non-empty ⇒ the compaction watermark
    /// was NOT advanced (the deposit withheld the real covers_seq) —
    /// callers must surface this loudly.
    pub skipped_streams: Vec<Vec<u8>>,
}

impl SnapshotDepositReport {
    /// True when the deposit was complete (no oversize snapshot skipped)
    /// and the final chunk carried the real `covers_seq`.
    pub fn complete(&self) -> bool {
        self.skipped_streams.is_empty()
    }
}

/// A snapshot entry sealed + serialized once up front, so 413 retries
/// re-send identical bytes instead of re-sealing (idempotent upserts).
struct SealedSnapshotEntry {
    stream_id: Vec<u8>,
    wire: serde_json::Value,
    size: usize,
}

/// Result of one `PUT /snapshot` request: accepted (with the relay's GC
/// count) or rejected as over the body cap (the only failure the chunked
/// deposit adapts to; everything else is an error).
enum SnapshotSendStatus {
    Accepted(u64),
    TooLarge,
}

/// One [`RelayClient::poll`] batch.
#[derive(Debug, Default)]
pub struct PollBatch {
    /// Envelopes that decoded + AEAD-opened cleanly, in seq order.
    pub rows: Vec<(i64, SyncEnvelope)>,
    /// Seqs of rows skipped because their outer payload failed to
    /// decode or decrypt (deterministic — retrying can never help).
    /// Surfaced so callers fold them into their cursor advancement;
    /// otherwise a poisoned row at the batch tail would be re-fetched
    /// and re-skipped on every poll.
    pub skipped: Vec<i64>,
    /// The relay's current per-group compaction watermark, read from the
    /// additive `X-Tesela-Compaction-Seq` response header on `GET /ops`.
    /// `None` when the relay omitted the header (older relay) or it failed
    /// to parse. When `> inbound_cursor` the consumer has fallen behind the
    /// compaction watermark — the ops it still needs were GC'd off the op
    /// log — and must bootstrap from snapshots instead of polling.
    pub compaction_seq: Option<i64>,
}

impl PollBatch {
    /// Highest seq seen in this batch across BOTH delivered and
    /// skipped rows — the watermark callers should advance their
    /// cursor to (after applying `rows`). `None` for an empty batch.
    pub fn max_seq(&self) -> Option<i64> {
        self.rows
            .iter()
            .map(|(seq, _)| *seq)
            .chain(self.skipped.iter().copied())
            .max()
    }
}

// ── Wire types ────────────────────────────────────────────────────

/// `GET /registration` response body. Older relays also send an
/// `auth_key_b64` field — serde ignores it; we never read the MAC key
/// off the wire (it's derived locally in `RelayClient::new`).
#[derive(Debug, Deserialize)]
struct RegistrationWire {
    registered_at: i64,
    intent_b64: String,
}

impl From<RegistrationWire> for StoredRegistration {
    fn from(w: RegistrationWire) -> Self {
        let b64 = base64::engine::general_purpose::STANDARD;
        let intent = b64.decode(&w.intent_b64).unwrap_or_default();
        Self {
            registered_at: w.registered_at,
            intent,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PutResponse {
    seq: i64,
    ts: f64,
}

#[derive(Debug, Deserialize)]
struct PutSnapshotResponse {
    gc: u64,
}

#[derive(Debug, Deserialize)]
struct SnapshotsWire {
    compaction_seq: i64,
    snapshots: Vec<SnapshotEntryWire>,
}

#[derive(Debug, Deserialize)]
struct SnapshotEntryWire {
    stream_id_b64: String,
    snapshot_seq: i64,
    payload_b64: String,
}

#[derive(Debug, Deserialize)]
struct RelayOpWire {
    seq: i64,
    from_device: String,
    #[allow(dead_code)] // informational; held for future telemetry.
    ts: f64,
    payload_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OuterPayload {
    nonce: [u8; 24],
    ciphertext: Vec<u8>,
}

// ── Free helpers ──────────────────────────────────────────────────

fn base64_std(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

type HmacSha256 = Hmac<Sha256>;

/// A v2 marker inside the AEAD nonce. Eight fixed bytes leave 128 random bits,
/// while making an authenticated routing suffix mandatory for new clients.
const SNAPSHOT_V2_NONCE_PREFIX: &[u8; 8] = b"TSNAPV2\0";
const SNAPSHOT_V2_ROUTE_MAGIC: &[u8; 4] = b"TSR2";
const SNAPSHOT_V2_ROUTE_LEN: usize = 4 + 8 + 32;
const SNAPSHOT_V2_ROUTE_DOMAIN: &[u8] = b"tesela-snapshot-route-v2\0";

/// Group-only AAD for AEAD-sealed full-note snapshots. V2 deposits retain it
/// so previous clients can decrypt their legacy-compatible `OuterPayload`;
/// new clients additionally verify the appended routing record.
///
/// Distinct from [`envelope_aad`]: snapshots bind the GROUP id only (no
/// depositing-device field), because the `/snapshots` GET response does
/// not echo the depositing device, so the opener has no device id to
/// reconstruct a per-device AAD with. A domain-separation prefix keeps
/// snapshot ciphertext from being interchangeable with an envelope sealed
/// under `envelope_aad(device == "tesela-snap-v1"[..16], group)`.
fn snapshot_aad(group_id: &[u8; 16]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(b"tesela-snap-v1\0\0");
    out[16..].copy_from_slice(group_id);
    out
}

fn snapshot_route_mac(
    group_key: &GroupKey,
    group_id: &[u8; 16],
    stream_id: &[u8],
    snapshot_seq: i64,
    outer: &OuterPayload,
) -> SyncResult<HmacSha256> {
    let mut mac = HmacSha256::new_from_slice(group_key.as_bytes())
        .map_err(|e| SyncError::Crypto(format!("snapshot route key: {e}")))?;
    mac.update(SNAPSHOT_V2_ROUTE_DOMAIN);
    mac.update(group_id);
    mac.update(&(stream_id.len() as u64).to_be_bytes());
    mac.update(stream_id);
    mac.update(&snapshot_seq.to_be_bytes());
    mac.update(&outer.nonce);
    mac.update(&(outer.ciphertext.len() as u64).to_be_bytes());
    mac.update(&outer.ciphertext);
    Ok(mac)
}

fn append_snapshot_route_record(
    payload: &mut Vec<u8>,
    group_key: &GroupKey,
    group_id: &[u8; 16],
    stream_id: &[u8],
    snapshot_seq: i64,
    outer: &OuterPayload,
) -> SyncResult<()> {
    let tag = snapshot_route_mac(group_key, group_id, stream_id, snapshot_seq, outer)?
        .finalize()
        .into_bytes();
    payload.reserve(SNAPSHOT_V2_ROUTE_LEN);
    payload.extend_from_slice(SNAPSHOT_V2_ROUTE_MAGIC);
    payload.extend_from_slice(&snapshot_seq.to_be_bytes());
    payload.extend_from_slice(&tag);
    Ok(())
}

/// Open one snapshot row with authenticated routing. A v2 nonce marker makes
/// its appended route record mandatory, so stripping that record cannot turn
/// it into a valid legacy row. Unmarked rows retain the explicit migration
/// path for snapshots deposited by previous clients.
fn open_snapshot_payload(
    group_key: &GroupKey,
    group_id: &[u8; 16],
    stream_id: &[u8],
    relay_snapshot_seq: i64,
    payload: &[u8],
) -> SyncResult<Vec<u8>> {
    let (outer, remainder): (OuterPayload, &[u8]) = postcard::take_from_bytes(payload)
        .map_err(|e| SyncError::Other(format!("postcard outer: {e}")))?;
    if outer.nonce.starts_with(SNAPSHOT_V2_NONCE_PREFIX) {
        if remainder.len() != SNAPSHOT_V2_ROUTE_LEN
            || &remainder[..SNAPSHOT_V2_ROUTE_MAGIC.len()] != SNAPSHOT_V2_ROUTE_MAGIC
        {
            return Err(SyncError::Crypto(
                "snapshot routing record missing or malformed".into(),
            ));
        }
        let mut seq_bytes = [0u8; 8];
        seq_bytes.copy_from_slice(&remainder[4..12]);
        let authenticated_snapshot_seq = i64::from_be_bytes(seq_bytes);
        snapshot_route_mac(
            group_key,
            group_id,
            stream_id,
            authenticated_snapshot_seq,
            &outer,
        )?
        .verify_slice(&remainder[12..])
        .map_err(|_| SyncError::Crypto("snapshot routing authentication failed".into()))?;
        if relay_snapshot_seq != authenticated_snapshot_seq {
            tracing::debug!(
                relay_snapshot_seq,
                authenticated_snapshot_seq,
                "relay snapshot sequence differs from authenticated writer sequence"
            );
        }
    } else if !remainder.is_empty() {
        return Err(SyncError::Crypto(
            "legacy snapshot has unexpected trailing bytes".into(),
        ));
    } else {
        tracing::debug!("opening legacy snapshot without authenticated routing");
    }
    aead_open(
        group_key,
        &outer.nonce,
        &outer.ciphertext,
        &snapshot_aad(group_id),
    )
}

fn now_secs_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn net_err(ctx: &'static str) -> impl Fn(reqwest::Error) -> SyncError {
    move |e| SyncError::Other(format!("{ctx}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group::GroupId;

    #[test]
    fn outer_payload_round_trips() {
        let outer = OuterPayload {
            nonce: [0x11; 24],
            ciphertext: b"hello, opaque relay".to_vec(),
        };
        let bytes = postcard::to_allocvec(&outer).unwrap();
        let back: OuterPayload = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(back.nonce, outer.nonce);
        assert_eq!(back.ciphertext, outer.ciphertext);
    }

    #[test]
    fn client_builds_with_well_formed_inputs() {
        let url = Url::parse("https://relay.example.com").unwrap();
        let group_id = GroupId::from_bytes([0xa1; 16]);
        let device_id = DeviceId::from_bytes([0xb2; 16]);
        let group_key = GroupKey::from_bytes([0xc3; 32]);
        let _client = RelayClient::new(url, group_id, device_id, group_key);
    }

    #[test]
    fn snapshot_v2_binds_stream_and_remains_legacy_readable() {
        let url = Url::parse("https://relay.example.com").unwrap();
        let group_id = GroupId::from_bytes([0xa1; 16]);
        let device_id = DeviceId::from_bytes([0xb2; 16]);
        let group_key = GroupKey::from_bytes([0xc3; 32]);
        let client = RelayClient::new(url, group_id, device_id, group_key);
        let stream_id = b"note-a".to_vec();
        let plaintext = b"authenticated snapshot routing".to_vec();
        let entry = client
            .seal_snapshot_entries(41, &[(stream_id.clone(), plaintext.clone())])
            .unwrap()
            .pop()
            .unwrap();
        let encoded = entry.wire["payload_b64"].as_str().unwrap();
        let payload = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();

        // The legacy client path still parses the OuterPayload at byte zero,
        // ignores the authenticated routing suffix, and opens with v1 AAD.
        let legacy_outer: OuterPayload = postcard::from_bytes(&payload).unwrap();
        assert!(legacy_outer.nonce.starts_with(SNAPSHOT_V2_NONCE_PREFIX));
        assert_eq!(
            aead_open(
                &client.group_key,
                &legacy_outer.nonce,
                &legacy_outer.ciphertext,
                &snapshot_aad(client.group_id.as_bytes()),
            )
            .unwrap(),
            plaintext
        );
        assert_eq!(
            open_snapshot_payload(
                &client.group_key,
                client.group_id.as_bytes(),
                &stream_id,
                41,
                &payload,
            )
            .unwrap(),
            plaintext
        );
        assert!(
            open_snapshot_payload(
                &client.group_key,
                client.group_id.as_bytes(),
                b"note-b",
                41,
                &payload,
            )
            .is_err(),
            "a relay-swapped stream id must fail authentication"
        );
        assert_eq!(
            open_snapshot_payload(
                &client.group_key,
                client.group_id.as_bytes(),
                &stream_id,
                0,
                &payload,
            )
            .unwrap(),
            plaintext,
            "a pre-upgrade relay may store its batch covers_seq instead of the authenticated row seq"
        );

        let (_, route_suffix): (OuterPayload, &[u8]) = postcard::take_from_bytes(&payload).unwrap();
        let stripped_len = payload.len() - route_suffix.len();
        assert!(
            open_snapshot_payload(
                &client.group_key,
                client.group_id.as_bytes(),
                &stream_id,
                41,
                &payload[..stripped_len],
            )
            .is_err(),
            "the nonce marker must prevent stripping the routing record as a downgrade"
        );

        let mut tampered = payload.clone();
        tampered[stripped_len + 4] ^= 0x01;
        assert!(
            open_snapshot_payload(
                &client.group_key,
                client.group_id.as_bytes(),
                &stream_id,
                41,
                &tampered,
            )
            .is_err(),
            "the authenticated writer sequence must reject suffix tampering"
        );
    }

    #[test]
    fn legacy_snapshot_payload_remains_readable() {
        let group_id = GroupId::from_bytes([0xd1; 16]);
        let group_key = GroupKey::from_bytes([0xe2; 32]);
        let plaintext = b"legacy snapshot";
        let sealed = aead_seal(&group_key, plaintext, &snapshot_aad(group_id.as_bytes())).unwrap();
        let payload = postcard::to_allocvec(&OuterPayload {
            nonce: sealed.nonce,
            ciphertext: sealed.ciphertext,
        })
        .unwrap();

        assert_eq!(
            open_snapshot_payload(
                &group_key,
                group_id.as_bytes(),
                b"legacy-stream",
                7,
                &payload
            )
            .unwrap(),
            plaintext
        );
    }
}
