use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use crate::error::{BackupError, Result};

/// Manifest schema version. Bump when the on-disk format changes in a
/// non-backward-compatible way.
///
/// v1 — export view only: notes/, attachments/, templates/,
///      .tesela/config.toml (+ optional VACUUM'd tesela.db).
/// v2 — authority capture: adds `.tesela/loro/` CRDT snapshots and the
///      sync identity (`device_id.hex`, `group_id.hex`, `group_key.bin`,
///      `relay_state.json`, `sync_peers.json`). Restore is manifest-
///      driven, so v1 backups remain restorable by this binary; older
///      binaries refuse v2 (they don't know it carries the authority).
pub const SCHEMA_VERSION: u32 = 2;

/// Backup manifest written as `manifest.json` at the backup root.
///
/// The manifest is the single source of truth for what's in a backup —
/// every file, its size, its SHA-256, plus where the backup was written
/// and whether it has been validated. Restore consults this file before
/// touching anything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub tesela_version: String,
    pub git_hash: Option<String>,
    pub created_at: DateTime<Local>,
    pub mosaic_root: PathBuf,
    pub destination: ManifestDestination,
    pub encryption: ManifestEncryption,
    pub files: Vec<FileEntry>,
    pub validated: Option<ValidationStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ManifestDestination {
    /// `<mosaic>/.tesela/backups/<name>/`
    Local { path: PathBuf },
    /// User-configured external directory (e.g. iCloud Drive)
    External { path: PathBuf },
    /// Git remote — push as commits
    Git { remote: String, branch: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ManifestEncryption {
    None,
    Age { recipient: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Path relative to the backup root (e.g. `notes/2026-05-10.md`).
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationStatus {
    pub ok: bool,
    pub checked_at: DateTime<Local>,
    pub elapsed_ms: u64,
    pub note: Option<String>,
}

impl Manifest {
    pub const FILENAME: &'static str = "manifest.json";

    pub fn new(
        mosaic_root: PathBuf,
        destination: ManifestDestination,
        encryption: ManifestEncryption,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            tesela_version: env!("CARGO_PKG_VERSION").to_string(),
            git_hash: option_env!("TESELA_GIT_HASH").map(String::from),
            created_at: Local::now(),
            mosaic_root,
            destination,
            encryption,
            files: Vec::new(),
            validated: None,
        }
    }

    pub fn write(&self, backup_root: &Path) -> Result<()> {
        let path = backup_root.join(Self::FILENAME);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load(backup_root: &Path) -> Result<Self> {
        let path = backup_root.join(Self::FILENAME);
        let bytes = std::fs::read(&path).map_err(|e| BackupError::InvalidManifest {
            path: path.clone(),
            message: e.to_string(),
        })?;
        serde_json::from_slice(&bytes).map_err(|e| BackupError::InvalidManifest {
            path,
            message: e.to_string(),
        })
    }

    /// True when this backup carries Loro CRDT state (`.tesela/loro/*`) —
    /// i.e. the authority, not just the materialized export view. A
    /// restore of such a backup needs no reseed (no twin-lineage risk).
    pub fn includes_loro_state(&self) -> bool {
        self.files.iter().any(|f| f.path.starts_with(".tesela/loro/"))
    }

    /// True when this backup carries the sync identity (`device_id.hex`).
    /// Group identity files ride along whenever they exist on disk.
    pub fn includes_sync_identity(&self) -> bool {
        self.files.iter().any(|f| f.path == ".tesela/device_id.hex")
    }
}

/// Compute SHA-256 of a file, streaming through 64 KiB chunks so we don't
/// load multi-megabyte attachments into memory.
pub fn sha256_file(path: &Path) -> Result<(String, u64)> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = hasher.finalize();
    Ok((format!("{:x}", hash), metadata.len()))
}
