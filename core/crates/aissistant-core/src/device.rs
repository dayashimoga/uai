//! Domain types for cross-device peering, trust states, and granular peer capability ACLs.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::capability::PlatformTarget;
use crate::error::CoreError;
use crate::ids::DeviceId;

/// Cryptographic trust state of a discovered or paired remote peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceTrustState {
    /// Fully paired and trusted device.
    Trusted,
    /// Temporarily blocked peer (cannot execute remote skills or sync data).
    Blocked,
    /// Discovery / handshake initiated; awaiting out-of-band SAS confirmation.
    PendingVerification,
    /// Permanently revoked peer identity; connection attempts are rejected.
    Revoked,
}

impl fmt::Display for DeviceTrustState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trusted => write!(f, "trusted"),
            Self::Blocked => write!(f, "blocked"),
            Self::PendingVerification => write!(f, "pending_verification"),
            Self::Revoked => write!(f, "revoked"),
        }
    }
}

impl FromStr for DeviceTrustState {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "trusted" => Ok(Self::Trusted),
            "blocked" => Ok(Self::Blocked),
            "pending_verification" => Ok(Self::PendingVerification),
            "revoked" => Ok(Self::Revoked),
            other => Err(CoreError::ConfigValidation {
                field: "trust_state".to_string(),
                reason: format!("Unknown device trust state: '{other}'"),
            }),
        }
    }
}

/// Access control level granted to a paired peer for a specific capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerPermissionLevel {
    /// Capability is allowed automatically without interactive confirmation.
    AlwaysAllow,
    /// Capability execution prompts the local user for confirmation each time.
    AskEachTime,
    /// Capability execution from this peer is rejected unconditionally.
    Deny,
}

impl fmt::Display for PeerPermissionLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlwaysAllow => write!(f, "always_allow"),
            Self::AskEachTime => write!(f, "ask_each_time"),
            Self::Deny => write!(f, "deny"),
        }
    }
}

impl FromStr for PeerPermissionLevel {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "always_allow" => Ok(Self::AlwaysAllow),
            "ask_each_time" => Ok(Self::AskEachTime),
            "deny" => Ok(Self::Deny),
            other => Err(CoreError::ConfigValidation {
                field: "permission_level".to_string(),
                reason: format!("Unknown peer permission level: '{other}'"),
            }),
        }
    }
}

/// Persistent record of a paired remote device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairedDevice {
    /// Unique cryptographic device identifier.
    pub device_id: DeviceId,
    /// User-friendly label (e.g. "Work Laptop", "Pixel 9 Pro").
    pub name: String,
    /// Operating system platform of the peer device.
    pub platform: PlatformTarget,
    /// Raw 32-byte Ed25519 public identity key.
    pub ed25519_public_key: [u8; 32],
    /// Unix timestamp in milliseconds when pairing was completed.
    pub paired_at_ms: u64,
    /// Unix timestamp in milliseconds of most recent observed activity or heartbeat.
    pub last_seen_ms: u64,
    /// Most recent observed network endpoint (e.g. "192.168.1.100:51820").
    pub last_endpoint: Option<String>,
    /// Trust status of the peer.
    pub trust_state: DeviceTrustState,
}

/// Access control rule binding a remote peer to a specific capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerPermission {
    /// Device identifier receiving this grant.
    pub device_id: DeviceId,
    /// Capability identifier (e.g. "remote.clipboard", "remote.file_transfer").
    pub capability_id: String,
    /// Permission grant level.
    pub permission_level: PeerPermissionLevel,
    /// Unix timestamp in milliseconds when the rule was granted or updated.
    pub granted_at_ms: u64,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_device_trust_state_roundtrips() {
        let states = [
            DeviceTrustState::Trusted,
            DeviceTrustState::Blocked,
            DeviceTrustState::PendingVerification,
            DeviceTrustState::Revoked,
        ];

        for state in states {
            let str_repr = state.to_string();
            let parsed: DeviceTrustState = str_repr.parse().unwrap();
            assert_eq!(parsed, state);

            let json = serde_json::to_string(&state).unwrap();
            let deserialized: DeviceTrustState = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, state);
        }

        assert!("unknown".parse::<DeviceTrustState>().is_err());
    }

    #[test]
    fn test_peer_permission_level_roundtrips() {
        let levels = [
            PeerPermissionLevel::AlwaysAllow,
            PeerPermissionLevel::AskEachTime,
            PeerPermissionLevel::Deny,
        ];

        for level in levels {
            let str_repr = level.to_string();
            let parsed: PeerPermissionLevel = str_repr.parse().unwrap();
            assert_eq!(parsed, level);

            let json = serde_json::to_string(&level).unwrap();
            let deserialized: PeerPermissionLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, level);
        }

        assert!("invalid".parse::<PeerPermissionLevel>().is_err());
    }

    #[test]
    fn test_paired_device_serde() {
        let device = PairedDevice {
            device_id: DeviceId::generate(),
            name: "Living Room Tablet".to_string(),
            platform: PlatformTarget::Android,
            ed25519_public_key: [7u8; 32],
            paired_at_ms: 1700000000000,
            last_seen_ms: 1700000005000,
            last_endpoint: Some("192.168.1.150:51820".to_string()),
            trust_state: DeviceTrustState::Trusted,
        };

        let json = serde_json::to_string(&device).unwrap();
        let decoded: PairedDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, device);
    }
}
