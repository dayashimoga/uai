//! Desktop clipboard management skills (`clipboard.read` and `clipboard.write`).

use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_platform::DesktopPlatformAdapter;
use aissistant_security::RiskTier;
use serde_json::Value;

/// Skill for reading the system clipboard.
#[derive(Debug, Clone)]
pub struct ClipboardReadSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl ClipboardReadSkill {
    /// Creates a new `ClipboardReadSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for ClipboardReadSkill {
    fn name(&self) -> &str {
        "clipboard.read"
    }

    fn description(&self) -> &str {
        "Reads text currently stored in the system clipboard."
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
        let text =
            self.adapter
                .read_clipboard()
                .map_err(|e| OrchestratorError::ToolExecutionFailed {
                    tool_name: "clipboard.read".to_string(),
                    reason: e.to_string(),
                })?;

        Ok(serde_json::json!({
            "text": text,
            "length": text.len(),
        }))
    }
}

/// Skill for writing text to the system clipboard.
#[derive(Debug, Clone)]
pub struct ClipboardWriteSkill {
    adapter: Arc<dyn DesktopPlatformAdapter>,
}

impl ClipboardWriteSkill {
    /// Creates a new `ClipboardWriteSkill` with the provided platform adapter.
    #[must_use]
    pub fn new(adapter: Arc<dyn DesktopPlatformAdapter>) -> Self {
        Self { adapter }
    }
}

impl ToolHandler for ClipboardWriteSkill {
    fn name(&self) -> &str {
        "clipboard.write"
    }

    fn description(&self) -> &str {
        "Writes or replaces text in the system clipboard."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text content to copy to the clipboard"
                }
            },
            "required": ["text"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::StateModifying
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let text = arguments
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| OrchestratorError::ToolExecutionFailed {
                tool_name: "clipboard.write".to_string(),
                reason: "Missing required string argument 'text'".to_string(),
            })?;

        self.adapter
            .write_clipboard(text)
            .map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "clipboard.write".to_string(),
                reason: e.to_string(),
            })?;

        Ok(serde_json::json!({
            "success": true,
            "bytes_written": text.len(),
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_platform::VirtualDesktopAdapter;

    #[test]
    fn test_clipboard_read_and_write_skills() {
        let adapter = Arc::new(VirtualDesktopAdapter::new());
        let read_skill = ClipboardReadSkill::new(adapter.clone());
        let write_skill = ClipboardWriteSkill::new(adapter);

        assert_eq!(read_skill.risk_tier(), RiskTier::ReadOnly);
        assert_eq!(write_skill.risk_tier(), RiskTier::StateModifying);

        let initial = read_skill.execute(serde_json::json!({})).unwrap();
        assert_eq!(initial["text"], "initial virtual clipboard text");

        let write_res = write_skill
            .execute(serde_json::json!({ "text": "new hello world text" }))
            .unwrap();
        assert_eq!(write_res["success"], true);

        let updated = read_skill.execute(serde_json::json!({})).unwrap();
        assert_eq!(updated["text"], "new hello world text");
    }
}
