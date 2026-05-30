use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast;

use tesela_core::{db::SqliteIndex, storage::filesystem::FsNoteStore, types::TypeRegistry, Note};
use tesela_sync::{GroupIdentity, LanDiscovery, SyncEngine};
use tokio::sync::RwLock;

use crate::reminders::auto::AutoSync;

pub struct AppState {
    pub mosaic_root: PathBuf,
    pub store: Arc<FsNoteStore>,
    pub index: Arc<SqliteIndex>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    /// Instant-multidevice (Phase A) — a SEPARATE binary broadcast channel
    /// for Loro delta frames, distinct from `ws_tx` (which is text-JSON
    /// only). iOS UTF-8-decodes every `ws_tx` frame as JSON and would
    /// silently drop a binary frame mixed onto that channel (spec finding
    /// #2), so live deltas ride here. Each frame is a `TLR2`-encoded
    /// `Vec<LoroDocUpdate>` plus an origin connection id used for
    /// echo-suppression (a delta is never sent back to the socket it
    /// arrived on; frames from the HTTP/relay path carry `origin: None` and
    /// fan out to everyone).
    pub ws_delta_tx: broadcast::Sender<WsDelta>,
    /// Monotonic source of per-connection ids for echo-suppression on
    /// `ws_delta_tx`. Each upgraded `/ws` socket claims one id at connect.
    pub ws_conn_seq: AtomicU64,
    pub type_registry: TypeRegistry,
    pub auto_sync: Arc<AutoSync>,
    /// Phase 1.5 multi-device sync engine. Records every local note
    /// write to the oplog and applies remote envelopes from peers.
    ///
    /// Held as `Arc<dyn SyncEngine>` so the server can run with either
    /// the canonical `SqliteEngine` (default) or a `DualEngine` wrapper
    /// during the Loro migration (decisions.md 2026-05-27). Routes call
    /// trait methods only; engine-specific surfaces live behind the
    /// concrete type and are reached via downcasting only when
    /// strictly necessary (currently nowhere).
    pub sync_engine: Arc<dyn SyncEngine>,
    /// Phase 2.1 mDNS-based LAN peer discovery. `None` if discovery was
    /// disabled or failed to start (we log and continue, since sync over
    /// manually-configured peers still works).
    pub lan_discovery: Option<Arc<LanDiscovery>>,
    /// Phase 2.2 — the symmetric group identity (id + key) used by the
    /// pairing flow. Wrapped in RwLock so `POST /sync/peer/pair-code`
    /// can swap it after a successful pair without restarting the
    /// server. Cleartext sync continues to function while the pending
    /// AEAD slice is unwritten.
    pub group_identity: Arc<RwLock<GroupIdentity>>,
    /// A human-readable display name advertised over mDNS and embedded
    /// in pairing codes. Captured once at startup.
    pub display_name: String,
    /// The reachable HTTP URL we hand to joining devices in pairing
    /// codes. `http://<lan-ip-or-bind-host>:<port>`.
    pub public_url: String,
    /// URL of the user-configured sync relay, if any. `None` means
    /// LAN-only sync. When set, pairing codes carry this URL so a
    /// joining device auto-configures the same relay without an
    /// extra copy-paste step. Populated from `[sync.relay] url = "…"`
    /// in the mosaic config (stage 5b).
    pub relay_url: Option<String>,
    /// Live runtime handle for the relay daemon — `None` when no
    /// relay is configured OR bring-up failed (the daemon retries
    /// on its tick). The status endpoint reads through this.
    pub relay: Option<crate::sync_relay::RelayHandle>,
}

/// Unique id assigned to each upgraded `/ws` socket, used to suppress
/// echoing a delta back to the connection it arrived on.
pub type ConnId = u64;

/// A Loro delta frame fanned out on [`AppState::ws_delta_tx`].
///
/// `frame` is the `TLR2`-encoded `Vec<LoroDocUpdate>` (the same bytes the
/// relay and the iOS FFI exchange). `origin` is the connection id of the
/// socket the delta arrived on, or `None` for HTTP/relay-originated deltas
/// that should fan out to every connected socket. The per-socket send loop
/// skips any frame whose `origin` equals its own id (echo-suppression;
/// spec finding #4) — Loro apply is idempotent so a stray echo is harmless,
/// but suppressing it keeps the fan-out finite and loop-free.
#[derive(Debug, Clone)]
pub struct WsDelta {
    pub origin: Option<ConnId>,
    pub frame: Vec<u8>,
}

/// Events broadcast to WebSocket clients when notes change.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum WsEvent {
    NoteCreated {
        note: Note,
    },
    NoteUpdated {
        note: Note,
    },
    NoteDeleted {
        id: String,
    },
    /// Phase 12.3 — fired once per (block, deadline) when the configured
    /// lead time is reached and the task is still open. Client decides
    /// whether to surface a desktop notification.
    DeadlineApproaching {
        block_id: String,
        title: String,
        note_id: String,
        deadline_iso: String,
        lead_minutes: i64,
    },
    /// Phase 12.3 — fired when `scheduled::` time-of-day is reached.
    ScheduledFires {
        block_id: String,
        title: String,
        note_id: String,
        scheduled_iso: String,
    },
    /// Phase 12.3 — fired when a recurring task auto-bumps to the next
    /// occurrence (so the user sees "rolled to today" in passing).
    RecurringRolled {
        block_id: String,
        title: String,
        note_id: String,
        next_deadline: String,
    },
}
