//! Scheduled backups are PROVABLE: spawn the server with a fast cadence,
//! create a note through the engine, and assert `GET /backup/status`
//! reports a fresh validated backup that captured the authority (Loro
//! state + sync identity) — plus a next-scheduled time and on-disk truth.
//!
//! Skipped on non-Unix (SIGTERM-based shutdown in the drop guard).

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

fn binary_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tesela-server"))
}

fn pick_free_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn wait_for_port(addr: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(addr).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn make_fixture_mosaic(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("notes"))?;
    fs::create_dir_all(root.join(".tesela"))?;
    fs::write(
        root.join("notes/2026-06-10.md"),
        "---\ntitle: 2026-06-10\n---\n- scheduled backup smoke\n",
    )?;
    // Only the scheduler should produce backups in this test.
    fs::write(
        root.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )?;
    Ok(())
}

struct ServerGuard(Option<Child>);

impl Drop for ServerGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let pid = child.id() as i32;
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let _ = child.wait();
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn scheduled_backup_appears_in_status_with_authority() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let base = format!("http://{}", addr);

    let child = Command::new(binary_path())
        .current_dir(&mosaic)
        .env("TESELA_SERVER_BIND", &addr)
        .env("TESELA_DISABLE_MDNS", "1")
        .env("TESELA_DISABLE_PEER_SYNC", "1")
        // Fast cadence so the test observes a scheduled (not just
        // startup) backup within seconds.
        .env("TESELA_BACKUP_INTERVAL_SECS", "2")
        .env("TESELA_BACKUP_ON_START", "1")
        .env("TESELA_BACKUP_STARTUP_DELAY_SECS", "0")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server");
    let _server = ServerGuard(Some(child));

    assert!(
        wait_for_port(&addr, Duration::from_secs(60)),
        "server never bound to {}",
        addr
    );

    let client = reqwest::Client::new();

    // Push a note through the engine so the mosaic has live Loro state
    // (a per-note snapshot in .tesela/loro/) for the next backup tick
    // to capture.
    client
        .post(format!("{}/notes", base))
        .json(&serde_json::json!({
            "title": "Authority Note",
            "content": "- this block lives in the CRDT\n",
        }))
        .send()
        .await
        .expect("POST /notes")
        .error_for_status()
        .expect("note created");

    // Poll /backup/status until a backup taken AFTER the note exists
    // shows up (the 2s cadence guarantees one quickly).
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        assert!(
            Instant::now() < deadline,
            "no scheduled backup with authority appeared within 30s"
        );
        let status: serde_json::Value = client
            .get(format!("{}/backup/status", base))
            .send()
            .await
            .expect("GET /backup/status")
            .error_for_status()
            .expect("status 200")
            .json()
            .await
            .expect("status json");
        let latest = &status["latest"];
        // Wait for a backup that is both captured (loro state present) AND
        // validated — validation is a separate pass after the files land, so
        // under parallel test load the status can briefly report a captured
        // but not-yet-validated backup. Asserting on that window is the flake.
        if latest.is_object()
            && latest["includes_loro_state"] == serde_json::Value::Bool(true)
            && latest["validated"] == serde_json::Value::Bool(true)
        {
            break status;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    };

    // ---- Provability assertions ----
    let latest = &status["latest"];
    assert_eq!(latest["validated"], serde_json::Value::Bool(true));
    assert_eq!(
        latest["includes_sync_identity"],
        serde_json::Value::Bool(true),
        "device_id.hex must be captured: {status}"
    );
    assert!(
        latest["total_bytes"].as_u64().unwrap_or(0) > 0,
        "size must be reported"
    );
    assert!(
        latest["contents"]["loro_docs"].as_u64().unwrap_or(0) >= 1,
        "contents manifest must count loro docs: {status}"
    );
    assert!(
        latest["contents"]["notes"].as_u64().unwrap_or(0) >= 1,
        "contents manifest must count notes"
    );
    let backup_path = PathBuf::from(latest["path"].as_str().expect("path"));
    assert!(backup_path.exists(), "reported backup path must exist");
    assert!(
        backup_path.join(".tesela/device_id.hex").exists(),
        "backup on disk must contain the sync identity"
    );

    // Scheduler block: enabled, correct cadence, a next run on the books,
    // and a recorded last run.
    let sched = &status["scheduler"];
    assert_eq!(sched["enabled"], serde_json::Value::Bool(true));
    assert_eq!(sched["interval_secs"].as_u64(), Some(2));
    assert!(
        sched["next_scheduled_at"].as_str().is_some(),
        "next_scheduled_at must be set while the scheduler runs: {status}"
    );
    assert!(
        sched["last_run"]["at"].as_str().is_some(),
        "scheduler must record its last run: {status}"
    );
    assert_eq!(sched["last_run"]["ok"], serde_json::Value::Bool(true));

    // On-disk truth the status is derived from.
    assert!(status["backup_count"].as_u64().unwrap_or(0) >= 1);
}
