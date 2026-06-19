//! APNs silent-push proxy (sync durability P3c).
//!
//! Sends a single content-available ("background") push to one device
//! token so a suspended iOS device wakes and drains pending ops from the
//! relay. This is the Rust port of the Cloudflare Worker's `apns.ts` —
//! the SAME JWT structure, request headers, and body, so either relay
//! drives the same iOS receiver (the `register_device` POST that iOS
//! build 36 already ships authenticates here unchanged).
//!
//! Zero-knowledge: the push carries NO note content (only
//! `content-available: 1`) — it just nudges the device to pull the
//! already-AEAD-sealed ops it would have polled for anyway.
//!
//! Best-effort: a single failed push is not a sync failure (the device
//! still polls / runs its BGProcessingTask), so nothing here returns an
//! error into the deposit path — it logs at routing level and returns a
//! bool. The whole feature is optional: with any `APNS_*` value unset,
//! `from_config` returns `None` and the relay runs exactly as before.

use std::sync::Mutex;
use std::time::Duration;

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;

const DEFAULT_HOST: &str = "https://api.push.apple.com";
/// Apple allows provider-token reuse up to ~60 min and rejects more than
/// one mint per 20 min (`TooManyProviderTokenUpdates`), so cache ~50 min.
const JWT_TTL_SECS: i64 = 50 * 60;

/// A configured APNs sender. Built ONCE at startup (a bad `.p8` fails
/// there, never per-push) and shared via `AppState`. Holds a pooled
/// HTTP/2 client to APNs + the parsed ES256 signing key + a cached
/// provider token.
pub struct Apns {
    http: reqwest::Client,
    encoding_key: EncodingKey,
    key_id: String,
    team_id: String,
    bundle_id: String,
    host: String,
    /// Cached provider-token JWT + its expiry (unix secs).
    jwt_cache: Mutex<Option<(String, i64)>>,
}

#[derive(Serialize)]
struct Claims {
    iss: String,
    iat: i64,
}

impl Apns {
    /// Build an ENABLED sender only if all four `APNS_*` values are
    /// present AND the `.p8` parses; otherwise `None` (the relay then
    /// runs with no push, exactly as before — never an error).
    ///
    /// `key_p8` is auto-detected: inline PEM if it starts with
    /// `-----BEGIN`, otherwise a filesystem path to the `.p8` (the
    /// HA-add-on-friendly form — mount the file, point the env at it).
    /// Never panics; never logs the key bytes.
    pub fn from_config(
        key_p8: Option<String>,
        key_id: Option<String>,
        team_id: Option<String>,
        bundle_id: Option<String>,
        host: Option<String>,
    ) -> Option<Self> {
        let nonempty = |s: Option<String>| s.filter(|v| !v.trim().is_empty());
        let (key_p8, key_id, team_id, bundle_id) = match (
            nonempty(key_p8),
            nonempty(key_id),
            nonempty(team_id),
            nonempty(bundle_id),
        ) {
            (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
            _ => return None,
        };

        let pem: Vec<u8> = if key_p8.trim_start().starts_with("-----BEGIN") {
            key_p8.into_bytes()
        } else {
            match std::fs::read(&key_p8) {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::warn!("APNs: cannot read key file {key_p8}: {e}; push disabled");
                    return None;
                }
            }
        };
        let encoding_key = match EncodingKey::from_ec_pem(&pem) {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!("APNs: invalid .p8 EC key: {e}; push disabled");
                return None;
            }
        };
        let http = match reqwest::Client::builder()
            // Bound a hung APNs POST so a stalled push can never wedge the
            // background task that drives it. HTTP/2 (the only protocol
            // APNs speaks) is negotiated via ALPN by the http2 feature.
            .timeout(Duration::from_secs(10))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("APNs: HTTP client build failed: {e}; push disabled");
                return None;
            }
        };

        Some(Self {
            http,
            encoding_key,
            key_id,
            team_id,
            bundle_id,
            host: nonempty(host).unwrap_or_else(|| DEFAULT_HOST.to_string()),
            jwt_cache: Mutex::new(None),
        })
    }

    /// Mint-or-reuse the ES256 provider token. `jsonwebtoken` emits the
    /// JWS with the ES256 signature already in JOSE raw-`r||s` form — the
    /// same structure the Worker hand-rolls via WebCrypto.
    fn provider_token(&self) -> Result<String, jsonwebtoken::errors::Error> {
        let now = chrono::Utc::now().timestamp();
        {
            let cache = self.jwt_cache.lock().expect("apns jwt cache poisoned");
            if let Some((tok, exp)) = cache.as_ref() {
                if now < *exp {
                    return Ok(tok.clone());
                }
            }
        }
        let mut header = Header::new(Algorithm::ES256);
        // Drop the default `typ: "JWT"` so the header is byte-identical to
        // the CF Worker's {alg, kid} (Apple tolerates typ, but parity keeps
        // the two relays interchangeable).
        header.typ = None;
        header.kid = Some(self.key_id.clone());
        let claims = Claims {
            iss: self.team_id.clone(),
            iat: now,
        };
        let token = encode(&header, &claims, &self.encoding_key)?;
        *self.jwt_cache.lock().expect("apns jwt cache poisoned") =
            Some((token.clone(), now + JWT_TTL_SECS));
        Ok(token)
    }

    /// Send ONE content-available push to `device_token_hex`. Returns
    /// `true` on a 2xx from APNs, `false` on any failure (config, crypto,
    /// network, non-2xx). Never panics. Logs at routing level only — a
    /// token PREFIX, the status, and Apple's `reason` code — never the
    /// full token, the JWT, or the key; the push body has no note content.
    pub async fn send_background_push(&self, device_token_hex: &str) -> bool {
        let tag = &device_token_hex[..device_token_hex.len().min(8)];
        let token = match self.provider_token() {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("[apns] push {tag}… jwt error: {e}");
                return false;
            }
        };
        let url = format!("{}/3/device/{}", self.host, device_token_hex);
        let res = self
            .http
            .post(&url)
            .header("authorization", format!("bearer {token}"))
            .header("apns-topic", &self.bundle_id)
            .header("apns-push-type", "background")
            .header("apns-priority", "5")
            .header("apns-expiration", "0")
            .header("content-type", "application/json")
            .body(r#"{"aps":{"content-available":1}}"#)
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => {
                tracing::info!("[apns] push {tag}… → {} OK", r.status().as_u16());
                true
            }
            Ok(r) => {
                let status = r.status().as_u16();
                // APNs returns {"reason":"BadDeviceToken"|...} on failure —
                // an error CODE, never note content. Surfacing it is what
                // makes the relay logs diagnostic.
                let reason = r
                    .text()
                    .await
                    .ok()
                    .and_then(|b| serde_json::from_str::<serde_json::Value>(&b).ok())
                    .and_then(|v| {
                        v.get("reason")
                            .and_then(|x| x.as_str().map(str::to_string))
                    })
                    .unwrap_or_default();
                tracing::warn!(
                    "[apns] push {tag}… → {status} FAIL reason={}",
                    if reason.is_empty() {
                        "?".to_string()
                    } else {
                        reason
                    }
                );
                false
            }
            Err(e) => {
                tracing::error!("[apns] push {tag}… error: {e}");
                false
            }
        }
    }
}
