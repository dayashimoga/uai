//! Universal AI Assistant local peer discovery and mDNS-SD protocol engine.
//!
//! Provides RFC 1035 / RFC 6762 / RFC 6763 DNS wire-format encoding and parsing,
//! privacy-preserving local service advertisements (`_aissistant._udp.local`),
//! deterministic virtual discovery simulation, and live dual-stack UDP multicast
//! network discovery.

pub mod cert;
pub mod discovery;
pub mod dns;
pub mod error;
pub mod transport;
pub mod types;

pub use cert::{
    extract_ed25519_public_key, generate_tls_cert, PinnedEd25519Verifier, ED25519_SPKI_PREFIX,
};
pub use discovery::{
    build_announcement_packet, build_query_packet, current_time_ms, format_fingerprint,
    parse_fingerprint, parse_peer_from_packet, DiscoveryEngine, MdnsDiscoveryEngine,
    VirtualDiscoveryEngine,
};
pub use dns::{DnsPacket, DnsQuestion, DnsRecord, RData};
pub use error::{NetworkError, Result};
pub use transport::{
    build_quic_client_config, build_quic_server_config, create_virtual_session_pair,
    extract_peer_public_key, QuicSession, QuicTransportEngine, VirtualSession, ALPN_P2P_PROTOCOL,
    DEFAULT_IDLE_TIMEOUT, DEFAULT_KEEPALIVE_INTERVAL,
};
pub use types::{
    DiscoveredPeer, DiscoveryEvent, ServiceConfig, DEFAULT_QUIC_PORT, MDNS_IPV4_MULTICAST,
    MDNS_PORT, MDNS_SERVICE_TYPE,
};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use aissistant_core::PlatformTarget;
    use std::collections::HashMap;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_public_reexports() {
        let config = ServiceConfig::default();
        assert_eq!(config.port, DEFAULT_QUIC_PORT);
        assert_eq!(MDNS_PORT, 5353);
        assert_eq!(MDNS_SERVICE_TYPE, "_aissistant._udp.local");
    }

    #[test]
    fn test_end_to_end_virtual_mesh_discovery() {
        let node_a = VirtualDiscoveryEngine::new();
        let node_b = VirtualDiscoveryEngine::new();

        let cfg_a = ServiceConfig {
            instance_name: "node-alpha".to_string(),
            port: 51820,
            platform: PlatformTarget::Windows,
            fingerprint: [0x12; 8],
            protocol_version: 1,
        };
        let cfg_b = ServiceConfig {
            instance_name: "node-beta".to_string(),
            port: 51821,
            platform: PlatformTarget::Linux,
            fingerprint: [0x34; 8],
            protocol_version: 1,
        };

        node_a.start_advertising(cfg_a.clone()).unwrap();
        node_b.start_advertising(cfg_b.clone()).unwrap();

        // Simulate mesh announcement cross-feeding
        let peer_a = DiscoveredPeer {
            instance_name: cfg_a.instance_name.clone(),
            platform: cfg_a.platform,
            port: cfg_a.port,
            addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10))],
            fingerprint: cfg_a.fingerprint,
            protocol_version: cfg_a.protocol_version,
            last_seen_ms: current_time_ms(),
            txt_records: HashMap::new(),
        };

        let peer_b = DiscoveredPeer {
            instance_name: cfg_b.instance_name.clone(),
            platform: cfg_b.platform,
            port: cfg_b.port,
            addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))],
            fingerprint: cfg_b.fingerprint,
            protocol_version: cfg_b.protocol_version,
            last_seen_ms: current_time_ms(),
            txt_records: HashMap::new(),
        };

        node_a.simulate_peer_announcement(peer_b);
        node_b.simulate_peer_announcement(peer_a);

        assert_eq!(node_a.active_peers().len(), 1);
        assert_eq!(node_b.active_peers().len(), 1);

        let events_a = node_a.poll_events();
        assert_eq!(events_a.len(), 1);
        match &events_a[0] {
            DiscoveryEvent::PeerDiscovered(p) => assert_eq!(p.instance_name, "node-beta"),
            _ => panic!("Expected PeerDiscovered"),
        }

        let events_b = node_b.poll_events();
        assert_eq!(events_b.len(), 1);
        match &events_b[0] {
            DiscoveryEvent::PeerDiscovered(p) => assert_eq!(p.instance_name, "node-alpha"),
            _ => panic!("Expected PeerDiscovered"),
        }
    }
}
