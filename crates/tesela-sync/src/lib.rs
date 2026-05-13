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
//!   locally produced op. Evolves via [`OpTranslator`]. Tracked here as
//!   [`SYNC_SCHEMA_VERSION`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod device;
pub mod engine;
pub mod error;
pub mod group;
pub mod hlc;
pub mod migrate;
pub mod oplog;
pub mod rebuild;
pub mod schema;
pub mod transport;
pub mod wire;

// Crypto module is a placeholder until Phase 2 (LAN transport + pairing).
pub mod crypto;

// Public re-exports. Keep this surface narrow and FFI-friendly.

pub use device::{DeviceId, DeviceMetadata};
pub use engine::{
    AppliedChanges, LocalCursor, ParkedSummary, PeerCursor, ProducedBatch, ReplayReport,
    SyncEngine,
};
pub use engine::sqlite_engine::SqliteEngine;
pub use oplog::parked::ParkReason;
pub use error::{SyncError, SyncResult};
pub use group::{GroupId, GroupMember};
pub use hlc::{Hlc, HlcTimestamp};
pub use migrate::{OpTranslator, TranslatorRegistry};
pub use oplog::op::{ContentHash, EncodedOp, OpKind, OpPayload};
pub use transport::loopback::LoopbackTransport;
pub use transport::{Transport, TransportSession, TransportTarget, TransportTickReport};
pub use wire::envelope::SyncEnvelope;

/// The sync op schema version stamped onto every locally produced op.
///
/// Bumps when [`OpPayload`] shape or semantics change. Each bump must
/// register an [`OpTranslator`] in `migrate::v{N}_to_v{N+1}` so older
/// peers' ops continue to apply.
pub const SYNC_SCHEMA_VERSION: u32 = 1;
