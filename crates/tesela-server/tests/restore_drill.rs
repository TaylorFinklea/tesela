//! Restore drill — the certainty artifact for "if we corrupt the data,
//! the backup brings it ALL back".
//!
//! Three layers:
//!
//! 1. `engine_level_restore_drill` — drives a live LoroEngine, backs up
//!    the mosaic, NUKES it, restores from the (offsite-copied) backup,
//!    reopens the engine, and proves LINEAGE survived — not just
//!    counters. The fixture uses the REAL note-id space (blake3 of the
//!    slug, the same derivation `reseed_from_disk` uses) and carries an
//!    op authored by a SECOND device in the doc's history. The decoded
//!    version vector must still contain that second peer's ops after
//!    restore: a reseed re-authors everything from the rendered
//!    markdown under THIS device's peer, reproducing the same note id,
//!    the same text, and (single-peer) even the same VV — but it can
//!    NEVER reproduce another peer's ops.
//!
//! 2. `reseed_cannot_reproduce_restored_lineage` — the perturb-proof,
//!    encoded permanently: the same drill with the reseed path swapped
//!    in (blow away `.tesela/loro`, reopen, `reseed_from_disk`). Same
//!    note id, same rendered text — and the lineage assertion FAILS
//!    (peer B's entry is gone). If this test ever breaks, the drill's
//!    certainty assertion has gone vacuous again.
//!
//! 3. `server_level_restore_drill` — the same drill end-to-end through
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
use std::time::Duration;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::ServerGuard;

use loro::VersionVector;
use tesela_sync::{DeviceId, GroupId, Hlc, LoroEngine, OpPayload, SyncEngine};

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

/// The slug every real note id is derived from — `derive_note_id`
/// (routes/notes.rs) and `reseed_from_disk` both compute
/// `blake3(slug)[..16]`. Using this space in the fixture means a
/// reseed reproduces the SAME note id, so id presence proves nothing —
/// only lineage does.
fn note_id_for_slug(slug: &str) -> [u8; 16] {
    tesela_core::stable_uuid_from_slug(slug)
}

/// Loro PeerID for a DeviceId — mirrors `LoroEngine::peer_id` (first 8
/// LE bytes, top bit cleared, 0 maps to 1).
fn peer_of(device: &DeviceId) -> u64 {
    let b = device.as_bytes();
    let raw = u64::from_le_bytes(b[..8].try_into().unwrap());
    let masked = raw & 0x7FFF_FFFF_FFFF_FFFF;
    if masked == 0 {
        1
    } else {
        masked
    }
}

const DRILL_SLUG: &str = "drill-note";
const DEVICE_B_LINE: &str = "delta authored by device B";

/// Ground truth captured before the disaster.
struct SeededMosaic {
    device: DeviceId,
    device_b: DeviceId,
    note_id: [u8; 16],
    version_before: Vec<u8>,
    rendered_before: String,
    materialized_before: Vec<u8>,
    identity_before: (Vec<u8>, Vec<u8>, Vec<u8>),
    group_id_before: GroupId,
}

/// Build the drill fixture: a live engine mosaic whose note carries
/// multi-op history from TWO peers. Device A authors two upserts;
/// device B (a second in-memory engine on the same Loro lineage)
/// authors a third op that is merged back into A's doc. Peer B's ops
/// in the version vector are the lineage marker no reseed can mint.
async fn seed_two_peer_mosaic(mosaic: &Path) -> SeededMosaic {
    // Hermetic test mosaic: keep the group key on the plaintext file store
    // rather than the real macOS Keychain (tesela-tp0.2's Keychain cutover
    // defaults there) — this test drives the identity round-trip directly.
    std::env::set_var("TESELA_GROUP_KEY_FILE_STORE", "1");
    fs::create_dir_all(mosaic.join("notes")).unwrap();
    fs::create_dir_all(mosaic.join(".tesela")).unwrap();
    fs::write(mosaic.join(".tesela/config.toml"), "[general]\n").unwrap();

    let device = DeviceId::new_random();
    fs::write(mosaic.join(".tesela/device_id.hex"), device.to_hex()).unwrap();
    let group = tesela_sync::load_or_create_group_identity(mosaic)
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

    let note_id = note_id_for_slug(DRILL_SLUG);
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(DRILL_SLUG.into()),
            title: "Drill Note".into(),
            content: "- alpha\n- beta\n".into(),
            created_at_millis: 1_750_000_000_000,
        })
        .await
        .expect("first upsert");
    // A second op so the doc carries multi-op HISTORY, not one commit. Use
    // the stamped materialization and assign the new block an explicit bid,
    // matching the identity-preserving whole-note edit path real clients use.
    let second_content = format!(
        "{}- gamma added later <!-- bid:33333333-3333-3333-3333-333333333333 -->\n",
        engine.render_note(note_id).await.expect("render first upsert")
    );
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(DRILL_SLUG.into()),
            title: "Drill Note".into(),
            content: second_content,
            created_at_millis: 1_750_000_000_000,
        })
        .await
        .expect("second upsert");

    // ── An op from a SECOND peer in the doc's history ──
    // Bootstrap engine B from A's snapshot (shared lineage), author a
    // block there, merge the delta back into A. Restore must bring
    // peer B's ops back; a reseed can only author under peer A.
    let device_b = DeviceId::new_random();
    assert_ne!(
        peer_of(&device),
        peer_of(&device_b),
        "fixture needs two distinct Loro peers"
    );
    let engine_b = LoroEngine::new(device_b, Arc::new(Hlc::new(device_b)));
    let full = engine
        .export_doc_update(note_id, None)
        .await
        .expect("export full snapshot for B");
    engine_b
        .import_doc_update(note_id, &full)
        .await
        .expect("B bootstraps from A");
    let device_b_content = format!(
        "{}- {} <!-- bid:44444444-4444-4444-4444-444444444444 -->\n",
        engine_b
            .render_note(note_id)
            .await
            .expect("B renders imported note"),
        DEVICE_B_LINE
    );
    engine_b
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(DRILL_SLUG.into()),
            title: "Drill Note".into(),
            content: device_b_content,
            created_at_millis: 1_750_000_000_000,
        })
        .await
        .expect("B authors");
    let vv_a = engine.doc_version(note_id).await.expect("A version");
    let delta = engine_b
        .export_doc_update(note_id, Some(&vv_a))
        .await
        .expect("B delta since A");
    engine
        .import_doc_update(note_id, &delta)
        .await
        .expect("A merges B's op");

    // ── Capture ground truth ──
    let version_before = engine
        .doc_version(note_id)
        .await
        .expect("doc version before");
    let rendered_before = engine
        .render_note_full(note_id)
        .await
        .expect("render before");
    assert!(
        rendered_before.contains(DEVICE_B_LINE),
        "fixture: B's edit must be in the merged note"
    );
    let materialized_before =
        fs::read(notes_dir.join(format!("{}.md", DRILL_SLUG))).expect("materialized md");
    let identity_before = read_identity(mosaic);
    let group_id_before = group.group_id;
    drop(engine);

    SeededMosaic {
        device,
        device_b,
        note_id,
        version_before,
        rendered_before,
        materialized_before,
        identity_before,
        group_id_before,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn engine_level_restore_drill() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    let seeded = seed_two_peer_mosaic(&mosaic).await;
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");
    let note_id = seeded.note_id;

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
    assert_eq!(
        device2, seeded.device,
        "device identity must survive the drill"
    );

    let engine2 = LoroEngine::with_dirs(
        device2,
        Arc::new(Hlc::new(device2)),
        snapshot_dir.clone(),
        Some(notes_dir.clone()),
    )
    .await
    .expect("reopen engine on restored mosaic");

    // ── LINEAGE, not counters — the "no reseed" proof ──
    // The restored doc's history must still contain device B's ops. A
    // reseed reproduces the same blake3-slug note id, the same rendered
    // text, and (everything re-authored under one peer) even a
    // plausible-looking VV — but it can never re-mint another peer's
    // ops. `reseed_cannot_reproduce_restored_lineage` below proves this
    // assertion fails on the reseed path.
    let version_after = engine2
        .doc_version(note_id)
        .await
        .expect("doc version after restore");
    let vv_before = VersionVector::decode(&seeded.version_before).expect("decode vv before");
    let vv_after = VersionVector::decode(&version_after).expect("decode vv after");
    let peer_a = peer_of(&seeded.device);
    let peer_b = peer_of(&seeded.device_b);
    let counter_b = *vv_before
        .get(&peer_b)
        .expect("fixture must carry peer B ops in the doc history");
    assert!(counter_b > 0, "peer B authored at least one op");
    assert_eq!(
        vv_after.get(&peer_b),
        Some(&counter_b),
        "restored doc must still carry device B's ops — a reseed re-authors \
         everything under this device's peer and cannot reproduce them"
    );
    assert_eq!(
        vv_after.get(&peer_a),
        vv_before.get(&peer_a),
        "device A's op count must survive the restore"
    );
    assert_eq!(
        vv_before, vv_after,
        "full decoded version vector must be identical after restore"
    );

    // Identical rendered content, from the CRDT and on disk.
    let rendered_after = engine2
        .render_note_full(note_id)
        .await
        .expect("render after restore");
    assert_eq!(seeded.rendered_before, rendered_after);
    let materialized_after = fs::read(notes_dir.join(format!("{}.md", DRILL_SLUG))).unwrap();
    assert_eq!(seeded.materialized_before, materialized_after);

    // Identical sync identity (device + group, including the group key).
    let identity_after = read_identity(&mosaic);
    assert_eq!(seeded.identity_before, identity_after);
    let group_after = tesela_sync::load_or_create_group_identity(&mosaic)
        .await
        .expect("group identity after restore");
    assert_eq!(seeded.group_id_before, group_after.group_id);

    // The restored engine keeps writing as the SAME lineage.
    let post_restore_content = format!(
        "{}- post-restore edit <!-- bid:55555555-5555-5555-5555-555555555555 -->\n",
        engine2
            .render_note_full(note_id)
            .await
            .expect("render before post-restore write")
    );
    engine2
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(DRILL_SLUG.into()),
            title: "Drill Note".into(),
            content: post_restore_content,
            created_at_millis: 1_750_000_000_000,
        })
        .await
        .expect("post-restore write");
    let rendered_post = engine2.render_note_full(note_id).await.unwrap();
    assert!(rendered_post.contains("post-restore edit"));
}

/// PERTURB-PROOF, encoded permanently: run the drill's disaster flow
/// but swap the restored authority for the reseed path — blow away
/// `.tesela/loro`, reopen the engine, `reseed_from_disk`. The reseed
/// reproduces the SAME note id (blake3 slug) and the SAME text — which
/// is exactly why id/content equality is vacuous as a restore proof —
/// but it CANNOT reproduce the lineage: device B's ops are gone from
/// the version vector, so `engine_level_restore_drill`'s lineage
/// assertion fails on a reseed. If this test ever breaks, the drill's
/// certainty assertion has gone vacuous again.
#[tokio::test(flavor = "current_thread")]
async fn reseed_cannot_reproduce_restored_lineage() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    let seeded = seed_two_peer_mosaic(&mosaic).await;
    let snapshot_dir = mosaic.join(".tesela").join("loro");
    let notes_dir = mosaic.join("notes");

    let outcome = tesela_backup::backup(
        &mosaic,
        tesela_backup::BackupOptions {
            retention: None,
            ..Default::default()
        },
    )
    .expect("backup");
    let offsite = temp.path().join("offsite-backup");
    copy_dir(&outcome.path, &offsite);
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

    // ── PERTURBATION: discard the restored authority and reseed ──
    fs::remove_dir_all(&snapshot_dir).expect("blow away restored .tesela/loro");
    let engine2 = LoroEngine::with_dirs(
        seeded.device,
        Arc::new(Hlc::new(seeded.device)),
        snapshot_dir.clone(),
        Some(notes_dir.clone()),
    )
    .await
    .expect("reopen engine without authority");
    let reseeded = engine2
        .reseed_from_disk(&notes_dir)
        .await
        .expect("reseed from markdown");
    assert!(reseeded >= 1, "reseed must process the drill note");

    // Same note id — blake3(slug) makes id equality prove NOTHING.
    let version_reseeded = engine2
        .doc_version(seeded.note_id)
        .await
        .expect("reseed mints the SAME blake3-slug note id");
    // Same text — content equality proves NOTHING either.
    let rendered = engine2
        .render_note_full(seeded.note_id)
        .await
        .expect("render after reseed");
    assert!(
        rendered.contains(DEVICE_B_LINE),
        "reseed reproduces the text (that's the vacuity trap)"
    );

    // But the LINEAGE is unreproducible: peer B's ops cannot be
    // re-authored from markdown.
    let vv_before = VersionVector::decode(&seeded.version_before).expect("decode vv before");
    let vv_reseeded = VersionVector::decode(&version_reseeded).expect("decode vv reseeded");
    let peer_b = peer_of(&seeded.device_b);
    assert!(
        vv_before.get(&peer_b).is_some(),
        "fixture carried peer B ops"
    );
    assert!(
        vv_reseeded.get(&peer_b).is_none(),
        "a reseed must NOT carry device B's ops — if it does, the drill's \
         lineage assertion is vacuous again"
    );
    assert_ne!(
        vv_before, vv_reseeded,
        "the drill's VV assertion must fail on the reseed path"
    );
}

// ───────────────────────── server-level drill ─────────────────────────

fn spawn_server_child(mosaic: &Path, addr: &str) -> Child {
    Command::new(common::binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("TESELA_DISABLE_MDNS", "1")
        .env("TESELA_DISABLE_PEER_SYNC", "1")
        // The drill drives backups explicitly via POST /backups.
        .env("TESELA_BACKUP_ON_START", "0")
        .env("TESELA_BACKUP_INTERVAL_SECS", "0")
        // Hermetic test mosaic: file-backed group key, not the real Keychain.
        .env("TESELA_GROUP_KEY_FILE_STORE", "1")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server")
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

    let client = reqwest::Client::new();

    // ── Phase 1: live server, engine-authored note, explicit backup ──
    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    let mut server = ServerGuard(Some(child));

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
    let (child2, _addr2, base2) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    let _server2 = ServerGuard(Some(child2));

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
