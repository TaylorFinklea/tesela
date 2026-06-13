//! LAN device discovery via mDNS (Phase 2.1).
//!
//! Each tesela-server instance advertises a `_tesela._tcp.local.` service
//! carrying its `DeviceId`, a user-visible display name, and its HTTP
//! port via TXT records. A sibling browse task listens for other
//! instances and tracks them in an in-memory map.
//!
//! Phase 2.1 only — pairing, TLS, and crypto come later. A peer surfaced
//! here is a *candidate*, not a trusted peer; the user (or the pairing
//! flow) still has to decide whether to add it.

use crate::device::DeviceId;
use crate::error::{SyncError, SyncResult};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// mDNS service type the entire Tesela fleet uses.
pub const TESELA_SERVICE_TYPE: &str = "_tesela._tcp.local.";

/// TXT property key carrying the lowercase-hex DeviceId.
pub const TXT_DEVICE_ID: &str = "did";
/// TXT property key carrying the user-visible display name.
pub const TXT_DISPLAY_NAME: &str = "name";
/// TXT property key carrying the wire-protocol major version.
pub const TXT_API_VERSION: &str = "v";

/// A peer surfaced by mDNS browse. Not necessarily paired or trusted.
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    /// Remote device's id, parsed from the TXT record.
    pub device_id: DeviceId,
    /// Remote device's user-visible display name.
    pub display_name: String,
    /// First resolved address (IPv4 preferred).
    pub host: IpAddr,
    /// HTTP API port from the SRV record.
    pub port: u16,
    /// Last time we received an mDNS update for this peer.
    pub last_seen: Instant,
}

impl DiscoveredPeer {
    /// HTTP base URL for this peer's tesela-server.
    pub fn http_url(&self) -> String {
        match self.host {
            IpAddr::V4(_) => format!("http://{}:{}", self.host, self.port),
            IpAddr::V6(addr) => format!("http://[{}]:{}", addr, self.port),
        }
    }
}

/// Live mDNS state. Owning instance keeps the daemon alive; dropping
/// it shuts the daemon down (best-effort).
pub struct LanDiscovery {
    daemon: ServiceDaemon,
    /// The fullname we registered. Needed to unregister on stop.
    self_fullname: String,
    /// Our own DeviceId, used to filter ourselves out of browse results.
    self_device: DeviceId,
    peers: Arc<RwLock<HashMap<DeviceId, DiscoveredPeer>>>,
}

impl LanDiscovery {
    /// Advertise this server and start browsing for siblings.
    pub fn start(device_id: DeviceId, display_name: &str, port: u16) -> SyncResult<Self> {
        let daemon = ServiceDaemon::new().map_err(map_mdns_err)?;

        let device_hex = device_id.to_hex();
        // Use the hex id as instance name so we never collide with another
        // device. Use it as hostname too so the SRV record is unique even
        // before name probing finishes.
        let instance_name = device_hex.clone();
        let host_name = format!("{device_hex}.local.");
        let properties: Vec<(String, String)> = vec![
            (TXT_DEVICE_ID.to_string(), device_hex.clone()),
            (TXT_DISPLAY_NAME.to_string(), display_name.to_string()),
            (TXT_API_VERSION.to_string(), "1".to_string()),
        ];

        // Empty address set + enable_addr_auto: let the library fill in the
        // host's reachable interfaces and update them when the network
        // changes (laptop sleep/wake, hotspot toggle, etc.).
        let info = ServiceInfo::new(
            TESELA_SERVICE_TYPE,
            &instance_name,
            &host_name,
            (),
            port,
            &properties[..],
        )
        .map_err(map_mdns_err)?
        .enable_addr_auto();
        let self_fullname = info.get_fullname().to_string();

        daemon.register(info).map_err(map_mdns_err)?;

        let peers: Arc<RwLock<HashMap<DeviceId, DiscoveredPeer>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let browse_rx = daemon.browse(TESELA_SERVICE_TYPE).map_err(map_mdns_err)?;

        // Spawn a background task that drains the mDNS event channel into
        // the peers map. Owning this task is fine — when the daemon is
        // dropped, the receiver closes and the loop exits.
        let peers_for_task = Arc::clone(&peers);
        tokio::spawn(async move {
            while let Ok(event) = browse_rx.recv_async().await {
                handle_event(event, &peers_for_task, device_id);
            }
            tracing::debug!("mDNS browse channel closed; discovery task exiting");
        });

        tracing::info!(
            target: "tesela_sync::discovery",
            device = %device_hex,
            display_name = %display_name,
            port,
            "mDNS advertise + browse started"
        );

        Ok(Self {
            daemon,
            self_fullname,
            self_device: device_id,
            peers,
        })
    }

    /// Snapshot the currently-known peers, freshest first. Filters out
    /// entries older than `max_age` (peers we haven't heard from in a
    /// while are likely offline or off-LAN).
    pub fn snapshot(&self, max_age: Duration) -> Vec<DiscoveredPeer> {
        let now = Instant::now();
        let map = self.peers.read().expect("peers RwLock poisoned");
        let mut out: Vec<DiscoveredPeer> = map
            .values()
            .filter(|p| now.duration_since(p.last_seen) <= max_age)
            .cloned()
            .collect();
        out.sort_by_key(|b| std::cmp::Reverse(b.last_seen));
        out
    }

    /// Our own DeviceId.
    pub fn self_device(&self) -> DeviceId {
        self.self_device
    }

    /// Stop advertising and tear down the daemon. Best-effort.
    pub fn stop(self) -> SyncResult<()> {
        // Unregister returns a channel that completes when the goodbye
        // packet is sent. We don't wait — the daemon shutdown will also
        // race, and either way the OS will clear caches within a minute.
        let _ = self.daemon.unregister(&self.self_fullname);
        let _ = self.daemon.shutdown().map_err(map_mdns_err)?;
        Ok(())
    }
}

fn handle_event(
    event: ServiceEvent,
    peers: &Arc<RwLock<HashMap<DeviceId, DiscoveredPeer>>>,
    self_device: DeviceId,
) {
    match event {
        ServiceEvent::ServiceResolved(info) => {
            let Some(did_hex) = info.get_property_val_str(TXT_DEVICE_ID) else {
                tracing::debug!(
                    fullname = info.get_fullname(),
                    "mDNS peer missing did TXT, ignoring"
                );
                return;
            };
            let Some(device_id) = parse_device_hex(did_hex) else {
                tracing::debug!(did_hex, "mDNS peer did TXT not valid hex");
                return;
            };
            if device_id == self_device {
                return;
            }
            let display_name = info
                .get_property_val_str(TXT_DISPLAY_NAME)
                .unwrap_or("Tesela device")
                .to_string();
            let port = info.get_port();
            let Some(host) = pick_host(&info) else {
                tracing::debug!(
                    fullname = info.get_fullname(),
                    "mDNS peer has no usable address, ignoring"
                );
                return;
            };
            let peer = DiscoveredPeer {
                device_id,
                display_name,
                host,
                port,
                last_seen: Instant::now(),
            };
            tracing::info!(
                target: "tesela_sync::discovery",
                device = %device_id.to_hex(),
                display_name = %peer.display_name,
                url = %peer.http_url(),
                "mDNS peer resolved"
            );
            peers
                .write()
                .expect("peers RwLock poisoned")
                .insert(device_id, peer);
        }
        ServiceEvent::ServiceRemoved(_, fullname) => {
            // The fullname doesn't carry our device id directly, but we
            // tagged it: it begins with the hex DeviceId before the dot.
            if let Some(instance) = fullname.split('.').next() {
                if let Some(device_id) = parse_device_hex(instance) {
                    if peers
                        .write()
                        .expect("peers RwLock poisoned")
                        .remove(&device_id)
                        .is_some()
                    {
                        tracing::info!(
                            target: "tesela_sync::discovery",
                            device = %device_id.to_hex(),
                            "mDNS peer removed"
                        );
                    }
                }
            }
        }
        // SearchStarted / ServiceFound / SearchStopped don't change our map.
        _ => {}
    }
}

fn pick_host(info: &ServiceInfo) -> Option<IpAddr> {
    // Prefer IPv4 — fewer link-local / scoped-id surprises on mixed networks.
    if let Some(ip) = info.get_addresses_v4().iter().next() {
        return Some(IpAddr::V4(**ip));
    }
    info.get_addresses().iter().next().copied()
}

fn parse_device_hex(hex: &str) -> Option<DeviceId> {
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (i, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        bytes[i] = (nibble(chunk[0])? << 4) | nibble(chunk[1])?;
    }
    Some(DeviceId::from_bytes(bytes))
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn map_mdns_err(e: mdns_sd::Error) -> SyncError {
    SyncError::Transport(format!("mdns: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_device_hex_roundtrip() {
        let id = DeviceId::from_bytes([0xab; 16]);
        let hex = id.to_hex();
        let back = parse_device_hex(&hex).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn parse_device_hex_rejects_bad_length() {
        assert!(parse_device_hex("abc").is_none());
        assert!(parse_device_hex(&"a".repeat(31)).is_none());
        assert!(parse_device_hex(&"a".repeat(33)).is_none());
    }

    #[test]
    fn parse_device_hex_rejects_non_hex() {
        let mut s = String::from("ab");
        s.push_str(&"!".repeat(30));
        assert!(parse_device_hex(&s).is_none());
    }

    #[test]
    fn discovered_peer_url_ipv4() {
        let p = DiscoveredPeer {
            device_id: DeviceId::from_bytes([0xcd; 16]),
            display_name: "Test".into(),
            host: "192.168.1.10".parse().unwrap(),
            port: 7474,
            last_seen: Instant::now(),
        };
        assert_eq!(p.http_url(), "http://192.168.1.10:7474");
    }
}
