//! `tesela-server` as a linkable library (L4). The HTTP server's whole boot
//! sequence lives in [`serve`], so it can run either as the standalone
//! `tesela-server` binary (thin `src/main.rs`) or **in-process** on an
//! embedder's tokio runtime (the desktop Tauri shell) without spawning a
//! child. Mirrors the `tesela-relay` lib+bin split.

pub mod backup_scheduler;
pub mod error;
pub mod notifications;
pub mod presence_relay;
pub mod reminders;
pub mod routes;
pub mod state;
pub mod sync_relay;

use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::broadcast;
use tracing::{info, warn};

use tesela_core::{
    config::{Config, ServerConfig},
    db::SqliteIndex,
    indexer::{Indexer, NoteEvent},
    link::extract_wiki_links,
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    types::TypeRegistry,
};
use tesela_sync::{DeviceId, LanDiscovery, SyncEngine};
use tokio::sync::RwLock;

use reminders::auto::AutoSync;
use state::{AppState, WsEvent};

/// Everything [`serve`] needs that isn't read from the process environment.
///
/// Kept deliberately small: the bin and the desktop embed both still drive
/// bind address / static dir / `TESELA_DISABLE_*` through env vars (the embed
/// sets them in-process before calling `serve`, exactly as the old child-spawn
/// path set them on the child), so only the mosaic — which the bin resolves
/// from `--mosaic` or the cwd-walk — needs to be passed explicitly.
pub struct ServeConfig {
    pub mosaic: PathBuf,
}

impl ServeConfig {
    /// Resolve the mosaic exactly as the standalone bin does: an explicit
    /// path (validated to be a real mosaic) takes precedence, else the
    /// `find_mosaic` cwd-walk / env / saved-config lookup.
    pub fn resolve(explicit: Option<PathBuf>) -> Result<Self> {
        let mosaic = match explicit {
            Some(p) => {
                if !p.join(".tesela").exists() {
                    anyhow::bail!(
                        "--mosaic {} is not a mosaic directory (no .tesela/ found)",
                        p.display()
                    );
                }
                p
            }
            None => find_mosaic()?,
        };
        Ok(Self { mosaic })
    }

    /// `resolve(None)` — the env/cwd-driven default.
    pub fn from_env() -> Result<Self> {
        Self::resolve(None)
    }
}

/// Boot and run the Tesela HTTP server until `shutdown` resolves.
///
/// This is the whole former `main()` body, minus arg parsing. It is a plain
/// `async fn` (NOT `#[tokio::main]`) so an embedder drives it on its OWN tokio
/// runtime — nesting a runtime would panic. The single-writer flock is held in
/// a local guard for the entire call, so the data-safety invariant holds for
/// the server's whole lifetime and releases when `serve` returns.
///
/// `on_bound` is invoked once with the actual bound `SocketAddr` right after
/// the listener binds (before the serve loop), so an embedder can read the
/// real port — handy when binding `127.0.0.1:0` — and build its webview URL
/// while `serve` keeps running. The standalone bin passes a no-op.
///
/// CAVEAT (resolved in L4 Phase B): the background daemons (sync, relay tick,
/// reminders, notifications, backup scheduler) are detached `tokio::spawn`
/// tasks with no shutdown handle — they currently rely on the PROCESS ending
/// to stop. That's correct for the standalone bin (serve returning ⇒ `main`
/// returns ⇒ exit) but an in-process embedder that calls `serve` more than once
/// in a long-lived process would leak them; the Tauri cutover adds a
/// `CancellationToken` that stops them when `serve` returns.
pub async fn serve(
    config: ServeConfig,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
    on_bound: impl FnOnce(std::net::SocketAddr),
) -> Result<()> {
    // One-shot config migration: older builds wrote config to
    // ~/Library/Application Support/tesela/config.toml on macOS via
    // `dirs::config_dir()`. New default is the XDG path. If the new
    // path is empty but the old one is populated, move it.
    if let Ok(Some(moved_to)) = Config::migrate_legacy_config() {
        info!("Migrated user config to XDG path: {}", moved_to.display());
    }

    let mosaic = config.mosaic;

    // Single-writer guard: only ONE tesela-server may write a mosaic at a time.
    // Held for the whole process lifetime (dropped on return / released by the
    // OS on exit). A second server — a double-launched desktop app, or a
    // standalone server racing the embedded one — fails fast here instead of
    // becoming a second writer and corrupting the Loro state.
    let _mosaic_lock = match acquire_mosaic_lock(&mosaic) {
        Ok(lock) => lock,
        Err(e) => {
            anyhow::bail!(
                "could not acquire the single-writer lock on {}: {e}. \
                 Another tesela-server (the desktop app, or a standalone server) \
                 is already using this mosaic — close it first.",
                mosaic.display()
            );
        }
    };

    // Idempotent system-widget backfill: every mosaic startup ensures
    // the default rail widgets exist. Catches the case where a mosaic
    // was created on an older binary (no seed call) or the user
    // deleted a widget. Won't overwrite user edits — `seed` only
    // creates missing files.
    match tesela_core::system_widgets::seed(&mosaic) {
        Ok(0) => {}
        Ok(n) => info!(
            "Seeded {} missing system widget(s) in {}",
            n,
            mosaic.display()
        ),
        Err(e) => warn!("System widget seed failed at {}: {}", mosaic.display(), e),
    }

    // Stamp persistent block ids on existing .md files so block-level
    // sync has stable identifiers to diff against. Idempotent — files
    // that already have bids are not touched. Runs before the indexer
    // boots so the watcher's first scan sees the canonical (stamped)
    // content.
    match tesela_core::note_tree::stamp_existing_notes(&mosaic.join("notes")).await {
        Ok(0) => {}
        Ok(n) => info!("Stamped block ids on {} note(s)", n),
        Err(e) => warn!("Block-id stamping failed: {}", e),
    }

    let config = load_config(&mosaic);
    let db_path = mosaic.join(".tesela").join("tesela.db");
    let notes_dir = mosaic.join("notes");
    let type_registry = TypeRegistry::load(&mosaic);
    info!("Loaded {} type definitions", type_registry.types.len());

    let store = Arc::new(FsNoteStore::new(mosaic.clone(), config.storage.clone()));
    let index = Arc::new(SqliteIndex::open(&db_path).await?);

    // Wire up the Indexer (same as TUI)
    let store_dyn: Arc<dyn NoteStore> = Arc::clone(&store) as Arc<dyn NoteStore>;
    let index_dyn: Arc<dyn SearchIndex> = Arc::clone(&index) as Arc<dyn SearchIndex>;
    let graph_dyn: Arc<dyn LinkGraph> = Arc::clone(&index) as Arc<dyn LinkGraph>;

    // WebSocket broadcast channel (text JSON WsEvents).
    let (ws_tx, _) = broadcast::channel::<WsEvent>(64);

    // Instant-multidevice (Phase A) — separate binary channel carrying
    // `TLR2`-framed Loro deltas, kept distinct from the text-only `ws_tx`
    // (spec finding #2).
    let (ws_delta_tx, _) = broadcast::channel::<state::WsDelta>(64);

    // Indexer notify channel — maps file-system events to WsEvents
    let (note_event_tx, _) = broadcast::channel::<NoteEvent>(64);

    let indexer =
        Indexer::new(store_dyn, index_dyn, graph_dyn).with_notify_tx(note_event_tx.clone());

    // Auto-create tag pages for any tags that don't have a corresponding page
    {
        let all_notes = store.list(None, usize::MAX, 0).await?;
        let existing_ids: std::collections::HashSet<String> = all_notes
            .iter()
            .map(|n| n.id.as_str().to_lowercase())
            .collect();
        for note in &all_notes {
            for tag in &note.metadata.tags {
                if tag == "daily" {
                    continue;
                }
                let tag_lower = tag.to_lowercase();
                if !existing_ids.contains(&tag_lower) {
                    let content = format!(
                        "---\ntitle: \"{}\"\ntype: \"Tag\"\nextends: \"Root Tag\"\ntag_properties: []\ntags: []\n---\n- Tag properties are inherited by all nodes using the tag.\n",
                        tag
                    );
                    match store.create(tag, &content, &[]).await {
                        Ok(tag_note) => {
                            let _ = index.reindex(&tag_note).await;
                            info!("Auto-created tag page: {}", tag);
                        }
                        Err(e) => warn!("Failed to auto-create tag page '{}': {}", tag, e),
                    }
                }
            }
        }
    }

    // Create built-in pages by writing files directly (store.create() overwrites frontmatter)
    {
        let _ = std::fs::create_dir_all(&notes_dir);

        let builtin_pages: Vec<(&str, &str)> = vec![
            ("root-tag.md", "---\ntitle: \"Root Tag\"\ntype: \"Tag\"\nicon: \"📄\"\ntag_properties: []\ntags: []\n---\n- The base tag that all other tags extend.\n"),
            ("task.md", "---\ntitle: \"Task\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"checkbox\"\nplural: \"Tasks\"\ntag_properties: [\"Status\", \"Priority\", \"Deadline\", \"Scheduled\", \"Points\"]\nproperty_overrides: {Status: {choices: [todo, doing, done, blocked], show: on_new, default: todo}}\ndetect_tokens: true\ndefault_date_property: \"scheduled\"\ntags: []\n---\n- Task tag page.\n"),
            ("project.md", "---\ntitle: \"Project\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"folder\"\nplural: \"Projects\"\ntag_properties: [\"Status\", \"Deadline\"]\nproperty_overrides: {Status: {choices: [planned, active, shipped]}}\ntags: []\n---\n- Project tag page.\n"),
            ("person.md", "---\ntitle: \"Person\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"user\"\nplural: \"People\"\ntag_properties: [\"Email\", \"Team\"]\ntags: []\n---\n- Person tag page.\n"),
            ("status.md", "---\ntitle: \"Status\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"backlog\", \"todo\", \"doing\", \"in-review\", \"done\", \"canceled\"]\ndefault: \"todo\"\ntags: []\n---\n- Status property.\n"),
            ("priority.md", "---\ntitle: \"Priority\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"p1\", \"p2\", \"p3\", \"p4\"]\ndefault: \"p4\"\nnl_triggers: [\"p1\", \"p2\", \"p3\", \"p4\"]\ntags: []\n---\n- Priority property.\n"),
            ("deadline.md", "---\ntitle: \"Deadline\"\ntype: \"Property\"\nvalue_type: \"date\"\nnl_triggers: [\"due\", \"deadline\"]\ntags: []\n---\n- Deadline property.\n"),
            ("scheduled.md", "---\ntitle: \"Scheduled\"\ntype: \"Property\"\nvalue_type: \"date\"\nnl_triggers: [\"scheduled\"]\ntags: []\n---\n- Scheduled property.\n"),
            ("points.md", "---\ntitle: \"Points\"\ntype: \"Property\"\nvalue_type: \"number\"\nnl_triggers: [\"points\", \"pts\"]\ntags: []\n---\n- Points property.\n"),

            // Life OS types
            ("domain.md", "---\ntitle: \"Domain\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"globe\"\nplural: \"Domains\"\ntag_properties: [\"Description\"]\ntags: []\n---\n- Top-level life area (Work, Family, Health, Home, etc.).\n"),
            ("lifeproject.md", "---\ntitle: \"LifeProject\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"folder\"\nplural: \"LifeProjects\"\ntag_properties: [\"Status\", \"DomainRef\", \"Deadline\", \"Description\"]\ntags: []\n---\n- Multi-task effort within a domain.\n"),
            ("issue.md", "---\ntitle: \"Issue\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"lightbulb\"\nplural: \"Issues\"\ntag_properties: [\"IssueStatus\", \"DomainRef\", \"Description\"]\ntags: []\n---\n- Deliberation item — needs thought, may become a project or task.\n"),
            ("ritual.md", "---\ntitle: \"Ritual\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"sparkles\"\nplural: \"Rituals\"\ntag_properties: [\"Cadence\", \"DomainRef\"]\ntags: []\n---\n- Daily or recurring mental check-in.\n"),
            ("scheduleditem.md", "---\ntitle: \"ScheduledItem\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"calendar\"\nplural: \"ScheduledItems\"\ntag_properties: [\"Cadence\", \"DomainRef\", \"LastCompleted\"]\ntags: []\n---\n- Recurring task with a cadence.\n"),

            // Life OS properties
            ("issuestatus.md", "---\ntitle: \"IssueStatus\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"inbox\", \"open\", \"thinking\", \"resolved\", \"became-project\", \"became-task\"]\ndefault: \"inbox\"\ntags: []\n---\n- Lifecycle status for issues.\n"),
            ("cadence.md", "---\ntitle: \"Cadence\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"daily\", \"weekly\", \"biweekly\", \"monthly\", \"quarterly\", \"yearly\"]\ntags: []\n---\n- How often a ritual or scheduled item recurs.\n"),
            ("description.md", "---\ntitle: \"Description\"\ntype: \"Property\"\nvalue_type: \"text\"\ntags: []\n---\n- Text description for any entity.\n"),
            ("lastcompleted.md", "---\ntitle: \"LastCompleted\"\ntype: \"Property\"\nvalue_type: \"date\"\ntags: []\n---\n- When a recurring item was last completed.\n"),
            ("domainref.md", "---\ntitle: \"DomainRef\"\ntype: \"Property\"\nvalue_type: \"node\"\ntags: []\n---\n- Links an item to its parent Domain page.\n"),
        ];

        for (filename, content) in builtin_pages {
            let path = notes_dir.join(filename);
            if !path.exists() {
                if let Err(e) = std::fs::write(&path, content) {
                    warn!("Failed to create built-in page {}: {}", filename, e);
                } else {
                    info!("Auto-created built-in page: {}", filename);
                }
            }
        }
    }

    let indexed = rebuild_query_index_from_files(&store, &index).await?;
    info!("Initial index complete: {} notes indexed", indexed);

    let indexer_handle = indexer.start().await?;

    // Bridge NoteEvents from the Indexer to WsEvents for WebSocket clients
    let ws_tx_bridge = ws_tx.clone();
    let note_event_tx_for_ws = note_event_tx.clone();
    tokio::spawn(async move {
        let mut rx = note_event_tx_for_ws.subscribe();
        while let Ok(event) = rx.recv().await {
            let ws_event = match event {
                NoteEvent::Created(note) => WsEvent::NoteCreated { note },
                NoteEvent::Updated(note) => WsEvent::NoteUpdated { note },
                NoteEvent::Deleted(id) => WsEvent::NoteDeleted { id: id.to_string() },
            };
            let _ = ws_tx_bridge.send(ws_event);
        }
    });

    // Phase 12.1 slice 3.4 — Apple Reminders auto-sync triggers.
    // The triggers (startup, periodic, debounced edit-driven) are
    // no-ops on non-macOS. The shared `AutoSync` also services manual
    // sync calls so the Settings UI's "last synced" line covers all
    // sources uniformly.
    let auto_sync = Arc::new(AutoSync::new());
    let store_for_auto: Arc<dyn NoteStore> = Arc::clone(&store) as Arc<dyn NoteStore>;
    reminders::auto::start_triggers(
        Arc::clone(&auto_sync),
        store_for_auto,
        note_event_tx.clone(),
    );

    // Phase 12.3 — periodic deadline/scheduled scanner. Fires WS events
    // that the web client converts to desktop notifications.
    let notifier = Arc::new(notifications::Notifier::new());
    let store_for_notify: Arc<dyn NoteStore> = Arc::clone(&store) as Arc<dyn NoteStore>;
    notifications::start(Arc::clone(&notifier), store_for_notify, ws_tx.clone());

    let mosaic_for_shutdown = mosaic.clone();
    let index_for_shutdown = Arc::clone(&index);
    let backup_cfg_for_shutdown = config.backup.clone();

    // Phase 4 (Loro migration, decisions.md 2026-05-27 → flag-day
    // 2026-05-29): `LoroEngine` is the SOLE sync engine and the
    // authoritative writer — it materializes `<mosaic>/notes/<slug>.md` on
    // every change so the file-watcher picks them up, and drives the relay
    // with the Loro v2 payload. Reads come from `FsNoteStore` off disk,
    // which Loro now owns. The legacy SqliteEngine/DualEngine/dual-write
    // stack was deleted at the flag-day; there is no fallback engine.
    //
    // `TESELA_LORO_RESEED` reseeds every note from disk at boot — the
    // canonical-device bootstrap (source of truth = disk, not the frozen
    // snapshots). Only ONE device should reseed; peers bootstrap by
    // importing from the relay.
    let sync_engine: Arc<dyn tesela_sync::SyncEngine> = {
        let device = load_or_create_device_id(&mosaic).await;
        let snapshot_dir = mosaic.join(".tesela").join("loro");
        let notes_dir = mosaic.join("notes");
        let hlc = Arc::new(tesela_sync::Hlc::new(device));
        let loro =
            tesela_sync::LoroEngine::with_dirs(device, hlc, snapshot_dir, Some(notes_dir.clone()))
                .await
                .map_err(|e| anyhow::anyhow!("open loro engine: {e}"))?;
        info!("tesela-sync: device id = {}", loro.device().to_hex());
        let reseed = std::env::var("TESELA_LORO_RESEED")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        if reseed {
            match loro.reseed_from_disk(&notes_dir).await {
                Ok(n) => info!(
                    "tesela-sync: reseeded {n} notes from disk into Loro \
                     (canonical bootstrap)"
                ),
                Err(e) => tracing::warn!("tesela-sync: reseed_from_disk failed: {e}"),
            }
        }
        Arc::new(loro)
    };

    let addr = resolve_bind_addr();
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    let bound_port = listener.local_addr().map(|a| a.port()).unwrap_or(7474);
    // Hand the embedder the real bound address (the port is freshly allocated
    // when binding `:0`), so it can build its webview URL while serve runs.
    if let Ok(actual) = listener.local_addr() {
        on_bound(actual);
    }

    // Phase 2.2 — group identity (id + symmetric key), persisted in
    // `<mosaic>/.tesela/`. A fresh install gets a freshly-minted group;
    // a join via pairing code overwrites both halves. Loaded BEFORE the
    // sync daemon so it can encrypt outgoing envelopes from tick 0.
    let group_identity = tesela_sync::load_or_create_group_identity(&mosaic_for_shutdown)
        .await
        .map_err(|e| anyhow::anyhow!("load group identity: {e}"))?;
    info!(
        "tesela-sync: group id = {:02x?}",
        group_identity.group_id.as_bytes()
    );
    let group_identity = Arc::new(RwLock::new(group_identity));

    // Phase 1.5 — background sync daemon. Every 5 seconds, pull from each
    // paired peer. Symmetric: both peers pull, so both converge.
    // Skipped in the desktop embed (TESELA_DISABLE_PEER_SYNC) — a loopback node
    // must not also participate as a LAN peer alongside a standalone server.
    if std::env::var_os("TESELA_DISABLE_PEER_SYNC").is_none() {
        let mosaic_clone = mosaic_for_shutdown.clone();
        let engine_clone = Arc::clone(&sync_engine);
        let ws_tx_clone = ws_tx.clone();
        let store_clone = Arc::clone(&store);
        let index_clone = Arc::clone(&index);
        let group_identity_clone = Arc::clone(&group_identity);
        tokio::spawn(async move {
            sync_daemon_loop(
                mosaic_clone,
                engine_clone,
                ws_tx_clone,
                store_clone,
                index_clone,
                group_identity_clone,
            )
            .await;
        });
    }

    let display_name = device_display_name();
    let public_url = build_public_url(&addr, bound_port);

    // Phase 2.1 — mDNS-based LAN discovery. Each tesela-server instance
    // advertises itself and listens for siblings, surfacing them through
    // `GET /sync/peer/discovered`. Failure here is non-fatal: manually
    // configured peers still work.
    let lan_discovery = if std::env::var("TESELA_DISABLE_MDNS").is_ok() {
        info!("tesela-sync: mDNS discovery disabled via TESELA_DISABLE_MDNS");
        None
    } else {
        let device = sync_engine.device();
        match LanDiscovery::start(device, &display_name, bound_port) {
            Ok(d) => {
                info!(
                    "tesela-sync: mDNS advertising as {} on port {}",
                    display_name, bound_port
                );
                Some(Arc::new(d))
            }
            Err(e) => {
                warn!("tesela-sync: mDNS discovery failed to start: {e}");
                None
            }
        }
    };

    // Backup scheduler status — shared with the `/backup/status` route.
    // Knobs resolved once from the environment (cadence, startup backup,
    // retention); the task itself is spawned after relay bring-up below.
    let backup_status =
        backup_scheduler::BackupStatusHandle::new(backup_scheduler::SchedulerConfig::from_env());

    let app_state = AppState {
        mosaic_root: mosaic_for_shutdown.clone(),
        store,
        index,
        ws_tx,
        ws_delta_tx,
        ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
        type_registry,
        auto_sync,
        sync_engine,
        lan_discovery,
        group_identity,
        display_name,
        public_url,
        relay_url: load_relay_url_from_config(&mosaic),
        // Brought up below if config has `[sync.relay] url`.
        relay: None,
        backup_status: backup_status.clone(),
    };
    let app_state = bring_up_relay_if_configured(app_state, &mosaic).await;

    // Saved-views registry (spec 2026-06-10; adversarial-review fix):
    // idempotently seed the built-in views (the Inbox) AFTER relay
    // bring-up, so a fresh/reinstalled device joining an existing group
    // runs `bootstrap_from_snapshots` first — the seed then sees the
    // synced registry (including a user-edited builtin) and no-ops,
    // instead of authoring a default entry that races the group's. A
    // device with no relay configured reaches here immediately (bring-up
    // is a no-op) and still seeds before serving. The seed itself is also
    // deterministic engine-side (fixed seed peer), so even a seed that
    // slips past a failed bring-up can't clobber a remote edit.
    // Non-fatal on error — a peer's seed converges the registry anyway.
    if let Err(e) = app_state.sync_engine.ensure_builtin_views().await {
        warn!("views: ensure_builtin_views at bring-up failed: {e}");
    }

    // Scheduled backups (audit 2026-06-09 "Back up the authority"):
    // one backup after bring-up + a periodic cadence, both env-tunable.
    // Spawned after relay bring-up so the startup backup captures the
    // post-bring-up state. The shutdown hook below shares the same
    // `run_configured_backup` policy.
    let backup_retention = backup_status.config.policy;
    backup_scheduler::start(
        backup_status,
        mosaic_for_shutdown.clone(),
        Arc::clone(&index_for_shutdown),
        backup_cfg_for_shutdown.clone(),
    );

    let router = routes::build(app_state);

    info!("tesela-server listening on http://{}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await?;

    indexer_handle.stop().await;

    // Phase 13.A.4 — auto-backup on clean shutdown. Runs after axum has
    // drained in-flight requests and the indexer has stopped, so the
    // mosaic is in a quiescent state. We deliberately do NOT block
    // shutdown indefinitely if backup fails — log + move on.
    if backup_cfg_for_shutdown.auto_on_quit {
        match backup_scheduler::run_configured_backup(
            &mosaic_for_shutdown,
            &index_for_shutdown,
            &backup_cfg_for_shutdown,
            Some(backup_retention),
        )
        .await
        {
            Ok(outcome) => info!("Auto-backup on shutdown: {}", outcome.path.display()),
            Err(e) => warn!("Auto-backup on shutdown failed: {}", e),
        }
    }

    Ok(())
}

/// Load or generate this device's persistent id.
///
/// Stored at `{mosaic}/.tesela/device_id.hex`. Created on first run with
/// a UUIDv7 (time-ordered). Reused thereafter so HLCs stay monotonic
/// across restarts.
async fn load_or_create_device_id(mosaic: &Path) -> DeviceId {
    let path = mosaic.join(".tesela").join("device_id.hex");
    if let Ok(bytes) = tokio::fs::read(&path).await {
        let s = String::from_utf8_lossy(&bytes).trim().to_string();
        if let Some(d) = parse_hex_device_id(&s) {
            return d;
        }
        warn!(
            "device_id.hex at {} is malformed; regenerating",
            path.display()
        );
    }
    let new_id = DeviceId::new_random();
    let tesela_dir = mosaic.join(".tesela");
    if let Err(e) = tokio::fs::create_dir_all(&tesela_dir).await {
        warn!(
            "Could not create {}: {} (device id will be ephemeral)",
            tesela_dir.display(),
            e
        );
        return new_id;
    }
    if let Err(e) = tokio::fs::write(&path, new_id.to_hex().as_bytes()).await {
        warn!(
            "Could not write {}: {} (device id will be ephemeral)",
            path.display(),
            e
        );
    }
    new_id
}

fn parse_hex_device_id(hex: &str) -> Option<DeviceId> {
    if hex.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, out_byte) in out.iter_mut().enumerate() {
        let hi = char_to_nibble(hex.as_bytes()[i * 2])?;
        let lo = char_to_nibble(hex.as_bytes()[i * 2 + 1])?;
        *out_byte = (hi << 4) | lo;
    }
    Some(DeviceId(out))
}

fn char_to_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Construct the URL we embed in pairing codes. Resolution order:
///
/// 1. `TESELA_ADVERTISE_URL` env override — full URL the operator wants
///    peers to use (e.g. a Tailscale IP, a hostname, or a tunnel like
///    `https://my-mosaic.example.com`). Wins unconditionally. Useful
///    when the autodetected first-LAN IPv4 isn't the address peers
///    can actually route to.
/// 2. Wildcard bind (`0.0.0.0` / `[::]`) → substitute the first
///    reachable non-loopback IPv4 from `if-addrs`.
/// 3. Otherwise use the bind host verbatim (the user picked it).
fn build_public_url(bind: &str, port: u16) -> String {
    if let Ok(advertised) = std::env::var("TESELA_ADVERTISE_URL") {
        let trimmed = advertised.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }
    let host = bind
        .rsplit_once(':')
        .map(|(h, _)| h.trim_matches(|c| c == '[' || c == ']'))
        .unwrap_or(bind);
    let public_host = match host {
        "0.0.0.0" | "::" | "[::]" => first_lan_ipv4().unwrap_or_else(|| host.to_string()),
        h => h.to_string(),
    };
    if public_host.contains(':') {
        format!("http://[{public_host}]:{port}")
    } else {
        format!("http://{public_host}:{port}")
    }
}

/// Pick the IPv4 address to advertise to peers in pairing codes.
///
/// A Tailscale address (CGNAT range `100.64.0.0/10`) is preferred when
/// present: for a multi-device personal setup it is the most reliable
/// address — stable, and reachable across networks, Wi-Fi AP
/// isolation, and odd subnets that defeat a plain LAN IP. A plain LAN
/// IP can also be silently unreachable when the peer routes that
/// subnet into its own Tailscale tunnel. Falls back to the first
/// ordinary LAN IPv4 when no Tailscale interface exists.
fn first_lan_ipv4() -> Option<String> {
    let addrs = if_addrs::get_if_addrs().ok()?;
    let candidates: Vec<std::net::Ipv4Addr> = addrs
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.ip() {
            std::net::IpAddr::V4(v4) if !v4.is_link_local() && !v4.is_unspecified() => Some(v4),
            _ => None,
        })
        .collect();
    candidates
        .iter()
        .find(|v4| is_tailscale_cgnat(v4))
        .or_else(|| candidates.first())
        .map(|v4| v4.to_string())
}

/// True for an address in Tailscale's CGNAT range `100.64.0.0/10`.
fn is_tailscale_cgnat(ip: &std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();
    octets[0] == 100 && (64..=127).contains(&octets[1])
}

/// Picks a user-visible name for this device, used in mDNS TXT records
/// and in any UI listing the local instance. Order of preference:
/// `TESELA_DEVICE_NAME` env override, then the OS hostname, then a
/// generic fallback so something always appears.
fn device_display_name() -> String {
    if let Ok(name) = std::env::var("TESELA_DEVICE_NAME") {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(out) = std::process::Command::new("hostname").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    "Tesela device".to_string()
}

/// Background sync daemon. Every 5 seconds, attempt one pull per known
/// peer. Errors are logged; the loop continues so a single broken peer
/// doesn't stop sync for everyone else.
async fn sync_daemon_loop(
    mosaic: PathBuf,
    engine: Arc<dyn tesela_sync::SyncEngine>,
    ws_tx: tokio::sync::broadcast::Sender<WsEvent>,
    store: Arc<FsNoteStore>,
    index: Arc<SqliteIndex>,
    group_identity: Arc<RwLock<tesela_sync::GroupIdentity>>,
) {
    let interval = std::env::var("TESELA_SYNC_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5);
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    info!(
        "tesela-sync: daemon started (interval = {}s, device = {})",
        interval,
        engine.device().to_hex()
    );
    loop {
        ticker.tick().await;
        let peers = read_peers_for_daemon(&mosaic).await;
        // Snapshot the identity once per tick. A concurrent pair-code
        // adopt will land on the next tick. We avoid holding the read
        // lock across `await` on the wire (drop before the loop body).
        let ident = group_identity.read().await.clone();
        for peer in peers {
            if let Err(e) = routes::peer_sync::sync_with_peer_minimal(
                &*engine, &mosaic, &store, &index, &ws_tx, &peer, &ident,
            )
            .await
            {
                tracing::debug!("sync to {}: {}", peer.url, e);
            }
        }
    }
}

async fn read_peers_for_daemon(mosaic: &Path) -> Vec<routes::peer_sync::Peer> {
    let path = mosaic.join(".tesela").join("sync_peers.json");
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            serde_json::from_slice::<Vec<routes::peer_sync::Peer>>(&bytes).unwrap_or_default()
        }
        Err(_) => Vec::new(),
    }
}

/// Acquire an exclusive advisory lock on `<mosaic>/.tesela/server.lock`, held
/// for the process lifetime, so only one tesela-server ever writes a given
/// mosaic. `flock(LOCK_EX | LOCK_NB)` returns an error if another process holds
/// it; the OS releases the lock when this process exits (even on SIGKILL), so
/// there is no stale-lock hazard. Mirrors tesela-backup's lock. The returned
/// `File` must be kept alive (closing it drops the lock).
fn acquire_mosaic_lock(mosaic: &Path) -> Result<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let tesela_dir = mosaic.join(".tesela");
    std::fs::create_dir_all(&tesela_dir)?;
    let lock_path = tesela_dir.join("server.lock");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;
    let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if rc != 0 {
        anyhow::bail!("lock held (EWOULDBLOCK)");
    }
    Ok(file)
}

/// When `TESELA_EXIT_WITH_PARENT` is set (the desktop / Tauri embed sets it),
/// exit this server promptly if its parent process disappears. The OS reparents
/// an orphan — its `getppid()` changes (→ launchd / init) — so we poll for that
/// and then raise `SIGTERM` on ourselves to run the normal graceful shutdown
/// (drain + backup), hard-exiting as a backstop if that stalls. Without the env
/// var this is a no-op, so the standalone server is unaffected.
pub fn spawn_parent_death_watchdog() {
    if std::env::var_os("TESELA_EXIT_WITH_PARENT").is_none() {
        return;
    }
    // Prefer the explicit `TESELA_PARENT_PID` the shell passes: if the parent
    // died DURING our spawn (before we could observe its ppid), `getppid()` is
    // already reparented (→ 1) at startup and a getppid-change check would never
    // fire. Keying off the known pid + a `kill(pid, 0) == ESRCH` liveness probe
    // closes that race. Falls back to the observed ppid if unset.
    let expected: i32 = std::env::var("TESELA_PARENT_PID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| unsafe { libc::getppid() });
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let reparented = unsafe { libc::getppid() } != expected;
        let gone = unsafe { libc::kill(expected, 0) } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        if reparented || gone {
            warn!(
                "parent process {} gone; shutting down embedded server",
                expected
            );
            unsafe {
                libc::raise(libc::SIGTERM);
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
            std::process::exit(0);
        }
    });
}

/// Resolves when the OS asks us to shut down (SIGINT or SIGTERM). On
/// non-Unix only ctrl_c is wired; SIGTERM-equivalent handling would
/// need platform-specific code we don't ship.
pub async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            warn!("Failed to install ctrl_c handler: {}", e);
        }
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(e) => {
                warn!("Failed to install SIGTERM handler: {}", e);
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("Shutdown signal received");
}

/// Resolve the address the HTTP server binds to. Precedence:
///
/// 1. `TESELA_SERVER_BIND` env var — explicit override for CI / dev.
/// 2. `[server] bind` in the global config (`~/.config/tesela/config.toml`).
/// 3. `127.0.0.1:7474` — loopback-only default.
///
/// `/server/restart` (`routes::data_ops::restart_server`) re-execs the binary
/// and DOES inherit the current environment (it does not `env_clear`), so a
/// `TESELA_SERVER_BIND` set by the launcher survives a restart. Do NOT add
/// `env_clear()` to the restart: the desktop embed relies on the inherited
/// `TESELA_SERVER_BIND=127.0.0.1:<port>` to stay loopback — clearing it would
/// fall back to step 2 / step 3 and could silently bind `0.0.0.0` from a global
/// config, exposing the embedded API on the LAN.
fn resolve_bind_addr() -> String {
    if let Ok(env) = std::env::var("TESELA_SERVER_BIND") {
        let trimmed = env.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let global = Config::default_path();
    if global.exists() {
        if let Ok(cfg) = Config::load(&global) {
            return cfg.server.bind;
        }
    }
    ServerConfig::default().bind
}

/// Read the configured relay URL: `TESELA_RELAY_URL` env wins (set by the Tauri
/// shell for the desktop embed, or for CLI/testing), else the mosaic's
/// `config.toml` `[sync.relay] url`. The env override mirrors the
/// `TESELA_DEFAULT_MOSAIC` / `TESELA_SERVER_BIND` injection pattern and lets a
/// node point at the relay without writing `[sync.relay]` into the shared mosaic.
fn load_relay_url_from_config(mosaic: &std::path::Path) -> Option<String> {
    if let Ok(url) = std::env::var("TESELA_RELAY_URL") {
        let url = url.trim().to_string();
        if !url.is_empty() {
            return Some(url);
        }
    }
    let cfg = load_config(mosaic);
    cfg.sync.relay.map(|r| r.url)
}

/// If `[sync.relay] url = "…"` is configured, build a `RelayClient`,
/// run register-or-recover + verify-registration, attach a
/// `RelayHandle` to `AppState`, and spawn the periodic
/// `sync_relay::tick` daemon. Hijack errors during `verify` are
/// surfaced via `RelayState.last_error` (no panic) so the web
/// settings page can show the user what went wrong.
async fn bring_up_relay_if_configured(
    mut state: state::AppState,
    mosaic: &std::path::Path,
) -> state::AppState {
    // The desktop embed is a LOOPBACK Loro-replica node, not a relay
    // participant — it must not register/tick the relay, or it becomes a second
    // writer under the shared `device_id` (HLC + cursor races) the instant relay
    // config is enabled in the shared mosaic. The shell sets TESELA_DISABLE_RELAY.
    if std::env::var_os("TESELA_DISABLE_RELAY").is_some() {
        return state;
    }
    let Some(url) = state.relay_url.clone() else {
        return state;
    };
    let url = match reqwest::Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("relay URL `{}` is not a valid URL: {}", url, e);
            return state;
        }
    };
    let ident = state.group_identity.read().await.clone();
    let device = state.sync_engine.device();
    let client = std::sync::Arc::new(tesela_sync::transport::relay::RelayClient::new(
        url.clone(),
        ident.group_id,
        device,
        ident.group_key.clone(),
    ));
    let mut persisted = sync_relay::RelayState::load(mosaic).await;
    // Scope the persisted cursors to the CURRENT (relay, group) identity:
    // a relay migration or a group re-pair means a fresh seq namespace, so
    // replaying the old cursor would silently skip every op below it. On
    // mismatch the state resets and `bootstrap_from_snapshots` below pulls
    // the new relay's full state (audit A5).
    if persisted.scope_to_identity(url.as_str(), &hex::encode(ident.group_id.as_bytes())) {
        tracing::warn!(
            "relay identity changed (url/group) — persisted cursors reset; \
             re-registering + re-bootstrapping against {}",
            url
        );
        if let Err(e) = persisted.save(mosaic).await {
            tracing::warn!("relay state save (post-identity-reset): {e}");
        }
    }
    let handle = sync_relay::RelayHandle {
        url: url.to_string(),
        client: client.clone(),
        state: std::sync::Arc::new(tokio::sync::RwLock::new(persisted)),
        mosaic_root: mosaic.to_path_buf(),
    };

    // Attempt one-shot bring-up; failure is recoverable on the next tick.
    if let Err(e) = sync_relay::bring_up(&handle).await {
        tracing::warn!("relay bring-up: {} (will retry on tick)", e);
        let mut s = handle.state.write().await;
        s.last_error = Some(e);
        let _ = s.save(mosaic).await;
        drop(s);
    } else {
        tracing::info!("relay: registered + verified at {}", url);
        // Fresh / long-offline restore: import the relay's compacted snapshots
        // before the first poll, so a device whose ops the relay already GC'd
        // still converges (the subsequent `?since=` poll collects the tail).
        sync_relay::bootstrap_from_snapshots(&*state.sync_engine, &handle).await;
        match rebuild_query_index_from_files(&state.store, &state.index).await {
            Ok(n) => tracing::info!("relay bootstrap: rebuilt query index from {n} file(s)"),
            Err(e) => tracing::warn!("relay bootstrap: query index rebuild failed: {e}"),
        }
    }

    // Spawn the periodic tick. Single task; runs alongside the LAN
    // peer-sync daemon. We get the poll interval from config or fall
    // back to the module default.
    let poll_interval = load_config(mosaic)
        .sync
        .relay
        .map(|r| std::time::Duration::from_millis(r.poll_interval_ms))
        .unwrap_or(sync_relay::DEFAULT_POLL_INTERVAL);
    let tick_handle = handle.clone();
    let tick_engine = state.sync_engine.clone();
    let tick_ident = ident.clone();
    // Instant-multidevice (Phase A, spec finding #4): give the relay loop
    // the WS fan-out handles so a relay-originated edit notifies web
    // (`ws_tx`) and reaches live device sockets (`ws_delta_tx`). Cloning
    // the channel/store/index handles into the task is cleaner than
    // threading the not-yet-assembled `Arc<AppState>` through the relay
    // bring-up. The relay is config-bypassed in the live mosaic today, so
    // this path is dormant there but kept correct for when it re-enables.
    let tick_ws_tx = state.ws_tx.clone();
    let tick_ws_delta_tx = state.ws_delta_tx.clone();
    let tick_store = std::sync::Arc::clone(&state.store);
    let tick_index = std::sync::Arc::clone(&state.index);
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(poll_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match sync_relay::tick(&*tick_engine, &tick_ident, &tick_handle).await {
                Ok(outcome) => {
                    if outcome.applied > 0 || outcome.sent > 0 {
                        tracing::debug!(
                            "relay tick: applied {}, sent {}",
                            outcome.applied,
                            outcome.sent
                        );
                    }
                    for note_id in &outcome.applied_note_ids {
                        // Notify web that this remote-originated edit landed
                        // (drives query invalidation — list/agenda/inbox).
                        routes::ws::emit_note_updated(
                            &*tick_engine,
                            &tick_store,
                            &tick_index,
                            &tick_ws_tx,
                            *note_id,
                        )
                        .await;
                    }
                    // Re-broadcast the EXACT applied delta bytes to live
                    // device sockets so their Loro docs converge without
                    // waiting on their own poll. `origin: None` — fan out to
                    // everyone (the relay has no originating WS socket). The
                    // post-apply `export_doc_update` returned None here — the
                    // engine's export cursor already consumed these bytes — so
                    // we carry them out of the tick and re-broadcast verbatim,
                    // exactly as the WS inbound handler does.
                    if !outcome.applied_updates.is_empty() {
                        if let Ok(frame) =
                            tesela_sync::encode_loro_relay_payload(&outcome.applied_updates)
                        {
                            let _ = tick_ws_delta_tx.send(state::WsDelta {
                                origin: None,
                                frame,
                            });
                        }
                    }
                }
                Err(e) => tracing::debug!("relay tick: {e}"),
            }
        }
    });

    // Presence bridge (Phase 3b, Stage 2): a separate long-lived WS to the CF
    // relay's /presence/ws. It observes locally-originated PRES frames on
    // ws_delta_tx (origin = Some) → seals → relay; opens relay-broadcast frames
    // → fans out on ws_delta_tx (origin = None). Same (group, device, key, url)
    // identity the tick uses. Independent of the poll/produce tick — presence is
    // ephemeral and never touches the engine.
    presence_relay::spawn(
        &url,
        ident.group_id,
        device,
        ident.group_key.clone(),
        state.ws_delta_tx.clone(),
    );

    state.relay = Some(handle);
    state
}

fn load_config(mosaic: &std::path::Path) -> Config {
    let path = mosaic.join(".tesela").join("config.toml");
    if !path.exists() {
        return Config::default();
    }
    match Config::load(&path) {
        Ok(cfg) => cfg,
        Err(e) => {
            warn!(
                "Failed to read {}: {}; falling back to defaults",
                path.display(),
                e
            );
            Config::default()
        }
    }
}

async fn rebuild_query_index_from_files(store: &FsNoteStore, index: &SqliteIndex) -> Result<usize> {
    let notes = store.list(None, usize::MAX, 0).await?;
    index.rebuild_from_notes(&notes).await?;
    for note in &notes {
        let links = extract_wiki_links(&note.content);
        index.update_links(&note.id, &links).await?;
    }
    Ok(notes.len())
}

fn find_mosaic() -> Result<PathBuf> {
    // 1. Explicit env override — CI / dev scripts / power users.
    if let Ok(env) = std::env::var("TESELA_DEFAULT_MOSAIC") {
        let p = PathBuf::from(env);
        if p.join(".tesela").exists() {
            return Ok(p);
        }
    }

    // 2. Cwd-walk: if the user is *inside* a mosaic dir, that's the
    // strongest "use this" signal short of an env var. Wins over the
    // saved config default so dev work in a sibling mosaic doesn't
    // require flipping config.
    if let Ok(start) = std::env::current_dir() {
        let mut dir = start;
        loop {
            if dir.join(".tesela").exists() {
                return Ok(dir);
            }
            if !dir.pop() {
                break;
            }
        }
    }

    // 3. Config-persisted default_mosaic (written by the Mosaic
    // Settings UI's "Switch" button).
    let config_path = Config::default_path();
    if config_path.exists() {
        if let Ok(cfg) = Config::load(&config_path) {
            if let Some(p) = cfg.general.default_mosaic {
                if p.join(".tesela").exists() {
                    return Ok(p);
                }
            }
        }
    }

    // 4. Fall back to the standard per-OS data dir. Auto-initialize
    // it on first launch so a fresh user gets a working server
    // without having to run `tesela init` first.
    let default = Config::default_mosaic_path();
    if !default.join(".tesela").exists() {
        info!(
            "No mosaic found; auto-initializing at {}",
            default.display()
        );
        ensure_blank_mosaic(&default)?;
    }
    Ok(default)
}

/// Mirror of `tesela init` minus the SQLite open (which needs async).
/// The caller does the SQLite open via `SqliteIndex::open` immediately
/// after, which will create the database file if missing.
fn ensure_blank_mosaic(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path.join(".tesela"))?;
    std::fs::create_dir_all(path.join("notes"))?;
    std::fs::create_dir_all(path.join("attachments"))?;
    let cfg_path = path.join(".tesela").join("config.toml");
    if !cfg_path.exists() {
        Config::default().save(&cfg_path)?;
    }
    // Seed the default system widgets so the rail nav is populated
    // from the very first request. Idempotent — preserves user edits.
    if let Err(e) = tesela_core::system_widgets::seed(path) {
        warn!("Failed to seed system widgets at {}: {}", path.display(), e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn tailscale_cgnat_range_detection() {
        // Inside 100.64.0.0/10.
        assert!(is_tailscale_cgnat(&Ipv4Addr::new(100, 112, 34, 59)));
        assert!(is_tailscale_cgnat(&Ipv4Addr::new(100, 64, 0, 0)));
        assert!(is_tailscale_cgnat(&Ipv4Addr::new(100, 127, 255, 255)));
        // Outside the range — 100.x but wrong second octet, and ordinary LAN IPs.
        assert!(!is_tailscale_cgnat(&Ipv4Addr::new(100, 63, 255, 255)));
        assert!(!is_tailscale_cgnat(&Ipv4Addr::new(100, 128, 0, 0)));
        assert!(!is_tailscale_cgnat(&Ipv4Addr::new(10, 15, 109, 184)));
        assert!(!is_tailscale_cgnat(&Ipv4Addr::new(192, 168, 1, 5)));
    }

    #[tokio::test]
    async fn rebuild_query_index_from_files_backfills_saved_view_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let mosaic = tmp.path().to_path_buf();
        std::fs::create_dir_all(mosaic.join(".tesela")).unwrap();
        std::fs::create_dir_all(mosaic.join("notes")).unwrap();
        std::fs::write(
            mosaic.join("notes").join("2026-06-11.md"),
            "---\ntitle: 2026-06-11\ntags: [daily]\n---\n- Write release notes <!-- bid:11111111-1111-1111-1111-111111111111 -->\n  tags:: Task\n  status:: todo\n",
        )
        .unwrap();

        let store = FsNoteStore::new(
            mosaic.clone(),
            tesela_core::config::StorageConfig::default(),
        );
        let index = SqliteIndex::open(&mosaic.join(".tesela").join("test.db"))
            .await
            .unwrap();
        let before = index
            .execute_query(&tesela_core::query::parse_query("tag:Task"), None, None)
            .await
            .unwrap();
        assert_eq!(before.groups[0].items.len(), 0);

        let count = rebuild_query_index_from_files(&store, &index)
            .await
            .unwrap();
        let after = index
            .execute_query(&tesela_core::query::parse_query("tag:Task"), None, None)
            .await
            .unwrap();

        assert_eq!(count, 1);
        assert_eq!(after.groups[0].items.len(), 1);
        assert_eq!(after.groups[0].items[0].text, "Write release notes");
    }

    /// End-to-end real-socket round-trip for the Phase A bidirectional WS:
    /// two clients connect to a live `/ws`; client A pushes a binary Loro
    /// delta; assert (a) client B receives the binary frame, (b) a text
    /// `WsEvent::NoteUpdated` is delivered (web invalidation), (c) client A
    /// does NOT receive its own delta back (echo-suppression), and (d) the
    /// server engine converged on A's edit.
    #[tokio::test]
    async fn ws_binary_delta_round_trip_over_real_socket() {
        use futures::{SinkExt, StreamExt};
        use tesela_sync::{DeviceId, Hlc, LoroDocUpdate, LoroEngine, OpPayload, SyncEngine};
        use tokio_tungstenite::tungstenite::Message as TMessage;

        // ── Build a full AppState over a tempdir ──────────────────────────
        let tmp = tempfile::tempdir().unwrap();
        let mosaic = tmp.path().to_path_buf();
        let notes_dir = mosaic.join("notes");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(mosaic.join(".tesela")).unwrap();

        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server_engine = LoroEngine::with_dirs(
            sdev,
            Arc::new(Hlc::new(sdev)),
            mosaic.join(".tesela").join("loro"),
            Some(notes_dir.clone()),
        )
        .await
        .unwrap();

        // Slug → note_id derivation matches notes.rs::stable_uuid_from_slug.
        let note_id = {
            let h = blake3::hash(b"n");
            let mut out = [0u8; 16];
            out.copy_from_slice(&h.as_bytes()[..16]);
            out
        };

        // Device A authors the note locally (separate engine), then we
        // pre-seed the SAME base into the server so its index resolves the
        // slug for the WsEvent re-read.
        let adev = DeviceId::from_bytes([0xa1; 16]);
        let device_a = LoroEngine::new(adev, Arc::new(Hlc::new(adev)));
        device_a
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("n".into()),
                title: "N".into(),
                content: "- seed <!-- bid:03030303-0303-0303-0303-030303030303 -->\n".into(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let base = device_a.export_doc_update(note_id, None).await.unwrap();
        server_engine
            .import_doc_update(note_id, &base)
            .await
            .unwrap();

        // Now A makes the edit we expect to propagate.
        device_a
            .record_local(OpPayload::BlockUpsert {
                block_id: [0xab; 16],
                note_id,
                parent_block_id: None,
                order_key: "z".into(),
                indent_level: 0,
                text: "live edit from A".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        // Export the device's current full state as the delta frame. The
        // server already holds the base, so importing this is a no-op merge
        // for the shared history plus A's new edit (Loro is idempotent +
        // commutative — exactly the live-path forward-the-applied-bytes
        // model). A pre-edit-VV delta would also work; full-state keeps the
        // test independent of VV-capture timing.
        let delta = device_a.export_doc_update(note_id, None).await.unwrap();
        let frame = tesela_sync::encode_loro_relay_payload(&[LoroDocUpdate {
            doc: note_id,
            update_bytes: delta,
        }])
        .unwrap();

        let store = Arc::new(FsNoteStore::new(
            mosaic.clone(),
            tesela_core::config::StorageConfig::default(),
        ));
        let index = Arc::new(
            SqliteIndex::open(&mosaic.join(".tesela").join("test.db"))
                .await
                .unwrap(),
        );
        let (ws_tx, _) = broadcast::channel::<WsEvent>(64);
        let (ws_delta_tx, _) = broadcast::channel::<state::WsDelta>(64);
        let group_identity = Arc::new(RwLock::new(tesela_sync::GroupIdentity {
            group_id: tesela_sync::GroupId::new_random(),
            group_key: tesela_sync::GroupKey::random(),
        }));
        let app_state = AppState {
            mosaic_root: mosaic.clone(),
            store,
            index,
            ws_tx,
            ws_delta_tx,
            ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
            type_registry: tesela_core::types::TypeRegistry::load(&mosaic),
            auto_sync: Arc::new(reminders::auto::AutoSync::new()),
            sync_engine: Arc::new(server_engine) as Arc<dyn tesela_sync::SyncEngine>,
            lan_discovery: None,
            group_identity,
            display_name: "test".into(),
            public_url: "http://127.0.0.1:0".into(),
            relay_url: None,
            relay: None,
            backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                crate::backup_scheduler::SchedulerConfig::from_env(),
            ),
        };
        let server_engine_handle = Arc::clone(&app_state.sync_engine);
        let router = routes::build(app_state);

        // ── Serve on an ephemeral port ────────────────────────────────────
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        let url = format!("ws://{}/ws", addr);
        let (mut client_b, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut client_a, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

        // Give both subscriptions a moment to register on the broadcast bus.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Client A sends the binary delta.
        client_a
            .send(TMessage::Binary(frame.clone().into()))
            .await
            .unwrap();

        // ── Client B must receive BOTH a binary delta and a text WsEvent ──
        let mut got_binary_b = false;
        let mut got_text_b = false;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        while (!got_binary_b || !got_text_b) && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_secs(2), client_b.next()).await {
                Ok(Some(Ok(TMessage::Binary(b)))) => {
                    assert_eq!(
                        b.as_ref(),
                        frame.as_slice(),
                        "B gets the exact applied bytes"
                    );
                    got_binary_b = true;
                }
                Ok(Some(Ok(TMessage::Text(t)))) => {
                    assert!(
                        t.contains("note_updated"),
                        "B gets a NoteUpdated event: {t}"
                    );
                    assert!(
                        t.contains("live edit from A"),
                        "event carries merged content"
                    );
                    got_text_b = true;
                }
                Ok(Some(Ok(_))) => {}
                _ => break,
            }
        }
        assert!(got_binary_b, "client B received the binary delta");
        assert!(got_text_b, "client B received the NoteUpdated text event");

        // ── Client A must NOT receive its own binary frame back ───────────
        // (it may receive the text event — that fans out to everyone).
        let mut echoed_to_a = false;
        let a_deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
        while tokio::time::Instant::now() < a_deadline {
            match tokio::time::timeout(std::time::Duration::from_millis(200), client_a.next()).await
            {
                Ok(Some(Ok(TMessage::Binary(_)))) => {
                    echoed_to_a = true;
                    break;
                }
                Ok(Some(Ok(_))) => {}
                _ => break,
            }
        }
        assert!(
            !echoed_to_a,
            "echo-suppression: A never gets its own delta back"
        );

        // ── Server engine converged on A's edit ───────────────────────────
        let rendered = server_engine_handle.render_note(note_id).await.unwrap();
        assert!(
            rendered.contains("live edit from A"),
            "server converged: {rendered:?}"
        );
    }

    /// End-to-end real-socket regression for the WS-push clobber (the final
    /// data-loss vector, Part C / commit 6623441): a device cold-launches and
    /// pushes a WHOLE-NOTE Loro SNAPSHOT over `/ws` carrying its STALE value
    /// for a block another peer (the server, via an HTTP block edit) just
    /// changed. On a RAW merge the device's stale, disjoint-lineage twin of
    /// that block could WIN the dedup and REVERT the server's edit. Part C's
    /// protected inbound apply (`apply_relay_updates` → `import_doc_update`)
    /// must apply ONLY the blocks the peer GENUINELY re-authored.
    ///
    /// This drives the GENUINE `apply_inbound_delta` handler over a live
    /// `tokio_tungstenite` WS client — the path the engine-level unit test
    /// (`ws_apply_stale_snapshot_does_not_revert_peer_edit` in
    /// `tesela_sync::engine::loro_engine`) couldn't exercise. That engine test
    /// proved the FAILING direction (raw import reverts A without the fix);
    /// this one proves the fix holds end-to-end through the real socket.
    ///
    /// Stale-op-wins reproduction (mirrors the engine test's `seed_disjoint`):
    /// the server and the device each author blocks A/B INDEPENDENTLY (no
    /// shared Loro import), so each mints its OWN `TreeID` for the same
    /// `block_id` — the residual disjoint lineage the incident's daily blocks
    /// carried. The server then HTTP-edits A → "Awesome sweet"; the device,
    /// holding its stale A="Awesome" twin, re-asserts A AND genuinely edits
    /// B → "Bee device", then ships a FULL SNAPSHOT (the cold-launch first
    /// push). A raw `doc.import` would union the rival A-twins and the
    /// non-causal dedup could keep the STALE one → revert. Protected: A stays
    /// "Awesome sweet", B becomes "Bee device".
    #[tokio::test]
    async fn ws_stale_snapshot_does_not_clobber_http_edit_over_real_socket() {
        use futures::{SinkExt, StreamExt};
        use tesela_sync::{DeviceId, Hlc, LoroDocUpdate, LoroEngine, OpPayload, SyncEngine};
        use tokio_tungstenite::tungstenite::Message as TMessage;

        // Fixed block ids — A and B. The DISJOINT-lineage seed (below) makes
        // each engine mint its own TreeID for these, which is the whole point.
        const A_BID: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
        const B_BID: &str = "0b0b0b0b-0b0b-0b0b-0b0b-0b0b0b0b0b0b";
        const A_BID_BYTES: [u8; 16] = [0x0a; 16];
        const B_BID_BYTES: [u8; 16] = [0x0b; 16];

        // ── Build a full AppState over a tempdir (mirrors the sibling test) ──
        let tmp = tempfile::tempdir().unwrap();
        let mosaic = tmp.path().to_path_buf();
        let notes_dir = mosaic.join("notes");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(mosaic.join(".tesela")).unwrap();

        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server_engine = LoroEngine::with_dirs(
            sdev,
            Arc::new(Hlc::new(sdev)),
            mosaic.join(".tesela").join("loro"),
            Some(notes_dir.clone()),
        )
        .await
        .unwrap();

        // Slug "clobber" → note_id (matches notes.rs::stable_uuid_from_slug,
        // the blake3-of-slug derivation), so the materialized file is
        // clobber.md and the WsEvent re-read / HTTP GET resolve. (A neutral
        // slug — "daily" collides with the special `/notes/daily` route.)
        let note_id = {
            let h = blake3::hash(b"clobber");
            let mut out = [0u8; 16];
            out.copy_from_slice(&h.as_bytes()[..16]);
            out
        };

        // ── DISJOINT base: server AND device each author the same note body
        // INDEPENDENTLY (no shared import) → rival TreeIDs for A/B. This is the
        // residual disjoint lineage that lets the device's stale A-twin compete
        // with (and, raw, revert) the server's edit. ───────────────────────
        // Device peer id is numerically SMALLER than the server's (0x5e), so on
        // a raw merge the min-`TreeID` twin dedup keeps the DEVICE's twin for
        // each shared block_id — meaning the device's STALE A="Awesome" twin
        // would WIN and REVERT the server's "Awesome sweet" (Case a, the
        // clobber). This is exactly the stale-op-wins condition: without Part
        // C's heal, A reverts. (Verified: with the heal disabled, A renders as
        // the stale "Awesome".)
        let ddev = DeviceId::from_bytes([0x11; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        let base_content = format!("- Awesome <!-- bid:{A_BID} -->\n- Bee <!-- bid:{B_BID} -->\n");
        for engine in [&server_engine, &device] {
            engine
                .record_local(OpPayload::NoteUpsert {
                    note_id,
                    display_alias: Some("clobber".into()),
                    title: "Clobber".into(),
                    content: base_content.clone(),
                    created_at_millis: 1,
                })
                .await
                .unwrap();
        }

        // ── The "web HTTP edit" on the SERVER's authoritative doc: A → "Awesome
        // sweet" (the protected, correct value). Applied BEFORE moving the
        // engine into AppState. ─────────────────────────────────────────────
        server_engine
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "Awesome sweet".into(),
                after_block_id: None,
            })
            .await
            .unwrap();

        // ── The DEVICE (stale: never saw the server edit) re-asserts its stale
        // A="Awesome" AND genuinely edits B → "Bee device", then exports a FULL
        // SNAPSHOT — the cold-launch first-push frame from the incident. ─────
        device
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "Awesome".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        device
            .record_local(OpPayload::BlockUpsert {
                block_id: B_BID_BYTES,
                note_id,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "Bee device".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let snapshot = device.export_doc_update(note_id, None).await.unwrap();
        let frame = tesela_sync::encode_loro_relay_payload(&[LoroDocUpdate {
            doc: note_id,
            update_bytes: snapshot,
        }])
        .unwrap();

        // ── Assemble AppState; keep a cloned engine handle for post-apply
        // rendering (the engine is moved into the Arc<dyn SyncEngine>). ──────
        let store = Arc::new(FsNoteStore::new(
            mosaic.clone(),
            tesela_core::config::StorageConfig::default(),
        ));
        let index = Arc::new(
            SqliteIndex::open(&mosaic.join(".tesela").join("test.db"))
                .await
                .unwrap(),
        );
        let (ws_tx, _) = broadcast::channel::<WsEvent>(64);
        let (ws_delta_tx, _) = broadcast::channel::<state::WsDelta>(64);
        let group_identity = Arc::new(RwLock::new(tesela_sync::GroupIdentity {
            group_id: tesela_sync::GroupId::new_random(),
            group_key: tesela_sync::GroupKey::random(),
        }));
        let app_state = AppState {
            mosaic_root: mosaic.clone(),
            store,
            index,
            ws_tx,
            ws_delta_tx,
            ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
            type_registry: tesela_core::types::TypeRegistry::load(&mosaic),
            auto_sync: Arc::new(reminders::auto::AutoSync::new()),
            sync_engine: Arc::new(server_engine) as Arc<dyn tesela_sync::SyncEngine>,
            lan_discovery: None,
            group_identity,
            display_name: "test".into(),
            public_url: "http://127.0.0.1:0".into(),
            relay_url: None,
            relay: None,
            backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                crate::backup_scheduler::SchedulerConfig::from_env(),
            ),
        };
        let server_engine_handle = Arc::clone(&app_state.sync_engine);
        let router = routes::build(app_state);

        // Sanity: the server's authoritative doc holds the HTTP edit BEFORE the
        // device's stale push — proves A is genuinely at risk of reversion.
        let pre = server_engine_handle.render_note(note_id).await.unwrap();
        assert!(
            pre.contains("Awesome sweet"),
            "precondition: server holds the HTTP edit before the stale push: {pre:?}"
        );

        // ── Serve on an ephemeral port ──────────────────────────────────────
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        // ── Connect two real WS clients: `pusher` ships the stale snapshot,
        // `watcher` awaits the NoteUpdated text frame so we know the server
        // finished applying. ────────────────────────────────────────────────
        let url = format!("ws://{}/ws", addr);
        let (mut watcher, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut pusher, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Pusher sends the device's full STALE snapshot as a binary frame.
        pusher
            .send(TMessage::Binary(frame.clone().into()))
            .await
            .unwrap();

        // Wait for the server to process: the watcher receives a NoteUpdated
        // text event once the inbound delta is applied + re-indexed.
        let mut got_event = false;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        while !got_event && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_secs(2), watcher.next()).await {
                Ok(Some(Ok(TMessage::Text(t)))) if t.contains("note_updated") => {
                    got_event = true;
                }
                Ok(Some(Ok(_))) => {}
                _ => break,
            }
        }
        assert!(
            got_event,
            "watcher received the NoteUpdated event (server finished applying the inbound delta)"
        );

        // ── ASSERT on the server's authoritative render: the clobber is closed.
        let rendered = server_engine_handle.render_note(note_id).await.unwrap();
        assert!(
            rendered.contains("Awesome sweet"),
            "A must NOT be reverted by the device's stale snapshot (got {rendered:?})"
        );
        assert!(
            !rendered.contains("- Awesome <!--"),
            "the stale A=\"Awesome\" twin must not resurface (got {rendered:?})"
        );
        assert!(
            rendered.contains("Bee device"),
            "B (the device's genuine edit) must apply (got {rendered:?})"
        );

        // ── End-to-end HTTP coverage: the materialized note served via
        // `GET /notes/clobber` shows the same merged state. ──────────────────
        let body = reqwest::get(format!("http://{}/notes/clobber", addr))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            body.contains("Awesome sweet"),
            "HTTP GET shows the protected HTTP edit: {body}"
        );
        assert!(
            body.contains("Bee device"),
            "HTTP GET shows the device's genuine B edit: {body}"
        );
    }

    /// The LoroText headline (2026-06-02): two peers editing the SAME block on
    /// a SHARED lineage MERGE character-level instead of one clobbering the
    /// other. This reproduces the wire-proven incident ("web gets clobbered by
    /// iOS") end-to-end over a real socket + real HTTP: the server takes the
    /// "web" edit via `POST /notes/{slug}/blocks` and the "device" pushes its
    /// concurrent edit to the same block over the WS. Block text being a
    /// `LoroText`, the two whole-text writes Myers-diff into splices that
    /// interleave — neither side's words are lost, and the result is NOT the
    /// LWW whole-string pick. (Pre-LoroText this asserted-FAILS: the map
    /// register is last-writer-wins, so one whole string vanishes.)
    #[tokio::test]
    async fn ws_concurrent_same_block_edit_merges_over_real_socket() {
        use futures::{SinkExt, StreamExt};
        use tesela_sync::{DeviceId, Hlc, LoroDocUpdate, LoroEngine, OpPayload, SyncEngine};
        use tokio_tungstenite::tungstenite::Message as TMessage;

        const A_BID: &str = "0a0a0a0a-0a0a-0a0a-0a0a-0a0a0a0a0a0a";
        const A_BID_BYTES: [u8; 16] = [0x0a; 16];

        // ── AppState over a tempdir (mirrors the sibling socket test) ──
        let tmp = tempfile::tempdir().unwrap();
        let mosaic = tmp.path().to_path_buf();
        let notes_dir = mosaic.join("notes");
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(mosaic.join(".tesela")).unwrap();

        let sdev = DeviceId::from_bytes([0x5e; 16]);
        let server_engine = LoroEngine::with_dirs(
            sdev,
            Arc::new(Hlc::new(sdev)),
            mosaic.join(".tesela").join("loro"),
            Some(notes_dir.clone()),
        )
        .await
        .unwrap();

        let note_id = {
            let h = blake3::hash(b"merge");
            let mut out = [0u8; 16];
            out.copy_from_slice(&h.as_bytes()[..16]);
            out
        };

        // ── SHARED base: the server seeds the note, then the device IMPORTS the
        // server's snapshot — so both hold the SAME TreeID + LoroText lineage
        // for block A (NOT disjoint twins). This is the post-bootstrap state the
        // shared-base flow keeps devices in, and the only state where character
        // merge applies. ────────────────────────────────────────────────────
        let base_content = format!("- The quick fox <!-- bid:{A_BID} -->\n");
        server_engine
            .record_local(OpPayload::NoteUpsert {
                note_id,
                display_alias: Some("merge".into()),
                title: "Merge".into(),
                content: base_content.clone(),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        let base_snap = server_engine
            .export_doc_update(note_id, None)
            .await
            .unwrap();

        let ddev = DeviceId::from_bytes([0x11; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        // Import the server's snapshot via the device's own apply path so it
        // adopts the server's lineage (shared TreeID for A).
        device.import_doc_update(note_id, &base_snap).await.unwrap();

        // ── The DEVICE's concurrent edit to A (exported BEFORE the server's web
        // edit, so the two are causally independent = a true concurrent merge). ─
        device
            .record_local(OpPayload::BlockUpsert {
                block_id: A_BID_BYTES,
                note_id,
                parent_block_id: None,
                order_key: "00000000".into(),
                indent_level: 0,
                text: "The quick red fox jumps".into(),
                after_block_id: None,
            })
            .await
            .unwrap();
        let dev_snap = device.export_doc_update(note_id, None).await.unwrap();
        let frame = tesela_sync::encode_loro_relay_payload(&[LoroDocUpdate {
            doc: note_id,
            update_bytes: dev_snap,
        }])
        .unwrap();

        // ── Assemble AppState; keep a handle for post-apply rendering ──
        let store = Arc::new(FsNoteStore::new(
            mosaic.clone(),
            tesela_core::config::StorageConfig::default(),
        ));
        let index = Arc::new(
            SqliteIndex::open(&mosaic.join(".tesela").join("test.db"))
                .await
                .unwrap(),
        );
        let (ws_tx, _) = broadcast::channel::<WsEvent>(64);
        let (ws_delta_tx, _) = broadcast::channel::<state::WsDelta>(64);
        let group_identity = Arc::new(RwLock::new(tesela_sync::GroupIdentity {
            group_id: tesela_sync::GroupId::new_random(),
            group_key: tesela_sync::GroupKey::random(),
        }));
        let app_state = AppState {
            mosaic_root: mosaic.clone(),
            store,
            index,
            ws_tx,
            ws_delta_tx,
            ws_conn_seq: std::sync::atomic::AtomicU64::new(0),
            type_registry: tesela_core::types::TypeRegistry::load(&mosaic),
            auto_sync: Arc::new(reminders::auto::AutoSync::new()),
            sync_engine: Arc::new(server_engine) as Arc<dyn tesela_sync::SyncEngine>,
            lan_discovery: None,
            group_identity,
            display_name: "test".into(),
            public_url: "http://127.0.0.1:0".into(),
            relay_url: None,
            relay: None,
            backup_status: crate::backup_scheduler::BackupStatusHandle::new(
                crate::backup_scheduler::SchedulerConfig::from_env(),
            ),
        };
        let server_engine_handle = Arc::clone(&app_state.sync_engine);
        let router = routes::build(app_state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });

        let url = format!("ws://{}/ws", addr);
        let (mut watcher, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        let (mut pusher, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // ── THE "WEB" EDIT over real HTTP: POST /notes/merge/blocks upserts A →
        // "The quick brown fox". This is exactly what the web client sends. ───
        let http = reqwest::Client::new();
        let resp = http
            .post(format!("http://{}/notes/merge/blocks", addr))
            .json(&serde_json::json!({
                "ops": [{
                    "kind": "upsert",
                    "bid": A_BID,
                    "text": "The quick brown fox",
                    "indent_level": 0,
                }]
            }))
            .send()
            .await
            .unwrap();
        assert!(
            resp.status().is_success(),
            "block-op POST ok: {:?}",
            resp.status()
        );

        // ── THE "DEVICE" EDIT over the real WS: push the concurrent A frame. ──
        pusher
            .send(TMessage::Binary(frame.clone().into()))
            .await
            .unwrap();

        // Drain WS events until the server has applied the inbound delta (the
        // HTTP edit + the WS apply each emit a note_updated; wait for at least
        // the post-push one).
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut updates = 0;
        while updates < 2 && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_secs(2), watcher.next()).await {
                Ok(Some(Ok(TMessage::Text(t)))) if t.contains("note_updated") => updates += 1,
                Ok(Some(Ok(_))) => {}
                _ => break,
            }
        }
        // A short settle so the WS-apply heal/materialize completes.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // ── ASSERT: A is a CHARACTER-LEVEL MERGE of both edits — neither side's
        // words lost — and NOT the LWW whole-string pick. ────────────────────
        let rendered = server_engine_handle.render_note(note_id).await.unwrap();
        let a_line = rendered
            .lines()
            .find(|l| l.contains(A_BID))
            .unwrap_or("")
            .to_string();
        assert!(
            a_line.contains("brown") && a_line.contains("red") && a_line.contains("jumps"),
            "block A must MERGE both concurrent edits (web 'brown' + device 'red'/'jumps'), \
             neither lost: {a_line:?}"
        );
        assert!(
            !a_line.contains("The quick brown fox <!--")
                && !a_line.contains("The quick red fox jumps <!--"),
            "the merge must NOT be a whole-string LWW pick of either input: {a_line:?}"
        );

        // ── End-to-end HTTP: GET /notes/merge shows the same merged text. ──
        let body = reqwest::get(format!("http://{}/notes/merge", addr))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            body.contains("brown") && body.contains("red") && body.contains("jumps"),
            "HTTP GET shows the merged A text: {body}"
        );
    }
}
