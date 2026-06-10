//! Restore drill — the certainty artifact for "if we corrupt the data,
//! the backup brings it ALL back".
//!
//! Two layers:
//!
//! 1. `engine_level_restore_drill` — drives a live LoroEngine, backs up
//!    the mosaic, NUKES it, restores from the (offsite-copied) backup,
//!    reopens the engine, and proves the CRDT history frontier
//!    (`doc_version`), the rendered content, and the sync identity are
//!    IDENTICAL. No reseed — a reseed would mint a disjoint lineage
//!    with a fresh version vector (the documented twin-clobber hazard),
//!    which the byte-equality on the encoded VV would catch.
//!
//! 2. `server_level_restore_drill` — the same drill end-to-end through
//!    the real tesela-server binary and HTTP API: create → backup →
//!    nuke → restore → relaunch → identical note + identity, and the
//!    restored server keeps accepting engine writes.
//!
//! Skipped on non-Unix (SIGTERM-based shutdown).

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

/// Recursive dir copy — simulates moving the backup to offsite media
/// before the primary disk "dies".
fn copy_dir(src: &Path, dst: &Path) {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry.unwrap();
        let rel = entry.path().strip_prefix(src).unwrap();
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).unwrap();
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn read_identity(mosaic: &Path) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    (
        fs::read(mosaic.join(".tesela/device_id.hex")).unwrap(),
        fs::read(mosaic.join(".tesela/group_id.hex")).unwrap(),
        fs::read(mosaic.join(".tesela/group_key.bin")).unwrap(),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn engine_level_restore_drill() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    fs::create_dir_all(mosaic.join("notes")).unwrap();
    fs::create_dir_all(mosaic.join(".tesela")).unwrap();
    fs::write(mosaic.join(".tesela/config.toml"), "[general]\n").unwrap();

    // ── Live engine with real ops (history, not just final state) ──
    let device = DeviceId::new_random();
    fs::write(mosaic.join(".tesela/device_id.hex"), device.to_hex()).unwrap();
    let group = tesela_sync::load_or_create_group_identity(&mosaic)
        .await
        .expect("group identity");
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshot_dir.clone(),
        Some(notes_dir.clone()),
    )
    .await
    .expect("open engine");

    let note_id: [u8; 16] = *uuid::Uuid::new_v4().as_bytes();
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("drill-note".into()),
            title: "Drill Note".into(),
            content: "- alpha\n- beta\n".into(),
            created_at_millis: 1_750_000_000_000,
        })
        .await
        .expect("first upsert");
    // A second op so the doc carries multi-op HISTORY, not one commit.
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("drill-note".into()),
            title: "Drill Note".into(),
            content: "- alpha\n- beta\n- gamma added later\n".into(),
            created_at_millis: 1_750_000_000_000,
        })
        .await
        .expect("second upsert");

    // ── Capture ground truth ──
    let version_before = engine
        .doc_version(note_id)
        .await
        .expect("doc version before");
    let rendered_before = engine
        .render_note_full(note_id)
        .await
        .expect("render before");
    let materialized_before = fs::read(notes_dir.join("drill-note.md")).expect("materialized md");
    let identity_before = read_identity(&mosaic);
    let group_id_before = group.group_id;
    drop(engine);

    // ── Backup, move it offsite, then NUKE the mosaic ──
    let outcome = tesela_backup::backup(
        &mosaic,
        tesela_backup::BackupOptions {
            retention: None,
            ..Default::default()
        },
    )
    .expect("backup");
    assert!(outcome.manifest.includes_loro_state());
    assert!(outcome.manifest.includes_sync_identity());

    let offsite = temp.path().join("offsite-backup");
    copy_dir(&outcome.path, &offsite);
    fs::remove_dir_all(&mosaic).expect("nuke the mosaic");
    assert!(!mosaic.exists());

    // ── Restore to the original location ──
    let restored = tesela_backup::restore(
        &offsite,
        &mosaic,
        tesela_backup::RestoreOptions {
            target_override: Some(mosaic.clone()),
            ..Default::default()
        },
    )
    .expect("restore");
    assert_eq!(restored.target, mosaic);

    // ── Reopen the engine on the restored mosaic — NO reseed ──
    let device_hex = fs::read_to_string(mosaic.join(".tesela/device_id.hex")).unwrap();
    let mut device_bytes = [0u8; 16];
    hex::decode_to_slice(device_hex.trim(), &mut device_bytes).expect("device hex");
    let device2 = DeviceId::from_bytes(device_bytes);
    assert_eq!(device2, device, "device identity must survive the drill");

    let engine2 = LoroEngine::with_dirs(
        device2,
        Arc::new(Hlc::new(device2)),
        snapshot_dir.clone(),
        Some(notes_dir.clone()),
    )
    .await
    .expect("reopen engine on restored mosaic");

    // Identical CRDT history frontier — the "no reseed, no twin
    // lineage" proof. A reseed would author fresh ops under a new
    // lineage and the encoded version vector would differ.
    let version_after = engine2
        .doc_version(note_id)
        .await
        .expect("doc version after restore");
    assert_eq!(
        version_before, version_after,
        "Loro version vector must be identical after restore (no reseed)"
    );

    // Identical rendered content, from the CRDT and on disk.
    let rendered_after = engine2
        .render_note_full(note_id)
        .await
        .expect("render after restore");
    assert_eq!(rendered_before, rendered_after);
    let materialized_after = fs::read(notes_dir.join("drill-note.md")).unwrap();
    assert_eq!(materialized_before, materialized_after);

    // Identical sync identity (device + group, including the group key).
    let identity_after = read_identity(&mosaic);
    assert_eq!(identity_before, identity_after);
    let group_after = tesela_sync::load_or_create_group_identity(&mosaic)
        .await
        .expect("group identity after restore");
    assert_eq!(group_id_before, group_after.group_id);

    // The restored engine keeps writing as the SAME lineage.
    engine2
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("drill-note".into()),
            title: "Drill Note".into(),
            content: "- alpha\n- beta\n- gamma added later\n- post-restore edit\n".into(),
            created_at_millis: 1_750_000_000_000,
        })
        .await
        .expect("post-restore write");
    let rendered_post = engine2.render_note_full(note_id).await.unwrap();
    assert!(rendered_post.contains("post-restore edit"));
}

// ───────────────────────── server-level drill ─────────────────────────

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

struct ServerGuard(Option<Child>);

impl ServerGuard {
    fn stop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let pid = child.id() as i32;
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let _ = child.wait();
        }
    }
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spawn_server(mosaic: &Path, addr: &str) -> ServerGuard {
    let child = Command::new(binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("TESELA_DISABLE_MDNS", "1")
        .env("TESELA_DISABLE_PEER_SYNC", "1")
        // The drill drives backups explicitly via POST /backups.
        .env("TESELA_BACKUP_ON_START", "0")
        .env("TESELA_BACKUP_INTERVAL_SECS", "0")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server");
    ServerGuard(Some(child))
}

#[tokio::test(flavor = "current_thread")]
async fn server_level_restore_drill() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    fs::create_dir_all(mosaic.join("notes")).unwrap();
    fs::create_dir_all(mosaic.join(".tesela")).unwrap();
    fs::write(
        mosaic.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )
    .unwrap();

    let port = pick_free_port();
    let addr = format!("127.0.0.1:{}", port);
    let base = format!("http://{}", addr);
    let client = reqwest::Client::new();

    // ── Phase 1: live server, engine-authored note, explicit backup ──
    let mut server = spawn_server(&mosaic, &addr);
    assert!(wait_for_port(&addr, Duration::from_secs(15)), "server #1");

    let note: serde_json::Value = client
        .post(format!("{}/notes", base))
        .json(&serde_json::json!({
            "title": "DR Drill",
            "content": "- the data we must not lose\n",
        }))
        .send()
        .await
        .expect("POST /notes")
        .error_for_status()
        .expect("created")
        .json()
        .await
        .expect("note json");
    let note_id = note["id"].as_str().expect("note id").to_string();

    let fetched: serde_json::Value = client
        .get(format!("{}/notes/{}", base, note_id))
        .send()
        .await
        .expect("GET note")
        .json()
        .await
        .expect("note json");
    let content_before = fetched["content"].as_str().expect("content").to_string();
    assert!(content_before.contains("the data we must not lose"));

    let run: serde_json::Value = client
        .post(format!("{}/backups", base))
        .json(&serde_json::json!({"destination": "local", "encrypt": false}))
        .send()
        .await
        .expect("POST /backups")
        .error_for_status()
        .expect("backup ran")
        .json()
        .await
        .expect("backup json");
    assert_eq!(run["validated"], serde_json::Value::Bool(true));
    let backup_path = PathBuf::from(run["path"].as_str().expect("path"));

    let identity_before = read_identity(&mosaic);

    server.stop();

    // ── Phase 2: offsite the backup, NUKE the mosaic, restore ──
    let offsite = temp.path().join("offsite-backup");
    copy_dir(&backup_path, &offsite);
    fs::remove_dir_all(&mosaic).expect("nuke the mosaic");

    tesela_backup::restore(
        &offsite,
        &mosaic,
        tesela_backup::RestoreOptions {
            target_override: Some(mosaic.clone()),
            ..Default::default()
        },
    )
    .expect("restore");

    // The authority came back with it.
    assert!(
        mosaic.join(".tesela/loro").is_dir(),
        "restored mosaic must contain Loro state"
    );
    assert_eq!(identity_before, read_identity(&mosaic));

    // ── Phase 3: relaunch on the restored mosaic — NO reseed env ──
    let port2 = pick_free_port();
    let addr2 = format!("127.0.0.1:{}", port2);
    let base2 = format!("http://{}", addr2);
    let _server2 = spawn_server(&mosaic, &addr2);
    assert!(wait_for_port(&addr2, Duration::from_secs(15)), "server #2");

    let fetched_after: serde_json::Value = client
        .get(format!("{}/notes/{}", base2, note_id))
        .send()
        .await
        .expect("GET note after restore")
        .error_for_status()
        .expect("note exists after restore")
        .json()
        .await
        .expect("note json");
    assert_eq!(
        fetched_after["content"].as_str().expect("content"),
        content_before,
        "rendered content must be IDENTICAL after the drill"
    );

    // And the restored server still accepts engine writes.
    client
        .post(format!("{}/notes", base2))
        .json(&serde_json::json!({
            "title": "Post Restore",
            "content": "- written after disaster recovery\n",
        }))
        .send()
        .await
        .expect("POST /notes after restore")
        .error_for_status()
        .expect("write works after restore");
}
