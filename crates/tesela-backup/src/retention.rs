use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;

/// GFS retention: keep N daily, M weekly, K monthly.
///
/// Defaults match the approved plan: 7 daily, 4 weekly, 6 monthly.
#[derive(Debug, Clone, Copy)]
pub struct GfsPolicy {
    pub daily: usize,
    pub weekly: usize,
    pub monthly: usize,
}

impl Default for GfsPolicy {
    fn default() -> Self {
        Self {
            daily: 7,
            weekly: 4,
            monthly: 6,
        }
    }
}

/// Pruning result so callers can log + tests can assert.
#[derive(Debug, Default, Clone)]
pub struct PruneOutcome {
    pub kept: Vec<PathBuf>,
    pub removed: Vec<PathBuf>,
}

/// Apply GFS retention to a directory of `backup-YYYYMMDD-HHMMSS` entries.
/// Anything that doesn't parse as our timestamp pattern is left alone.
///
/// The algorithm:
/// 1. Sort all parseable backups newest-first.
/// 2. Take the most recent N as "daily" survivors.
/// 3. From what remains, walk by ISO week — keep one per week up to M.
/// 4. From what still remains, walk by month — keep one per month up to K.
/// 5. Anything not selected is removed.
pub fn prune_gfs(
    backup_root: &Path,
    policy: GfsPolicy,
    dry_run: bool,
) -> Result<PruneOutcome> {
    if !backup_root.exists() {
        return Ok(PruneOutcome::default());
    }

    let mut candidates: Vec<(PathBuf, DateTime<Local>)> = Vec::new();
    for entry in fs::read_dir(backup_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(ts) = parse_backup_name(&name) {
            candidates.push((path, ts));
        }
    }
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    let mut kept = Vec::new();
    let mut keep_set = std::collections::HashSet::new();

    for (path, _) in candidates.iter().take(policy.daily) {
        keep_set.insert(path.clone());
    }

    let mut last_week_key: Option<(i32, u32)> = None;
    let mut weekly_kept = 0usize;
    for (path, ts) in candidates.iter().skip(policy.daily) {
        if weekly_kept >= policy.weekly {
            break;
        }
        let iso = ts.iso_week();
        let key = (iso.year(), iso.week());
        if Some(key) != last_week_key {
            keep_set.insert(path.clone());
            last_week_key = Some(key);
            weekly_kept += 1;
        }
    }

    let mut last_month_key: Option<(i32, u32)> = None;
    let mut monthly_kept = 0usize;
    for (path, ts) in candidates.iter().skip(policy.daily) {
        if keep_set.contains(path) {
            continue;
        }
        if monthly_kept >= policy.monthly {
            break;
        }
        let key = (ts.year(), ts.month());
        if Some(key) != last_month_key {
            keep_set.insert(path.clone());
            last_month_key = Some(key);
            monthly_kept += 1;
        }
    }

    let mut removed = Vec::new();
    for (path, _) in &candidates {
        if keep_set.contains(path) {
            kept.push(path.clone());
        } else {
            if !dry_run {
                fs::remove_dir_all(path)?;
            }
            removed.push(path.clone());
        }
    }

    Ok(PruneOutcome { kept, removed })
}

/// Parse `backup-YYYYMMDD-HHMMSS` into a Local timestamp.
fn parse_backup_name(name: &str) -> Option<DateTime<Local>> {
    let stripped = name.strip_prefix("backup-")?;
    let naive = NaiveDateTime::parse_from_str(stripped, "%Y%m%d-%H%M%S").ok()?;
    Local.from_local_datetime(&naive).single()
}
