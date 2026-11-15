//! Out-of-band SAS (Short Authentication String) and QR code device pairing ceremony engine.

use std::net::IpAddr;

use aissistant_core::{DeviceId, PlatformTarget};
use aissistant_security::DeviceIdentity;
use aissistant_storage::DeviceRegistry;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{NetworkError, Result};
use crate::types::DiscoveredPeer;

/// Number of failed verification attempts before lockout.
pub const MAX_FAILED_PAIRING_ATTEMPTS: u32 = 3;

/// Lockout duration in seconds following repeated failures (5 minutes).
pub const PAIRING_LOCKOUT_DURATION_SECS: u64 = 300;

/// Derives a deterministic, symmetrical 6-digit decimal SAS code (e.g. `481-923`).
///
/// Both pairing devices will compute the exact same string regardless of which device
/// initiated the ceremony, because public keys are canonicalized before hashing.
#[must_use]
pub fn derive_sas_code(key_a: &[u8; 32], key_b: &[u8; 32], session_salt: &[u8; 16]) -> String {
    let (first, second) = if key_a <= key_b {
        (key_a, key_b)
    } else {
        (key_b, key_a)
    };

    let mut hasher = Sha256::new();
    hasher.update(b"aissistant-pairing-sas-v1\0");
    hasher.update(first);
    hasher.update(second);
    hasher.update(session_salt);
    let digest = hasher.finalize();

    let num = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
    format!("{:03}-{:03}", num / 1000, num % 1000)
}

/// Compact QR code exchange payload for out-of-band device discovery and pairing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingQrPayload {
    /// Instance identifier of the device advertising the QR code.
    pub instance_name: String,
    /// Operating system platform.
    pub platform: PlatformTarget,
    /// QUIC transport port.
    pub port: u16,
    /// Reachable local IP addresses.
    pub addresses: Vec<IpAddr>,
    /// Truncated public key fingerprint.
    pub fingerprint: [u8; 8],
    /// Ephemeral random session salt.
    pub session_salt: [u8; 16],
    /// Full 32-byte Ed25519 verifying public key.
    pub public_key: [u8; 32],
}

impl PairingQrPayload {
    /// Serializes this payload into a compact JSON string suitable for QR code encoding.
    pub fn to_qr_string(&self) -> Result<String> {
        serde_json::to_string(self)
            .map_err(|e| NetworkError::Internal(format!("Failed to serialize QR payload: {e}")))
    }

    /// Deserializes a pairing payload from a scanned QR code string.
    pub fn from_qr_string(qr_str: &str) -> Result<Self> {
        serde_json::from_str(qr_str)
            .map_err(|e| NetworkError::Internal(format!("Invalid QR payload JSON: {e}")))
    }
}

/// Brute-force rate limiter defending against LAN spoofing and automated SAS guessing.
#[derive(Debug, Clone)]
pub struct PairingRateLimiter {
    max_failures: u32,
    lockout_duration_secs: u64,
    failure_count: u32,
    locked_until_secs: u64,
}

impl Default for PairingRateLimiter {
    fn default() -> Self {
        Self {
            max_failures: MAX_FAILED_PAIRING_ATTEMPTS,
            lockout_duration_secs: PAIRING_LOCKOUT_DURATION_SECS,
            failure_count: 0,
            locked_until_secs: 0,
        }
    }
}

impl PairingRateLimiter {
    /// Creates a rate limiter with custom threshold and duration.
    #[must_use]
    pub const fn new(max_failures: u32, lockout_duration_secs: u64) -> Self {
        Self {
            max_failures,
            lockout_duration_secs,
            failure_count: 0,
            locked_until_secs: 0,
        }
    }

    /// Verifies if pairing operations are currently allowed.
    pub fn check_allowed(&self, now_secs: u64) -> Result<()> {
        if now_secs < self.locked_until_secs {
            let remaining_secs = self.locked_until_secs.saturating_sub(now_secs);
            return Err(NetworkError::RateLimited { remaining_secs });
        }
        Ok(())
    }

    /// Records a failed SAS verification attempt, potentially triggering lockout.
    pub fn record_failure(&mut self, now_secs: u64) {
        self.failure_count = self.failure_count.saturating_add(1);
        if self.failure_count >= self.max_failures {
            self.locked_until_secs = now_secs.saturating_add(self.lockout_duration_secs);
        }
    }

    /// Resets the failure counter upon successful verification.
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.locked_until_secs = 0;
    }

    /// Returns remaining lockout time in seconds, or 0 if active.
    #[must_use]
    pub fn remaining_lockout_secs(&self, now_secs: u64) -> u64 {
        self.locked_until_secs.saturating_sub(now_secs)
    }

    /// Returns the current failure count within this window.
    #[must_use]
    pub const fn failure_count(&self) -> u32 {
        self.failure_count
    }
}

/// Metadata and SAS details for an active pairing ceremony.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingSessionInfo {
    /// Unique ephemeral session token.
    pub session_id: String,
    /// Instance identifier of the target peer.
    pub peer_instance: String,
    /// Platform of the peer.
    pub peer_platform: PlatformTarget,
    /// Derived 6-digit decimal Short Authentication String.
    pub sas_code: String,
    /// Scannable QR payload string.
    pub qr_payload: String,
    /// Epoch timestamp when the session was initiated.
    pub created_at_secs: u64,
}

/// Manages pairing session state, SAS verification, and registry enrollment.
#[derive(Debug, Default)]
pub struct PairingSessionManager {
    rate_limiter: PairingRateLimiter,
    active_session: Option<PairingSessionInfo>,
    target_peer_key: Option<[u8; 32]>,
    target_peer_info: Option<DiscoveredPeer>,
}

impl PairingSessionManager {
    /// Creates a new pairing session manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Initiates a pairing ceremony with a peer discovered over mDNS or direct input.
    pub fn start_pairing(
        &mut self,
        local_identity: &DeviceIdentity,
        peer: DiscoveredPeer,
        now_secs: u64,
    ) -> Result<PairingSessionInfo> {
        self.rate_limiter.check_allowed(now_secs)?;

        let mut salt = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut salt);

        // Derive symmetrical SAS code
        let local_key = local_identity.verifying_key_bytes();
        let peer_key = if peer.txt_records.contains_key("ed25519_pk") {
            let mut k = [0u8; 32];
            if let Some(hex_pk) = peer.txt_records.get("ed25519_pk") {
                if hex_pk.len() == 64 {
                    for (i, slot) in k.iter_mut().enumerate() {
                        if let Ok(b) = u8::from_str_radix(&hex_pk[i * 2..i * 2 + 2], 16) {
                            *slot = b;
                        }
                    }
                }
            }
            k
        } else {
            // Expand truncated 8-byte fingerprint into 32-byte key buffer for pairing
            let mut k = [0u8; 32];
            k[..8].copy_from_slice(&peer.fingerprint);
            k
        };

        let sas_code = derive_sas_code(&local_key, &peer_key, &salt);

        let qr_payload = PairingQrPayload {
            instance_name: peer.instance_name.clone(),
            platform: peer.platform,
            port: peer.port,
            addresses: peer.addresses.clone(),
            fingerprint: peer.fingerprint,
            session_salt: salt,
            public_key: peer_key,
        };

        let session_info = PairingSessionInfo {
            session_id: format!("pair_{now_secs}_{}", peer.instance_name),
            peer_instance: peer.instance_name.clone(),
            peer_platform: peer.platform,
            sas_code,
            qr_payload: qr_payload.to_qr_string()?,
            created_at_secs: now_secs,
        };

        self.active_session = Some(session_info.clone());
        self.target_peer_key = Some(peer_key);
        self.target_peer_info = Some(peer);

        Ok(session_info)
    }

    /// Verifies the 6-digit SAS code and registers the peer into the local SQLite DeviceRegistry.
    pub fn confirm_sas(
        &mut self,
        entered_code: &str,
        registry: &DeviceRegistry,
        now_secs: u64,
    ) -> Result<aissistant_core::PairedDevice> {
        self.rate_limiter.check_allowed(now_secs)?;

        let session = match &self.active_session {
            Some(s) => s,
            None => {
                return Err(NetworkError::Internal(
                    "No active pairing ceremony to confirm".to_string(),
                ))
            }
        };

        // Normalize code by stripping dashes and whitespace
        let norm_entered = entered_code.replace(['-', ' '], "");
        let norm_expected = session.sas_code.replace(['-', ' '], "");

        if norm_entered != norm_expected {
            self.rate_limiter.record_failure(now_secs);
            return Err(NetworkError::InvalidSasCode);
        }

        self.rate_limiter.record_success();

        let peer = self
            .target_peer_info
            .as_ref()
            .ok_or_else(|| NetworkError::Internal("Missing target peer metadata".to_string()))?;
        let peer_key = self
            .target_peer_key
            .ok_or_else(|| NetworkError::Internal("Missing target peer key".to_string()))?;

        let hex_prefix: String = peer_key
            .iter()
            .take(16)
            .map(|b| format!("{b:02x}"))
            .collect();

        let device_id = DeviceId::new(format!("dev_{hex_prefix}"))
            .map_err(|e| NetworkError::Internal(format!("Failed to derive DeviceId: {e}")))?;

        let paired_device = aissistant_core::PairedDevice {
            device_id,
            name: peer.instance_name.clone(),
            platform: peer.platform,
            ed25519_public_key: peer_key,
            paired_at_ms: now_secs * 1000,
            last_seen_ms: now_secs * 1000,
            last_endpoint: peer
                .addresses
                .first()
                .map(|ip| format!("{ip}:{}", peer.port)),
            trust_state: aissistant_core::DeviceTrustState::Trusted,
        };

        registry
            .register_device(&paired_device)
            .map_err(|e| NetworkError::Internal(format!("Failed to persist paired device: {e}")))?;

        self.active_session = None;
        self.target_peer_key = None;
        self.target_peer_info = None;

        Ok(paired_device)
    }

    /// Cancels any active pairing ceremony.
    pub fn cancel(&mut self) {
        self.active_session = None;
        self.target_peer_key = None;
        self.target_peer_info = None;
    }

    /// Returns the active pairing session info, if any.
    #[must_use]
    pub fn active_session(&self) -> Option<&PairingSessionInfo> {
        self.active_session.as_ref()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use aissistant_storage::SqliteDb;
    use std::collections::HashMap;
    use std::net::Ipv4Addr;

    #[test]
    fn test_sas_derivation_is_strictly_symmetrical() {
        let key_a = [0x11; 32];
        let key_b = [0x22; 32];
        let salt = [0x77; 16];

        let sas_ab = derive_sas_code(&key_a, &key_b, &salt);
        let sas_ba = derive_sas_code(&key_b, &key_a, &salt);

        assert_eq!(sas_ab, sas_ba);
        assert_eq!(sas_ab.len(), 7); // XXX-XXX format
        assert!(sas_ab.contains('-'));
    }

    #[test]
    fn test_qr_payload_serde_roundtrip() {
        let payload = PairingQrPayload {
            instance_name: "phone-device".to_string(),
            platform: PlatformTarget::Android,
            port: 51820,
            addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 15))],
            fingerprint: [1, 2, 3, 4, 5, 6, 7, 8],
            session_salt: [9; 16],
            public_key: [0xaa; 32],
        };

        let qr_string = payload.to_qr_string().unwrap();
        let decoded = PairingQrPayload::from_qr_string(&qr_string).unwrap();

        assert_eq!(decoded, payload);
    }

    #[test]
    fn test_rate_limiter_locks_after_max_failures() {
        let mut limiter = PairingRateLimiter::new(3, 300);
        let now = 1000;

        assert!(limiter.check_allowed(now).is_ok());

        limiter.record_failure(now);
        assert!(limiter.check_allowed(now).is_ok());
        assert_eq!(limiter.failure_count(), 1);

        limiter.record_failure(now);
        assert!(limiter.check_allowed(now).is_ok());

        limiter.record_failure(now); // 3rd failure -> lock out
        assert!(limiter.check_allowed(now).is_err());
        assert_eq!(limiter.remaining_lockout_secs(now), 300);

        // Before lockout expires
        assert!(limiter.check_allowed(now + 299).is_err());

        // After lockout expires
        assert!(limiter.check_allowed(now + 301).is_ok());

        // Success resets counter
        limiter.record_success();
        assert_eq!(limiter.failure_count(), 0);
    }

    #[test]
    fn test_pairing_session_lifecycle_and_registry_persistence() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = DeviceRegistry::new(db);

        let local_identity = DeviceIdentity::generate();
        let mut manager = PairingSessionManager::new();

        let peer = DiscoveredPeer {
            instance_name: "desktop-peer".to_string(),
            platform: PlatformTarget::Windows,
            port: 51820,
            addresses: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))],
            fingerprint: [0x33; 8],
            protocol_version: 1,
            last_seen_ms: 5000,
            txt_records: HashMap::new(),
        };

        let session = manager.start_pairing(&local_identity, peer, 1000).unwrap();
        assert_eq!(session.peer_instance, "desktop-peer");
        assert_eq!(session.sas_code.len(), 7);

        // Wrong code fails and increments failure count
        let err = manager.confirm_sas("000-000", &registry, 1001).unwrap_err();
        assert!(matches!(err, NetworkError::InvalidSasCode));

        // Correct code succeeds and registers device
        let paired = manager
            .confirm_sas(&session.sas_code, &registry, 1002)
            .unwrap();
        assert_eq!(paired.name, "desktop-peer");
        assert_eq!(
            paired.trust_state,
            aissistant_core::DeviceTrustState::Trusted
        );

        // Verified in registry
        let fetched = registry.get_device(&paired.device_id).unwrap().unwrap();
        assert_eq!(fetched.name, "desktop-peer");
    }
}
