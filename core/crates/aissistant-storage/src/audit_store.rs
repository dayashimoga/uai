//! Persistent SQLite store for the cryptographic, hash-chained security audit log.

use aissistant_core::{AuditEntryId, SkillId};
use aissistant_security::{
    Actor, AuditChainVerifier, AuditEntry, AuditExecutionStatus, PolicyDecision, GENESIS_PREV_HASH,
};

use crate::error::StorageError;
use crate::pool::SqliteDb;

/// Repository managing persistent storage and integrity verification of audit log records.
#[derive(Debug, Clone)]
pub struct AuditStore {
    db: SqliteDb,
}

impl AuditStore {
    /// Creates a new audit store using the given database handle.
    #[must_use]
    pub const fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    /// Returns the tip `entry_hash` of the active chain, or `GENESIS_PREV_HASH` if empty.
    pub fn latest_hash(&self) -> Result<[u8; 32], StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT entry_hash FROM security_audit_log ORDER BY rowid DESC LIMIT 1;",
            )?;
            let mut rows = stmt.query([])?;

            if let Some(row) = rows.next()? {
                let blob: Vec<u8> = row.get(0)?;
                if blob.len() == 32 {
                    let mut hash = [0u8; 32];
                    hash.copy_from_slice(&blob);
                    Ok(hash)
                } else {
                    Err(StorageError::CorruptAuditChain {
                        reason: format!(
                            "Stored entry_hash is invalid length ({} bytes)",
                            blob.len()
                        ),
                    })
                }
            } else {
                Ok(GENESIS_PREV_HASH)
            }
        })
    }

    /// Appends a new verified audit entry to the log, enforcing chain linkage.
    pub fn append(&self, entry: &AuditEntry) -> Result<(), StorageError> {
        if !entry.is_internally_valid() {
            return Err(StorageError::CorruptAuditChain {
                reason: format!(
                    "Audit entry '{}' internal digest does not match computed hash",
                    entry.id
                ),
            });
        }

        let expected_prev_hash = self.latest_hash()?;
        if entry.prev_hash != expected_prev_hash {
            return Err(StorageError::CorruptAuditChain {
                reason: format!(
                    "Audit entry '{}' prev_hash does not match chain tip",
                    entry.id
                ),
            });
        }

        let actor_json = serde_json::to_string(&entry.actor)?;
        let decision_json = serde_json::to_string(&entry.policy_decision)?;
        let status_json = serde_json::to_string(&entry.execution_status)?;

        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO security_audit_log (
                    id, timestamp_ms, actor_json, skill_id,
                    arguments_redacted_json, policy_decision_json,
                    execution_status_json, prev_hash, entry_hash
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
                rusqlite::params![
                    entry.id.as_str(),
                    entry.timestamp_ms as i64,
                    actor_json,
                    entry.skill_id.as_str(),
                    entry.arguments_redacted_json,
                    decision_json,
                    status_json,
                    entry.prev_hash.as_slice(),
                    entry.entry_hash.as_slice(),
                ],
            )?;
            Ok(())
        })
    }

    /// Retrieves a paginated slice of historical audit entries in sequential order.
    pub fn get_entries(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditEntry>, StorageError> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp_ms, actor_json, skill_id,
                        arguments_redacted_json, policy_decision_json,
                        execution_status_json, prev_hash, entry_hash
                 FROM security_audit_log
                 ORDER BY rowid ASC
                 LIMIT ?1 OFFSET ?2;",
            )?;

            let rows = stmt.query_map(rusqlite::params![limit as i64, offset as i64], |row| {
                let id_str: String = row.get(0)?;
                let timestamp_ms: i64 = row.get(1)?;
                let actor_json: String = row.get(2)?;
                let skill_id_str: String = row.get(3)?;
                let arguments_redacted_json: String = row.get(4)?;
                let decision_json: String = row.get(5)?;
                let status_json: String = row.get(6)?;
                let prev_hash_blob: Vec<u8> = row.get(7)?;
                let entry_hash_blob: Vec<u8> = row.get(8)?;

                Ok((
                    id_str,
                    timestamp_ms,
                    actor_json,
                    skill_id_str,
                    arguments_redacted_json,
                    decision_json,
                    status_json,
                    prev_hash_blob,
                    entry_hash_blob,
                ))
            })?;

            let mut entries = Vec::new();
            for item in rows {
                let (
                    id_str,
                    timestamp_ms,
                    actor_json,
                    skill_id_str,
                    arguments_redacted_json,
                    decision_json,
                    status_json,
                    prev_hash_blob,
                    entry_hash_blob,
                ) = item?;

                let id = AuditEntryId::new(&id_str).map_err(|e| StorageError::Serialization {
                    message: e.to_string(),
                })?;
                let skill_id =
                    SkillId::new(&skill_id_str).map_err(|e| StorageError::Serialization {
                        message: e.to_string(),
                    })?;
                let actor: Actor = serde_json::from_str(&actor_json)?;
                let policy_decision: PolicyDecision = serde_json::from_str(&decision_json)?;
                let execution_status: AuditExecutionStatus = serde_json::from_str(&status_json)?;

                if prev_hash_blob.len() != 32 || entry_hash_blob.len() != 32 {
                    return Err(StorageError::CorruptAuditChain {
                        reason: format!("Invalid hash length in database for audit entry '{id}'"),
                    });
                }

                let mut prev_hash = [0u8; 32];
                prev_hash.copy_from_slice(&prev_hash_blob);

                let mut entry_hash = [0u8; 32];
                entry_hash.copy_from_slice(&entry_hash_blob);

                entries.push(AuditEntry {
                    id,
                    timestamp_ms: timestamp_ms as u64,
                    actor,
                    skill_id,
                    arguments_redacted_json,
                    policy_decision,
                    execution_status,
                    prev_hash,
                    entry_hash,
                });
            }

            Ok(entries)
        })
    }

    /// Verifies the entire persistent audit chain from genesis, detecting any SQL tampering.
    pub fn verify_integrity(&self) -> Result<[u8; 32], StorageError> {
        // Read all entries in order
        let all_entries = self.get_entries(usize::MAX, 0)?;
        AuditChainVerifier::verify_chain(&all_entries, &GENESIS_PREV_HASH).map_err(|e| {
            StorageError::CorruptAuditChain {
                reason: e.to_string(),
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_security::RiskTier;

    #[test]
    fn test_audit_store_append_and_verify_chain() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = AuditStore::new(db);

        assert_eq!(store.latest_hash().unwrap(), GENESIS_PREV_HASH);

        // Append 3 entries
        for i in 1..=3 {
            let prev = store.latest_hash().unwrap();
            let entry = AuditEntry::create(
                AuditEntryId::from_sequence(i),
                1000 + i * 10,
                Actor::LocalUser,
                SkillId::new("system.time").unwrap(),
                "{}",
                PolicyDecision::Allow,
                AuditExecutionStatus::Success,
                prev,
            );
            store.append(&entry).unwrap();
        }

        let tip = store.latest_hash().unwrap();
        assert_ne!(tip, GENESIS_PREV_HASH);

        // Verify chain integrity
        let verified_tip = store.verify_integrity().unwrap();
        assert_eq!(tip, verified_tip);

        let entries = store.get_entries(10, 0).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_audit_store_rejects_broken_link() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = AuditStore::new(db);

        // Attempt to append with invalid prev_hash
        let bad_entry = AuditEntry::create(
            AuditEntryId::from_sequence(1),
            1000,
            Actor::LocalUser,
            SkillId::new("system.time").unwrap(),
            "{}",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
            [0xFF; 32], // Wrong prev_hash
        );

        assert!(store.append(&bad_entry).is_err());
    }

    #[test]
    fn test_adversarial_sql_tampering_detected() {
        let db = SqliteDb::open_in_memory().unwrap();
        let store = AuditStore::new(db.clone());

        for i in 1..=3 {
            let prev = store.latest_hash().unwrap();
            let entry = AuditEntry::create(
                AuditEntryId::from_sequence(i),
                1000 + i * 10,
                Actor::LlmOrchestrator,
                SkillId::new("file.read").unwrap(),
                format!("{{\"file\": {i}}}"),
                PolicyDecision::RequireConfirmation {
                    reason: "File read".to_string(),
                    risk_tier: RiskTier::FileAndApp,
                },
                AuditExecutionStatus::Success,
                prev,
            );
            store.append(&entry).unwrap();
        }

        assert!(store.verify_integrity().is_ok());

        // Simulate an attacker directly executing an UPDATE on the SQLite table to tamper with entry 2
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE security_audit_log SET arguments_redacted_json = '{\"file\": 999}' WHERE id = 'audit_0000000002';",
                [],
            )?;
            Ok(())
        })
        .unwrap();

        // Integrity verification MUST fail and report tampering
        let result = store.verify_integrity();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("tampered with"));
    }
}
