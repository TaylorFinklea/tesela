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

use crate::crypto::aead::{open as aead_open, seal as aead_seal, envelope_aad};
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
/// fields the joiner-verification path needs.
#[derive(Debug, Clone)]
pub struct StoredRegistration {
    /// Deterministic per-group auth key the relay verifies request
    /// MACs against. Joiners cross-check this matches their own
    /// HKDF derivation as a hijack/wrong-group sanity check.
    pub auth_key: [u8; 32],
    /// Wall-clock seconds at first-write registration. Stable across
    /// idempotent re-registrations; used to reconstruct the signed
    /// intent locally during joiner verification.
    pub registered_at: i64,
    /// `HMAC-SHA256(group_key, intent_msg(group_id, auth_key,
    /// registered_at))`. Joiners recompute this with their local
    /// `group_key` + verify match — that's the hijack-detection
    /// invariant the design rests on.
    pub intent: Vec<u8>,
}

impl RelayClient {
    /// Build a new client. Does no I/O — call `register()` /
    /// `verify_registration()` explicitly so callers control error
    /// reporting + retry policy.
    pub fn new(
        base_url: Url,
        group_id: GroupId,
        device_id: DeviceId,
        group_key: GroupKey,
    ) -> Self {
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
                let intent_text =
                    intent_msg(&self.group_id, &self.auth_key, stored.registered_at);
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
        let intent_text = intent_msg(&self.group_id, &self.auth_key, stored.registered_at);
        if !verify_intent(&self.group_key, &intent_text, &stored.intent) {
            return Err(SyncError::Crypto(
                "relay registration intent does not verify under our group_key — HIJACKED. \
                 Use admin recovery to delete the bogus registration and re-pair."
                    .into(),
            ));
        }
        if stored.auth_key != self.auth_key {
            return Err(SyncError::Crypto(
                "relay-stored auth_key disagrees with our derivation — wrong group_key, \
                 wrong group_id, or hijacked relay state."
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
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| SyncError::Other(format!("json body: {e}")))?;
        let path = format!("/groups/{}/ops", hex::encode(self.group_id.as_bytes()));
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
        let nonce_b64 = self.fresh_nonce_b64();
        let ts = now_secs_i64();
        let canonical =
            canonical_request("PUT", &path, "", &nonce_b64, ts, &body_hash_hex(&body_bytes));
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
    /// the caller's cursor. Each tuple is `(seq, envelope)` — the
    /// caller advances its cursor to the highest seq + calls `ack`
    /// once the SyncEngine has applied them.
    pub async fn poll(&self, since: i64) -> SyncResult<Vec<(i64, SyncEnvelope)>> {
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
            return Err(SyncError::Crypto(format!("poll: relay returned {}", resp.status())));
        }
        let rows: Vec<RelayOpWire> = resp.json().await.map_err(net_err("poll response body"))?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
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
            let envelope = SyncEnvelope {
                from_device: DeviceId::from_bytes(from_device_arr),
                to_group: self.group_id,
                nonce: outer.nonce,
                ciphertext: plaintext,
            };
            out.push((row.seq, envelope));
        }
        Ok(out)
    }

    /// Tell the relay "this device has applied every op up to and
    /// including `applied_seq`". Drives server-side GC.
    pub async fn ack(&self, applied_seq: i64) -> SyncResult<()> {
        let body = serde_json::json!({
            "device": hex::encode(self.device_id.as_bytes()),
            "applied_seq": applied_seq,
        });
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| SyncError::Other(format!("json body: {e}")))?;
        let path = format!("/groups/{}/ack", hex::encode(self.group_id.as_bytes()));
        let url = self
            .base_url
            .join(&path)
            .map_err(|e| SyncError::Other(format!("url join: {e}")))?;
        let nonce_b64 = self.fresh_nonce_b64();
        let ts = now_secs_i64();
        let canonical =
            canonical_request("POST", &path, "", &nonce_b64, ts, &body_hash_hex(&body_bytes));
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
            return Err(SyncError::Crypto(format!("ack: relay returned {}", resp.status())));
        }
        Ok(())
    }

    // ── Helpers ────────────────────────────────────────────────────

    fn group_url(&self, suffix: &str) -> Url {
        let path = format!("/groups/{}{}", hex::encode(self.group_id.as_bytes()), suffix);
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

// ── Wire types ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RegistrationWire {
    auth_key_b64: String,
    registered_at: i64,
    intent_b64: String,
}

impl From<RegistrationWire> for StoredRegistration {
    fn from(w: RegistrationWire) -> Self {
        let b64 = base64::engine::general_purpose::STANDARD;
        let auth_key = b64.decode(&w.auth_key_b64).unwrap_or_default();
        let intent = b64.decode(&w.intent_b64).unwrap_or_default();
        let auth_key_arr: [u8; 32] = auth_key.try_into().unwrap_or([0u8; 32]);
        Self {
            auth_key: auth_key_arr,
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
