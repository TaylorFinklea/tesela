//! HTTP-level backup → list → verify → restore round-trip.
//!
//! Trust artifact for the **web UI**: proves the endpoints the
//! `BackupSettings.svelte` component drives deliver the same lossless
//! round-trip the CLI integration test verifies for `tesela backup`
//! and `tesela restore`. If this is green, the web UI is as
//! trustworthy as the CLI for real notes.
//!
//! Skipped on non-Unix (the test spawns the server binary and uses
//! SIGTERM to shut it down cleanly).

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
    fs::create_dir_all(root.join("attachments"))?;
    fs::create_dir_all(root.join(".tesela"))?;
    fs::write(
        root.join("notes/2026-05-19.md"),
        "---\ntitle: 2026-05-19\n---\n- hello\n- world\n",
    )?;
    fs::write(
        root.join("notes/page-with-link.md"),
        "---\ntitle: Linked\n---\n- See [[2026-05-19]]\n",
    )?;
    fs::write(root.join("attachments/photo.jpg"), b"\xff\xd8\xffFAKEJPG")?;
    // auto_on_quit defaults to true; explicitly disable so this test
    // exercises only the HTTP-driven backup, not the shutdown hook.
    fs::write(
        root.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )?;
    Ok(())
}

/// Owns the spawned server process and SIGTERMs it on drop so the
/// server is reaped even if the test panics mid-flight.
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

fn spawn_server(mosaic: &Path, addr: &str) -> ServerGuard {
    let child = Command::new(binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server");
    ServerGuard(Some(child))
}

/// The captured-set definition matches the backup pipeline's policy
/// (notes/, attachments/, templates/, .tesela/config.toml). Transient
/// state (.tesela/.lock, search history, rebuildable SQLite db) is
/// intentionally not in the backup.
fn captured(rel: &Path) -> bool {
    let s = rel.to_string_lossy();
    s.starts_with("notes/")
        || s.starts_with("attachments/")
        || s.starts_with("templates/")
        || s == ".tesela/config.toml"
}

#[tokio::test(flavor = "current_thread")]
async fn http_backup_list_verify_restore_round_trip() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let base = format!("http://{}", addr);
    // _server is the RAII guard — SIGTERMs the child on drop, including
    // if any assertion below panics.
    let _server = spawn_server(&mosaic, &addr);

    assert!(
        wait_for_port(&addr, Duration::from_secs(15)),
        "server never bound to {}",
        addr
    );

    let client = reqwest::Client::new();
    let base = &base;
    let mosaic = mosaic.as_path();

    // 1. Run a local (unencrypted) backup via the same endpoint
    //    BackupSettings.svelte hits when the user clicks "Run backup
    //    now" with destination=local.
    let run: serde_json::Value = client
        .post(format!("{}/backups", base))
        .json(&serde_json::json!({
            "destination": "local",
            "encrypt": false,
        }))
        .send()
        .await
        .expect("POST /backups")
        .error_for_status()
        .expect("backup ran")
        .json()
        .await
        .expect("backup response json");

    let backup_path = run["path"].as_str().expect("path in response");
    let backup_name = Path::new(backup_path)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();

    // 2. List endpoint must show the new backup.
    let list: serde_json::Value = client
        .get(format!("{}/backups", base))
        .send()
        .await
        .expect("GET /backups")
        .json()
        .await
        .expect("list json");
    let names: Vec<String> = list
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|item| item["name"].as_str().map(str::to_string))
        .collect();
    assert!(
        names.contains(&backup_name),
        "new backup {} not in list {:?}",
        backup_name,
        names
    );

    // 3. Verify endpoint must report ok.
    let verify: serde_json::Value = client
        .post(format!("{}/backups/{}/verify", base, backup_name))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("POST /backups/<n>/verify")
        .error_for_status()
        .expect("verify ran")
        .json()
        .await
        .expect("verify json");
    assert_eq!(
        verify["ok"],
        serde_json::Value::Bool(true),
        "verify failed: {:?}",
        verify
    );

    // 4. Restore endpoint into the default sibling location (in_place
    //    = false). This is what the web UI's "Restore → sibling"
    //    button drives.
    let restore: serde_json::Value = client
        .post(format!("{}/backups/{}/restore", base, backup_name))
        .json(&serde_json::json!({"in_place": false, "allow_newer": false}))
        .send()
        .await
        .expect("POST /backups/<n>/restore")
        .error_for_status()
        .expect("restore ran")
        .json()
        .await
        .expect("restore json");
    let target = restore["target"]
        .as_str()
        .expect("restore target")
        .to_string();
    let restored = PathBuf::from(&target);
    assert!(
        restored.exists(),
        "restore target {} doesn't exist",
        restored.display()
    );

    // 5. Byte-exact diff across the captured set. The whole point.
    let mut captured_files: Vec<PathBuf> = walkdir::WalkDir::new(mosaic)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(mosaic).unwrap().to_path_buf())
        .filter(|rel| captured(rel))
        .collect();
    captured_files.sort();
    assert!(
        !captured_files.is_empty(),
        "fixture should produce captured files"
    );
    for rel in &captured_files {
        let orig = fs::read(mosaic.join(rel))
            .unwrap_or_else(|e| panic!("read original {}: {}", rel.display(), e));
        let rest = fs::read(restored.join(rel)).unwrap_or_else(|e| {
            panic!(
                "restored missing {} (under {}): {}",
                rel.display(),
                restored.display(),
                e
            )
        });
        assert_eq!(
            orig,
            rest,
            "byte mismatch in {} after HTTP-driven restore",
            rel.display()
        );
    }

    // Touch the tempdir so the compiler keeps it alive until here.
    let _ = temp.path();
}

// (round_trip moved inline into the test fn above.)
