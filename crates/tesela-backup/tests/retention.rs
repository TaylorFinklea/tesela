//! GFS retention exercises across a fabricated 100-day backup history.
//! Builds bare directories matching `backup-YYYYMMDD-HHMMSS` and asserts
//! the prune algorithm keeps the right shape.

use chrono::{Duration, Local};
use std::fs;
use tempfile::TempDir;
use tesela_backup::{prune_gfs, GfsPolicy, DEFAULT_DAILY, DEFAULT_MONTHLY, DEFAULT_WEEKLY};

#[test]
fn gfs_keeps_seven_daily_then_weekly_then_monthly() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let now = Local::now();

    // Fabricate 100 daily backup directories, one per day going back.
    for day in 0..100 {
        let when = now - Duration::days(day);
        let name = format!("backup-{}", when.format("%Y%m%d-%H%M%S"));
        fs::create_dir_all(root.join(&name)).unwrap();
    }

    let outcome = prune_gfs(root, GfsPolicy::default(), false).unwrap();

    // DEFAULT_DAILY + DEFAULT_WEEKLY + DEFAULT_MONTHLY = 17 survivors
    // at most. The monthly tier may collide with the weekly tier on
    // month boundaries, so the actual upper bound is exactly 17 when
    // the window is wide enough to fit all three tiers without
    // overlap.
    assert!(
        outcome.kept.len() >= 13 && outcome.kept.len() <= 17,
        "expected ~13-17 survivors, got {}",
        outcome.kept.len()
    );
    assert_eq!(
        outcome.kept.len() + outcome.removed.len(),
        100,
        "every directory should be classified"
    );

    // The DEFAULT_DAILY most recent must all be kept.
    for day in 0..DEFAULT_DAILY as i64 {
        let when = now - Duration::days(day);
        let name = format!("backup-{}", when.format("%Y%m%d-%H%M%S"));
        let expected = root.join(&name);
        assert!(
            outcome.kept.iter().any(|p| p == &expected),
            "missing daily survivor {}",
            name
        );
        assert!(expected.exists(), "{} should still exist on disk", name);
    }

    // Survivors persist on disk; non-survivors are gone.
    for path in &outcome.kept {
        assert!(path.exists(), "{} should still exist", path.display());
    }
    for path in &outcome.removed {
        assert!(!path.exists(), "{} should be removed", path.display());
    }
}

#[test]
fn gfs_dry_run_does_not_delete() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let now = Local::now();
    for day in 0..30 {
        let when = now - Duration::days(day);
        let name = format!("backup-{}", when.format("%Y%m%d-%H%M%S"));
        fs::create_dir_all(root.join(&name)).unwrap();
    }

    // Drive the test off the named constants so a future change to
    // the default cadence doesn't silently leave the test lagging
    // behind production.
    assert!(DEFAULT_DAILY > 0);
    assert!(DEFAULT_WEEKLY > 0);
    assert!(DEFAULT_MONTHLY > 0);
    let policy = GfsPolicy {
        daily: DEFAULT_DAILY,
        weekly: DEFAULT_WEEKLY,
        monthly: DEFAULT_MONTHLY,
    };

    let outcome = prune_gfs(root, policy, true).unwrap();
    assert!(!outcome.removed.is_empty());
    for path in &outcome.removed {
        assert!(path.exists(), "dry run kept {} on disk", path.display());
    }
}

#[test]
fn gfs_ignores_non_matching_directories() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    fs::create_dir_all(root.join("not-a-backup")).unwrap();
    fs::create_dir_all(root.join("backup-stragglers")).unwrap();
    fs::create_dir_all(root.join("backup-20260510-120000")).unwrap();

    let outcome = prune_gfs(root, GfsPolicy::default(), false).unwrap();
    // Only the one parseable backup is classified; the two unparseable
    // dirs are left untouched (not in either kept or removed).
    assert_eq!(outcome.kept.len() + outcome.removed.len(), 1);
    assert!(root.join("not-a-backup").exists());
    assert!(root.join("backup-stragglers").exists());
}
