//! Interactive user confirmation models and thread-safe pending confirmation manager.

use std::collections::HashMap;
use std::sync::Mutex;

use aissistant_core::{ConversationId, TurnId};
use aissistant_security::RiskTier;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{OrchestratorError, Result};

/// Strongly typed identifier for a pending user confirmation request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConfirmationId(String);

impl ConfirmationId {
    /// Generates a new random UUIDv4 confirmation identifier.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Creates a confirmation identifier from a raw string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConfirmationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Request for explicit user confirmation before executing a privileged skill or tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingConfirmation {
    /// Unique confirmation request identifier.
    pub id: ConfirmationId,
    /// Associated conversation session.
    pub conversation_id: ConversationId,
    /// Associated turn within the conversation.
    pub turn_id: TurnId,
    /// Identifier of the requested tool/skill.
    pub tool_name: String,
    /// Input arguments payload submitted to the tool.
    pub arguments: serde_json::Value,
    /// Assessed risk classification tier.
    pub risk_tier: RiskTier,
    /// Human-readable rationale displayed to the user.
    pub reason: String,
    /// Millisecond Unix epoch timestamp when created.
    pub created_at_ms: i64,
}

/// Thread-safe manager tracking pending confirmation requests.
#[derive(Debug, Default)]
pub struct ConfirmationManager {
    pending: Mutex<HashMap<ConfirmationId, PendingConfirmation>>,
}

impl ConfirmationManager {
    /// Creates a new empty confirmation manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new pending confirmation.
    pub fn insert(&self, confirmation: PendingConfirmation) -> Result<()> {
        let mut map = self.pending.lock().map_err(|_| {
            OrchestratorError::Internal("ConfirmationManager mutex poisoned".to_string())
        })?;
        map.insert(confirmation.id.clone(), confirmation);
        Ok(())
    }

    /// Retrieves a clone of a pending confirmation by ID if it exists.
    pub fn get(&self, id: &ConfirmationId) -> Result<Option<PendingConfirmation>> {
        let map = self.pending.lock().map_err(|_| {
            OrchestratorError::Internal("ConfirmationManager mutex poisoned".to_string())
        })?;
        Ok(map.get(id).cloned())
    }

    /// Removes and returns a pending confirmation by ID.
    pub fn take(&self, id: &ConfirmationId) -> Result<PendingConfirmation> {
        let mut map = self.pending.lock().map_err(|_| {
            OrchestratorError::Internal("ConfirmationManager mutex poisoned".to_string())
        })?;
        map.remove(id)
            .ok_or_else(|| OrchestratorError::ConfirmationNotFound(id.to_string()))
    }

    /// Lists all currently active pending confirmations.
    pub fn list(&self) -> Result<Vec<PendingConfirmation>> {
        let map = self.pending.lock().map_err(|_| {
            OrchestratorError::Internal("ConfirmationManager mutex poisoned".to_string())
        })?;
        Ok(map.values().cloned().collect())
    }

    /// Cancels and removes all pending confirmations for a given conversation.
    pub fn cancel_all_for_conversation(
        &self,
        conversation_id: &ConversationId,
    ) -> Result<Vec<PendingConfirmation>> {
        let mut map = self.pending.lock().map_err(|_| {
            OrchestratorError::Internal("ConfirmationManager mutex poisoned".to_string())
        })?;
        let mut removed = Vec::new();
        map.retain(|_, v| {
            if &v.conversation_id == conversation_id {
                removed.push(v.clone());
                false
            } else {
                true
            }
        });
        Ok(removed)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_confirmation_manager_lifecycle() {
        let manager = ConfirmationManager::new();
        let conv_id = ConversationId::new("conv-test-1").unwrap();
        let turn_id = TurnId::new("turn-1").unwrap();
        let conf_id = ConfirmationId::generate();

        let req = PendingConfirmation {
            id: conf_id.clone(),
            conversation_id: conv_id.clone(),
            turn_id,
            tool_name: "file.delete".to_string(),
            arguments: serde_json::json!({"path": "/tmp/test.txt"}),
            risk_tier: RiskTier::Destructive,
            reason: "User requested file deletion".to_string(),
            created_at_ms: 1000,
        };

        manager.insert(req.clone()).unwrap();

        let retrieved = manager.get(&conf_id).unwrap();
        assert_eq!(retrieved, Some(req.clone()));

        let list = manager.list().unwrap();
        assert_eq!(list.len(), 1);

        let taken = manager.take(&conf_id).unwrap();
        assert_eq!(taken, req);

        // Subsequent take should fail with NotFound
        assert!(manager.take(&conf_id).is_err());
    }

    #[test]
    fn test_cancel_all_for_conversation() {
        let manager = ConfirmationManager::new();
        let conv1 = ConversationId::new("conv-1").unwrap();
        let conv2 = ConversationId::new("conv-2").unwrap();

        let req1 = PendingConfirmation {
            id: ConfirmationId::generate(),
            conversation_id: conv1.clone(),
            turn_id: TurnId::new("turn-1").unwrap(),
            tool_name: "file.write".to_string(),
            arguments: serde_json::json!({}),
            risk_tier: RiskTier::FileAndApp,
            reason: "write file".to_string(),
            created_at_ms: 1000,
        };
        let req2 = PendingConfirmation {
            id: ConfirmationId::generate(),
            conversation_id: conv2.clone(),
            turn_id: TurnId::new("turn-2").unwrap(),
            tool_name: "app.launch".to_string(),
            arguments: serde_json::json!({}),
            risk_tier: RiskTier::FileAndApp,
            reason: "launch app".to_string(),
            created_at_ms: 1000,
        };

        manager.insert(req1).unwrap();
        manager.insert(req2).unwrap();

        let cancelled = manager.cancel_all_for_conversation(&conv1).unwrap();
        assert_eq!(cancelled.len(), 1);
        assert_eq!(cancelled[0].conversation_id, conv1);

        let remaining = manager.list().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].conversation_id, conv2);
    }
}
