//! Deterministic permission policy engine and risk classification.

use aissistant_core::{DeviceId, SkillId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use crate::taint::TaintState;

/// Risk tier classifications for assistant skills and commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    /// Safe read-only inspection (telemetry, query time, read memory).
    ReadOnly = 0,
    /// Modifies user session state (timers, media playback, volume, notes).
    StateModifying = 1,
    /// Operates on files or launches applications.
    FileAndApp = 2,
    /// Privileged OS automation (screen streaming, window management, input injection).
    PrivilegedSystem = 3,
    /// Irreversible or destructive operations (delete files, kill processes, wipe data).
    Destructive = 4,
}

impl RiskTier {
    /// Returns the integer ordinal rank of this risk tier.
    #[must_use]
    pub const fn ordinal(&self) -> u8 {
        *self as u8
    }

    /// Returns the canonical string representation of this risk tier.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::StateModifying => "state_modifying",
            Self::FileAndApp => "file_and_app",
            Self::PrivilegedSystem => "privileged_system",
            Self::Destructive => "destructive",
        }
    }
}

impl fmt::Display for RiskTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for RiskTier {
    type Err = aissistant_core::CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "read_only" | "0" => Ok(Self::ReadOnly),
            "state_modifying" | "1" => Ok(Self::StateModifying),
            "file_and_app" | "2" => Ok(Self::FileAndApp),
            "privileged_system" | "3" => Ok(Self::PrivilegedSystem),
            "destructive" | "4" => Ok(Self::Destructive),
            other => Err(aissistant_core::CoreError::InvalidId {
                entity: "RiskTier",
                value: other.to_string(),
                reason: "supported risk tiers are read_only, state_modifying, file_and_app, privileged_system, destructive".to_string(),
            }),
        }
    }
}

/// The decision produced by the policy engine when evaluating a skill execution request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyDecision {
    /// Operation is authorized to execute immediately.
    Allow,
    /// Operation requires explicit user confirmation before execution.
    RequireConfirmation {
        /// Rationale presented to the user on the prompt dialog.
        reason: String,
        /// Assessed risk tier of the operation.
        risk_tier: RiskTier,
    },
    /// Operation is strictly denied.
    Deny {
        /// Explanation for the denial.
        reason: String,
    },
}

impl PolicyDecision {
    /// Returns true if the policy allows immediate execution.
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow)
    }

    /// Returns true if explicit user confirmation is required.
    #[must_use]
    pub const fn requires_confirmation(&self) -> bool {
        matches!(self, Self::RequireConfirmation { .. })
    }

    /// Returns true if the policy denies execution.
    #[must_use]
    pub const fn is_denied(&self) -> bool {
        matches!(self, Self::Deny { .. })
    }
}

/// The entity requesting skill execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "actor_type", rename_all = "snake_case")]
pub enum Actor {
    /// Physical local user acting via UI button or direct CLI.
    LocalUser,
    /// Autonomous local LLM turn processing user request.
    LlmOrchestrator,
    /// Remote paired device issuing an RPC command over QUIC.
    RemotePeer {
        /// Authenticated device identifier of the peer.
        device_id: DeviceId,
    },
}

/// Context passed to the policy engine for evaluating a skill request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyContext {
    /// The initiator of the action.
    pub actor: Actor,
    /// Taint state of the active context.
    pub taint_state: TaintState,
}

impl PolicyContext {
    /// Context for direct local user actions.
    #[must_use]
    pub fn local_user() -> Self {
        Self {
            actor: Actor::LocalUser,
            taint_state: TaintState::Clean,
        }
    }

    /// Context for local LLM orchestration turns.
    #[must_use]
    pub fn llm(taint_state: TaintState) -> Self {
        Self {
            actor: Actor::LlmOrchestrator,
            taint_state,
        }
    }

    /// Context for requests arriving from a remote peer device.
    #[must_use]
    pub fn remote_peer(device_id: DeviceId, taint_state: TaintState) -> Self {
        Self {
            actor: Actor::RemotePeer { device_id },
            taint_state,
        }
    }
}

/// Central deterministic authorization and security policy engine.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyEngine {
    skill_overrides: HashMap<SkillId, PolicyDecision>,
    peer_permissions: HashMap<DeviceId, HashMap<SkillId, PolicyDecision>>,
}

impl PolicyEngine {
    /// Creates a new policy engine with default fail-closed security rules.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets an explicit policy decision override for a specific skill.
    pub fn set_skill_override(&mut self, skill_id: SkillId, decision: PolicyDecision) {
        self.skill_overrides.insert(skill_id, decision);
    }

    /// Sets a peer-specific permission decision for a skill.
    pub fn set_peer_permission(
        &mut self,
        device_id: DeviceId,
        skill_id: SkillId,
        decision: PolicyDecision,
    ) {
        self.peer_permissions
            .entry(device_id)
            .or_default()
            .insert(skill_id, decision);
    }

    /// Evaluates an execution request against security policies and taint state.
    #[must_use]
    pub fn evaluate(
        &self,
        skill_id: &SkillId,
        risk_tier: RiskTier,
        context: &PolicyContext,
    ) -> PolicyDecision {
        // 1. Check peer-specific ACL if request originates from a remote peer
        if let Actor::RemotePeer { device_id } = &context.actor {
            if let Some(peer_rules) = self.peer_permissions.get(device_id) {
                if let Some(decision) = peer_rules.get(skill_id) {
                    return decision.clone();
                }
            }
        }

        // 2. Check global skill override
        if let Some(decision) = self.skill_overrides.get(skill_id) {
            return decision.clone();
        }

        // 3. Taint Escalation Rule: Untrusted external data forces mandatory user confirmation
        // for any state-modifying, file, system, or destructive action.
        if context.taint_state.is_tainted() && risk_tier >= RiskTier::StateModifying {
            if risk_tier == RiskTier::Destructive
                && matches!(context.actor, Actor::RemotePeer { .. })
            {
                return PolicyDecision::Deny {
                    reason:
                        "Destructive action from remote peer blocked: untrusted input in context."
                            .to_string(),
                };
            }

            return PolicyDecision::RequireConfirmation {
                reason: format!(
                    "Untrusted input detected in conversation context. Manual confirmation required for '{}'.",
                    skill_id
                ),
                risk_tier,
            };
        }

        // 4. Baseline Default Policy Matrix
        match &context.actor {
            Actor::LocalUser => {
                // Direct physical user action: allowed immediately
                PolicyDecision::Allow
            }

            Actor::LlmOrchestrator => match risk_tier {
                RiskTier::ReadOnly | RiskTier::StateModifying => PolicyDecision::Allow,
                RiskTier::FileAndApp | RiskTier::PrivilegedSystem | RiskTier::Destructive => {
                    PolicyDecision::RequireConfirmation {
                        reason: format!(
                            "Local LLM requested execution of {} skill '{}'.",
                            risk_tier, skill_id
                        ),
                        risk_tier,
                    }
                }
            },

            Actor::RemotePeer { device_id } => match risk_tier {
                RiskTier::ReadOnly => PolicyDecision::Allow,
                RiskTier::StateModifying | RiskTier::FileAndApp | RiskTier::PrivilegedSystem => {
                    PolicyDecision::RequireConfirmation {
                        reason: format!(
                            "Remote peer '{}' requested execution of {} skill '{}'.",
                            device_id, risk_tier, skill_id
                        ),
                        risk_tier,
                    }
                }
                RiskTier::Destructive => PolicyDecision::Deny {
                    reason: format!(
                        "Remote peer '{}' is forbidden from invoking destructive skill '{}'.",
                        device_id, skill_id
                    ),
                },
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::taint::TaintSource;

    #[test]
    fn test_risk_tier_ordering_and_parsing() {
        assert!(RiskTier::ReadOnly < RiskTier::StateModifying);
        assert!(RiskTier::StateModifying < RiskTier::FileAndApp);
        assert!(RiskTier::FileAndApp < RiskTier::PrivilegedSystem);
        assert!(RiskTier::PrivilegedSystem < RiskTier::Destructive);

        assert_eq!("read_only".parse::<RiskTier>().unwrap(), RiskTier::ReadOnly);
        assert_eq!(
            "destructive".parse::<RiskTier>().unwrap(),
            RiskTier::Destructive
        );
        assert!("invalid_tier".parse::<RiskTier>().is_err());
    }

    #[test]
    fn test_local_user_always_allowed() {
        let engine = PolicyEngine::new();
        let ctx = PolicyContext::local_user();
        let skill = SkillId::new("file.delete").unwrap();

        let decision = engine.evaluate(&skill, RiskTier::Destructive, &ctx);
        assert!(decision.is_allowed());
    }

    #[test]
    fn test_llm_orchestrator_baseline_tiers() {
        let engine = PolicyEngine::new();
        let ctx = PolicyContext::llm(TaintState::Clean);

        let read_skill = SkillId::new("system.time").unwrap();
        assert!(engine
            .evaluate(&read_skill, RiskTier::ReadOnly, &ctx)
            .is_allowed());

        let state_skill = SkillId::new("media.pause").unwrap();
        assert!(engine
            .evaluate(&state_skill, RiskTier::StateModifying, &ctx)
            .is_allowed());

        let file_skill = SkillId::new("file.write").unwrap();
        assert!(engine
            .evaluate(&file_skill, RiskTier::FileAndApp, &ctx)
            .requires_confirmation());

        let dest_skill = SkillId::new("file.delete").unwrap();
        assert!(engine
            .evaluate(&dest_skill, RiskTier::Destructive, &ctx)
            .requires_confirmation());
    }

    #[test]
    fn test_remote_peer_baseline_tiers() {
        let engine = PolicyEngine::new();
        let peer_id = DeviceId::new("paired_laptop").unwrap();
        let ctx = PolicyContext::remote_peer(peer_id, TaintState::Clean);

        let read_skill = SkillId::new("telemetry.battery").unwrap();
        assert!(engine
            .evaluate(&read_skill, RiskTier::ReadOnly, &ctx)
            .is_allowed());

        let state_skill = SkillId::new("media.pause").unwrap();
        assert!(engine
            .evaluate(&state_skill, RiskTier::StateModifying, &ctx)
            .requires_confirmation());

        let dest_skill = SkillId::new("system.wipe").unwrap();
        let decision = engine.evaluate(&dest_skill, RiskTier::Destructive, &ctx);
        assert!(decision.is_denied());
    }

    #[test]
    fn test_prompt_injection_taint_escalation() {
        let engine = PolicyEngine::new();
        let mut taint_state = TaintState::Clean;
        taint_state.add_taint(
            TaintSource::LocalFile {
                path: "malicious_document.pdf".to_string(),
            },
            1000,
            "Adversarial prompt injection payload",
        );

        let ctx = PolicyContext::llm(taint_state);
        let state_skill = SkillId::new("media.volume").unwrap();

        // Normally StateModifying is Allow for LLM; but tainted context MUST escalate to RequireConfirmation
        let decision = engine.evaluate(&state_skill, RiskTier::StateModifying, &ctx);
        assert!(decision.requires_confirmation());

        match decision {
            PolicyDecision::RequireConfirmation { reason, risk_tier } => {
                assert_eq!(risk_tier, RiskTier::StateModifying);
                assert!(reason.contains("Untrusted input detected"));
            }
            _ => panic!("Expected RequireConfirmation"),
        }
    }

    #[test]
    fn test_policy_overrides() {
        let mut engine = PolicyEngine::new();
        let skill = SkillId::new("custom.tool").unwrap();

        engine.set_skill_override(
            skill.clone(),
            PolicyDecision::Deny {
                reason: "Administrator disabled this tool".to_string(),
            },
        );

        let ctx = PolicyContext::local_user();
        let decision = engine.evaluate(&skill, RiskTier::ReadOnly, &ctx);
        assert!(decision.is_denied());
    }

    #[test]
    fn test_peer_specific_permission_grant() {
        let mut engine = PolicyEngine::new();
        let peer_id = DeviceId::new("trusted_phone").unwrap();
        let skill = SkillId::new("media.pause").unwrap();

        // Grant remote peer permission to pause media without prompt
        engine.set_peer_permission(peer_id.clone(), skill.clone(), PolicyDecision::Allow);

        let ctx = PolicyContext::remote_peer(peer_id, TaintState::Clean);
        let decision = engine.evaluate(&skill, RiskTier::StateModifying, &ctx);
        assert!(decision.is_allowed());
    }
}
