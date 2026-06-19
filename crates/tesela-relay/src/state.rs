//! Shared application state — clone-cheap, immutable after startup.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::apns::Apns;
use crate::store::Store;

/// Window of nonces seen recently per `(group_id, nonce)`. Anything
/// older than `NONCE_TTL` is pruned on lookup; max in-memory
/// footprint is bounded by request rate × TTL.
pub(crate) const NONCE_TTL: Duration = Duration::from_secs(300);

/// Per-IP rate limit: at most `RATE_LIMIT_MAX` requests in
/// `RATE_LIMIT_WINDOW`. Defense against scan-and-flood — legit clients
/// won't approach this.
pub(crate) const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(10);
pub(crate) const RATE_LIMIT_MAX: usize = 1_000;

/// Tracks `(group_id, nonce_b64) -> seen_at` for replay protection.
/// Lock contention is fine — this is microsecond work per lookup.
pub(crate) type NonceCache = Arc<Mutex<HashMap<(Vec<u8>, String), Instant>>>;

/// Sliding-window per-IP request counter — VecDeque of request
/// timestamps, prune older-than-window on each check.
pub(crate) type IpRateCache = Arc<Mutex<HashMap<IpAddr, VecDeque<Instant>>>>;

/// Cloneable handle holding everything the request handlers need.
/// Wrapped in `Arc` so handlers can share without per-request locking
/// on the inner state.
#[derive(Clone)]
pub struct AppState {
    // Read in stages 3a-3d once handlers grow past the health probe;
    // committing the field up front so the AppState shape is stable
    // before handlers depend on it.
    #[allow(dead_code)]
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) store: Store,
    pub(crate) max_body: usize,
    /// Set iff `--admin-token` was passed. `None` means admin
    /// endpoints are disabled and respond `404`.
    pub(crate) admin_token: Option<String>,
    /// Replay-window nonce dedupe. See `NONCE_TTL`.
    pub(crate) nonces: NonceCache,
    /// Per-IP rate-limit counters. See `RATE_LIMIT_*`.
    pub(crate) ip_rates: IpRateCache,
    /// APNs push sender, iff all `APNS_*` config was supplied + the `.p8`
    /// parsed. `None` = the relay sends no silent pushes (default).
    pub(crate) apns: Option<Arc<Apns>>,
}

impl AppState {
    /// Open a relay state against the given SQLite path. Used by both
    /// the binary (`main.rs`, real config from CLI args) and integration
    /// tests (random tmp paths).
    pub async fn open(
        db_path: &Path,
        max_body: usize,
        admin_token: Option<String>,
    ) -> Result<Self> {
        Self::open_with_apns(db_path, max_body, admin_token, None).await
    }

    /// Like [`open`](Self::open) but also wires an APNs sender (sync
    /// durability P3c). `main.rs` builds the sender from `APNS_*` config
    /// and passes it here; tests + every other caller use `open` (no
    /// push). Kept as a separate constructor so the common 3-arg
    /// signature stays stable across the test suite.
    pub async fn open_with_apns(
        db_path: &Path,
        max_body: usize,
        admin_token: Option<String>,
        apns: Option<Arc<Apns>>,
    ) -> Result<Self> {
        let store = Store::open(db_path).await?;
        Ok(Self {
            inner: Arc::new(Inner {
                store,
                max_body,
                admin_token,
                nonces: Arc::new(Mutex::new(HashMap::new())),
                ip_rates: Arc::new(Mutex::new(HashMap::new())),
                apns,
            }),
        })
    }

    /// Check + record a request-window nonce. `true` means this nonce
    /// is fresh; `false` means it was already used inside `NONCE_TTL`
    /// and the request must be rejected as a replay.
    pub(crate) fn record_nonce(&self, group_id: &[u8; 16], nonce_b64: &str) -> bool {
        let key = (group_id.to_vec(), nonce_b64.to_string());
        let mut g = self.inner.nonces.lock().expect("nonce mutex poisoned");
        let now = Instant::now();
        // Best-effort prune anything outside the window so the map
        // doesn't grow unbounded under sustained traffic.
        g.retain(|_, seen_at| now.duration_since(*seen_at) < NONCE_TTL);
        match g.entry(key) {
            std::collections::hash_map::Entry::Occupied(_) => false,
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(now);
                true
            }
        }
    }

    /// Per-IP rate gate — `false` means this IP has exceeded the
    /// window cap and the request should be refused (429). Prunes
    /// timestamps outside the window on each check.
    pub(crate) fn check_ip_rate(&self, ip: IpAddr) -> bool {
        let mut g = self.inner.ip_rates.lock().expect("ip rate mutex poisoned");
        let now = Instant::now();
        let entry = g.entry(ip).or_default();
        while let Some(front) = entry.front() {
            if now.duration_since(*front) >= RATE_LIMIT_WINDOW {
                entry.pop_front();
            } else {
                break;
            }
        }
        if entry.len() >= RATE_LIMIT_MAX {
            return false;
        }
        entry.push_back(now);
        true
    }
}
