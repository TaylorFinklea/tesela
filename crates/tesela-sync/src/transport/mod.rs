//! Transport adapter trait. Implementations:
//!
//! - [`loopback::LoopbackTransport`] (Phase 1, in-process, used for tests)
//! - `lan::LanTransport` (Phase 2 placeholder)
//! - `relay::RelayClient` (Phase 3 placeholder)

pub mod lan;
pub mod loopback;
pub mod relay;

pub use loopback::LoopbackTransport;

use crate::device::DeviceId;
use crate::error::SyncResult;
use crate::group::GroupId;
use crate::wire::envelope::SyncEnvelope;
use async_trait::async_trait;
use futures::stream::Stream;
use std::pin::Pin;

/// Target of a transport `open` call.
#[derive(Debug, Clone)]
pub enum TransportTarget {
    /// A specific peer device, resolved via the transport's own discovery.
    Peer(DeviceId),
    /// A relay (WAN) routing by group id.
    Relay {
        /// The group to subscribe to via the relay.
        group: GroupId,
        /// Relay URL.
        relay_url: String,
    },
}

/// Per-tick summary from a transport's background maintenance.
#[derive(Debug, Clone, Default)]
pub struct TransportTickReport {
    /// Whether this tick discovered any new peers.
    pub new_peers: u32,
    /// Whether this tick dropped any peers (connections lost).
    pub dropped_peers: u32,
}

/// The transport abstraction. Concrete implementations handle discovery,
/// handshake, and connection lifecycle.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Open a session to a specific target.
    async fn open(&self, target: TransportTarget) -> SyncResult<Box<dyn TransportSession>>;

    /// Periodic maintenance: announce ourselves, scan for peers, etc.
    async fn tick(&self) -> SyncResult<TransportTickReport>;

    /// Stream of inbound sessions (other peers opening to us).
    fn incoming(&self) -> Pin<Box<dyn Stream<Item = Box<dyn TransportSession>> + Send>>;
}

/// A bidirectional message stream over the underlying transport.
#[async_trait]
pub trait TransportSession: Send + Sync {
    /// The peer at the other end.
    fn peer(&self) -> DeviceId;

    /// Send one envelope.
    async fn send(&mut self, envelope: SyncEnvelope) -> SyncResult<()>;

    /// Receive one envelope, or None if the session is closed.
    async fn recv(&mut self) -> SyncResult<Option<SyncEnvelope>>;

    /// Close the session.
    async fn close(&mut self) -> SyncResult<()>;
}
