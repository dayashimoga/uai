//! Host hardware telemetry skills (`system.battery`).

use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_platform::DesktopPlatformAdapter;
use aissistant_security::RiskTier;
use serde_json::Value;

/// Skill for reading host battery and AC power supply telemetry.
#[derive(Debug, Clone)]
pub struct SystemBatterySkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl SystemBatterySkill {
    /// Creates a new `SystemBatterySkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for SystemBatterySkill {
    fn name(&self) -> &str {
        "system.battery"
    }

    fn description(&self) -> &str {
        "Queries host device battery level percentage, charging state, and external power status."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, _arguments: Value) -> Result<Value, OrchestratorError> {
        let battery = self.adapter.get_battery_status().map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "system.battery".to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(serde_json::json!({
            "percentage": battery.percentage,
            "is_charging": battery.is_charging,
            "on_ac_power": battery.on_ac_power,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_platform::VirtualDesktopAdapter;

    #[test]
    fn test_system_battery_skill() {
        let adapter = Arc::new(VirtualDesktopAdapter::new());
        let skill = SystemBatterySkill::new(adapter);

        assert_eq!(skill.risk_tier(), RiskTier::ReadOnly);

        let res = skill.execute(serde_json::json!({})).unwrap();
        assert_eq!(res["percentage"], 85);
        assert_eq!(res["is_charging"], true);
        assert_eq!(res["on_ac_power"], true);
    }
}
