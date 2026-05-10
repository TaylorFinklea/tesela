use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("mosaic not found: {0}")]
    MosaicNotFound(PathBuf),

    #[error("backup not found: {0}")]
    BackupNotFound(PathBuf),

    #[error("manifest missing or invalid: {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },

    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("validation roundtrip failed: {0}")]
    ValidationFailed(String),

    #[error("destination unsupported in this build: {0}")]
    UnsupportedDestination(String),

    #[error("backup older than this binary supports (manifest says schema_version={manifest}, binary supports={supported})")]
    SchemaTooNew { manifest: u32, supported: u32 },

    #[error("backup written by a newer Tesela ({manifest}) than this binary ({current}); pass --allow-newer to override")]
    BinaryNewerRequired { manifest: String, current: String },

    #[error("another backup is already running on this mosaic (lock held)")]
    LockHeld,

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde_json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("walkdir: {0}")]
    Walkdir(#[from] walkdir::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, BackupError>;
