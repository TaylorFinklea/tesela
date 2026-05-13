use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast;

use tesela_core::{db::SqliteIndex, storage::filesystem::FsNoteStore, types::TypeRegistry, Note};
use tesela_sync::{GroupIdentity, LanDiscovery, SqliteEngine};
use tokio::sync::RwLock;

use crate::reminders::auto::AutoSync;

pub struct AppState {
    pub mosaic_root: PathBuf,
    pub store: Arc<FsNoteStore>,
    pub index: Arc<SqliteIndex>,
    pub ws_tx: broadcast::Sender<WsEvent>,
    pub type_registry: TypeRegistry,
    pub auto_sync: Arc<AutoSync>,
    /// Phase 1.5 multi-device sync engine. Records every local note
    /// write to the oplog and applies remote envelopes from peers.
    pub sync_engine: Arc<SqliteEngine>,
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
