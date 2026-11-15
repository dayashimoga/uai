//! Tamper-evident, hash-chained security audit log primitives.

use aissistant_core::{AuditEntryId, CoreError, SkillId};
use serde::{Deserialize, Serialize};

use crate::policy::{Actor, PolicyDecision};

/// Genesis previous hash constant for the first entry in an audit chain.
pub const GENESIS_PREV_HASH: [u8; 32] = [0u8; 32];

/// Execution outcome recorded for an audited action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuditExecutionStatus {
    /// Skill or action completed successfully.
    Success,
    /// Skill failed with an error reason.
    Failed {
        /// Explanation of failure.
        reason: String,
    },
    /// Action was aborted or rejected by user confirmation.
    Cancelled,
}

/// A tamper-evident, cryptographically hash-chained audit log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique identifier of this audit record.
    pub id: AuditEntryId,
    /// Timestamp in epoch milliseconds.
    pub timestamp_ms: u64,
    /// The initiator of the action.
    pub actor: Actor,
    /// Target skill or system action identifier.
    pub skill_id: SkillId,
    /// Redacted JSON payload of input arguments (secrets/passwords redacted).
    pub arguments_redacted_json: String,
    /// Policy engine evaluation result.
    pub policy_decision: PolicyDecision,
    /// Final execution outcome.
    pub execution_status: AuditExecutionStatus,
    /// BLAKE3 hash of the preceding entry in the chain (`GENESIS_PREV_HASH` for first record).
    pub prev_hash: [u8; 32],
    /// BLAKE3 digest over all canonical entry fields and `prev_hash`.
    pub entry_hash: [u8; 32],
}

/// Parameters for constructing an audit log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewAuditEntry {
    /// Unique identifier of this audit record.
    pub id: AuditEntryId,
    /// Timestamp in epoch milliseconds.
    pub timestamp_ms: u64,
    /// The initiator of the action.
    pub actor: Actor,
    /// Target skill or system action identifier.
    pub skill_id: SkillId,
    /// Redacted JSON payload of input arguments (secrets/passwords redacted).
    pub arguments_redacted_json: String,
    /// Policy engine evaluation result.
    pub policy_decision: PolicyDecision,
    /// Final execution outcome.
    pub execution_status: AuditExecutionStatus,
    /// BLAKE3 hash of the preceding entry in the chain (`GENESIS_PREV_HASH` for first record).
    pub prev_hash: [u8; 32],
}

impl AuditEntry {
    /// Constructs and cryptographically seals a new audit log entry from parameters.
    #[must_use]
    pub fn new(params: NewAuditEntry) -> Self {
        let mut entry = Self {
            id: params.id,
            timestamp_ms: params.timestamp_ms,
            actor: params.actor,
            skill_id: params.skill_id,
            arguments_redacted_json: params.arguments_redacted_json,
            policy_decision: params.policy_decision,
            execution_status: params.execution_status,
            prev_hash: params.prev_hash,
            entry_hash: [0u8; 32],
        };
        entry.entry_hash = entry.compute_hash();
        entry
    }

    /// Constructs and cryptographically seals a new audit log entry.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn create(
        id: AuditEntryId,
        timestamp_ms: u64,
        actor: Actor,
        skill_id: SkillId,
        arguments_redacted_json: impl Into<String>,
        policy_decision: PolicyDecision,
        execution_status: AuditExecutionStatus,
        prev_hash: [u8; 32],
    ) -> Self {
        Self::new(NewAuditEntry {
            id,
            timestamp_ms,
            actor,
            skill_id,
            arguments_redacted_json: arguments_redacted_json.into(),
            policy_decision,
            execution_status,
            prev_hash,
        })
    }

    /// Computes the deterministic BLAKE3 digest across canonical fields.
    #[must_use]
    pub fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.prev_hash);
        hasher.update(self.id.as_str().as_bytes());
        hasher.update(&self.timestamp_ms.to_le_bytes());

        // Stable JSON representations of actor, decision, and status
        let actor_bytes = serde_json::to_vec(&self.actor).unwrap_or_default();
        let decision_bytes = serde_json::to_vec(&self.policy_decision).unwrap_or_default();
        let status_bytes = serde_json::to_vec(&self.execution_status).unwrap_or_default();

        hasher.update(&actor_bytes);
        hasher.update(self.skill_id.as_str().as_bytes());
        hasher.update(self.arguments_redacted_json.as_bytes());
        hasher.update(&decision_bytes);
        hasher.update(&status_bytes);

        *hasher.finalize().as_bytes()
    }

    /// Validates internal integrity between canonical fields and the recorded `entry_hash`.
    #[must_use]
    pub fn is_internally_valid(&self) -> bool {
        self.compute_hash() == self.entry_hash
    }
}

/// Cryptographic verifier for historical audit log chains.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuditChainVerifier;

impl AuditChainVerifier {
    /// Verifies an individual entry against the expected previous hash.
    pub fn verify_entry(
        entry: &AuditEntry,
        expected_prev_hash: &[u8; 32],
    ) -> Result<[u8; 32], CoreError> {
        if entry.prev_hash != *expected_prev_hash {
            return Err(CoreError::PermissionDenied {
                action: "audit.verify".to_string(),
                reason: format!(
                    "Audit log broken chain link at entry '{}': prev_hash mismatch",
                    entry.id
                ),
            });
        }

        let computed_hash = entry.compute_hash();
        if computed_hash != entry.entry_hash {
            return Err(CoreError::PermissionDenied {
                action: "audit.verify".to_string(),
                reason: format!(
                    "Audit log entry '{}' has been tampered with: entry_hash does not match computed digest",
                    entry.id
                ),
            });
        }

        Ok(entry.entry_hash)
    }

    /// Verifies an entire sequence of audit log entries from genesis.
    /// Returns the final tip hash on successful verification.
    pub fn verify_chain(
        entries: &[AuditEntry],
        initial_prev_hash: &[u8; 32],
    ) -> Result<[u8; 32], CoreError> {
        let mut current_hash = *initial_prev_hash;

        for (idx, entry) in entries.iter().enumerate() {
            current_hash = Self::verify_entry(entry, &current_hash).map_err(|e| match e {
                CoreError::PermissionDenied { action, reason } => CoreError::PermissionDenied {
                    action,
                    reason: format!("Verification failed at chain index {idx}: {reason}"),
                },
                other => other,
            })?;
        }

        Ok(current_hash)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::policy::RiskTier;

    #[test]
    fn test_single_entry_hash_integrity() {
        let entry = AuditEntry::create(
            AuditEntryId::from_sequence(1),
            1000,
            Actor::LocalUser,
            SkillId::new("system.time").unwrap(),
            "{}",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
            GENESIS_PREV_HASH,
        );

        assert!(entry.is_internally_valid());
        assert_ne!(entry.entry_hash, [0u8; 32]);

        let verified = AuditChainVerifier::verify_entry(&entry, &GENESIS_PREV_HASH);
        assert!(verified.is_ok());
        assert_eq!(verified.unwrap(), entry.entry_hash);
    }

    #[test]
    fn test_valid_chain_verification() {
        let mut entries = Vec::new();
        let mut prev_hash = GENESIS_PREV_HASH;

        for i in 1..=5 {
            let entry = AuditEntry::create(
                AuditEntryId::from_sequence(i),
                1000 + i * 100,
                Actor::LlmOrchestrator,
                SkillId::new("file.read").unwrap(),
                format!("{{\"path\": \"file_{i}.txt\"}}"),
                PolicyDecision::RequireConfirmation {
                    reason: "File read".to_string(),
                    risk_tier: RiskTier::FileAndApp,
                },
                AuditExecutionStatus::Success,
                prev_hash,
            );
            prev_hash = entry.entry_hash;
            entries.push(entry);
        }

        let tip = AuditChainVerifier::verify_chain(&entries, &GENESIS_PREV_HASH);
        assert!(tip.is_ok());
        assert_eq!(tip.unwrap(), prev_hash);
    }

    #[test]
    fn test_tampered_entry_payload_detected() {
        let mut entries = Vec::new();
        let mut prev_hash = GENESIS_PREV_HASH;

        for i in 1..=3 {
            let entry = AuditEntry::create(
                AuditEntryId::from_sequence(i),
                1000 + i * 10,
                Actor::LocalUser,
                SkillId::new("media.play").unwrap(),
                "{}",
                PolicyDecision::Allow,
                AuditExecutionStatus::Success,
                prev_hash,
            );
            prev_hash = entry.entry_hash;
            entries.push(entry);
        }

        // Tamper with entry 2 payload
        entries[1].arguments_redacted_json = "{\"tampered\": true}".to_string();

        let result = AuditChainVerifier::verify_chain(&entries, &GENESIS_PREV_HASH);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("chain index 1"));
    }

    #[test]
    fn test_deleted_entry_breaks_chain() {
        let mut entries = Vec::new();
        let mut prev_hash = GENESIS_PREV_HASH;

        for i in 1..=4 {
            let entry = AuditEntry::create(
                AuditEntryId::from_sequence(i),
                1000 + i * 10,
                Actor::LocalUser,
                SkillId::new("media.play").unwrap(),
                "{}",
                PolicyDecision::Allow,
                AuditExecutionStatus::Success,
                prev_hash,
            );
            prev_hash = entry.entry_hash;
            entries.push(entry);
        }

        // Remove intermediate entry at index 1
        entries.remove(1);

        let result = AuditChainVerifier::verify_chain(&entries, &GENESIS_PREV_HASH);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("chain index 1"));
    }
}
