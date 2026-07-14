use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast;

use tesela_core::{db::SqliteIndex, storage::filesystem::FsNoteStore, types::TypeRegistry, Note};
use tesela_sync::{GroupId, GroupIdentity, LanDiscovery, SyncEngine, ViewRecord};
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
    /// Set atomically with publication of a newly adopted group identity.
    /// The current process still owns relay/bootstrap handles captured for the
    /// old group, so HTTP data-plane work fails closed until process restart.
    pub group_transition_pending_restart: AtomicBool,
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
    /// Scheduled-backup state (last run, next run, cadence) shared with
    /// the scheduler task. `GET /backup/status` reads through this and
    /// combines it with the on-disk backup listing.
    pub backup_status: crate::backup_scheduler::BackupStatusHandle,
}

/// Unique id assigned to each upgraded `/ws` socket, used to suppress
/// echoing a delta back to the connection it arrived on.
pub type ConnId = u64;

/// An exact, non-serializable identity token for one live group runtime.
///
/// `group_id` alone is insufficient: adopting a rotated key for the same id
/// must retire every daemon and socket that captured the old key. The key
/// bytes stay private and the custom `Debug` implementation never prints
/// them.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GroupScope {
    group_id: GroupId,
    group_key: [u8; 32],
}

impl GroupScope {
    pub fn capture(identity: &GroupIdentity) -> Self {
        Self {
            group_id: identity.group_id,
            group_key: *identity.group_key.as_bytes(),
        }
    }

    pub fn group_id(self) -> GroupId {
        self.group_id
    }

    pub fn matches(self, identity: &GroupIdentity) -> bool {
        self.group_id == identity.group_id && self.group_key == *identity.group_key.as_bytes()
    }
}

impl std::fmt::Debug for GroupScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GroupScope")
            .field("group_id", &self.group_id)
            .finish_non_exhaustive()
    }
}

/// Lease fence for a daemon that captured one group identity at boot.
///
/// Every identity-sensitive operation runs while holding the shared read
/// lease. Group adoption takes the matching write lease, so it waits for an
/// active operation to finish. Once the replacement is published, this fence
/// refuses to poll any more old-group work.
#[derive(Clone)]
pub(crate) struct GroupRuntimeFence {
    current: Arc<RwLock<GroupIdentity>>,
    captured: GroupScope,
}

impl GroupRuntimeFence {
    pub(crate) fn capture(current: Arc<RwLock<GroupIdentity>>, captured: &GroupIdentity) -> Self {
        Self {
            current,
            captured: GroupScope::capture(captured),
        }
    }

    pub(crate) fn scope(&self) -> GroupScope {
        self.captured
    }

    pub(crate) async fn run_if_current<F>(&self, operation: F) -> Option<F::Output>
    where
        F: Future,
    {
        let current = self.current.read().await;
        if !self.captured.matches(&current) {
            return None;
        }
        Some(operation.await)
    }
}

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
    /// Exact source identity for boot-captured daemon/socket frames. `None`
    /// is reserved for HTTP work already serialized by the request's group
    /// read lease; broadcasts are not replayed to later subscribers.
    pub source_group: Option<GroupScope>,
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
    /// Saved-views registry changed (create/update/delete/reorder via the
    /// `/views` routes — saved-views spec 2026-06-10). Carries the full
    /// ordered registry so clients refresh the view switcher without a
    /// refetch, mirroring how `NoteUpdated` carries the whole note.
    ViewsChanged {
        views: Vec<ViewRecord>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::oneshot;

    fn identity(group: u8, key: u8) -> GroupIdentity {
        GroupIdentity {
            group_id: tesela_sync::GroupId::from_bytes([group; 16]),
            group_key: tesela_sync::GroupKey::from_bytes([key; 32]),
        }
    }

    #[tokio::test]
    async fn replacement_waits_for_active_group_tick_and_stale_daemons_cannot_publish() {
        let current = Arc::new(RwLock::new(identity(0x11, 0x22)));
        let fence = GroupRuntimeFence::capture(Arc::clone(&current), &identity(0x11, 0x22));
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let applied = Arc::new(AtomicBool::new(false));
        let applied_by_tick = Arc::clone(&applied);
        let tick_fence = fence.clone();
        let tick = tokio::spawn(async move {
            tick_fence
                .run_if_current(async move {
                    entered_tx.send(()).unwrap();
                    release_rx.await.unwrap();
                    applied_by_tick.store(true, Ordering::SeqCst);
                })
                .await
        });

        entered_rx.await.unwrap();
        let replacement_identity = Arc::clone(&current);
        let mut replacement = tokio::spawn(async move {
            *replacement_identity.write().await = identity(0x33, 0x44);
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), &mut replacement)
                .await
                .is_err(),
            "group replacement must wait for the complete active relay tick"
        );

        release_tx.send(()).unwrap();
        assert_eq!(tick.await.unwrap(), Some(()));
        assert!(applied.load(Ordering::SeqCst));
        replacement.await.unwrap();

        let stale_relay_applied = AtomicBool::new(false);
        assert_eq!(
            fence
                .run_if_current(async {
                    stale_relay_applied.store(true, Ordering::SeqCst);
                })
                .await,
            None,
            "an old relay daemon stops after its captured identity is replaced"
        );
        assert!(!stale_relay_applied.load(Ordering::SeqCst));

        let stale_presence_published = AtomicBool::new(false);
        assert_eq!(
            fence
                .run_if_current(async {
                    stale_presence_published.store(true, Ordering::SeqCst);
                })
                .await,
            None,
            "an old presence bridge cannot publish into the replacement group"
        );
        assert!(!stale_presence_published.load(Ordering::SeqCst));
    }
}
