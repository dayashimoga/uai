//! Discovery data types representing peer advertisements, service configuration, and discovered network endpoints.

use std::collections::HashMap;
use std::net::IpAddr;

use aissistant_core::PlatformTarget;
use serde::{Deserialize, Serialize};

/// Standard service type for Universal AI Assistant mDNS discovery.
pub const MDNS_SERVICE_TYPE: &str = "_aissistant._udp.local";

/// Standard mDNS multicast IPv4 address (`224.0.0.251`).
pub const MDNS_IPV4_MULTICAST: &str = "224.0.0.251";

/// Standard mDNS UDP port (`5353`).
pub const MDNS_PORT: u16 = 5353;

/// Default listening port for encrypted QUIC cross-device transport.
pub const DEFAULT_QUIC_PORT: u16 = 51820;

/// Local mDNS-SD service advertisement parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Instance identifier broadcast over mDNS (e.g. "aissistant-a1b2c3d4").
    pub instance_name: String,
    /// QUIC transport listening port.
    pub port: u16,
    /// Operating system platform of this device.
    pub platform: PlatformTarget,
    /// Truncated 8-byte cryptographic public key fingerprint.
    pub fingerprint: [u8; 8],
    /// Protocol version for cross-device compatibility negotiation.
    pub protocol_version: u16,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            instance_name: "aissistant-local".to_string(),
            port: DEFAULT_QUIC_PORT,
            platform: PlatformTarget::current(),
            fingerprint: [0u8; 8],
            protocol_version: 1,
        }
    }
}

/// Metadata and resolved IP endpoints of a peer discovered over mDNS-SD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    /// Unique instance identifier of the peer.
    pub instance_name: String,
    /// Operating system platform of the peer.
    pub platform: PlatformTarget,
    /// QUIC transport port.
    pub port: u16,
    /// Resolved IP addresses (IPv4 / IPv6).
    pub addresses: Vec<IpAddr>,
    /// Truncated public key fingerprint.
    pub fingerprint: [u8; 8],
    /// Protocol version supported by the peer.
    pub protocol_version: u16,
    /// Unix timestamp in milliseconds when the peer was most recently observed.
    pub last_seen_ms: u64,
    /// Parsed TXT record key-value pairs.
    pub txt_records: HashMap<String, String>,
}

/// Lifecycle events emitted as network peers appear, roam, or expire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum DiscoveryEvent {
    /// New peer detected on the local network.
    PeerDiscovered(DiscoveredPeer),
    /// Existing peer updated its addresses or metadata.
    PeerUpdated(DiscoveredPeer),
    /// Peer expired due to missing heartbeats or explicit goodbye packet.
    PeerLost {
        /// Instance identifier of the expired peer.
        instance_name: String,
    },
}
