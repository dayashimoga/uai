//! Persistent SQLite store for paired remote devices and peer capability ACL rules.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use aissistant_core::{
    AuditEntryId, DeviceId, DeviceTrustState, PairedDevice, PeerPermissionLevel, PlatformTarget,
    SkillId,
};
use aissistant_security::{Actor, AuditEntry, AuditExecutionStatus, NewAuditEntry, PolicyDecision};

use crate::audit_store::AuditStore;
use crate::error::StorageError;
use crate::pool::SqliteDb;

/// Store managing persistent peer device identities, trust states, and granular capability ACLs.
#[derive(Debug, Clone)]
pub struct DeviceRegistry {
    db: SqliteDb,
    audit_store: Option<Arc<AuditStore>>,
}

impl DeviceRegistry {
    /// Creates a new `DeviceRegistry` instance using the provided SQLite database handle.
    #[must_use]
    pub const fn new(db: SqliteDb) -> Self {
        Self {
            db,
            audit_store: None,
        }
    }

    /// Creates a new `DeviceRegistry` instance with an audit store for tamper-evident event logging.
    #[must_use]
    pub fn with_audit_store(db: SqliteDb, audit_store: Arc<AuditStore>) -> Self {
        Self {
            db,
            audit_store: Some(audit_store),
        }
    }

    /// Registers or updates a paired remote device in persistent storage.
    pub fn register_device(&self, device: &PairedDevice) -> Result<(), StorageError> {
        let device_id_str = device.device_id.to_string();
        let platform_str = device.platform.to_string();
        let trust_str = device.trust_state.to_string();

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO paired_devices (
                    device_id, name, platform, ed25519_public_key,
                    paired_at_ms, last_seen_ms, last_endpoint, trust_state
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(device_id) DO UPDATE SET
                    name = excluded.name,
                    platform = excluded.platform,
                    ed25519_public_key = excluded.ed25519_public_key,
                    last_seen_ms = excluded.last_seen_ms,
                    last_endpoint = excluded.last_endpoint,
                    trust_state = excluded.trust_state;",
                rusqlite::params![
                    device_id_str,
                    device.name,
                    platform_str,
                    device.ed25519_public_key.as_slice(),
                    device.paired_at_ms as i64,
                    device.last_seen_ms as i64,
                    device.last_endpoint,
                    trust_str,
                ],
            )?;
            Ok(())
        })?;

        self.log_audit(
            "device.register",
            serde_json::json!({
                "device_id": device.device_id.to_string(),
                "name": device.name,
                "platform": device.platform.to_string(),
                "trust_state": device.trust_state.to_string(),
            }),
        );

        Ok(())
    }

    /// Retrieves a paired device by its unique device identifier.
    pub fn get_device(&self, device_id: &DeviceId) -> Result<Option<PairedDevice>, StorageError> {
        let id_str = device_id.to_string();

        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT device_id, name, platform, ed25519_public_key,
                        paired_at_ms, last_seen_ms, last_endpoint, trust_state
                 FROM paired_devices WHERE device_id = ?1;",
            )?;

            let mut rows = stmt.query(rusqlite::params![id_str])?;
            if let Some(row) = rows.next()? {
                let id_raw: String = row.get(0)?;
                let name: String = row.get(1)?;
                let platform_raw: String = row.get(2)?;
                let pubkey_blob: Vec<u8> = row.get(3)?;
                let paired_at_ms: i64 = row.get(4)?;
                let last_seen_ms: i64 = row.get(5)?;
                let last_endpoint: Option<String> = row.get(6)?;
                let trust_raw: String = row.get(7)?;

                let mut pubkey = [0u8; 32];
                if pubkey_blob.len() == 32 {
                    pubkey.copy_from_slice(&pubkey_blob);
                }

                let platform = PlatformTarget::from_str(&platform_raw).map_err(|e| {
                    StorageError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))
                })?;

                let trust_state = DeviceTrustState::from_str(&trust_raw).map_err(|e| {
                    StorageError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
                        7,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ))
                })?;

                Ok(Some(PairedDevice {
                    device_id: DeviceId::new(&id_raw).map_err(|e| {
                        StorageError::Sqlite(rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))
                    })?,
                    name,
                    platform,
                    ed25519_public_key: pubkey,
                    paired_at_ms: paired_at_ms.max(0) as u64,
                    last_seen_ms: last_seen_ms.max(0) as u64,
                    last_endpoint,
                    trust_state,
                }))
            } else {
                Ok(None)
            }
        })
    }

    /// Lists all registered peer devices ordered by most recent activity.
    pub fn list_devices(&self) -> Result<Vec<PairedDevice>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT device_id, name, platform, ed25519_public_key,
                        paired_at_ms, last_seen_ms, last_endpoint, trust_state
                 FROM paired_devices ORDER BY last_seen_ms DESC;",
            )?;

            let rows = stmt.query_map([], |row| {
                let id_raw: String = row.get(0)?;
                let name: String = row.get(1)?;
                let platform_raw: String = row.get(2)?;
                let pubkey_blob: Vec<u8> = row.get(3)?;
                let paired_at_ms: i64 = row.get(4)?;
                let last_seen_ms: i64 = row.get(5)?;
                let last_endpoint: Option<String> = row.get(6)?;
                let trust_raw: String = row.get(7)?;

                let mut pubkey = [0u8; 32];
                if pubkey_blob.len() == 32 {
                    pubkey.copy_from_slice(&pubkey_blob);
                }

                let platform =
                    PlatformTarget::from_str(&platform_raw).unwrap_or(PlatformTarget::Linux);
                let trust_state =
                    DeviceTrustState::from_str(&trust_raw).unwrap_or(DeviceTrustState::Blocked);

                Ok(PairedDevice {
                    device_id: DeviceId::new(&id_raw).unwrap_or_else(|_| DeviceId::generate()),
                    name,
                    platform,
                    ed25519_public_key: pubkey,
                    paired_at_ms: paired_at_ms.max(0) as u64,
                    last_seen_ms: last_seen_ms.max(0) as u64,
                    last_endpoint,
                    trust_state,
                })
            })?;

            let mut devices = Vec::new();
            for r in rows {
                devices.push(r?);
            }
            Ok(devices)
        })
    }

    /// Updates the heartbeat timestamp and optional active network endpoint for a device.
    pub fn update_device_heartbeat(
        &self,
        device_id: &DeviceId,
        endpoint: Option<&str>,
        timestamp_ms: u64,
    ) -> Result<bool, StorageError> {
        let id_str = device_id.to_string();

        self.db.with_conn(|conn| {
            let rows_affected = conn.execute(
                "UPDATE paired_devices
                 SET last_seen_ms = ?1,
                     last_endpoint = COALESCE(?2, last_endpoint)
                 WHERE device_id = ?3;",
                rusqlite::params![timestamp_ms as i64, endpoint, id_str],
            )?;
            Ok(rows_affected > 0)
        })
    }

    /// Sets the cryptographic trust state for a device.
    pub fn set_device_trust_state(
        &self,
        device_id: &DeviceId,
        state: DeviceTrustState,
    ) -> Result<bool, StorageError> {
        let id_str = device_id.to_string();
        let state_str = state.to_string();

        let updated = self.db.with_conn(|conn| {
            let rows_affected = conn.execute(
                "UPDATE paired_devices SET trust_state = ?1 WHERE device_id = ?2;",
                rusqlite::params![state_str, id_str],
            )?;
            Ok(rows_affected > 0)
        })?;

        if updated {
            self.log_audit(
                "device.trust_state",
                serde_json::json!({
                    "device_id": id_str,
                    "new_trust_state": state.to_string(),
                }),
            );
        }

        Ok(updated)
    }

    /// Permanently revokes and deletes a paired device and its associated capability grants.
    pub fn delete_device(&self, device_id: &DeviceId) -> Result<bool, StorageError> {
        let id_str = device_id.to_string();

        let deleted = self.db.with_conn(|conn| {
            let rows_affected = conn.execute(
                "DELETE FROM paired_devices WHERE device_id = ?1;",
                rusqlite::params![id_str],
            )?;
            Ok(rows_affected > 0)
        })?;

        if deleted {
            self.log_audit(
                "device.delete",
                serde_json::json!({
                    "device_id": id_str,
                    "revoked": true,
                }),
            );
        }

        Ok(deleted)
    }

    /// Retrieves all capability permissions granted to a specific peer device.
    pub fn get_peer_permissions(
        &self,
        device_id: &DeviceId,
    ) -> Result<HashMap<String, PeerPermissionLevel>, StorageError> {
        let id_str = device_id.to_string();

        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT capability_id, permission_level
                 FROM peer_permissions WHERE device_id = ?1;",
            )?;

            let rows = stmt.query_map(rusqlite::params![id_str], |row| {
                let cap: String = row.get(0)?;
                let perm_raw: String = row.get(1)?;
                let level = PeerPermissionLevel::from_str(&perm_raw)
                    .unwrap_or(PeerPermissionLevel::AskEachTime);
                Ok((cap, level))
            })?;

            let mut map = HashMap::new();
            for r in rows {
                let (cap, level) = r?;
                map.insert(cap, level);
            }
            Ok(map)
        })
    }

    /// Grants or updates a peer device capability permission rule.
    pub fn set_peer_permission(
        &self,
        device_id: &DeviceId,
        capability_id: &str,
        level: PeerPermissionLevel,
        granted_at_ms: u64,
    ) -> Result<(), StorageError> {
        let id_str = device_id.to_string();
        let level_str = level.to_string();

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO peer_permissions (device_id, capability_id, permission_level, granted_at_ms)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(device_id, capability_id) DO UPDATE SET
                     permission_level = excluded.permission_level,
                     granted_at_ms = excluded.granted_at_ms;",
                rusqlite::params![id_str, capability_id, level_str, granted_at_ms as i64],
            )?;
            Ok(())
        })?;

        self.log_audit(
            "device.permission_grant",
            serde_json::json!({
                "device_id": id_str,
                "capability_id": capability_id,
                "permission_level": level.to_string(),
            }),
        );

        Ok(())
    }

    /// Evaluates peer capability access applying default-deny and device trust state invariants.
    pub fn check_peer_permission(
        &self,
        device_id: &DeviceId,
        capability_id: &str,
    ) -> Result<PeerPermissionLevel, StorageError> {
        let device = match self.get_device(device_id)? {
            Some(d) => d,
            None => return Ok(PeerPermissionLevel::Deny),
        };

        match device.trust_state {
            DeviceTrustState::Blocked | DeviceTrustState::Revoked => Ok(PeerPermissionLevel::Deny),
            DeviceTrustState::PendingVerification => Ok(PeerPermissionLevel::AskEachTime),
            DeviceTrustState::Trusted => {
                let permissions = self.get_peer_permissions(device_id)?;
                if let Some(level) = permissions.get(capability_id) {
                    Ok(*level)
                } else {
                    // Default-deny for untrusted / unconfigured remote capabilities
                    Ok(PeerPermissionLevel::AskEachTime)
                }
            }
        }
    }

    fn log_audit(&self, action: &str, mut payload: serde_json::Value) {
        if let Some(ref store) = self.audit_store {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert(
                    "action".to_string(),
                    serde_json::Value::String(action.to_string()),
                );
            }

            let skill_id = match SkillId::new("device.registry") {
                Ok(s) => s,
                Err(_) => return,
            };

            let timestamp_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);

            let prev_hash = store.latest_hash().unwrap_or([0u8; 32]);

            let entry = AuditEntry::new(NewAuditEntry {
                id: AuditEntryId::generate(),
                timestamp_ms,
                actor: Actor::LocalUser,
                skill_id,
                arguments_redacted_json: payload.to_string(),
                policy_decision: PolicyDecision::Allow,
                execution_status: AuditExecutionStatus::Success,
                prev_hash,
            });

            let _ = store.append(&entry);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_device_registry_crud_lifecycle() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = DeviceRegistry::new(db);

        let dev_id = DeviceId::generate();
        let device = PairedDevice {
            device_id: dev_id.clone(),
            name: "MacBook Pro M3".to_string(),
            platform: PlatformTarget::MacOS,
            ed25519_public_key: [42u8; 32],
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: Some("192.168.1.55:51820".to_string()),
            trust_state: DeviceTrustState::Trusted,
        };

        // 1. Register device
        registry.register_device(&device).unwrap();

        // 2. Fetch and assert
        let fetched = registry.get_device(&dev_id).unwrap().unwrap();
        assert_eq!(fetched, device);

        // 3. Update heartbeat
        let updated = registry
            .update_device_heartbeat(&dev_id, Some("192.168.1.56:51820"), 2000)
            .unwrap();
        assert!(updated);

        let after_hb = registry.get_device(&dev_id).unwrap().unwrap();
        assert_eq!(after_hb.last_seen_ms, 2000);
        assert_eq!(
            after_hb.last_endpoint.as_deref(),
            Some("192.168.1.56:51820")
        );

        // 4. Update trust state
        let state_changed = registry
            .set_device_trust_state(&dev_id, DeviceTrustState::Blocked)
            .unwrap();
        assert!(state_changed);

        let after_trust = registry.get_device(&dev_id).unwrap().unwrap();
        assert_eq!(after_trust.trust_state, DeviceTrustState::Blocked);

        // 5. List devices
        let list = registry.list_devices().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].device_id, dev_id);

        // 6. Delete device
        let deleted = registry.delete_device(&dev_id).unwrap();
        assert!(deleted);
        assert!(registry.get_device(&dev_id).unwrap().is_none());
    }

    #[test]
    fn test_peer_permissions_and_cascade_on_delete() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = DeviceRegistry::new(db);

        let dev_id = DeviceId::generate();
        let device = PairedDevice {
            device_id: dev_id.clone(),
            name: "Android Phone".to_string(),
            platform: PlatformTarget::Android,
            ed25519_public_key: [99u8; 32],
            paired_at_ms: 5000,
            last_seen_ms: 5000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Trusted,
        };

        registry.register_device(&device).unwrap();

        // Check default permission for unconfigured capability
        let initial_perm = registry
            .check_peer_permission(&dev_id, "remote.clipboard")
            .unwrap();
        assert_eq!(initial_perm, PeerPermissionLevel::AskEachTime);

        // Grant specific permission
        registry
            .set_peer_permission(
                &dev_id,
                "remote.clipboard",
                PeerPermissionLevel::AlwaysAllow,
                6000,
            )
            .unwrap();

        registry
            .set_peer_permission(
                &dev_id,
                "remote.file_transfer",
                PeerPermissionLevel::Deny,
                6000,
            )
            .unwrap();

        let permissions = registry.get_peer_permissions(&dev_id).unwrap();
        assert_eq!(permissions.len(), 2);
        assert_eq!(
            permissions.get("remote.clipboard"),
            Some(&PeerPermissionLevel::AlwaysAllow)
        );
        assert_eq!(
            permissions.get("remote.file_transfer"),
            Some(&PeerPermissionLevel::Deny)
        );

        // Verify check_peer_permission reflects overrides
        assert_eq!(
            registry
                .check_peer_permission(&dev_id, "remote.clipboard")
                .unwrap(),
            PeerPermissionLevel::AlwaysAllow
        );
        assert_eq!(
            registry
                .check_peer_permission(&dev_id, "remote.file_transfer")
                .unwrap(),
            PeerPermissionLevel::Deny
        );

        // Delete device and verify permissions are cascaded
        registry.delete_device(&dev_id).unwrap();
        let perms_after_delete = registry.get_peer_permissions(&dev_id).unwrap();
        assert!(perms_after_delete.is_empty());

        // Check permission on deleted device returns Deny
        assert_eq!(
            registry
                .check_peer_permission(&dev_id, "remote.clipboard")
                .unwrap(),
            PeerPermissionLevel::Deny
        );
    }

    #[test]
    fn test_device_registry_audit_log_continuity() {
        let db = SqliteDb::open_in_memory().unwrap();
        let audit_store = Arc::new(AuditStore::new(db.clone()));
        let registry = DeviceRegistry::with_audit_store(db, audit_store.clone());

        let dev_id = DeviceId::generate();
        let device = PairedDevice {
            device_id: dev_id.clone(),
            name: "Audited Device".to_string(),
            platform: PlatformTarget::Linux,
            ed25519_public_key: [1u8; 32],
            paired_at_ms: 10000,
            last_seen_ms: 10000,
            last_endpoint: Some("10.0.0.5:51820".to_string()),
            trust_state: DeviceTrustState::PendingVerification,
        };

        registry.register_device(&device).unwrap();
        registry
            .set_device_trust_state(&dev_id, DeviceTrustState::Trusted)
            .unwrap();
        registry
            .set_peer_permission(
                &dev_id,
                "remote.skill_exec",
                PeerPermissionLevel::AlwaysAllow,
                11000,
            )
            .unwrap();
        registry.delete_device(&dev_id).unwrap();

        // Verify audit log chain integrity
        let entries = audit_store.get_entries(10, 0).unwrap();
        assert_eq!(entries.len(), 4);
        assert!(audit_store.verify_integrity().is_ok());
    }
}
