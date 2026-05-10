use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{BackupError, Result};
use crate::manifest::ManifestDestination;

/// Where a backup ends up. Local + External + Git are all dated
/// subdirectories under a single "destination root" — the only thing
/// that differs is *where* that root lives and what happens *after*
/// the directory is written (push to git, etc).
#[derive(Debug, Clone)]
pub enum Destination {
    /// `<mosaic>/.tesela/backups/`
    Local,
    /// User-supplied directory (could be iCloud Drive, an external SSD,
    /// or any path the user wants).
    External { path: PathBuf },
    /// Git remote. We maintain a working repo at `local_mirror` and
    /// push each backup commit upstream. Restore reads back from the
    /// mirror (or a fresh clone of the remote) — there's no
    /// streaming-from-network path.
    Git {
        remote: String,
        branch: String,
        local_mirror: PathBuf,
    },
}

impl Destination {
    /// Where to materialize a single backup whose name is `backup_name`.
    pub fn resolve_target(&self, mosaic_root: &Path, backup_name: &str) -> Result<PathBuf> {
        match self {
            Destination::Local => Ok(mosaic_root
                .join(".tesela")
                .join("backups")
                .join(backup_name)),
            Destination::External { path } => Ok(path.join(backup_name)),
            Destination::Git { local_mirror, .. } => Ok(local_mirror.join(backup_name)),
        }
    }

    /// Where backups for this destination live, so retention can scan.
    pub fn root_for_listing(&self, mosaic_root: &Path) -> Result<PathBuf> {
        match self {
            Destination::Local => Ok(mosaic_root.join(".tesela").join("backups")),
            Destination::External { path } => Ok(path.clone()),
            Destination::Git { local_mirror, .. } => Ok(local_mirror.clone()),
        }
    }

    pub fn manifest_record(&self) -> ManifestDestination {
        match self {
            Destination::Local => ManifestDestination::Local {
                path: PathBuf::from(".tesela/backups"),
            },
            Destination::External { path } => ManifestDestination::External { path: path.clone() },
            Destination::Git { remote, branch, .. } => ManifestDestination::Git {
                remote: remote.clone(),
                branch: branch.clone(),
            },
        }
    }
}

/// Atomically promote a staging directory to its final path. We `rename`
/// when both are on the same filesystem (the common case). If `rename`
/// fails with `EXDEV` (cross-filesystem move, e.g. mosaic on local SSD
/// and external destination on a USB drive), fall back to a recursive
/// copy + remove-staging.
pub fn promote_atomic(staging: &Path, final_path: &Path) -> Result<()> {
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if final_path.exists() {
        // Refuse to clobber an existing backup of the same name.
        return Err(BackupError::Other(anyhow::anyhow!(
            "destination already exists: {}",
            final_path.display()
        )));
    }
    match fs::rename(staging, final_path) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc_exdev()) => {
            copy_dir_recursive(staging, final_path)?;
            fs::remove_dir_all(staging)?;
            Ok(())
        }
        Err(e) => Err(BackupError::Io(e)),
    }
}

#[cfg(target_os = "macos")]
fn libc_exdev() -> i32 {
    18
}

#[cfg(target_os = "linux")]
fn libc_exdev() -> i32 {
    18
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn libc_exdev() -> i32 {
    -1 // never matches
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
