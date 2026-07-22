//! The backup must capture the AUTHORITY, not just the export view.
//!
//! Post-Loro-flag-day, `.tesela/loro/` per-note snapshots are the source
//! of truth and `notes/*.md` is the deterministic export. A restore from
//! .md alone loses CRDT history and sync identity (device_id, group key)
//! and forces a reseed — which mints a disjoint lineage (the documented
//! TreeID-twin clobber hazard). These tests pin the capture set: Loro
//! state + sync identity round-trip byte-exact through backup/restore.

use std::fs;
use std::path::Path;
use tempfile::TempDir;

use tesela_backup::{backup, restore, BackupOptions, Manifest, RestoreOptions};

/// All the authority files a live engine mosaic carries.
const AUTHORITY_FILES: &[&str] = &[
    ".tesela/loro/0123456789abcdef0123456789abcdef.bin",
    // Reserved synced page-directory authority (`tesela.page.dir!`).
    ".tesela/loro/746573656c612e706167652e64697221.bin",
    ".tesela/loro/_index.bin",
    ".tesela/loro/_broadcast.bin",
    ".tesela/device_id.hex",
    ".tesela/group_id.hex",
    ".tesela/group_key.bin",
    ".tesela/relay_state.json",
    ".tesela/sync_peers.json",
];

fn make_engine_mosaic(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("notes"))?;
    fs::create_dir_all(root.join("attachments"))?;
    fs::create_dir_all(root.join(".tesela/loro"))?;
    fs::write(
        root.join("notes/2026-06-10.md"),
        "---\ntitle: 2026-06-10\n---\n- materialized export view\n",
    )?;
    fs::write(root.join(".tesela/config.toml"), "[general]\n")?;
    // Authority: per-note Loro snapshot + index/broadcast docs.
    fs::write(
        root.join(".tesela/loro/0123456789abcdef0123456789abcdef.bin"),
        b"\x01loro-snapshot-bytes",
    )?;
    fs::write(
        root.join(".tesela/loro/746573656c612e706167652e64697221.bin"),
        b"\x04page-directory",
    )?;
    fs::write(root.join(".tesela/loro/_index.bin"), b"\x02index-doc")?;
    fs::write(root.join(".tesela/loro/_broadcast.bin"), b"\x03cursors")?;
    // Sync identity.
    fs::write(
        root.join(".tesela/device_id.hex"),
        "019ea829a5a07031a96852941094aea3",
    )?;
    fs::write(
        root.join(".tesela/group_id.hex"),
        "aabbccddeeff00112233445566778899",
    )?;
    fs::write(root.join(".tesela/group_key.bin"), [0x42u8; 32])?;
    fs::write(root.join(".tesela/relay_state.json"), "{\"cursor\":7}")?;
    fs::write(root.join(".tesela/sync_peers.json"), "[]")?;
    Ok(())
}

fn local_backup(mosaic: &Path) -> tesela_backup::BackupOutcome {
    backup(
        mosaic,
        BackupOptions {
            retention: None,
            ..Default::default()
        },
    )
    .expect("backup")
}

#[test]
fn backup_captures_loro_state_and_sync_identity() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_engine_mosaic(&mosaic).unwrap();

    let outcome = local_backup(&mosaic);

    for rel in AUTHORITY_FILES {
        assert!(
            outcome.path.join(rel).exists(),
            "backup must capture authority file {rel}"
        );
        assert!(
            outcome.manifest.files.iter().any(|f| f.path == *rel),
            "manifest must list authority file {rel}"
        );
    }
    // Manifest knows it carries the authority.
    assert!(outcome.manifest.includes_loro_state());
    assert!(outcome.manifest.includes_sync_identity());
    // Capture-set change is a schema bump (v2 = authority included).
    assert!(outcome.manifest.schema_version >= 2);
}

#[test]
fn loro_tmp_files_are_skipped() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_engine_mosaic(&mosaic).unwrap();
    // The engine writes snapshots via tmp+rename; an in-flight tmp file
    // (`<name>.tmp.<n>`) must not be captured (it may vanish or be torn).
    fs::write(
        mosaic.join(".tesela/loro/0123456789abcdef0123456789abcdef.tmp.3"),
        b"half-written",
    )
    .unwrap();

    let outcome = local_backup(&mosaic);
    assert!(
        !outcome
            .manifest
            .files
            .iter()
            .any(|f| f.path.contains(".tmp.")),
        "in-flight tmp snapshot files must not be captured"
    );
}

#[test]
fn restore_round_trips_authority_byte_exact() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("source");
    make_engine_mosaic(&mosaic).unwrap();

    let outcome = local_backup(&mosaic);

    let restored = restore(
        &outcome.path,
        &mosaic,
        RestoreOptions {
            target_override: Some(temp.path().join("restored")),
            ..Default::default()
        },
    )
    .expect("restore");

    for rel in AUTHORITY_FILES {
        let orig = fs::read(mosaic.join(rel)).unwrap();
        let rest = fs::read(restored.target.join(rel))
            .unwrap_or_else(|e| panic!("restored missing {rel}: {e}"));
        assert_eq!(orig, rest, "byte mismatch in {rel}");
    }
}

#[test]
fn legacy_v1_backup_without_authority_still_restores() {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("source");
    // A pre-authority mosaic shape: notes + config only.
    fs::create_dir_all(mosaic.join("notes")).unwrap();
    fs::create_dir_all(mosaic.join(".tesela")).unwrap();
    fs::write(mosaic.join("notes/old.md"), "---\ntitle: Old\n---\n- hi\n").unwrap();
    fs::write(mosaic.join(".tesela/config.toml"), "[general]\n").unwrap();

    let outcome = local_backup(&mosaic);

    // Rewrite the manifest as schema v1 — exactly what an existing
    // on-disk backup taken by the previous binary looks like.
    let manifest_path = outcome.path.join("manifest.json");
    let raw = fs::read_to_string(&manifest_path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    v["schema_version"] = serde_json::json!(1);
    fs::write(&manifest_path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let manifest = Manifest::load(&outcome.path).unwrap();
    assert_eq!(manifest.schema_version, 1);
    assert!(!manifest.includes_loro_state());
    assert!(!manifest.includes_sync_identity());

    let restored = restore(
        &outcome.path,
        &mosaic,
        RestoreOptions {
            target_override: Some(temp.path().join("restored-v1")),
            ..Default::default()
        },
    )
    .expect("v1 backup must remain restorable");
    assert!(restored.target.join("notes/old.md").exists());
}
