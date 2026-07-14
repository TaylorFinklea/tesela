//! Sync error type.
//!
//! Owned, FFI-friendly. No lifetimes, no generics. Variants carry strings
//! and primitive types so UniFFI bindings are mechanical.

use thiserror::Error;

/// Result alias for sync operations.
pub type SyncResult<T> = std::result::Result<T, SyncError>;

/// Errors produced by the sync substrate.
#[derive(Debug, Error)]
pub enum SyncError {
    /// SQLite or sqlx error during a sync operation.
    #[error("storage error: {0}")]
    Storage(String),

    /// Postcard serialize or deserialize failed.
    #[error("wire format error: {0}")]
    Wire(String),

    /// HLC saw a remote timestamp too far in the future to accept.
    #[error("clock skew: remote ahead by {drift_millis}ms, max allowed {max_drift_millis}ms")]
    ClockSkew {
        /// How far ahead the remote was, in milliseconds.
        drift_millis: i64,
        /// The configured maximum drift, in milliseconds.
        max_drift_millis: i64,
    },

    /// Op was produced under a schema version this device does not know.
    #[error("schema mismatch: op at v{op_version}, local at v{local_version}")]
    SchemaMismatch {
        /// Schema version the op was produced under.
        op_version: u32,
        /// Local schema version.
        local_version: u32,
    },

    /// Decryption or authentication failed for an incoming envelope.
    #[error("crypto error: {0}")]
    Crypto(String),

    /// An invariant of the protocol was violated.
    #[error("protocol violation: {0}")]
    Protocol(String),

    /// A subtree relocation request failed authoritative precondition checks.
    #[error("relocation rejected: {0}")]
    RelocationRejected(String),

    /// An idempotency key was reused for a different relocation request.
    #[error("relocation conflict: {0}")]
    RelocationConflict(String),

    /// An interrupted relocation must be recovered before this request can continue.
    #[error("relocation {move_id:?} requires recovery: {message}")]
    RelocationRecoveryRequired {
        /// Idempotency key of the interrupted move.
        move_id: [u8; 16],
        /// Recovery failure detail.
        message: String,
    },

    /// Transport-level error (connection refused, channel closed, etc.).
    #[error("transport error: {0}")]
    Transport(String),

    /// A pure-translator chain could not be assembled between two versions.
    #[error("no translator chain from v{from} to v{to}")]
    NoTranslatorChain {
        /// Version translation is requested from.
        from: u32,
        /// Version translation is requested to.
        to: u32,
    },

    /// Op was already parked or applied (content_hash match).
    #[error("op already seen: {hash_hex}")]
    DuplicateOp {
        /// Hex-encoded content hash of the duplicate op.
        hash_hex: String,
    },

    /// Generic error with a string message. Avoid in new code; prefer a
    /// specific variant.
    #[error("{0}")]
    Other(String),
}

impl From<sqlx::Error> for SyncError {
    fn from(e: sqlx::Error) -> Self {
        SyncError::Storage(e.to_string())
    }
}

impl From<postcard::Error> for SyncError {
    fn from(e: postcard::Error) -> Self {
        SyncError::Wire(e.to_string())
    }
}
