//! Extensible skill framework, sandboxed executor, and built-in safe utility skills.
//!
//! Provides the core registry, execution sandbox, and offline safe utility tools
//! (`system.time`, `system.timer`, `system.calc`, `system.convert`, and `system.telemetry`)
//! conforming to strict risk tiers (Risk Tier 0 and 1) and audit logging requirements.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod error;
pub mod registry;
pub mod sandbox;
pub mod utility;

pub use error::{Result, SkillError};
pub use registry::SkillRegistry;
pub use sandbox::{SandboxedSkill, DEFAULT_SKILL_TIMEOUT_MS};
pub use utility::{CalcSkill, ConvertSkill, TelemetrySkill, TimeSkill, TimerSkill};

#[cfg(test)]
mod tests {
    use super::*;
    use aissistant_orchestrator::ToolDispatcher;
    use aissistant_security::RiskTier;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_skill_registry_all_builtins_registration_and_execution() {
        let registry = SkillRegistry::new();
        let dispatcher = ToolDispatcher::new();

        registry.register_all_builtins(&dispatcher).unwrap();

        // 1. system.time
        let time_tool = dispatcher.get("system.time").unwrap().unwrap();
        assert_eq!(time_tool.name(), "system.time");
        assert_eq!(time_tool.risk_tier(), RiskTier::ReadOnly);
        let res = dispatcher
            .execute("system.time", json!({"format": "full"}))
            .unwrap();
        assert!(res.to_string().contains("iso8601"));

        // 2. system.timer
        let timer_tool = dispatcher.get("system.timer").unwrap().unwrap();
        assert_eq!(timer_tool.name(), "system.timer");
        assert_eq!(timer_tool.risk_tier(), RiskTier::StateModifying);
        let start_res = dispatcher
            .execute(
                "system.timer",
                json!({
                    "action": "set",
                    "duration_seconds": 60,
                    "name": "pasta"
                }),
            )
            .unwrap();
        assert!(start_res.to_string().contains("pasta"));

        // 3. system.calc
        let calc_tool = dispatcher.get("system.calc").unwrap().unwrap();
        assert_eq!(calc_tool.name(), "system.calc");
        assert_eq!(calc_tool.risk_tier(), RiskTier::ReadOnly);
        let calc_res = dispatcher
            .execute("system.calc", json!({"expression": "3 * (4 + 6) / 2"}))
            .unwrap();
        assert!(calc_res.to_string().contains("15"));

        // 4. system.convert
        let convert_tool = dispatcher.get("system.convert").unwrap().unwrap();
        assert_eq!(convert_tool.name(), "system.convert");
        assert_eq!(convert_tool.risk_tier(), RiskTier::ReadOnly);
        let conv_res = dispatcher
            .execute(
                "system.convert",
                json!({
                    "value": 100.0,
                    "from_unit": "celsius",
                    "to_unit": "fahrenheit"
                }),
            )
            .unwrap();
        assert!(conv_res.to_string().contains("212"));

        // 5. system.telemetry
        let telemetry_tool = dispatcher.get("system.telemetry").unwrap().unwrap();
        assert_eq!(telemetry_tool.name(), "system.telemetry");
        assert_eq!(telemetry_tool.risk_tier(), RiskTier::ReadOnly);
        let telem_res = dispatcher
            .execute("system.telemetry", json!({"metric": "all"}))
            .unwrap();
        assert!(telem_res.to_string().contains("total_bytes"));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_sandboxed_skill_dispatch_error_propagation() {
        let dispatcher = ToolDispatcher::new();
        dispatcher
            .register(Arc::new(SandboxedSkill::with_default_timeout(Arc::new(
                CalcSkill::new(),
            ))))
            .unwrap();

        // Division by zero error
        let err = dispatcher.execute("system.calc", json!({"expression": "42 / 0"}));
        assert!(err.is_err());
    }
}
