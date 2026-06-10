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
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};

use crate::crypto::aead::{envelope_aad, open as aead_open, seal as aead_seal};
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
        let body = serde_json::json!({
            "auth_key_b64": base64_std(&self.auth_key),
            "registered_at": registered_at,
            "intent_b64": base64_std(&intent),
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
            s => Err(SyncError::Crypto(format!("register: relay returned {s}"))),
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
        let rows: Vec<RelayOpWire> = resp.json().await.map_err(net_err("poll response body"))?;
        let mut out = PollBatch {
            rows: Vec::with_capacity(rows.len()),
            skipped: Vec::new(),
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
        // Snapshots are sealed under a GROUP-only AAD (not the per-device
        // envelope AAD): unlike `/ops`, the `/snapshots` GET response does
        // not echo a depositing-device field, so the opener can't
        // reconstruct a per-device AAD. Binding only the group id lets any
        // group member open any member's deposited snapshot, while still
        // authenticating the ciphertext to this group's key.
        let aad = snapshot_aad(self.group_id.as_bytes());
        let mut entries = Vec::with_capacity(snapshots.len());
        for (stream_id, plaintext) in &snapshots {
            let sealed = aead_seal(&self.group_key, plaintext, &aad)?;
            let outer = OuterPayload {
                nonce: sealed.nonce,
                ciphertext: sealed.ciphertext,
            };
            let outer_bytes = postcard::to_allocvec(&outer)
                .map_err(|e| SyncError::Other(format!("postcard serialize outer: {e}")))?;
            entries.push(serde_json::json!({
                "stream_id_b64": base64_std(stream_id),
                "payload_b64": base64_std(&outer_bytes),
            }));
        }
        let body = serde_json::json!({
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
        Ok(ack.gc)
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
        // Snapshots are sealed under the GROUP-only AAD by `put_snapshots`
        // (the GET response carries no depositing-device field), so any
        // group member opens them with the same group-bound AAD.
        let b64 = base64::engine::general_purpose::STANDARD;
        let mut out = Vec::with_capacity(wire.snapshots.len());
        for entry in wire.snapshots {
            let stream_id = b64
                .decode(&entry.stream_id_b64)
                .map_err(|e| SyncError::Other(format!("stream_id base64: {e}")))?;
            let outer_bytes = b64
                .decode(&entry.payload_b64)
                .map_err(|e| SyncError::Other(format!("payload base64: {e}")))?;
            let outer: OuterPayload = postcard::from_bytes(&outer_bytes)
                .map_err(|e| SyncError::Other(format!("postcard outer: {e}")))?;
            let aad = snapshot_aad(self.group_id.as_bytes());
            let plaintext = aead_open(&self.group_key, &outer.nonce, &outer.ciphertext, &aad)?;
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

/// AAD for AEAD-sealed full-note snapshots pushed via `put_snapshots`.
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
}
