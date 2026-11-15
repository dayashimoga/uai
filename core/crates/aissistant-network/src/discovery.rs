//! mDNS-SD discovery engine trait, virtual in-memory engine, and live UDP multicast engine.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use aissistant_core::PlatformTarget;

use crate::dns::{
    DnsPacket, DnsQuestion, DnsRecord, RData, CLASS_IN, TYPE_A, TYPE_AAAA, TYPE_PTR, TYPE_SRV,
    TYPE_TXT,
};
use crate::error::{NetworkError, Result};
use crate::types::{
    DiscoveredPeer, DiscoveryEvent, ServiceConfig, DEFAULT_QUIC_PORT, MDNS_IPV4_MULTICAST,
    MDNS_PORT, MDNS_SERVICE_TYPE,
};

/// Maximum UDP datagram buffer size for DNS packets.
const MAX_DATAGRAM_SIZE: usize = 4096;

/// Default time-to-live (seconds) for mDNS resource records.
const DEFAULT_MDNS_TTL: u32 = 120;

/// Formats an 8-byte public key fingerprint as a 16-character hexadecimal string.
#[must_use]
pub fn format_fingerprint(fp: &[u8; 8]) -> String {
    let mut s = String::with_capacity(16);
    for b in fp {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Parses a 16-character hexadecimal string into an 8-byte public key fingerprint.
#[must_use]
pub fn parse_fingerprint(hex_str: &str) -> Option<[u8; 8]> {
    if hex_str.len() != 16 {
        return None;
    }
    let mut bytes = [0u8; 8];
    for (i, slot) in bytes.iter_mut().enumerate() {
        let chunk = &hex_str[i * 2..i * 2 + 2];
        match u8::from_str_radix(chunk, 16) {
            Ok(b) => *slot = b,
            Err(_) => return None,
        }
    }
    Some(bytes)
}

/// Returns the current Unix epoch time in milliseconds.
#[must_use]
pub fn current_time_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_millis() as u64,
        Err(_) => 0,
    }
}

/// Core interface for advertising local services and discovering remote peers over mDNS-SD.
pub trait DiscoveryEngine: std::fmt::Debug + Send + Sync {
    /// Starts advertising the local service over mDNS with the given configuration.
    fn start_advertising(&self, config: ServiceConfig) -> Result<()>;

    /// Stops advertising the local service.
    fn stop_advertising(&self) -> Result<()>;

    /// Returns whether the engine is currently advertising.
    fn is_advertising(&self) -> bool;

    /// Returns the active local service configuration if advertising.
    fn service_config(&self) -> Option<ServiceConfig>;

    /// Returns the list of currently known discovered peers.
    fn active_peers(&self) -> Vec<DiscoveredPeer>;

    /// Broadcasts an mDNS query packet searching for Universal AI Assistant services on the local network.
    fn send_query(&self) -> Result<()>;

    /// Drains and returns all queued discovery lifecycle events.
    fn poll_events(&self) -> Vec<DiscoveryEvent>;
}

/// Constructs an authoritative mDNS response packet announcing the given service.
#[must_use]
pub fn build_announcement_packet(
    config: &ServiceConfig,
    local_ips: &[IpAddr],
    ttl: u32,
) -> DnsPacket {
    let service_name = format!("{}.{}", config.instance_name, MDNS_SERVICE_TYPE);
    let target_host = format!("{}.local", config.instance_name);

    let mut answers = Vec::with_capacity(3);
    // PTR Record: _aissistant._udp.local -> instance._aissistant._udp.local
    answers.push(DnsRecord {
        name: MDNS_SERVICE_TYPE.to_string(),
        rtype: TYPE_PTR,
        rclass: CLASS_IN,
        cache_flush: false,
        ttl,
        rdata: RData::Ptr(service_name.clone()),
    });

    // SRV Record: instance._aissistant._udp.local -> port & target
    answers.push(DnsRecord {
        name: service_name.clone(),
        rtype: TYPE_SRV,
        rclass: CLASS_IN,
        cache_flush: true,
        ttl,
        rdata: RData::Srv {
            priority: 0,
            weight: 0,
            port: config.port,
            target: target_host.clone(),
        },
    });

    // TXT Record: instance._aissistant._udp.local -> metadata
    let txt_entries = vec![
        format!("proto_ver={}", config.protocol_version),
        format!("platform={}", config.platform.as_str()),
        format!("fp={}", format_fingerprint(&config.fingerprint)),
    ];
    answers.push(DnsRecord {
        name: service_name,
        rtype: TYPE_TXT,
        rclass: CLASS_IN,
        cache_flush: true,
        ttl,
        rdata: RData::Txt(txt_entries),
    });

    // Additionals: A and AAAA records for target_host
    let mut additionals = Vec::new();
    for &ip in local_ips {
        match ip {
            IpAddr::V4(v4) => {
                additionals.push(DnsRecord {
                    name: target_host.clone(),
                    rtype: TYPE_A,
                    rclass: CLASS_IN,
                    cache_flush: true,
                    ttl,
                    rdata: RData::A(v4),
                });
            }
            IpAddr::V6(v6) => {
                additionals.push(DnsRecord {
                    name: target_host.clone(),
                    rtype: TYPE_AAAA,
                    rclass: CLASS_IN,
                    cache_flush: true,
                    ttl,
                    rdata: RData::Aaaa(v6),
                });
            }
        }
    }

    DnsPacket::new_response(0, answers, additionals)
}

/// Constructs a standard mDNS query packet searching for `_aissistant._udp.local`.
#[must_use]
pub fn build_query_packet() -> DnsPacket {
    DnsPacket::new_query(
        0,
        vec![DnsQuestion {
            name: MDNS_SERVICE_TYPE.to_string(),
            qtype: TYPE_PTR,
            qclass: CLASS_IN,
            unicast_response: false,
        }],
    )
}

/// Parses a DNS packet to extract a discovered peer, if present.
#[must_use]
pub fn parse_peer_from_packet(
    packet: &DnsPacket,
    now_ms: u64,
    source_ip: Option<IpAddr>,
) -> Option<DiscoveredPeer> {
    let mut instance_name = None;
    let mut port = DEFAULT_QUIC_PORT;
    let mut platform = PlatformTarget::Linux;
    let mut fingerprint = [0u8; 8];
    let mut protocol_version = 1u16;
    let mut addresses = Vec::new();
    let mut txt_records = HashMap::new();

    let all_records = packet.answers.iter().chain(packet.additionals.iter());

    for record in all_records {
        match &record.rdata {
            RData::Ptr(target) => {
                let suffix = format!(".{}", MDNS_SERVICE_TYPE);
                if let Some(prefix) = target.strip_suffix(&suffix) {
                    instance_name = Some(prefix.to_string());
                } else if target.contains(MDNS_SERVICE_TYPE) {
                    if let Some(pos) = target.find(&format!(".{MDNS_SERVICE_TYPE}")) {
                        instance_name = Some(target[..pos].to_string());
                    }
                }
            }
            RData::Srv { port: p, .. } => {
                port = *p;
                let suffix = format!(".{}", MDNS_SERVICE_TYPE);
                if let Some(prefix) = record.name.strip_suffix(&suffix) {
                    if instance_name.is_none() {
                        instance_name = Some(prefix.to_string());
                    }
                }
            }
            RData::Txt(entries) => {
                for entry in entries {
                    if let Some((k, v)) = entry.split_once('=') {
                        txt_records.insert(k.to_string(), v.to_string());
                        match k {
                            "proto_ver" => {
                                if let Ok(ver) = v.parse::<u16>() {
                                    protocol_version = ver;
                                }
                            }
                            "platform" => {
                                if let Ok(plt) = v.parse::<PlatformTarget>() {
                                    platform = plt;
                                }
                            }
                            "fp" => {
                                if let Some(fp_bytes) = parse_fingerprint(v) {
                                    fingerprint = fp_bytes;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            RData::A(v4) => {
                let ip = IpAddr::V4(*v4);
                if !addresses.contains(&ip) {
                    addresses.push(ip);
                }
            }
            RData::Aaaa(v6) => {
                let ip = IpAddr::V6(*v6);
                if !addresses.contains(&ip) {
                    addresses.push(ip);
                }
            }
            RData::Raw(_) => {}
        }
    }

    // If no addresses were contained in the packet, use the sender's IP
    if addresses.is_empty() {
        if let Some(ip) = source_ip {
            addresses.push(ip);
        }
    }

    let name = instance_name?;

    Some(DiscoveredPeer {
        instance_name: name,
        platform,
        port,
        addresses,
        fingerprint,
        protocol_version,
        last_seen_ms: now_ms,
        txt_records,
    })
}

/// Virtual in-memory discovery engine for deterministic unit, integration, and CI testing.
#[derive(Debug, Default)]
pub struct VirtualDiscoveryEngine {
    service_config: RwLock<Option<ServiceConfig>>,
    peers: RwLock<HashMap<String, DiscoveredPeer>>,
    events: RwLock<Vec<DiscoveryEvent>>,
}

impl VirtualDiscoveryEngine {
    /// Creates a new virtual discovery engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Simulates a peer announcement packet being received.
    pub fn simulate_peer_announcement(&self, peer: DiscoveredPeer) {
        let mut peers = match self.peers.write() {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut events = match self.events.write() {
            Ok(e) => e,
            Err(_) => return,
        };

        if let Some(existing) = peers.get_mut(&peer.instance_name) {
            let changed = existing.port != peer.port
                || existing.addresses != peer.addresses
                || existing.fingerprint != peer.fingerprint
                || existing.platform != peer.platform
                || existing.txt_records != peer.txt_records;

            *existing = peer.clone();
            if changed {
                events.push(DiscoveryEvent::PeerUpdated(peer));
            }
        } else {
            peers.insert(peer.instance_name.clone(), peer.clone());
            events.push(DiscoveryEvent::PeerDiscovered(peer));
        }
    }

    /// Simulates a peer leaving or timing out.
    pub fn simulate_peer_loss(&self, instance_name: &str) {
        let mut peers = match self.peers.write() {
            Ok(p) => p,
            Err(_) => return,
        };
        let mut events = match self.events.write() {
            Ok(e) => e,
            Err(_) => return,
        };

        if peers.remove(instance_name).is_some() {
            events.push(DiscoveryEvent::PeerLost {
                instance_name: instance_name.to_string(),
            });
        }
    }

    /// Prunes peers that have not been observed within `max_idle_ms`.
    pub fn prune_expired(&self, max_idle_ms: u64, now_ms: u64) -> usize {
        let mut peers = match self.peers.write() {
            Ok(p) => p,
            Err(_) => return 0,
        };
        let mut events = match self.events.write() {
            Ok(e) => e,
            Err(_) => return 0,
        };

        let mut expired = Vec::new();
        for (name, peer) in peers.iter() {
            if now_ms.saturating_sub(peer.last_seen_ms) > max_idle_ms {
                expired.push(name.clone());
            }
        }

        let count = expired.len();
        for name in expired {
            peers.remove(&name);
            events.push(DiscoveryEvent::PeerLost {
                instance_name: name,
            });
        }
        count
    }

    /// Clears all discovered peers and pending events.
    pub fn clear(&self) {
        if let Ok(mut peers) = self.peers.write() {
            peers.clear();
        }
        if let Ok(mut events) = self.events.write() {
            events.clear();
        }
    }
}

impl DiscoveryEngine for VirtualDiscoveryEngine {
    fn start_advertising(&self, config: ServiceConfig) -> Result<()> {
        let mut lock = self
            .service_config
            .write()
            .map_err(|_| NetworkError::Internal("Lock poisoned".to_string()))?;
        *lock = Some(config);
        Ok(())
    }

    fn stop_advertising(&self) -> Result<()> {
        let mut lock = self
            .service_config
            .write()
            .map_err(|_| NetworkError::Internal("Lock poisoned".to_string()))?;
        *lock = None;
        Ok(())
    }

    fn is_advertising(&self) -> bool {
        self.service_config
            .read()
            .map(|opt| opt.is_some())
            .unwrap_or(false)
    }

    fn service_config(&self) -> Option<ServiceConfig> {
        self.service_config.read().ok().and_then(|opt| opt.clone())
    }

    fn active_peers(&self) -> Vec<DiscoveredPeer> {
        self.peers
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    fn send_query(&self) -> Result<()> {
        // Virtual engine query is an in-memory no-op
        Ok(())
    }

    fn poll_events(&self) -> Vec<DiscoveryEvent> {
        self.events
            .write()
            .map(|mut e| e.drain(..).collect())
            .unwrap_or_default()
    }
}

/// Live mDNS discovery engine operating over UDP multicast sockets.
#[derive(Debug)]
pub struct MdnsDiscoveryEngine {
    socket: Option<UdpSocket>,
    service_config: RwLock<Option<ServiceConfig>>,
    peers: RwLock<HashMap<String, DiscoveredPeer>>,
    events: RwLock<Vec<DiscoveryEvent>>,
    local_ips: RwLock<Vec<IpAddr>>,
    ttl: u32,
    multicast_dest: SocketAddr,
}

impl MdnsDiscoveryEngine {
    /// Attempts to create an mDNS discovery engine bound to the specified port.
    ///
    /// If port 5353 cannot be bound (e.g. OS mDNS responder is active), it will attempt
    /// to bind to an ephemeral port (0) while still participating in multicast discovery.
    pub fn new(bind_port: u16) -> Result<Self> {
        let socket = match UdpSocket::bind(("0.0.0.0", bind_port)) {
            Ok(s) => s,
            Err(_) if bind_port != 0 => {
                UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| NetworkError::SocketBindFailed {
                    endpoint: "0.0.0.0:0".to_string(),
                    reason: e.to_string(),
                })?
            }
            Err(e) => {
                return Err(NetworkError::SocketBindFailed {
                    endpoint: format!("0.0.0.0:{bind_port}"),
                    reason: e.to_string(),
                })
            }
        };

        let _ = socket.set_nonblocking(true);
        let _ = socket.set_multicast_loop_v4(true);
        let _ =
            socket.join_multicast_v4(&Ipv4Addr::new(224, 0, 0, 251), &Ipv4Addr::new(0, 0, 0, 0));

        let multicast_ip: Ipv4Addr = match MDNS_IPV4_MULTICAST.parse() {
            Ok(ip) => ip,
            Err(_) => Ipv4Addr::new(224, 0, 0, 251),
        };
        let multicast_dest = SocketAddr::new(IpAddr::V4(multicast_ip), MDNS_PORT);

        Ok(Self {
            socket: Some(socket),
            service_config: RwLock::new(None),
            peers: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            local_ips: RwLock::new(Vec::new()),
            ttl: DEFAULT_MDNS_TTL,
            multicast_dest,
        })
    }

    /// Creates an mDNS discovery engine wrapping an existing UDP socket.
    #[must_use]
    pub fn with_socket(socket: UdpSocket, multicast_dest: SocketAddr) -> Self {
        let _ = socket.set_nonblocking(true);
        Self {
            socket: Some(socket),
            service_config: RwLock::new(None),
            peers: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            local_ips: RwLock::new(Vec::new()),
            ttl: DEFAULT_MDNS_TTL,
            multicast_dest,
        }
    }

    /// Registers a local IP address to include in mDNS A / AAAA advertisements.
    pub fn add_local_ip(&self, ip: IpAddr) {
        if let Ok(mut ips) = self.local_ips.write() {
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
    }

    /// Sends an announcement packet for the currently advertised service.
    pub fn announce(&self) -> Result<()> {
        let config = match self.service_config.read() {
            Ok(guard) => match guard.as_ref() {
                Some(c) => c.clone(),
                None => return Ok(()),
            },
            Err(_) => return Err(NetworkError::Internal("Lock poisoned".to_string())),
        };

        let local_ips = self
            .local_ips
            .read()
            .map(|ips| ips.clone())
            .unwrap_or_default();

        let packet = build_announcement_packet(&config, &local_ips, self.ttl);
        let bytes = packet.to_bytes();

        if let Some(socket) = &self.socket {
            let _ = socket.send_to(&bytes, self.multicast_dest);
        }
        Ok(())
    }

    /// Processes a single raw incoming DNS packet buffer.
    pub fn handle_packet(
        &self,
        buf: &[u8],
        source_addr: Option<SocketAddr>,
    ) -> Result<Option<DiscoveryEvent>> {
        let packet = DnsPacket::from_bytes(buf)?;

        // If it's a query for our service and we are advertising, respond
        let is_query = (packet.flags & 0x8000) == 0;
        if is_query {
            let matches_service = packet.questions.iter().any(|q| {
                q.name == MDNS_SERVICE_TYPE || q.name.ends_with(&format!(".{MDNS_SERVICE_TYPE}"))
            });

            if matches_service && self.is_advertising() {
                let _ = self.announce();
            }
            return Ok(None);
        }

        // Process response records
        let now_ms = current_time_ms();
        let src_ip = source_addr.map(|sa| sa.ip());

        if let Some(peer) = parse_peer_from_packet(&packet, now_ms, src_ip) {
            // Ignore ourselves if instance_name matches our own config
            if let Ok(cfg_guard) = self.service_config.read() {
                if let Some(cfg) = cfg_guard.as_ref() {
                    if cfg.instance_name == peer.instance_name {
                        return Ok(None);
                    }
                }
            }

            let mut peers = self
                .peers
                .write()
                .map_err(|_| NetworkError::Internal("Lock poisoned".to_string()))?;
            let mut events = self
                .events
                .write()
                .map_err(|_| NetworkError::Internal("Lock poisoned".to_string()))?;

            if let Some(existing) = peers.get_mut(&peer.instance_name) {
                let changed = existing.port != peer.port
                    || existing.addresses != peer.addresses
                    || existing.fingerprint != peer.fingerprint
                    || existing.platform != peer.platform
                    || existing.txt_records != peer.txt_records;

                *existing = peer.clone();
                if changed {
                    let ev = DiscoveryEvent::PeerUpdated(peer);
                    events.push(ev.clone());
                    return Ok(Some(ev));
                }
            } else {
                peers.insert(peer.instance_name.clone(), peer.clone());
                let ev = DiscoveryEvent::PeerDiscovered(peer);
                events.push(ev.clone());
                return Ok(Some(ev));
            }
        }

        Ok(None)
    }

    /// Receives and processes any pending packets from the UDP socket.
    pub fn process_incoming(&self) -> Result<Vec<DiscoveryEvent>> {
        let socket = match &self.socket {
            Some(s) => s,
            None => return Ok(Vec::new()),
        };

        let mut buf = [0u8; MAX_DATAGRAM_SIZE];
        let mut new_events = Vec::new();

        // Non-blocking drain loop: process up to 32 packets per call
        for _ in 0..32 {
            match socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    if let Ok(Some(ev)) = self.handle_packet(&buf[..len], Some(src)) {
                        new_events.push(ev);
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    break;
                }
                Err(e) => {
                    tracing::debug!("mDNS socket recv error: {}", e);
                    break;
                }
            }
        }

        Ok(new_events)
    }
}

impl DiscoveryEngine for MdnsDiscoveryEngine {
    fn start_advertising(&self, config: ServiceConfig) -> Result<()> {
        {
            let mut lock = self
                .service_config
                .write()
                .map_err(|_| NetworkError::Internal("Lock poisoned".to_string()))?;
            *lock = Some(config);
        }
        self.announce()
    }

    fn stop_advertising(&self) -> Result<()> {
        let mut lock = self
            .service_config
            .write()
            .map_err(|_| NetworkError::Internal("Lock poisoned".to_string()))?;
        *lock = None;
        Ok(())
    }

    fn is_advertising(&self) -> bool {
        self.service_config
            .read()
            .map(|opt| opt.is_some())
            .unwrap_or(false)
    }

    fn service_config(&self) -> Option<ServiceConfig> {
        self.service_config.read().ok().and_then(|opt| opt.clone())
    }

    fn active_peers(&self) -> Vec<DiscoveredPeer> {
        self.peers
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    fn send_query(&self) -> Result<()> {
        let packet = build_query_packet();
        let bytes = packet.to_bytes();
        if let Some(socket) = &self.socket {
            let _ = socket.send_to(&bytes, self.multicast_dest);
        }
        Ok(())
    }

    fn poll_events(&self) -> Vec<DiscoveryEvent> {
        let _ = self.process_incoming();
        self.events
            .write()
            .map(|mut e| e.drain(..).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_fingerprint_formatting_and_parsing() {
        let fp: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
        let s = format_fingerprint(&fp);
        assert_eq!(s, "0123456789abcdef");

        let parsed = parse_fingerprint(&s);
        assert_eq!(parsed, Some(fp));

        assert_eq!(parse_fingerprint("short"), None);
        assert_eq!(parse_fingerprint("0123456789abcdeg"), None); // 'g' invalid hex
    }

    #[test]
    fn test_build_and_parse_announcement() {
        let config = ServiceConfig {
            instance_name: "test-node-1".to_string(),
            port: 51820,
            platform: PlatformTarget::MacOS,
            fingerprint: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11],
            protocol_version: 2,
        };

        let local_ips = vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50))];
        let packet = build_announcement_packet(&config, &local_ips, 120);

        let bytes = packet.to_bytes();
        let parsed_packet = DnsPacket::from_bytes(&bytes).unwrap();

        let peer = parse_peer_from_packet(&parsed_packet, 1000, None).unwrap();
        assert_eq!(peer.instance_name, "test-node-1");
        assert_eq!(peer.port, 51820);
        assert_eq!(peer.platform, PlatformTarget::MacOS);
        assert_eq!(
            peer.fingerprint,
            [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11]
        );
        assert_eq!(peer.protocol_version, 2);
        assert_eq!(
            peer.addresses,
            vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50))]
        );
        assert_eq!(
            peer.txt_records.get("platform").map(|s| s.as_str()),
            Some("macos")
        );
        assert_eq!(
            peer.txt_records.get("proto_ver").map(|s| s.as_str()),
            Some("2")
        );
    }

    #[test]
    fn test_virtual_discovery_engine_lifecycle() {
        let engine = VirtualDiscoveryEngine::new();
        assert!(!engine.is_advertising());

        let config = ServiceConfig {
            instance_name: "virt-node".to_string(),
            port: 51820,
            platform: PlatformTarget::Linux,
            fingerprint: [1; 8],
            protocol_version: 1,
        };
        engine.start_advertising(config.clone()).unwrap();
        assert!(engine.is_advertising());
        assert_eq!(engine.service_config(), Some(config));

        // Simulate discovering a peer
        let peer1 = DiscoveredPeer {
            instance_name: "remote-peer".to_string(),
            platform: PlatformTarget::Windows,
            port: 51820,
            addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))],
            fingerprint: [2; 8],
            protocol_version: 1,
            last_seen_ms: 5000,
            txt_records: HashMap::new(),
        };

        engine.simulate_peer_announcement(peer1.clone());
        assert_eq!(engine.active_peers().len(), 1);

        let events = engine.poll_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DiscoveryEvent::PeerDiscovered(_)));

        // Polling again should be empty
        assert!(engine.poll_events().is_empty());

        // Update peer
        let mut peer1_updated = peer1.clone();
        peer1_updated.port = 51822;
        engine.simulate_peer_announcement(peer1_updated);

        let update_events = engine.poll_events();
        assert_eq!(update_events.len(), 1);
        assert!(matches!(update_events[0], DiscoveryEvent::PeerUpdated(_)));

        // Pruning with high idle threshold keeps the peer
        let pruned = engine.prune_expired(10000, 6000);
        assert_eq!(pruned, 0);

        // Pruning with low idle threshold removes the peer
        let pruned = engine.prune_expired(500, 10000);
        assert_eq!(pruned, 1);
        assert_eq!(engine.active_peers().len(), 0);

        let lost_events = engine.poll_events();
        assert_eq!(lost_events.len(), 1);
        assert!(matches!(lost_events[0], DiscoveryEvent::PeerLost { .. }));

        // Stop advertising
        engine.stop_advertising().unwrap();
        assert!(!engine.is_advertising());
    }

    #[test]
    fn test_mdns_engine_loopback_datagram_exchange() {
        let socket_a = UdpSocket::bind("127.0.0.1:0").unwrap();
        let socket_b = UdpSocket::bind("127.0.0.1:0").unwrap();

        let addr_a = socket_a.local_addr().unwrap();
        let addr_b = socket_b.local_addr().unwrap();

        let engine_a = MdnsDiscoveryEngine::with_socket(socket_a, addr_b);
        let engine_b = MdnsDiscoveryEngine::with_socket(socket_b, addr_a);

        let config_a = ServiceConfig {
            instance_name: "node-a".to_string(),
            port: 51820,
            platform: PlatformTarget::Linux,
            fingerprint: [0x11; 8],
            protocol_version: 1,
        };
        engine_a.add_local_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        engine_a.start_advertising(config_a).unwrap();

        // Process incoming on engine_b
        let events = engine_b.process_incoming().unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            DiscoveryEvent::PeerDiscovered(peer) => {
                assert_eq!(peer.instance_name, "node-a");
                assert_eq!(peer.port, 51820);
                assert_eq!(peer.platform, PlatformTarget::Linux);
                assert_eq!(peer.fingerprint, [0x11; 8]);
            }
            _ => panic!("Expected PeerDiscovered"),
        }

        assert_eq!(engine_b.active_peers().len(), 1);
    }
}
