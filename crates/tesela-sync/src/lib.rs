//! `tesela-sync` is the multi-device sync substrate for Tesela.
//!
//! See `plan/sync-architecture.md` for the design. This crate provides:
//!
//! - An append-only oplog with hybrid logical clock (HLC) timestamps.
//! - A row-level last-writer-wins `SyncEngine` trait, with a SQLite-backed
//!   default implementation.
//! - A pluggable `Transport` abstraction (loopback for tests in Phase 1;
//!   LAN with mDNS + TLS pinning in Phase 2; WAN WebSocket relay in Phase 3).
//! - Wire format using `postcard` for compact, Rust-native serialization.
//!
//! ## FFI discipline
//!
//! Public API surface intentionally avoids:
//!
//! - Borrowed types / lifetimes in trait methods.
//! - Generics in public methods.
//! - Non-owned error types.
//!
//! This keeps the eventual `tesela-sync-ffi` UniFFI shim a mechanical wrap
//! rather than a refactor.
//!
//! ## Schema versioning
//!
//! Two distinct version concepts:
//!
//! - DDL schema version: SQLite schema (managed by `tesela-core`'s
//!   `MIGRATIONS` mechanism).
//! - Sync op schema version: the shape of [`OpPayload`]. Stamped onto every
//!   locally produced op. Tracked here as [`SYNC_SCHEMA_VERSION`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod device;
pub mod diff;
pub mod discovery;
pub mod engine;
pub mod error;
pub mod group;
pub mod hlc;
pub mod oplog;
pub mod recovery;
pub mod schema;
pub mod transport;
pub mod wire;

// Crypto module is a placeholder until Phase 2 (LAN transport + pairing).
pub mod crypto;

// Public re-exports. Keep this surface narrow and FFI-friendly.

pub use crypto::aead::{envelope_aad, open as aead_open, seal as aead_seal, SealedPayload};
pub use crypto::keys::{
    adopt as adopt_group_identity, load_or_create as load_or_create_group_identity, GroupIdentity,
    GroupKey,
};
pub use crypto::pairing::{
    decode as decode_pairing_code, encode as encode_pairing_code, PairingCode,
};
pub use device::{DeviceId, DeviceMetadata};
pub use discovery::{DiscoveredPeer, LanDiscovery, TESELA_SERVICE_TYPE};
pub use engine::loro_engine::{LoroEngine, INBOX_DEFAULT_DSL, INBOX_VIEW_ID, VIEWS_DOC_ID};
pub use engine::{
    hydrate_note, AppliedChanges, BlockRelocationOutcome, BlockRelocationRequest,
    BlockRelocationStatus, EngineImportNoteWriter, LocalCursor, MovePlacement, PeerCursor,
    PendingImport, RelayApplyReport, RelocatedNoteVersion, RelocationNoteSeed, SyncEngine,
    TableColumnConfig, ViewRecord,
};
pub use error::{SyncError, SyncResult};
pub use group::{GroupId, GroupMember};
pub use hlc::{Hlc, HlcTimestamp};
pub use oplog::op::{ContentHash, EncodedOp, OpKind, OpPayload, PropOp};
pub use tesela_core::property::PropScalar;
pub use transport::loopback::LoopbackTransport;
pub use transport::{Transport, TransportSession, TransportTarget, TransportTickReport};
pub use wire::envelope::SyncEnvelope;
pub use wire::{
    decode_loro_relay_payload, encode_loro_relay_payload, pack_loro_relay_batches, LoroDocUpdate,
    LORO_RELAY_MAGIC, MAX_RELAY_PLAINTEXT_BYTES,
};

/// The sync op schema version stamped onto every locally produced op.
///
/// Bumps when [`OpPayload`] shape or semantics change.
pub const SYNC_SCHEMA_VERSION: u32 = 1;
