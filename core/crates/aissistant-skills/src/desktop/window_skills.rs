//! Desktop application window inspection and focus skills.

use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_platform::DesktopPlatformAdapter;
use aissistant_security::RiskTier;
use serde_json::Value;

/// Skill for listing top-level visible desktop application windows.
#[derive(Debug, Clone)]
pub struct WindowListSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl WindowListSkill {
    /// Creates a new `WindowListSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for WindowListSkill {
    fn name(&self) -> &str {
        "window.list"
    }

    fn description(&self) -> &str {
        "Enumerates visible desktop windows with handles, titles, owning process IDs, and bounding geometry."
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
        let windows =
            self.adapter
                .list_windows()
                .map_err(|e| OrchestratorError::ToolExecutionFailed {
                    tool_name: "window.list".to_string(),
                    reason: e.to_string(),
                })?;

        let items: Vec<Value> = windows
            .into_iter()
            .map(|w| {
                serde_json::json!({
                    "id": w.id,
                    "title": w.title,
                    "pid": w.pid,
                    "process_name": w.process_name,
                    "is_minimized": w.is_minimized,
                    "bounds": {
                        "x": w.bounds.x,
                        "y": w.bounds.y,
                        "width": w.bounds.width,
                        "height": w.bounds.height,
                    }
                })
            })
            .collect();

        Ok(serde_json::json!({
            "count": items.len(),
            "windows": items,
        }))
    }
}

/// Skill for focusing and foregrounding an application window.
#[derive(Debug, Clone)]
pub struct WindowFocusSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl WindowFocusSkill {
    /// Creates a new `WindowFocusSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for WindowFocusSkill {
    fn name(&self) -> &str {
        "window.focus"
    }

    fn description(&self) -> &str {
        "Brings the specified application window to the foreground and assigns input focus."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "window_id": {
                    "type": "integer",
                    "description": "Unique numeric handle or identifier of the window to focus."
                }
            },
            "required": ["window_id"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::StateModifying
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let window_id = arguments
            .get("window_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "window.focus".to_string(),
                reason: "Missing required 'window_id' argument".to_string(),
            })?;

        self.adapter.focus_window(window_id).map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "window.focus".to_string(),
                reason: e.to_string(),
            }
        })?;

        Ok(serde_json::json!({
            "window_id": window_id,
            "focused": true,
        }))
    }
}
