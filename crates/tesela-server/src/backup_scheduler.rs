//! Periodic in-server backups + the shared status that makes them
//! PROVABLE (`GET /backup/status`).
//!
//! Mirrors the notifications-scanner pattern (`notifications::start`):
//! one spawned tokio task, an interval loop, failures logged and
//! retried on the next tick. Every run goes through the same
//! `run_configured_backup` the shutdown hook uses, so destination
//! (local / external / git) and encryption policy are identical across
//! all triggers.
//!
//! Cadence and retention are env-tunable:
//!
//! - `TESELA_BACKUP_INTERVAL_SECS`  — default 21600 (6h); `0` disables
//!   the periodic loop (startup backup still honors ON_START).
//! - `TESELA_BACKUP_ON_START`      — default on; `0`/empty disables the
//!   one-shot backup taken after server bring-up.
//! - `TESELA_BACKUP_STARTUP_DELAY_SECS` — default 15; lets bring-up
//!   (initial index, relay tick) settle before the startup backup.
//! - `TESELA_BACKUP_KEEP_DAILY` / `_WEEKLY` / `_MONTHLY` — GFS retention
//!   overrides (defaults 7/4/6). "Daily" is the most-recent-N bucket.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Local};
use tesela_core::config::BackupConfig;
use tesela_core::db::SqliteIndex;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Default cadence: every 6 hours.
pub const DEFAULT_INTERVAL_SECS: u64 = 6 * 60 * 60;
/// Default settle time before the startup backup.
pub const DEFAULT_STARTUP_DELAY_SECS: u64 = 15;

pub const ENV_INTERVAL: &str = "TESELA_BACKUP_INTERVAL_SECS";
pub const ENV_ON_START: &str = "TESELA_BACKUP_ON_START";
pub const ENV_STARTUP_DELAY: &str = "TESELA_BACKUP_STARTUP_DELAY_SECS";
pub const ENV_KEEP_DAILY: &str = "TESELA_BACKUP_KEEP_DAILY";
pub const ENV_KEEP_WEEKLY: &str = "TESELA_BACKUP_KEEP_WEEKLY";
pub const ENV_KEEP_MONTHLY: &str = "TESELA_BACKUP_KEEP_MONTHLY";

/// Scheduler knobs, resolved once at startup.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Seconds between periodic backups. `0` = periodic loop disabled.
    pub interval_secs: u64,
    /// Take one backup shortly after server bring-up.
    pub backup_on_start: bool,
    /// Delay before that startup backup.
    pub startup_delay_secs: u64,
    /// GFS retention applied after each successful run.
    pub policy: tesela_backup::GfsPolicy,
}

impl SchedulerConfig {
    pub fn from_env() -> Self {
        Self::parse(
            std::env::var(ENV_INTERVAL).ok().as_deref(),
            std::env::var(ENV_ON_START).ok().as_deref(),
            std::env::var(ENV_STARTUP_DELAY).ok().as_deref(),
            std::env::var(ENV_KEEP_DAILY).ok().as_deref(),
            std::env::var(ENV_KEEP_WEEKLY).ok().as_deref(),
            std::env::var(ENV_KEEP_MONTHLY).ok().as_deref(),
        )
    }

    /// Pure parse so tests don't race on process-global env vars.
    /// Unparseable values fall back to the defaults (never panic the
    /// server over a typo'd knob).
    fn parse(
        interval: Option<&str>,
        on_start: Option<&str>,
        startup_delay: Option<&str>,
        daily: Option<&str>,
        weekly: Option<&str>,
        monthly: Option<&str>,
    ) -> Self {
        let parse_u64 = |raw: Option<&str>, default: u64| -> u64 {
            raw.and_then(|s| s.trim().parse::<u64>().ok())
                .unwrap_or(default)
        };
        let parse_usize = |raw: Option<&str>, default: usize| -> usize {
            raw.and_then(|s| s.trim().parse::<usize>().ok())
                .unwrap_or(default)
        };
        let defaults = tesela_backup::GfsPolicy::default();
        Self {
            interval_secs: parse_u64(interval, DEFAULT_INTERVAL_SECS),
            // Anything but an explicit "0"/"false"/empty keeps the
            // startup backup on.
            backup_on_start: !matches!(
                on_start.map(str::trim),
                Some("0") | Some("false") | Some("")
            ),
            startup_delay_secs: parse_u64(startup_delay, DEFAULT_STARTUP_DELAY_SECS),
            policy: tesela_backup::GfsPolicy {
                daily: parse_usize(daily, defaults.daily),
                weekly: parse_usize(weekly, defaults.weekly),
                monthly: parse_usize(monthly, defaults.monthly),
            },
        }
    }
}

/// One completed scheduler run (success or failure), for `/backup/status`.
#[derive(Debug, Clone)]
pub struct RunRecord {
    pub at: DateTime<Local>,
    pub ok: bool,
    /// Backup path on success, error message on failure.
    pub detail: String,
    /// "startup" | "scheduled" | "shutdown"
    pub trigger: &'static str,
}

#[derive(Debug, Default)]
pub struct StatusInner {
    pub last_run: Option<RunRecord>,
    pub last_error: Option<RunRecord>,
    pub next_scheduled_at: Option<DateTime<Local>>,
}

/// Shared between the scheduler task, the shutdown hook, and the
/// `/backup/status` route.
#[derive(Clone)]
pub struct BackupStatusHandle {
    pub inner: Arc<RwLock<StatusInner>>,
    pub config: SchedulerConfig,
}

impl BackupStatusHandle {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(StatusInner::default())),
            config,
        }
    }

    pub fn enabled(&self) -> bool {
        self.config.interval_secs > 0 || self.config.backup_on_start
    }

    async fn record(&self, record: RunRecord) {
        let mut inner = self.inner.write().await;
        if !record.ok {
            inner.last_error = Some(record.clone());
        }
        inner.last_run = Some(record);
    }

    async fn set_next(&self, next: Option<DateTime<Local>>) {
        self.inner.write().await.next_scheduled_at = next;
    }
}

/// Build the `Destination` + encryption the backup config asks for and
/// run one backup. Shared by the scheduler, the shutdown hook, and any
/// future trigger — one policy, every path. The SQLite snapshot is
/// VACUUMed in-process (we hold the live index handle) and handed to
/// the sync `tesela_backup` crate as an extra file.
pub async fn run_configured_backup(
    mosaic: &Path,
    index: &Arc<SqliteIndex>,
    cfg: &BackupConfig,
    retention: Option<tesela_backup::GfsPolicy>,
) -> anyhow::Result<tesela_backup::BackupOutcome> {
    let snapshot = tempfile::Builder::new()
        .prefix("tesela-vacuum-")
        .suffix(".db")
        .tempfile()?;
    let snap_path = snapshot.path().to_path_buf();
    index.vacuum_into(&snap_path).await?;

    let mosaic_owned = mosaic.to_path_buf();
    let cfg = cfg.clone();

    // tesela_backup is sync; offload so git + sha hashing don't stall
    // the runtime.
    let outcome = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let destination = destination_from_config(&mosaic_owned, &cfg);

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
                    // No keypair — warn but don't refuse to back up.
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
                extra_files: vec![(".tesela/tesela.db".to_string(), snap_path)],
                retention,
                encryption,
            },
        )
        .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(outcome)
    })
    .await??;

    drop(snapshot);
    Ok(outcome)
}

/// Map the user's `[backup]` config onto a destination, identically for
/// every trigger.
pub fn destination_from_config(mosaic: &Path, cfg: &BackupConfig) -> tesela_backup::Destination {
    if let Some(remote) = cfg.git_remote.as_ref() {
        let branch = cfg.git_branch.clone().unwrap_or_else(|| "main".to_string());
        let mirror = mosaic.join(".tesela").join("backups").join(".git-mirror");
        tesela_backup::Destination::Git {
            remote: remote.clone(),
            branch,
            local_mirror: mirror,
        }
    } else if let Some(path) = cfg.external_path.as_ref() {
        tesela_backup::Destination::External { path: path.clone() }
    } else {
        tesela_backup::Destination::Local
    }
}

/// Spawn the periodic backup task. Returns immediately; the task runs
/// for the process lifetime. Pattern mirrors `notifications::start`.
pub fn start(
    status: BackupStatusHandle,
    mosaic: PathBuf,
    index: Arc<SqliteIndex>,
    cfg: BackupConfig,
) {
    let sched = status.config.clone();
    if !status.enabled() {
        info!(
            "backup scheduler disabled ({}=0, {}=0)",
            ENV_INTERVAL, ENV_ON_START
        );
        return;
    }
    tokio::spawn(async move {
        if sched.backup_on_start {
            if sched.startup_delay_secs > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(sched.startup_delay_secs)).await;
            }
            run_once(&status, &mosaic, &index, &cfg, "startup").await;
        }
        if sched.interval_secs == 0 {
            status.set_next(None).await;
            return;
        }
        let period = std::time::Duration::from_secs(sched.interval_secs);
        loop {
            let next = Local::now() + chrono::Duration::seconds(sched.interval_secs as i64);
            status.set_next(Some(next)).await;
            tokio::time::sleep(period).await;
            run_once(&status, &mosaic, &index, &cfg, "scheduled").await;
        }
    });
}

/// One scheduler-driven backup: run, log, record. Failures (including
/// a held `.backup.lock` from a concurrent manual backup) are recorded
/// and retried on the next tick — never fatal.
async fn run_once(
    status: &BackupStatusHandle,
    mosaic: &Path,
    index: &Arc<SqliteIndex>,
    cfg: &BackupConfig,
    trigger: &'static str,
) {
    match run_configured_backup(mosaic, index, cfg, Some(status.config.policy)).await {
        Ok(outcome) => {
            let total_bytes: u64 = outcome.manifest.files.iter().map(|f| f.size).sum();
            info!(
                "backup ({trigger}): {} — {} files, {} bytes, validated={}, pruned {}",
                outcome.path.display(),
                outcome.manifest.files.len(),
                total_bytes,
                outcome.manifest.validated.as_ref().map(|v| v.ok).unwrap_or(false),
                outcome.pruned.removed.len(),
            );
            status
                .record(RunRecord {
                    at: Local::now(),
                    ok: true,
                    detail: outcome.path.to_string_lossy().into_owned(),
                    trigger,
                })
                .await;
        }
        Err(e) => {
            warn!("backup ({trigger}) failed: {e}");
            status
                .record(RunRecord {
                    at: Local::now(),
                    ok: false,
                    detail: e.to_string(),
                    trigger,
                })
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_defaults_when_unset() {
        let cfg = SchedulerConfig::parse(None, None, None, None, None, None);
        assert_eq!(cfg.interval_secs, DEFAULT_INTERVAL_SECS);
        assert!(cfg.backup_on_start);
        assert_eq!(cfg.startup_delay_secs, DEFAULT_STARTUP_DELAY_SECS);
        assert_eq!(cfg.policy.daily, 7);
        assert_eq!(cfg.policy.weekly, 4);
        assert_eq!(cfg.policy.monthly, 6);
    }

    #[test]
    fn parse_overrides() {
        let cfg = SchedulerConfig::parse(
            Some("2"),
            Some("0"),
            Some("0"),
            Some("10"),
            Some("2"),
            Some("3"),
        );
        assert_eq!(cfg.interval_secs, 2);
        assert!(!cfg.backup_on_start);
        assert_eq!(cfg.startup_delay_secs, 0);
        assert_eq!(cfg.policy.daily, 10);
        assert_eq!(cfg.policy.weekly, 2);
        assert_eq!(cfg.policy.monthly, 3);
    }

    #[test]
    fn parse_garbage_falls_back_to_defaults() {
        let cfg = SchedulerConfig::parse(
            Some("six hours"),
            Some("yes"),
            Some("-3"),
            Some("a"),
            Some(""),
            Some("NaN"),
        );
        assert_eq!(cfg.interval_secs, DEFAULT_INTERVAL_SECS);
        // "yes" is not an explicit off → stays on.
        assert!(cfg.backup_on_start);
        assert_eq!(cfg.startup_delay_secs, DEFAULT_STARTUP_DELAY_SECS);
        assert_eq!(cfg.policy.daily, 7);
    }

    #[test]
    fn zero_interval_with_on_start_still_enabled() {
        let cfg = SchedulerConfig::parse(Some("0"), Some("1"), None, None, None, None);
        let handle = BackupStatusHandle::new(cfg);
        assert!(handle.enabled());
        let cfg = SchedulerConfig::parse(Some("0"), Some("0"), None, None, None, None);
        let handle = BackupStatusHandle::new(cfg);
        assert!(!handle.enabled());
    }
}
