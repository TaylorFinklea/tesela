//! In-process loopback transport.
//!
//! Two `LoopbackTransport` instances paired by [`LoopbackTransport::pair`]
//! share an mpsc channel pair and look like remote peers to each other.
//! Used for tests and the `two_node` example.
//!
//! Phase 1 keeps things simple: both sides call [`Transport::open`]
//! explicitly; `incoming()` yields nothing. This is enough to validate
//! engine + oplog + HLC convergence over a transport boundary.

use crate::device::DeviceId;
use crate::error::{SyncError, SyncResult};
use crate::transport::{Transport, TransportSession, TransportTarget, TransportTickReport};
use crate::wire::envelope::SyncEnvelope;
use async_trait::async_trait;
use futures::stream::{self, Stream};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// One end of a loopback channel pair.
pub struct LoopbackTransport {
    self_device: DeviceId,
    peer_device: DeviceId,
    /// Outbound: messages we send.
    outbound: mpsc::Sender<SyncEnvelope>,
    /// Inbound: messages we receive. Mutex because `recv` requires &mut
    /// but the trait passes `&self` everywhere for Sync ergonomics.
    inbound: Arc<Mutex<Option<mpsc::Receiver<SyncEnvelope>>>>,
}

impl LoopbackTransport {
    /// Create a pair of loopback transports that talk to each other.
    pub fn pair(device_a: DeviceId, device_b: DeviceId) -> (Self, Self) {
        let (a_to_b_tx, a_to_b_rx) = mpsc::channel::<SyncEnvelope>(64);
        let (b_to_a_tx, b_to_a_rx) = mpsc::channel::<SyncEnvelope>(64);

        let a = LoopbackTransport {
            self_device: device_a,
            peer_device: device_b,
            outbound: a_to_b_tx,
            inbound: Arc::new(Mutex::new(Some(b_to_a_rx))),
        };
        let b = LoopbackTransport {
            self_device: device_b,
            peer_device: device_a,
            outbound: b_to_a_tx,
            inbound: Arc::new(Mutex::new(Some(a_to_b_rx))),
        };
        (a, b)
    }

    /// Device id of the peer at the other end of this loopback.
    pub fn peer_device(&self) -> DeviceId {
        self.peer_device
    }

    /// Device id of this transport's local side.
    pub fn self_device(&self) -> DeviceId {
        self.self_device
    }
}

#[async_trait]
impl Transport for LoopbackTransport {
    async fn open(&self, target: TransportTarget) -> SyncResult<Box<dyn TransportSession>> {
        let target_peer = match target {
            TransportTarget::Peer(id) => id,
            TransportTarget::Relay { .. } => {
                return Err(SyncError::Transport(
                    "LoopbackTransport does not support relay targets".to_string(),
                ))
            }
        };
        if target_peer != self.peer_device {
            return Err(SyncError::Transport(format!(
                "loopback only knows one peer ({}); asked for {}",
                self.peer_device, target_peer,
            )));
        }
        let inbound = {
            let mut slot = self.inbound.lock().await;
            slot.take().ok_or_else(|| {
                SyncError::Transport(
                    "loopback session already opened (only one session per pair allowed)"
                        .to_string(),
                )
            })?
        };
        Ok(Box::new(LoopbackSession {
            peer_device: self.peer_device,
            outbound: self.outbound.clone(),
            inbound,
            closed: false,
        }))
    }

    async fn tick(&self) -> SyncResult<TransportTickReport> {
        Ok(TransportTickReport::default())
    }

    fn incoming(&self) -> Pin<Box<dyn Stream<Item = Box<dyn TransportSession>> + Send>> {
        // Phase 1 loopback: both sides open explicitly. Empty stream is fine.
        Box::pin(stream::empty())
    }
}

/// Loopback session: bidirectional envelope exchange over mpsc channels.
pub struct LoopbackSession {
    peer_device: DeviceId,
    outbound: mpsc::Sender<SyncEnvelope>,
    inbound: mpsc::Receiver<SyncEnvelope>,
    closed: bool,
}

#[async_trait]
impl TransportSession for LoopbackSession {
    fn peer(&self) -> DeviceId {
        self.peer_device
    }

    async fn send(&mut self, envelope: SyncEnvelope) -> SyncResult<()> {
        if self.closed {
            return Err(SyncError::Transport("session closed".to_string()));
        }
        self.outbound
            .send(envelope)
            .await
            .map_err(|_| SyncError::Transport("peer dropped".to_string()))?;
        Ok(())
    }

    async fn recv(&mut self) -> SyncResult<Option<SyncEnvelope>> {
        if self.closed {
            return Ok(None);
        }
        Ok(self.inbound.recv().await)
    }

    async fn close(&mut self) -> SyncResult<()> {
        self.closed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn loopback_send_recv_one_envelope() {
        let dev_a = DeviceId::new_random();
        let dev_b = DeviceId::new_random();
        let (a, b) = LoopbackTransport::pair(dev_a, dev_b);

        let mut session_a = a.open(TransportTarget::Peer(dev_b)).await.unwrap();
        let mut session_b = b.open(TransportTarget::Peer(dev_a)).await.unwrap();

        let env = SyncEnvelope {
            from_device: dev_a,
            to_group: crate::group::GroupId::new_random(),
            nonce: [0u8; 24],
            ciphertext: vec![1, 2, 3],
        };
        session_a.send(env.clone()).await.unwrap();

        let received = session_b.recv().await.unwrap().expect("got envelope");
        assert_eq!(received.from_device, env.from_device);
        assert_eq!(received.ciphertext, env.ciphertext);
    }
}
