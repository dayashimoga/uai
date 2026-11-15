//! Built-in timer and stopwatch management skill (`system.timer`).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ActiveTimer {
    label: String,
    start_ms: i64,
    duration_sec: u64,
    target_ms: i64,
}

/// Skill providing countdown timers, stopwatch inspection, and timer cancellation.
#[derive(Debug, Default)]
pub struct TimerSkill {
    timers: Mutex<HashMap<String, ActiveTimer>>,
}

impl TimerSkill {
    /// Creates a new `TimerSkill` instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

fn current_epoch_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl ToolHandler for TimerSkill {
    fn name(&self) -> &str {
        "system.timer"
    }

    fn description(&self) -> &str {
        "Sets countdown timers, checks remaining time, or cancels active timers."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["set", "status", "cancel", "list"],
                    "description": "Timer action: 'set' (create new), 'status' (inspect), 'cancel' (stop), or 'list'."
                },
                "name": {
                    "type": "string",
                    "description": "Name or label of the timer (e.g. 'pasta', 'tea')."
                },
                "duration_seconds": {
                    "type": "integer",
                    "description": "Timer countdown duration in seconds (required for 'set')."
                }
            },
            "required": ["action"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::StateModifying
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let action = arguments
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "system.timer".to_string(),
                reason: "Missing 'action' parameter".to_string(),
            })?;

        let mut map = self
            .timers
            .lock()
            .map_err(|_| OrchestratorError::Internal("TimerSkill lock poisoned".to_string()))?;

        let now = current_epoch_ms();

        match action {
            "set" => {
                let name = arguments
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("timer");
                let duration_sec = arguments
                    .get("duration_seconds")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                        tool_name: "system.timer".to_string(),
                        reason: "Missing or invalid 'duration_seconds' parameter".to_string(),
                    })?;

                if duration_sec == 0 {
                    return Err(OrchestratorError::InvalidToolArguments {
                        tool_name: "system.timer".to_string(),
                        reason: "Duration must be greater than 0 seconds".to_string(),
                    });
                }

                let target_ms = now + (duration_sec as i64 * 1000);
                let timer = ActiveTimer {
                    label: name.to_string(),
                    start_ms: now,
                    duration_sec,
                    target_ms,
                };

                map.insert(name.to_string(), timer.clone());

                Ok(serde_json::json!({
                    "action": "set",
                    "name": name,
                    "duration_seconds": duration_sec,
                    "remaining_seconds": duration_sec,
                    "target_ms": target_ms,
                    "status": "running"
                }))
            }

            "status" => {
                let name = arguments
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("timer");
                let timer =
                    map.get(name)
                        .ok_or_else(|| OrchestratorError::ToolExecutionFailed {
                            tool_name: "system.timer".to_string(),
                            reason: format!("No active timer found with name '{name}'"),
                        })?;

                let remaining_ms = timer.target_ms - now;
                let remaining_sec = (remaining_ms / 1000).max(0);
                let is_expired = remaining_ms <= 0;

                Ok(serde_json::json!({
                    "action": "status",
                    "name": name,
                    "duration_seconds": timer.duration_sec,
                    "remaining_seconds": remaining_sec,
                    "is_expired": is_expired,
                    "status": if is_expired { "expired" } else { "running" }
                }))
            }

            "cancel" => {
                let name = arguments
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("timer");
                let removed = map.remove(name);

                Ok(serde_json::json!({
                    "action": "cancel",
                    "name": name,
                    "cancelled": removed.is_some()
                }))
            }

            "list" => {
                let mut list = Vec::new();
                for (name, timer) in map.iter() {
                    let remaining_ms = timer.target_ms - now;
                    let remaining_sec = (remaining_ms / 1000).max(0);
                    list.push(serde_json::json!({
                        "name": name,
                        "duration_seconds": timer.duration_sec,
                        "remaining_seconds": remaining_sec,
                        "is_expired": remaining_ms <= 0
                    }));
                }
                list.sort_by(|a, b| {
                    a["name"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["name"].as_str().unwrap_or(""))
                });

                Ok(serde_json::json!({
                    "action": "list",
                    "count": list.len(),
                    "timers": list
                }))
            }

            other => Err(OrchestratorError::InvalidToolArguments {
                tool_name: "system.timer".to_string(),
                reason: format!("Unknown timer action: '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_lifecycle() {
        let skill = TimerSkill::new();
        assert_eq!(skill.risk_tier(), RiskTier::StateModifying);

        // 1. Set timer
        let set_res = skill
            .execute(serde_json::json!({
                "action": "set",
                "name": "tea",
                "duration_seconds": 180
            }))
            .unwrap();
        assert_eq!(set_res["status"], "running");
        assert_eq!(set_res["remaining_seconds"], 180);

        // 2. Query status
        let status_res = skill
            .execute(serde_json::json!({
                "action": "status",
                "name": "tea"
            }))
            .unwrap();
        assert_eq!(status_res["status"], "running");
        assert!(!status_res["is_expired"].as_bool().unwrap());

        // 3. List
        let list_res = skill
            .execute(serde_json::json!({ "action": "list" }))
            .unwrap();
        assert_eq!(list_res["count"], 1);

        // 4. Cancel
        let cancel_res = skill
            .execute(serde_json::json!({
                "action": "cancel",
                "name": "tea"
            }))
            .unwrap();
        assert!(cancel_res["cancelled"].as_bool().unwrap());

        // 5. Query after cancel should fail
        assert!(skill
            .execute(serde_json::json!({
                "action": "status",
                "name": "tea"
            }))
            .is_err());
    }
}
