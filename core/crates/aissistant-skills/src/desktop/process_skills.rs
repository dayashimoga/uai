//! Operating system process inspection and lifecycle termination skills.

use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_platform::DesktopPlatformAdapter;
use aissistant_security::RiskTier;
use serde_json::Value;

/// Skill for enumerating active operating system processes.
#[derive(Debug, Clone)]
pub struct ProcessListSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl ProcessListSkill {
    /// Creates a new `ProcessListSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for ProcessListSkill {
    fn name(&self) -> &str {
        "process.list"
    }

    fn description(&self) -> &str {
        "Enumerates running processes with process IDs, memory footprint, and CPU usage metrics."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Optional name filter for process query."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of processes to return (default 50, maximum 200)."
                }
            }
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .map(|q| q.to_lowercase());

        let max_results = arguments
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(50)
            .min(200) as usize;

        let procs =
            self.adapter
                .list_processes()
                .map_err(|e| OrchestratorError::ToolExecutionFailed {
                    tool_name: "process.list".to_string(),
                    reason: e.to_string(),
                })?;

        let filtered: Vec<Value> = procs
            .into_iter()
            .filter(|p| {
                if let Some(ref q) = query {
                    p.name.to_lowercase().contains(q)
                } else {
                    true
                }
            })
            .take(max_results)
            .map(|p| {
                serde_json::json!({
                    "pid": p.pid,
                    "name": p.name,
                    "cpu_usage_percent": format!("{:.1}%", p.cpu_usage_percent),
                    "memory_bytes": p.memory_bytes,
                    "memory_mb": p.memory_bytes / (1024 * 1024),
                    "executable_path": p.executable_path,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "count": filtered.len(),
            "processes": filtered,
        }))
    }
}

/// Skill for terminating an active operating system process with mandatory confirmation.
#[derive(Debug, Clone)]
pub struct ProcessKillSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl ProcessKillSkill {
    /// Creates a new `ProcessKillSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for ProcessKillSkill {
    fn name(&self) -> &str {
        "process.kill"
    }

    fn description(&self) -> &str {
        "Terminates an active process by PID (destructive operation requiring explicit user confirmation)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pid": {
                    "type": "integer",
                    "description": "Process ID (PID) of the process to terminate."
                }
            },
            "required": ["pid"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::Destructive
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let pid = arguments
            .get("pid")
            .and_then(Value::as_u64)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "process.kill".to_string(),
                reason: "Missing required 'pid' argument".to_string(),
            })? as u32;

        self.adapter
            .kill_process(pid)
            .map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "process.kill".to_string(),
                reason: e.to_string(),
            })?;

        Ok(serde_json::json!({
            "pid": pid,
            "killed": true,
        }))
    }
}
