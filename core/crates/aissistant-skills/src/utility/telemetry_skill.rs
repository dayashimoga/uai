//! Built-in cross-platform host system telemetry skill (`system.telemetry`).

use std::sync::Mutex;
use std::time::Instant;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;
use sysinfo::System;

/// Tool querying host system memory headroom, CPU core utilization, and OS details.
pub struct TelemetrySkill {
    system: Mutex<System>,
    last_refresh: Mutex<Instant>,
}

impl std::fmt::Debug for TelemetrySkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetrySkill").finish()
    }
}

impl Default for TelemetrySkill {
    fn default() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        sys.refresh_cpu_usage();
        Self {
            system: Mutex::new(sys),
            last_refresh: Mutex::new(Instant::now()),
        }
    }
}

impl TelemetrySkill {
    /// Creates a new `TelemetrySkill` instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ToolHandler for TelemetrySkill {
    fn name(&self) -> &str {
        "system.telemetry"
    }

    fn description(&self) -> &str {
        "Returns host operating system status, available RAM headroom, and CPU utilization."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "metric": {
                    "type": "string",
                    "enum": ["all", "memory", "cpu", "host"],
                    "description": "Optional telemetry category filter."
                }
            }
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let mut sys = self
            .system
            .lock()
            .map_err(|_| OrchestratorError::Internal("TelemetrySkill lock poisoned".to_string()))?;

        // Throttle refreshes to at most once per 200ms to prevent CPU spikes
        let should_refresh = {
            let mut last = self.last_refresh.lock().map_err(|_| {
                OrchestratorError::Internal("TelemetrySkill timer lock poisoned".to_string())
            })?;
            if last.elapsed().as_millis() > 200 {
                *last = Instant::now();
                true
            } else {
                false
            }
        };

        if should_refresh {
            sys.refresh_memory();
            sys.refresh_cpu_usage();
        }

        let metric = arguments
            .get("metric")
            .and_then(Value::as_str)
            .unwrap_or("all");

        let total_mem = sys.total_memory();
        let available_mem = sys.available_memory();
        let used_mem = sys.used_memory();
        let mem_usage_percent = if total_mem > 0 {
            (used_mem as f64 / total_mem as f64) * 100.0
        } else {
            0.0
        };

        let cpu_cores = sys.cpus().len();
        let cpu_global_usage = sys.global_cpu_usage();

        let host_name = System::host_name().unwrap_or_else(|| "unknown-host".to_string());
        let os_name = System::name().unwrap_or_else(|| "unknown-os".to_string());
        let os_version = System::os_version().unwrap_or_else(|| "unknown-version".to_string());

        let payload = match metric {
            "memory" => serde_json::json!({
                "total_bytes": total_mem,
                "available_bytes": available_mem,
                "used_bytes": used_mem,
                "usage_percent": format!("{:.1}%", mem_usage_percent),
                "total_mb": total_mem / (1024 * 1024),
                "available_mb": available_mem / (1024 * 1024)
            }),
            "cpu" => serde_json::json!({
                "logical_cores": cpu_cores,
                "global_usage_percent": format!("{:.1}%", cpu_global_usage)
            }),
            "host" => serde_json::json!({
                "host_name": host_name,
                "os": os_name,
                "version": os_version
            }),
            _ => serde_json::json!({
                "host": {
                    "host_name": host_name,
                    "os": os_name,
                    "version": os_version
                },
                "memory": {
                    "total_bytes": total_mem,
                    "available_bytes": available_mem,
                    "used_bytes": used_mem,
                    "usage_percent": format!("{:.1}%", mem_usage_percent),
                    "total_mb": total_mem / (1024 * 1024),
                    "available_mb": available_mem / (1024 * 1024)
                },
                "cpu": {
                    "logical_cores": cpu_cores,
                    "global_usage_percent": format!("{:.1}%", cpu_global_usage)
                }
            }),
        };

        Ok(payload)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_skill_live_probe() {
        let skill = TelemetrySkill::new();
        assert_eq!(skill.name(), "system.telemetry");
        assert_eq!(skill.risk_tier(), RiskTier::ReadOnly);

        let res = skill.execute(serde_json::json!({})).unwrap();
        assert!(res["memory"]["total_bytes"].as_u64().unwrap() > 0);
        assert!(res["cpu"]["logical_cores"].as_u64().unwrap() > 0);
        assert!(res["host"]["os"].as_str().is_some());

        let mem_only = skill
            .execute(serde_json::json!({ "metric": "memory" }))
            .unwrap();
        assert!(mem_only["total_bytes"].as_u64().unwrap() > 0);
        assert!(mem_only.get("cpu").is_none());
    }
}
