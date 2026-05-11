//! Round-trip integration: backup → push to a local bare git remote →
//! clone elsewhere → restore. Skipped silently if `git` isn't on PATH
//! (which would be highly unusual in our development environment).

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use tesela_backup::{
    backup, restore, BackupOptions, Destination, ManifestEncryption, RestoreOptions,
};

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn make_fixture_mosaic(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join("notes"))?;
    std::fs::create_dir_all(root.join("attachments"))?;
    std::fs::create_dir_all(root.join(".tesela"))?;
    std::fs::write(
        root.join("notes/2026-05-10.md"),
        "---\ntitle: 2026-05-10\n---\n- git-destination smoke test\n",
    )?;
    std::fs::write(root.join(".tesela/config.toml"), "[general]\n")?;
    Ok(())
}

#[test]
fn backup_pushes_to_bare_remote_then_restore_pulls_back() {
    if !git_available() {
        eprintln!("git not on PATH; skipping git_destination test");
        return;
    }

    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    // Bare remote acts as the "upstream" — same shape a user's
    // private GitHub/Gitea repo would have.
    let remote = temp.path().join("remote.git");
    let status = Command::new("git")
        .args(["init", "--bare", "--quiet", "--initial-branch", "main"])
        .arg(&remote)
        .status()
        .unwrap();
    assert!(status.success(), "git init --bare failed");

    let local_mirror = temp.path().join("mirror");
    let dest = Destination::Git {
        remote: remote.to_string_lossy().into_owned(),
        branch: "main".to_string(),
        local_mirror: local_mirror.clone(),
    };

    let outcome = backup(
        &mosaic,
        BackupOptions {
            destination: dest,
            validate: true,
            extra_files: Vec::new(),
            retention: None,
            encryption: ManifestEncryption::None,
        },
    )
    .expect("backup to git destination");

    assert!(outcome.path.starts_with(&local_mirror));
    assert!(outcome.path.join("manifest.json").exists());
    assert!(outcome.path.join("notes/2026-05-10.md").exists());

    // Confirm the commit + files were actually pushed: clone the
    // remote into a fresh dir and inspect.
    let clone = temp.path().join("clone");
    let status = Command::new("git")
        .args(["clone", "--quiet"])
        .arg(&remote)
        .arg(&clone)
        .status()
        .unwrap();
    assert!(status.success(), "git clone of bare remote failed");

    let backup_name = outcome
        .path
        .file_name()
        .and_then(|s| s.to_str())
        .expect("backup_name");
    assert!(
        clone.join(backup_name).join("manifest.json").exists(),
        "cloned remote should contain the backup directory"
    );
    assert!(clone.join(backup_name).join("notes/2026-05-10.md").exists());

    // Restore from the cloned mirror, byte-exact.
    let restored = restore(
        &clone.join(backup_name),
        &mosaic,
        RestoreOptions {
            target_override: Some(temp.path().join("restored")),
            ..Default::default()
        },
    )
    .expect("restore from git-cloned backup");

    let original = std::fs::read(mosaic.join("notes/2026-05-10.md")).unwrap();
    let restored_bytes = std::fs::read(restored.target.join("notes/2026-05-10.md")).unwrap();
    assert_eq!(original, restored_bytes);
}

#[test]
fn second_backup_reuses_mirror_and_appends_commit() {
    if !git_available() {
        return;
    }

    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let remote = temp.path().join("remote.git");
    Command::new("git")
        .args(["init", "--bare", "--quiet", "--initial-branch", "main"])
        .arg(&remote)
        .status()
        .unwrap();

    let local_mirror = temp.path().join("mirror");
    let dest = || Destination::Git {
        remote: remote.to_string_lossy().into_owned(),
        branch: "main".to_string(),
        local_mirror: local_mirror.clone(),
    };

    let first = backup(
        &mosaic,
        BackupOptions {
            destination: dest(),
            validate: false,
            extra_files: Vec::new(),
            retention: None,
            encryption: ManifestEncryption::None,
        },
    )
    .unwrap();

    // Wait a second so the backup-name timestamp differs.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Touch the mosaic so something actually changed.
    std::fs::write(
        mosaic.join("notes/2026-05-11.md"),
        "---\ntitle: 2026-05-11\n---\n- another day\n",
    )
    .unwrap();

    let second = backup(
        &mosaic,
        BackupOptions {
            destination: dest(),
            validate: false,
            extra_files: Vec::new(),
            retention: None,
            encryption: ManifestEncryption::None,
        },
    )
    .unwrap();

    assert_ne!(first.path, second.path);
    assert!(first.path.exists());
    assert!(second.path.exists());

    // git log should now show at least the two backup commits + the
    // init commit.
    let log = Command::new("git")
        .arg("-C")
        .arg(&local_mirror)
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .unwrap();
    let count: u32 = std::str::from_utf8(&log.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(count >= 2, "expected ≥2 commits in mirror, got {}", count);
}
