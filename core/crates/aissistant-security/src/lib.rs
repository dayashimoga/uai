//! Security policy engine, risk classification, cryptographic identity, and tamper-evident audit logging.

#![forbid(unsafe_code)]

pub mod audit;
pub mod crypto;
pub mod policy;
pub mod taint;

pub use audit::{AuditChainVerifier, AuditEntry, AuditExecutionStatus, GENESIS_PREV_HASH};
pub use crypto::{AeadKey, DeviceIdentity, MIN_AEAD_PAYLOAD_LEN, XCHACHA20_NONCE_LEN};
pub use policy::{Actor, PolicyContext, PolicyDecision, PolicyEngine, RiskTier};
pub use taint::{TaintRecord, TaintSource, TaintState, TaintTracker};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_core::SkillId;

    #[test]
    fn test_security_subsystem_integration() {
        let engine = PolicyEngine::new();
        let mut tracker = TaintTracker::new();

        let skill = SkillId::new("file.append").unwrap();

        // 1. Clean LLM context -> FileAndApp requires confirmation
        let ctx_clean = PolicyContext::llm(tracker.state().clone());
        let dec1 = engine.evaluate(&skill, RiskTier::FileAndApp, &ctx_clean);
        assert!(dec1.requires_confirmation());

        // 2. Mark tainted from untrusted clipboard
        tracker.mark_tainted(TaintSource::Clipboard, 100, "Untrusted clipboard data");
        let ctx_tainted = PolicyContext::llm(tracker.state().clone());

        // StateModifying skill now forced to RequireConfirmation
        let pause_skill = SkillId::new("media.pause").unwrap();
        let dec2 = engine.evaluate(&pause_skill, RiskTier::StateModifying, &ctx_tainted);
        assert!(dec2.requires_confirmation());
    }
}
