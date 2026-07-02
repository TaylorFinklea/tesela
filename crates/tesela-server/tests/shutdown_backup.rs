//! End-to-end test for the Phase 13.A.4 auto-backup-on-clean-shutdown
//! hook: spawn `tesela-server`, send it SIGTERM, and assert a new
//! manifest-validated backup directory landed in
//! `<mosaic>/.tesela/backups/`.
//!
//! Skipped on non-Unix (no SIGTERM there).

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;

fn make_fixture_mosaic(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("notes"))?;
    fs::create_dir_all(root.join("attachments"))?;
    fs::create_dir_all(root.join(".tesela"))?;
    fs::write(
        root.join("notes/2026-05-10.md"),
        "---\ntitle: 2026-05-10\n---\n- shutdown hook smoke test\n",
    )?;
    // Defaults are fine — auto_on_quit is true by default.
    fs::write(root.join(".tesela/config.toml"), "")?;
    Ok(())
}

fn spawn_server_child(mosaic: &Path, addr: &str) -> Child {
    Command::new(common::binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server")
}

#[test]
fn sigterm_triggers_validated_backup() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let (mut child, _addr, _base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });

    // SIGTERM should drive `wait_for_shutdown_signal` -> graceful
    // axum drain -> auto-backup-on-quit.
    let pid = child.id() as i32;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    let status = child.wait().expect("server exit");
    assert!(status.success(), "server exited non-zero");

    // The backup directory should now exist.
    let backups_root = mosaic.join(".tesela").join("backups");
    let mut entries: Vec<_> = fs::read_dir(&backups_root)
        .expect("backups dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("backup-"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    assert!(
        !entries.is_empty(),
        "no backup directory created under {}",
        backups_root.display()
    );

    let backup = entries.last().unwrap().path();
    let manifest = backup.join("manifest.json");
    assert!(
        manifest.exists(),
        "manifest.json missing at {}",
        manifest.display()
    );

    let raw = fs::read_to_string(&manifest).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(parsed["validated"]["ok"], true, "validated=true expected");
    assert!(
        backup.join("notes/2026-05-10.md").exists(),
        "captured note missing"
    );
}
