//! Desktop application discovery and launch skills (`app.list` and `app.launch`).

use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_platform::DesktopPlatformAdapter;
use aissistant_security::RiskTier;
use serde_json::Value;

/// Skill for listing registered desktop applications.
#[derive(Debug, Clone)]
pub struct AppListSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl AppListSkill {
    /// Creates a new `AppListSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for AppListSkill {
    fn name(&self) -> &str {
        "app.list"
    }

    fn description(&self) -> &str {
        "Lists installed desktop applications discovered on the host operating system."
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
        let apps =
            self.adapter
                .list_apps()
                .map_err(|e| OrchestratorError::ToolExecutionFailed {
                    tool_name: "app.list".to_string(),
                    reason: e.to_string(),
                })?;

        let items: Vec<Value> = apps
            .into_iter()
            .map(|a| {
                serde_json::json!({
                    "id": a.id,
                    "name": a.name,
                    "executable_path": a.executable_path,
                    "description": a.description,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "count": items.len(),
            "apps": items,
        }))
    }
}

/// Skill for launching a desktop application without shell invocation.
#[derive(Debug, Clone)]
pub struct AppLaunchSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl AppLaunchSkill {
    /// Creates a new `AppLaunchSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for AppLaunchSkill {
    fn name(&self) -> &str {
        "app.launch"
    }

    fn description(&self) -> &str {
        "Spawns a desktop application executable directly with arguments (strictly forbids shell wrappers)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "executable": {
                    "type": "string",
                    "description": "Path or executable name to launch directly."
                },
                "arguments": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of command line arguments."
                }
            },
            "required": ["executable"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::FileAndApp
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let executable = arguments
            .get("executable")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "app.launch".to_string(),
                reason: "Missing required 'executable' argument".to_string(),
            })?;

        let args: Vec<String> = arguments
            .get("arguments")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(ToString::to_string)
                    .collect()
            })
            .unwrap_or_default();

        let pid = self.adapter.launch_app(executable, &args).map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "app.launch".to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(serde_json::json!({
            "executable": executable,
            "pid": pid,
            "launched": true,
        }))
    }
}
