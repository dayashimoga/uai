//! Built-in safe system time, date, and timezone query skill (`system.time`).

use std::time::{SystemTime, UNIX_EPOCH};

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

/// Tool querying current system time, UTC timestamp, and formatted date.
#[derive(Debug, Default, Clone, Copy)]
pub struct TimeSkill;

impl TimeSkill {
    /// Creates a new `TimeSkill` instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ToolHandler for TimeSkill {
    fn name(&self) -> &str {
        "system.time"
    }

    fn description(&self) -> &str {
        "Returns the current date, time, and UTC timestamp from the system clock."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": ["full", "date_only", "time_only", "epoch_ms"],
                    "description": "Optional format specifier for desired time output representation."
                }
            }
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let now = SystemTime::now();
        let duration = now
            .duration_since(UNIX_EPOCH)
            .map_err(|e| OrchestratorError::Internal(format!("Clock went backwards: {e}")))?;

        let epoch_ms = duration.as_millis() as i64;
        let epoch_sec = duration.as_secs() as i64;

        let days = epoch_sec / 86400;
        let rem_sec = epoch_sec % 86400;
        let hours = (rem_sec / 3600) as u32;
        let minutes = ((rem_sec % 3600) / 60) as u32;
        let seconds = (rem_sec % 60) as u32;

        let (year, month, day) = civil_from_days(days);
        let iso8601 =
            format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z");
        let date_str = format!("{year:04}-{month:02}-{day:02}");
        let time_str = format!("{hours:02}:{minutes:02}:{seconds:02} UTC");

        let format_choice = arguments
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("full");

        let payload = match format_choice {
            "date_only" => serde_json::json!({ "date": date_str }),
            "time_only" => serde_json::json!({ "time": time_str }),
            "epoch_ms" => serde_json::json!({ "epoch_ms": epoch_ms }),
            _ => serde_json::json!({
                "iso8601": iso8601,
                "date": date_str,
                "time": time_str,
                "epoch_ms": epoch_ms,
                "timezone": "UTC"
            }),
        };

        Ok(payload)
    }
}

/// Computes (year, month, day) from days since 1970-01-01 using Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1020 + doe / 1461 - doe / 36524) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_civil_from_days() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2026-11-15 is 20772 days since epoch
        assert_eq!(civil_from_days(20772), (2026, 11, 15));
    }

    #[test]
    fn test_time_skill_execution() {
        let skill = TimeSkill::new();
        assert_eq!(skill.name(), "system.time");
        assert_eq!(skill.risk_tier(), RiskTier::ReadOnly);

        let res = skill.execute(serde_json::json!({})).unwrap();
        assert!(res.get("iso8601").is_some());
        assert!(res.get("epoch_ms").is_some());

        let date_only = skill
            .execute(serde_json::json!({"format": "date_only"}))
            .unwrap();
        assert!(date_only.get("date").is_some());
        assert!(date_only.get("iso8601").is_none());
    }
}
