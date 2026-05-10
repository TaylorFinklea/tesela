use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::{BackupError, Result};
use crate::manifest::{sha256_file, FileEntry};

/// Subpaths inside a mosaic that we capture in a backup. Anything else is
/// either rebuildable (the SQLite DB cache) or transient (.tesela/.lock,
/// .tesela/search_history.json) and skipped.
///
/// The SQLite DB *is* captured — but via VACUUM INTO, not file copy, so
/// it's listed separately from this walk.
const CAPTURE_DIRS: &[&str] = &["notes", "attachments", "templates"];
const CAPTURE_TESELA_FILES: &[&str] = &["config.toml"];

/// Walk a mosaic and copy every captured file into a staging directory,
/// computing SHA-256 + size as we go so the manifest can be assembled
/// without re-reading anything.
pub fn pack_mosaic(mosaic_root: &Path, staging: &Path) -> Result<Vec<FileEntry>> {
    if !mosaic_root.exists() {
        return Err(BackupError::MosaicNotFound(mosaic_root.to_path_buf()));
    }
    fs::create_dir_all(staging)?;

    let mut entries = Vec::new();

    for dir_name in CAPTURE_DIRS {
        let src_dir = mosaic_root.join(dir_name);
        if !src_dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&src_dir).follow_links(false) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(mosaic_root)
                .expect("walk under mosaic_root")
                .to_path_buf();
            copy_one(mosaic_root, staging, &rel, &mut entries)?;
        }
    }

    let tesela_dir = mosaic_root.join(".tesela");
    if tesela_dir.exists() {
        for fname in CAPTURE_TESELA_FILES {
            let src = tesela_dir.join(fname);
            if src.exists() {
                let rel = PathBuf::from(".tesela").join(fname);
                copy_one(mosaic_root, staging, &rel, &mut entries)?;
            }
        }
    }

    Ok(entries)
}

fn copy_one(
    mosaic_root: &Path,
    staging: &Path,
    rel: &Path,
    out: &mut Vec<FileEntry>,
) -> Result<()> {
    let src = mosaic_root.join(rel);
    let dst = staging.join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&src, &dst)?;
    let (sha, size) = sha256_file(&dst)?;
    out.push(FileEntry {
        path: rel.to_string_lossy().replace('\\', "/"),
        size,
        sha256: sha,
    });
    Ok(())
}

/// Add an extra file to the backup that isn't part of the mosaic walk —
/// used for the SQLite snapshot produced by VACUUM INTO.
pub fn add_extra_file(staging: &Path, rel: &str, source: &Path) -> Result<FileEntry> {
    let dst = staging.join(rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, &dst)?;
    let (sha, size) = sha256_file(&dst)?;
    Ok(FileEntry {
        path: rel.to_string(),
        size,
        sha256: sha,
    })
}

/// Restore a backup directory's captured files into a target mosaic root.
/// Verifies each file's SHA-256 against the manifest as it copies.
pub fn unpack_to_mosaic(
    backup_root: &Path,
    target_root: &Path,
    files: &[FileEntry],
) -> Result<()> {
    fs::create_dir_all(target_root)?;
    for entry in files {
        let src = backup_root.join(&entry.path);
        let dst = target_root.join(&entry.path);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        let (actual_sha, _) = sha256_file(&src)?;
        if actual_sha != entry.sha256 {
            return Err(BackupError::ChecksumMismatch {
                path: entry.path.clone(),
                expected: entry.sha256.clone(),
                actual: actual_sha,
            });
        }
        fs::copy(&src, &dst)?;
    }
    Ok(())
}
