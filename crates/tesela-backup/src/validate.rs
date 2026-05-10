use chrono::Local;
use std::path::Path;
use std::time::Instant;
use tempfile::TempDir;

use crate::archive::unpack_to_mosaic;
use crate::error::Result;
use crate::manifest::{sha256_file, Manifest, ValidationStatus};

/// Round-trip a freshly-written backup: unpack into a temp directory,
/// re-checksum every file, compare against the manifest, and report.
///
/// The act of `unpack_to_mosaic` already verifies SHA-256 against the
/// manifest as it copies, so by the time we return Ok we have proof
/// that every byte landed correctly. The temp dir is removed on drop.
pub fn roundtrip(backup_root: &Path, manifest: &Manifest) -> Result<ValidationStatus> {
    let started = Instant::now();
    let result = run_roundtrip(backup_root, manifest);
    let elapsed_ms = started.elapsed().as_millis() as u64;

    Ok(match result {
        Ok(()) => ValidationStatus {
            ok: true,
            checked_at: Local::now(),
            elapsed_ms,
            note: None,
        },
        Err(e) => ValidationStatus {
            ok: false,
            checked_at: Local::now(),
            elapsed_ms,
            note: Some(e.to_string()),
        },
    })
}

fn run_roundtrip(backup_root: &Path, manifest: &Manifest) -> Result<()> {
    let temp = TempDir::new()?;
    let target = temp.path().join("restored");
    unpack_to_mosaic(backup_root, &target, &manifest.files)?;
    // Belt-and-braces: re-verify every restored file's SHA matches the
    // manifest. unpack_to_mosaic already does this for the source side;
    // this confirms the *destination* side is also correct (catches a
    // bad fs::copy or a partial-write).
    for entry in &manifest.files {
        let restored = target.join(&entry.path);
        let (sha, _) = sha256_file(&restored)?;
        if sha != entry.sha256 {
            return Err(crate::error::BackupError::ChecksumMismatch {
                path: entry.path.clone(),
                expected: entry.sha256.clone(),
                actual: sha,
            });
        }
    }
    Ok(())
}
