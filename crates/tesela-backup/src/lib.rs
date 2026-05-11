//! Trustworthy backup and restore for a Tesela mosaic.
//!
//! Phase 13.A core: manifest with per-file SHA-256, atomic-rename
//! promotion, auto-validated round-trip, GFS retention, and a local
//! destination. Encryption (`age` + Keychain) and the git destination
//! land in follow-up commits.
//!
//! # Shape of a backup
//!
//! ```text
//! <destination>/
//!   backup-YYYYMMDD-HHMMSS/
//!     manifest.json
//!     notes/...
//!     attachments/...
//!     .tesela/config.toml
//!     .tesela/tesela.db        (VACUUM INTO snapshot, optional)
//! ```
//!
//! # Why per-file (not tarball)
//!
//! Keeping each file as a regular file on disk means: (a) `cat` works,
//! (b) git destination can diff, (c) corrupt restore can resume.

use chrono::Local;
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub mod archive;
pub mod destination;
pub mod encrypt;
pub mod error;
pub mod git;
pub mod manifest;
pub mod retention;
pub mod validate;

pub use destination::Destination;
pub use error::{BackupError, Result};
pub use manifest::{Manifest, ManifestEncryption, SCHEMA_VERSION};
pub use retention::{prune_gfs, GfsPolicy, PruneOutcome};

/// Options for a single `backup()` call.
pub struct BackupOptions {
    pub destination: Destination,
    /// If true, run round-trip validation immediately after writing.
    /// Defaults to true. Disable only for performance in nested test
    /// loops; production code should always validate.
    pub validate: bool,
    /// Pre-staged extra files to drop into the backup alongside the
    /// regular mosaic walk. Tuples are `(rel_path_in_backup, source)`.
    /// Used by callers that want to include a SQLite VACUUM-INTO
    /// snapshot — they produce the snapshot themselves (since this
    /// crate doesn't depend on sqlx) and hand it over here.
    pub extra_files: Vec<(String, std::path::PathBuf)>,
    /// GFS retention to apply after a successful backup. Use
    /// `Some(GfsPolicy::default())` for the standard 7/4/6 cadence,
    /// or `None` to skip pruning (e.g. during tests).
    pub retention: Option<GfsPolicy>,
    /// Encryption mode. `None` (default) writes plaintext on disk.
    /// `Age { recipient }` runs each captured file through `age` after
    /// packing. The manifest records the recipient so restore knows
    /// which Keychain identity to fetch.
    pub encryption: ManifestEncryption,
}

impl Default for BackupOptions {
    fn default() -> Self {
        Self {
            destination: Destination::Local,
            validate: true,
            extra_files: Vec::new(),
            retention: Some(GfsPolicy::default()),
            encryption: ManifestEncryption::None,
        }
    }
}

/// Result of a successful backup. The path is wherever the backup
/// landed (e.g. `<mosaic>/.tesela/backups/backup-YYYYMMDD-HHMMSS/`).
pub struct BackupOutcome {
    pub path: PathBuf,
    pub manifest: Manifest,
    pub pruned: PruneOutcome,
}

/// Take a backup of a mosaic. Acquires an advisory file lock, walks
/// notes/attachments/templates/.tesela/config.toml, optionally produces
/// a SQLite VACUUM INTO snapshot, writes a manifest with SHA-256 per
/// file, atomically promotes the staging dir to its final location,
/// runs round-trip validation, and prunes per the retention policy.
pub fn backup(mosaic_root: &Path, opts: BackupOptions) -> Result<BackupOutcome> {
    if !mosaic_root.exists() {
        return Err(BackupError::MosaicNotFound(mosaic_root.to_path_buf()));
    }

    let _lock = MosaicLock::acquire(mosaic_root)?;

    // For git destinations the mirror must exist before we resolve
    // paths so `resolve_target` lands the staging-final inside a real
    // repo, not a bare directory we then have to git-init after the
    // fact.
    if let Destination::Git {
        remote,
        branch,
        local_mirror,
    } = &opts.destination
    {
        git::ensure_mirror(local_mirror, remote, branch)?;
    }

    let backup_name = format!("backup-{}", Local::now().format("%Y%m%d-%H%M%S"));
    let final_path = opts.destination.resolve_target(mosaic_root, &backup_name)?;

    let staging = TempDir::new()?;
    let staging_root = staging.path().join(&backup_name);
    std::fs::create_dir_all(&staging_root)?;

    let mut entries = archive::pack_mosaic(mosaic_root, &staging_root)?;

    for (rel, source) in &opts.extra_files {
        let entry = archive::add_extra_file(&staging_root, rel, source)?;
        entries.retain(|e| e.path != entry.path);
        entries.push(entry);
    }

    let mut manifest = Manifest::new(
        mosaic_root.to_path_buf(),
        opts.destination.manifest_record(),
        opts.encryption.clone(),
    );
    manifest.files = entries;

    // Encrypt after packing + recording plaintext SHA in the manifest.
    // The manifest itself stays plaintext on disk so `backup-list` and
    // `backup-verify` can read metadata without unlocking the keychain.
    if let ManifestEncryption::Age { recipient } = &opts.encryption {
        encrypt::encrypt_staging(&staging_root, recipient)?;
    }

    manifest.write(&staging_root)?;

    destination::promote_atomic(&staging_root, &final_path)?;
    // After promote, the TempDir's drop will only clean up the parent
    // directory shell — staging_root has been moved out.

    if opts.validate {
        let status = validate::roundtrip(&final_path, &manifest)?;
        manifest.validated = Some(status.clone());
        manifest.write(&final_path)?;
        if !status.ok {
            // Rename to .FAILED so it doesn't get pruned away by
            // retention and so list/verify can see what went wrong.
            let failed_path = final_path.with_extension("FAILED");
            std::fs::rename(&final_path, &failed_path)?;
            return Err(BackupError::ValidationFailed(
                status.note.unwrap_or_else(|| "unknown".to_string()),
            ));
        }
    }

    let pruned = if let Some(policy) = opts.retention {
        let root = opts.destination.root_for_listing(mosaic_root)?;
        retention::prune_gfs(&root, policy, false)?
    } else {
        PruneOutcome::default()
    };

    // After the directory is on disk + retention has run, push the
    // mirror upstream. This happens *after* validation so we never
    // publish a failed backup.
    if let Destination::Git {
        branch,
        local_mirror,
        ..
    } = &opts.destination
    {
        git::commit_and_push(local_mirror, branch, &backup_name)?;
    }

    Ok(BackupOutcome {
        path: final_path,
        manifest,
        pruned,
    })
}

/// Options for `restore()`.
#[derive(Default)]
pub struct RestoreOptions {
    /// `false` (default): restore into a sibling `<mosaic>-restored/`.
    /// `true`: replace the current mosaic in place (after renaming the
    /// existing root to `<root>.before-restore-<timestamp>`).
    pub in_place: bool,
    /// Override the destination directory entirely. Takes precedence
    /// over `in_place`. Used by `verify` when round-tripping into a
    /// throwaway location.
    pub target_override: Option<PathBuf>,
    /// Allow restoring a backup written by a *newer* tesela than this
    /// binary. Off by default — refuse rather than silently corrupt.
    pub allow_newer: bool,
}

#[derive(Debug)]
pub struct RestoreOutcome {
    pub manifest: Manifest,
    pub target: PathBuf,
    pub renamed_previous: Option<PathBuf>,
}

pub fn restore(
    backup_root: &Path,
    current_mosaic: &Path,
    opts: RestoreOptions,
) -> Result<RestoreOutcome> {
    if !backup_root.exists() {
        return Err(BackupError::BackupNotFound(backup_root.to_path_buf()));
    }
    let manifest = Manifest::load(backup_root)?;

    if manifest.schema_version > SCHEMA_VERSION {
        return Err(BackupError::SchemaTooNew {
            manifest: manifest.schema_version,
            supported: SCHEMA_VERSION,
        });
    }

    let current_version = env!("CARGO_PKG_VERSION");
    if !opts.allow_newer && version_is_newer(&manifest.tesela_version, current_version) {
        return Err(BackupError::BinaryNewerRequired {
            manifest: manifest.tesela_version.clone(),
            current: current_version.to_string(),
        });
    }

    let target = if let Some(t) = opts.target_override.as_ref() {
        t.clone()
    } else if opts.in_place {
        current_mosaic.to_path_buf()
    } else {
        let parent = current_mosaic
            .parent()
            .ok_or_else(|| BackupError::Other(anyhow::anyhow!("mosaic has no parent dir")))?;
        let basename = current_mosaic
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "mosaic".to_string());
        parent.join(format!("{}-restored", basename))
    };

    let mut renamed_previous = None;
    if opts.in_place && target.exists() {
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let backup_dir = target.with_extension(format!("before-restore-{}", stamp));
        std::fs::rename(&target, &backup_dir)?;
        renamed_previous = Some(backup_dir);
    }

    if target.exists() {
        return Err(BackupError::Other(anyhow::anyhow!(
            "restore target already exists; refusing to overwrite: {}",
            target.display()
        )));
    }

    archive::unpack_to_mosaic(backup_root, &target, &manifest)?;

    Ok(RestoreOutcome {
        manifest,
        target,
        renamed_previous,
    })
}

/// Re-run validation on an existing backup. Returns the freshly-stamped
/// status (also persisted into the manifest on disk).
pub fn verify(backup_root: &Path) -> Result<manifest::ValidationStatus> {
    let mut manifest = Manifest::load(backup_root)?;
    let status = validate::roundtrip(backup_root, &manifest)?;
    manifest.validated = Some(status.clone());
    manifest.write(backup_root)?;
    Ok(status)
}

/// List backups under a destination root. Reads each manifest. Skips
/// directories whose manifest is missing or invalid (logs a warning).
pub fn list(destination_root: &Path) -> Result<Vec<(PathBuf, Manifest)>> {
    let mut out = Vec::new();
    if !destination_root.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(destination_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        match Manifest::load(&path) {
            Ok(m) => out.push((path, m)),
            Err(e) => {
                tracing::warn!("skipping {}: {}", path.display(), e);
            }
        }
    }
    out.sort_by_key(|entry| std::cmp::Reverse(entry.1.created_at));
    Ok(out)
}

/// Best-effort comparison of dotted-numeric Cargo versions. Returns
/// true if `a` is strictly newer than `b`. Falls back to lexicographic
/// comparison if either side doesn't parse as numeric segments — that
/// will be wrong for prerelease tags (`0.2.0-rc1` vs `0.2.0`) but
/// won't fire false-positive "newer" alarms for the common case.
fn version_is_newer(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Option<Vec<u64>> {
        s.split('.')
            .map(|seg| seg.split('-').next().unwrap_or(seg).parse::<u64>().ok())
            .collect()
    };
    match (parse(a), parse(b)) {
        (Some(av), Some(bv)) => av > bv,
        _ => a > b,
    }
}

/// File-based advisory lock so two `tesela backup` invocations on the
/// same mosaic can't race. Released on drop.
struct MosaicLock {
    _file: File,
}

impl MosaicLock {
    fn acquire(mosaic_root: &Path) -> Result<Self> {
        let tesela_dir = mosaic_root.join(".tesela");
        std::fs::create_dir_all(&tesela_dir)?;
        let lock_path = tesela_dir.join(".backup.lock");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)?;
        file.try_lock_exclusive()
            .map_err(|_| BackupError::LockHeld)?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_fixture_mosaic(root: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(root.join("notes"))?;
        std::fs::create_dir_all(root.join("attachments"))?;
        std::fs::create_dir_all(root.join(".tesela"))?;
        std::fs::write(
            root.join("notes/2026-05-10.md"),
            "---\ntitle: 2026-05-10\n---\n- hello\n",
        )?;
        std::fs::write(
            root.join("notes/task.md"),
            "---\ntitle: Task\ntype: Tag\n---\n",
        )?;
        std::fs::write(root.join("attachments/foo.bin"), b"\x00\x01\x02\x03")?;
        std::fs::write(root.join(".tesela/config.toml"), "[general]\n")?;
        Ok(())
    }

    #[test]
    fn backup_writes_manifest_and_validates() {
        let temp = TempDir::new().unwrap();
        let mosaic = temp.path().join("mosaic");
        make_fixture_mosaic(&mosaic).unwrap();

        let outcome = backup(
            &mosaic,
            BackupOptions {
                destination: Destination::Local,
                validate: true,
                extra_files: Vec::new(),
                retention: None,
                encryption: ManifestEncryption::None,
            },
        )
        .unwrap();

        assert!(outcome.path.join("manifest.json").exists());
        assert!(outcome.path.join("notes/2026-05-10.md").exists());
        assert!(outcome.path.join("notes/task.md").exists());
        assert!(outcome.path.join("attachments/foo.bin").exists());
        assert!(outcome.path.join(".tesela/config.toml").exists());

        let validated = outcome.manifest.validated.expect("validation ran");
        assert!(validated.ok, "validation should pass: {:?}", validated.note);
        assert!(outcome.manifest.files.iter().all(|f| !f.sha256.is_empty()));
    }

    #[test]
    fn restore_into_new_mosaic_byte_exact() {
        let temp = TempDir::new().unwrap();
        let mosaic = temp.path().join("source");
        make_fixture_mosaic(&mosaic).unwrap();

        let outcome = backup(&mosaic, BackupOptions::default()).unwrap();

        let restored = restore(
            &outcome.path,
            &mosaic,
            RestoreOptions {
                in_place: false,
                target_override: Some(temp.path().join("restored")),
                allow_newer: false,
            },
        )
        .unwrap();

        let original = std::fs::read(mosaic.join("notes/2026-05-10.md")).unwrap();
        let restored_bytes = std::fs::read(restored.target.join("notes/2026-05-10.md")).unwrap();
        assert_eq!(original, restored_bytes);

        let original_attach = std::fs::read(mosaic.join("attachments/foo.bin")).unwrap();
        let restored_attach = std::fs::read(restored.target.join("attachments/foo.bin")).unwrap();
        assert_eq!(original_attach, restored_attach);
    }

    #[test]
    fn restore_refuses_corrupted_backup() {
        let temp = TempDir::new().unwrap();
        let mosaic = temp.path().join("source");
        make_fixture_mosaic(&mosaic).unwrap();

        let outcome = backup(&mosaic, BackupOptions::default()).unwrap();

        // Corrupt one of the backup files, then restore should fail
        // with ChecksumMismatch.
        std::fs::write(outcome.path.join("notes/task.md"), b"tampered").unwrap();
        let err = restore(
            &outcome.path,
            &mosaic,
            RestoreOptions {
                target_override: Some(temp.path().join("should-not-exist")),
                ..Default::default()
            },
        )
        .unwrap_err();

        match err {
            BackupError::ChecksumMismatch { .. } => {}
            other => panic!("expected ChecksumMismatch, got {:?}", other),
        }
    }

    #[test]
    fn backup_with_encryption_round_trips() {
        let identity = age::x25519::Identity::generate();
        let recipient = identity.to_public().to_string();

        let temp = TempDir::new().unwrap();
        let mosaic = temp.path().join("encrypted-source");
        make_fixture_mosaic(&mosaic).unwrap();

        let outcome = backup(
            &mosaic,
            BackupOptions {
                destination: Destination::Local,
                validate: false, // would consult real Keychain otherwise
                extra_files: Vec::new(),
                retention: None,
                encryption: ManifestEncryption::Age {
                    recipient: recipient.clone(),
                },
            },
        )
        .unwrap();

        // Plain notes file must be gone; .age sibling must be present.
        assert!(!outcome.path.join("notes/2026-05-10.md").exists());
        assert!(outcome.path.join("notes/2026-05-10.md.age").exists());
        // Manifest stays plaintext for `backup-list` readability.
        assert!(outcome.path.join("manifest.json").exists());

        // Round-trip restore with the in-memory identity override.
        encrypt::TEST_IDENTITY_OVERRIDE.with(|cell| {
            *cell.borrow_mut() = Some(identity.clone());
        });

        let restored = restore(
            &outcome.path,
            &mosaic,
            RestoreOptions {
                target_override: Some(temp.path().join("restored-encrypted")),
                ..Default::default()
            },
        )
        .unwrap();

        encrypt::TEST_IDENTITY_OVERRIDE.with(|cell| {
            *cell.borrow_mut() = None;
        });

        let plain = std::fs::read_to_string(restored.target.join("notes/2026-05-10.md")).unwrap();
        assert!(plain.contains("hello"));
    }

    #[test]
    fn version_compare_known_cases() {
        assert!(version_is_newer("0.2.0", "0.1.99"));
        assert!(!version_is_newer("0.1.0", "0.1.0"));
        assert!(!version_is_newer("0.1.0", "0.2.0"));
        assert!(version_is_newer("1.0.0", "0.99.99"));
    }
}
