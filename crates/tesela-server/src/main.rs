mod error;
mod notifications;
mod reminders;
mod routes;
mod state;

use anyhow::Result;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::broadcast;
use tracing::{info, warn};

use tesela_core::{
    config::{BackupConfig, Config},
    db::SqliteIndex,
    indexer::{Indexer, NoteEvent},
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
    types::TypeRegistry,
};

use reminders::auto::AutoSync;
use state::{AppState, WsEvent};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let mosaic = find_mosaic()?;

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

    // WebSocket broadcast channel
    let (ws_tx, _) = broadcast::channel::<WsEvent>(64);

    // Indexer notify channel — maps file-system events to WsEvents
    let (note_event_tx, _) = broadcast::channel::<NoteEvent>(64);

    let indexer =
        Indexer::new(store_dyn, index_dyn, graph_dyn).with_notify_tx(note_event_tx.clone());
    indexer.initial_index().await?;

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
            ("task.md", "---\ntitle: \"Task\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"☑\"\ntag_properties: [\"Status\", \"Priority\", \"Deadline\", \"Scheduled\"]\ntags: []\n---\n- Task tag page.\n"),
            ("project.md", "---\ntitle: \"Project\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"🗂\"\ntag_properties: [\"Status\", \"Deadline\"]\ntags: []\n---\n- Project tag page.\n"),
            ("person.md", "---\ntitle: \"Person\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"👤\"\ntag_properties: [\"Email\", \"Team\"]\ntags: []\n---\n- Person tag page.\n"),
            ("status.md", "---\ntitle: \"Status\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"backlog\", \"todo\", \"doing\", \"in-review\", \"done\", \"canceled\"]\ndefault: \"todo\"\ntags: []\n---\n- Status property.\n"),
            ("priority.md", "---\ntitle: \"Priority\"\ntype: \"Property\"\nvalue_type: \"select\"\nchoices: [\"critical\", \"high\", \"medium\", \"low\"]\ndefault: \"medium\"\ntags: []\n---\n- Priority property.\n"),
            ("deadline.md", "---\ntitle: \"Deadline\"\ntype: \"Property\"\nvalue_type: \"date\"\ntags: []\n---\n- Deadline property.\n"),
            ("scheduled.md", "---\ntitle: \"Scheduled\"\ntype: \"Property\"\nvalue_type: \"date\"\ntags: []\n---\n- Scheduled property.\n"),

            // Life OS types
            ("domain.md", "---\ntitle: \"Domain\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"globe\"\ntag_properties: [\"Description\"]\ntags: []\n---\n- Top-level life area (Work, Family, Health, Home, etc.).\n"),
            ("lifeproject.md", "---\ntitle: \"LifeProject\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"folder\"\ntag_properties: [\"Status\", \"DomainRef\", \"Deadline\", \"Description\"]\ntags: []\n---\n- Multi-task effort within a domain.\n"),
            ("issue.md", "---\ntitle: \"Issue\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"lightbulb\"\ntag_properties: [\"IssueStatus\", \"DomainRef\", \"Description\"]\ntags: []\n---\n- Deliberation item — needs thought, may become a project or task.\n"),
            ("ritual.md", "---\ntitle: \"Ritual\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"sparkles\"\ntag_properties: [\"Cadence\", \"DomainRef\"]\ntags: []\n---\n- Daily or recurring mental check-in.\n"),
            ("scheduleditem.md", "---\ntitle: \"ScheduledItem\"\ntype: \"Tag\"\nextends: \"Root Tag\"\nicon: \"calendar\"\ntag_properties: [\"Cadence\", \"DomainRef\", \"LastCompleted\"]\ntags: []\n---\n- Recurring task with a cadence.\n"),

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

    let app_state = AppState {
        mosaic_root: mosaic_for_shutdown.clone(),
        store,
        index,
        ws_tx,
        type_registry,
        auto_sync,
    };
    let router = routes::build(app_state);

    let addr = std::env::var("TESELA_SERVER_BIND").unwrap_or_else(|_| "127.0.0.1:7474".to_string());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("tesela-server listening on http://{}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(wait_for_shutdown_signal())
        .await?;

    indexer_handle.stop().await;

    // Phase 13.A.4 — auto-backup on clean shutdown. Runs after axum has
    // drained in-flight requests and the indexer has stopped, so the
    // mosaic is in a quiescent state. We deliberately do NOT block
    // shutdown indefinitely if backup fails — log + move on.
    if backup_cfg_for_shutdown.auto_on_quit {
        match auto_backup_on_quit(&mosaic_for_shutdown, &index_for_shutdown, &backup_cfg_for_shutdown).await
        {
            Ok(path) => info!("Auto-backup on shutdown: {}", path.display()),
            Err(e) => warn!("Auto-backup on shutdown failed: {}", e),
        }
    }

    Ok(())
}

/// Resolves when the OS asks us to shut down (SIGINT or SIGTERM). On
/// non-Unix only ctrl_c is wired; SIGTERM-equivalent handling would
/// need platform-specific code we don't ship.
async fn wait_for_shutdown_signal() {
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

async fn auto_backup_on_quit(
    mosaic: &std::path::Path,
    index: &Arc<SqliteIndex>,
    cfg: &BackupConfig,
) -> Result<PathBuf> {
    // Pre-stage the SQLite VACUUM INTO snapshot in-process while we
    // still hold the live index handle.
    let snapshot = tempfile::Builder::new()
        .prefix("tesela-vacuum-")
        .suffix(".db")
        .tempfile()?;
    let snap_path = snapshot.path().to_path_buf();
    index.vacuum_into(&snap_path).await?;

    let mosaic_owned = mosaic.to_path_buf();
    let cfg = cfg.clone();
    let snap_path_for_blocking = snap_path.clone();

    // tesela_backup is sync; offload to a blocking task so we don't
    // stall the runtime while git + sha hashing run.
    let outcome = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let destination = if let Some(remote) = cfg.git_remote.as_ref() {
            let branch = cfg.git_branch.clone().unwrap_or_else(|| "main".to_string());
            let mirror = mosaic_owned.join(".tesela").join("backups").join(".git-mirror");
            tesela_backup::Destination::Git {
                remote: remote.clone(),
                branch,
                local_mirror: mirror,
            }
        } else if let Some(path) = cfg.external_path.as_ref() {
            tesela_backup::Destination::External { path: path.clone() }
        } else {
            tesela_backup::Destination::Local
        };

        // Encrypt if destination is non-local and a keypair exists.
        let encryption = match &destination {
            tesela_backup::Destination::Local => tesela_backup::ManifestEncryption::None,
            _ => match tesela_backup::encrypt::load_identity_for_mosaic(&mosaic_owned)
                .map_err(|e| anyhow::anyhow!("{}", e))?
            {
                Some(id) => tesela_backup::ManifestEncryption::Age {
                    recipient: id.to_public().to_string(),
                },
                None => {
                    // No keypair — emit a warning but don't refuse to
                    // back up. Non-local destinations would be
                    // plaintext, which is suboptimal but better than
                    // failing the shutdown hook silently.
                    tracing::warn!(
                        "No age identity in Keychain for this mosaic; non-local backup will be unencrypted"
                    );
                    tesela_backup::ManifestEncryption::None
                }
            },
        };

        let outcome = tesela_backup::backup(
            &mosaic_owned,
            tesela_backup::BackupOptions {
                destination,
                validate: true,
                extra_files: vec![(".tesela/tesela.db".to_string(), snap_path_for_blocking)],
                retention: Some(tesela_backup::GfsPolicy::default()),
                encryption,
            },
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(outcome)
    })
    .await??;

    drop(snapshot);
    Ok(outcome.path)
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

fn find_mosaic() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(".tesela").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    anyhow::bail!("No mosaic found. Run 'tesela init' first.")
}

