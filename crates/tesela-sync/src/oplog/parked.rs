//! Parked ops (received but not yet applicable).

use serde::{Deserialize, Serialize};

/// Reason an op was parked rather than applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParkReason {
    /// Op's schema version is newer than this device understands.
    NewerSchemaVersion,
    /// No translator chain available to bring the op forward.
    NoTranslatorChain,
    /// Generic park reason with a free-form string.
    Other(String),
}

impl ParkReason {
    /// Render as a stable string for storage in the `park_reason` column.
    pub fn as_db_string(&self) -> String {
        match self {
            ParkReason::NewerSchemaVersion => "newer_schema_version".to_string(),
            ParkReason::NoTranslatorChain => "no_translator_chain".to_string(),
            ParkReason::Other(s) => format!("other:{s}"),
        }
    }
}
