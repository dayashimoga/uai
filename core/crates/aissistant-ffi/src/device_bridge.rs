//! Device management and out-of-band pairing ceremony bridge for Flutter and FFI consumers.

use std::net::IpAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use aissistant_core::{DeviceId, PeerPermissionLevel, PlatformTarget};
pub use aissistant_network::PairingSessionInfo;
use aissistant_network::{
    current_time_ms, format_fingerprint, DiscoveredPeer, DiscoveryEngine, PairingSessionManager,
    VirtualDiscoveryEngine,
};
use aissistant_security::DeviceIdentity;
use aissistant_storage::{DeviceRegistry, SqliteDb};
use serde::{Deserialize, Serialize};

use crate::error::{FfiError, FfiResult};
use crate::events::{BridgeEvent, EventStreamSink};

/// Discovered network peer metadata representation for Flutter UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredPeerItem {
    /// Service instance identifier (e.g. "aissistant-macbook").
    pub instance_name: String,
    /// Operating system platform string ("windows", "macos", "linux", "android", "ios").
    pub platform: String,
    /// QUIC transport port.
    pub port: u16,
    /// Resolved IP addresses.
    pub addresses: Vec<String>,
    /// Hexadecimal public key fingerprint.
    pub fingerprint: String,
    /// Unix timestamp in milliseconds when peer was last observed.
    pub last_seen_ms: u64,
}

/// Granular capability permission assignment for a paired device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerPermissionItem {
    /// Capability name (e.g. "file_system", "local_llm").
    pub capability: String,
    /// ACL level ("always_allow", "ask_each_time", "deny").
    pub permission_level: String,
}

/// Persisted paired device record with capability permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDeviceItem {
    /// Unique device identifier slug ("dev_...").
    pub device_id: String,
    /// User-friendly device name.
    pub name: String,
    /// Operating system platform.
    pub platform: String,
    /// Hexadecimal public key fingerprint.
    pub fingerprint: String,
    /// Trust status ("trusted", "pending_verification", "blocked", "revoked").
    pub trust_state: String,
    /// Most recent observed IP endpoint.
    pub last_endpoint: Option<String>,
    /// Unix timestamp in milliseconds of most recent activity.
    pub last_seen_ms: u64,
    /// Unix timestamp in milliseconds when paired.
    pub paired_at_ms: u64,
    /// Assigned capability ACL rules.
    pub permissions: Vec<PeerPermissionItem>,
}

/// Snapshot of the active pairing ceremony state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingStateSnapshot {
    /// Whether a pairing ceremony is currently in progress.
    pub is_pairing: bool,
    /// Target peer instance name being paired with.
    pub peer_instance: Option<String>,
    /// Derived 6-digit decimal SAS code (e.g. "123-456").
    pub sas_code: Option<String>,
    /// Compact scannable QR payload string.
    pub qr_payload: Option<String>,
    /// Optional failure reason message.
    pub error: Option<String>,
}

/// Device discovery, pairing ceremony, and ACL management supervisor.
#[derive(Debug)]
pub struct DeviceSupervisor {
    identity: Arc<DeviceIdentity>,
    registry: Arc<DeviceRegistry>,
    discovery: Arc<VirtualDiscoveryEngine>,
    pairing_manager: Mutex<PairingSessionManager>,
    event_sink: Arc<EventStreamSink>,
}

impl DeviceSupervisor {
    /// Creates a new device supervisor.
    #[must_use]
    pub fn new(
        identity: Arc<DeviceIdentity>,
        db: SqliteDb,
        event_sink: Arc<EventStreamSink>,
    ) -> Self {
        let registry = Arc::new(DeviceRegistry::new(db));
        let discovery = Arc::new(VirtualDiscoveryEngine::new());
        let pairing_manager = Mutex::new(PairingSessionManager::new());

        Self {
            identity,
            registry,
            discovery,
            pairing_manager,
            event_sink,
        }
    }

    /// Returns the list of currently discovered peers on the local network.
    #[must_use]
    pub fn discovered_peers(&self) -> Vec<DiscoveredPeerItem> {
        self.discovery
            .active_peers()
            .into_iter()
            .map(|p| DiscoveredPeerItem {
                instance_name: p.instance_name,
                platform: p.platform.as_str().to_string(),
                port: p.port,
                addresses: p.addresses.into_iter().map(|ip| ip.to_string()).collect(),
                fingerprint: format_fingerprint(&p.fingerprint),
                last_seen_ms: p.last_seen_ms,
            })
            .collect()
    }

    /// Injects or simulates a discovered peer (useful for loopback or UI testing).
    pub fn simulate_discovered_peer(&self, item: DiscoveredPeerItem) -> FfiResult<()> {
        let platform = PlatformTarget::from_str(&item.platform)
            .map_err(|e| FfiError::InvalidParameter(e.to_string()))?;

        let mut fp = [0u8; 8];
        if item.fingerprint.len() == 16 {
            for (i, slot) in fp.iter_mut().enumerate() {
                if let Ok(b) = u8::from_str_radix(&item.fingerprint[i * 2..i * 2 + 2], 16) {
                    *slot = b;
                }
            }
        }

        let addresses = item
            .addresses
            .into_iter()
            .filter_map(|s| s.parse::<IpAddr>().ok())
            .collect();

        let peer = DiscoveredPeer {
            instance_name: item.instance_name.clone(),
            platform,
            port: item.port,
            addresses,
            fingerprint: fp,
            protocol_version: 1,
            last_seen_ms: item.last_seen_ms,
            txt_records: std::collections::HashMap::new(),
        };

        self.discovery.simulate_peer_announcement(peer);

        self.event_sink.broadcast(BridgeEvent::DeviceDiscovered {
            instance_name: item.instance_name,
            platform: item.platform,
            endpoint: Some(format!("127.0.0.1:{}", item.port)),
            fingerprint: item.fingerprint,
        });

        Ok(())
    }

    /// Returns the list of paired devices and their assigned capability permissions.
    pub fn paired_devices(&self) -> FfiResult<Vec<PairedDeviceItem>> {
        let devices = self.registry.list_devices()?;
        let mut items = Vec::with_capacity(devices.len());

        for dev in devices {
            let perms = self.registry.get_peer_permissions(&dev.device_id)?;
            let perm_items = perms
                .into_iter()
                .map(|(cap, lvl)| PeerPermissionItem {
                    capability: cap,
                    permission_level: lvl.to_string(),
                })
                .collect();

            let mut fp = [0u8; 8];
            fp.copy_from_slice(&dev.ed25519_public_key[..8]);

            items.push(PairedDeviceItem {
                device_id: dev.device_id.to_string(),
                name: dev.name,
                platform: dev.platform.to_string(),
                fingerprint: format_fingerprint(&fp),
                trust_state: dev.trust_state.to_string(),
                last_endpoint: dev.last_endpoint,
                last_seen_ms: dev.last_seen_ms,
                paired_at_ms: dev.paired_at_ms,
                permissions: perm_items,
            });
        }

        Ok(items)
    }

    /// Initiates a pairing ceremony with the specified discovered peer.
    pub fn start_pairing(&self, peer_instance: &str) -> FfiResult<PairingSessionInfo> {
        let peer = self
            .discovery
            .active_peers()
            .into_iter()
            .find(|p| p.instance_name == peer_instance)
            .ok_or_else(|| {
                FfiError::Core(aissistant_core::CoreError::NotFound {
                    entity: "DiscoveredPeer",
                    id: peer_instance.to_string(),
                })
            })?;

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut mgr = self
            .pairing_manager
            .lock()
            .map_err(|_| FfiError::LockPoisoned("Pairing manager lock poisoned".to_string()))?;

        let info = mgr
            .start_pairing(&self.identity, peer, now_secs)
            .map_err(|e| FfiError::InvalidParameter(e.to_string()))?;

        self.event_sink.broadcast(BridgeEvent::PairingStateChanged {
            peer_instance: info.peer_instance.clone(),
            status: "started".to_string(),
            sas_code: Some(info.sas_code.clone()),
            qr_payload: Some(info.qr_payload.clone()),
            error: None,
        });

        Ok(info)
    }

    /// Confirms the 6-digit SAS code, registers the device as Trusted, and grants baseline permissions.
    pub fn confirm_pairing(&self, entered_code: &str) -> FfiResult<PairedDeviceItem> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut mgr = self
            .pairing_manager
            .lock()
            .map_err(|_| FfiError::LockPoisoned("Pairing manager lock poisoned".to_string()))?;

        let paired = match mgr.confirm_sas(entered_code, &self.registry, now_secs) {
            Ok(d) => d,
            Err(e) => {
                self.event_sink.broadcast(BridgeEvent::PairingStateChanged {
                    peer_instance: String::new(),
                    status: "failed".to_string(),
                    sas_code: None,
                    qr_payload: None,
                    error: Some(e.to_string()),
                });
                return Err(FfiError::InvalidParameter(e.to_string()));
            }
        };

        // Assign default baseline capability permissions: LLM/LAN allowed, File/App AskEachTime
        let default_rules = [
            ("local_llm", PeerPermissionLevel::AlwaysAllow),
            ("file_system", PeerPermissionLevel::AskEachTime),
            ("application_management", PeerPermissionLevel::AskEachTime),
            ("lan_peer_networking", PeerPermissionLevel::AlwaysAllow),
        ];

        for (cap, lvl) in default_rules {
            let _ = self
                .registry
                .set_peer_permission(&paired.device_id, cap, lvl, now_secs * 1000);
        }

        self.event_sink.broadcast(BridgeEvent::PairingStateChanged {
            peer_instance: paired.name.clone(),
            status: "confirmed".to_string(),
            sas_code: None,
            qr_payload: None,
            error: None,
        });

        let mut fp = [0u8; 8];
        fp.copy_from_slice(&paired.ed25519_public_key[..8]);

        let perms = self
            .registry
            .get_peer_permissions(&paired.device_id)?
            .into_iter()
            .map(|(cap, lvl)| PeerPermissionItem {
                capability: cap,
                permission_level: lvl.to_string(),
            })
            .collect();

        Ok(PairedDeviceItem {
            device_id: paired.device_id.to_string(),
            name: paired.name,
            platform: paired.platform.to_string(),
            fingerprint: format_fingerprint(&fp),
            trust_state: paired.trust_state.to_string(),
            last_endpoint: paired.last_endpoint,
            last_seen_ms: paired.last_seen_ms,
            paired_at_ms: paired.paired_at_ms,
            permissions: perms,
        })
    }

    /// Cancels any active pairing ceremony.
    pub fn cancel_pairing(&self) -> FfiResult<()> {
        let mut mgr = self
            .pairing_manager
            .lock()
            .map_err(|_| FfiError::LockPoisoned("Pairing manager lock poisoned".to_string()))?;
        mgr.cancel();

        self.event_sink.broadcast(BridgeEvent::PairingStateChanged {
            peer_instance: String::new(),
            status: "cancelled".to_string(),
            sas_code: None,
            qr_payload: None,
            error: None,
        });

        Ok(())
    }

    /// Sets or updates a capability permission level for a paired device.
    pub fn set_peer_permission(
        &self,
        device_id_str: &str,
        capability_str: &str,
        level_str: &str,
    ) -> FfiResult<()> {
        let device_id =
            DeviceId::new(device_id_str).map_err(|e| FfiError::InvalidParameter(e.to_string()))?;
        let permission_level = PeerPermissionLevel::from_str(level_str)
            .map_err(|e| FfiError::InvalidParameter(e.to_string()))?;

        let now_ms = current_time_ms();

        self.registry
            .set_peer_permission(&device_id, capability_str, permission_level, now_ms)?;
        Ok(())
    }

    /// Unpairs a device, revoking trust and purging capability grants.
    pub fn unpair_device(&self, device_id_str: &str) -> FfiResult<()> {
        let device_id =
            DeviceId::new(device_id_str).map_err(|e| FfiError::InvalidParameter(e.to_string()))?;
        self.registry.delete_device(&device_id)?;
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_device_supervisor_pairing_and_acl_lifecycle() {
        let db = SqliteDb::open_in_memory().unwrap();
        let identity = Arc::new(DeviceIdentity::generate());
        let event_sink = Arc::new(EventStreamSink::new());

        let supervisor = DeviceSupervisor::new(identity, db, event_sink);

        // Discovered peer simulation
        let peer_item = DiscoveredPeerItem {
            instance_name: "test-laptop".to_string(),
            platform: "windows".to_string(),
            port: 51820,
            addresses: vec!["192.168.1.55".to_string()],
            fingerprint: "1122334455667788".to_string(),
            last_seen_ms: 1000,
        };
        supervisor.simulate_discovered_peer(peer_item).unwrap();

        let peers = supervisor.discovered_peers();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].instance_name, "test-laptop");

        // Start pairing
        let info = supervisor.start_pairing("test-laptop").unwrap();
        assert_eq!(info.peer_instance, "test-laptop");
        assert_eq!(info.sas_code.len(), 7);

        // Confirm pairing
        let paired = supervisor.confirm_pairing(&info.sas_code).unwrap();
        assert_eq!(paired.name, "test-laptop");
        assert_eq!(paired.trust_state, "trusted");
        assert!(!paired.permissions.is_empty());

        // Update ACL permission
        supervisor
            .set_peer_permission(&paired.device_id, "file_system", "deny")
            .unwrap();

        let paired_list = supervisor.paired_devices().unwrap();
        assert_eq!(paired_list.len(), 1);
        let fs_perm = paired_list[0]
            .permissions
            .iter()
            .find(|p| p.capability == "file_system")
            .unwrap();
        assert_eq!(fs_perm.permission_level, "deny");

        // Unpair device
        supervisor.unpair_device(&paired.device_id).unwrap();
        assert!(supervisor.paired_devices().unwrap().is_empty());
    }
}
