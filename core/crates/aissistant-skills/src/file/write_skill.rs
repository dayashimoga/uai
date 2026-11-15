//! Safe file writing skill (`file.write`).

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

use super::jail::PathJail;

/// Safe file writing skill supporting atomic write, append, and create-new modes.
#[derive(Debug, Clone)]
pub struct FileWriteSkill {
    jail: Arc<PathJail>,
}

impl Default for FileWriteSkill {
    fn default() -> Self {
        Self {
            jail: Arc::new(PathJail::default()),
        }
    }
}

impl FileWriteSkill {
    /// Creates a new `FileWriteSkill` with default jail constraints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `FileWriteSkill` with a customized path jail.
    #[must_use]
    pub fn with_jail(jail: Arc<PathJail>) -> Self {
        Self { jail }
    }
}

impl ToolHandler for FileWriteSkill {
    fn name(&self) -> &str {
        "file.write"
    }

    fn description(&self) -> &str {
        "Writes text content to a destination file with overwrite, append, or create_new modes."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Destination file path to write to."
                },
                "content": {
                    "type": "string",
                    "description": "Text content to write into the file."
                },
                "mode": {
                    "type": "string",
                    "enum": ["overwrite", "append", "create_new"],
                    "description": "Write mode: 'overwrite' (replace), 'append' (add to end), or 'create_new' (fails if exists)."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::FileAndApp
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let raw_path = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.write".to_string(),
                reason: "Missing required 'path' argument".to_string(),
            })?;

        let content = arguments
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.write".to_string(),
                reason: "Missing required 'content' argument".to_string(),
            })?;

        let mode = arguments
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("overwrite");

        let canonical_target = self
            .jail
            .validate_for_write(raw_path)
            .map_err(|err| err.to_orchestrator_error())?;

        let file_existed_before = canonical_target.exists();

        let mut options = OpenOptions::new();
        options.write(true);

        match mode {
            "append" => {
                options.create(true).append(true);
            }
            "create_new" => {
                options.create_new(true);
            }
            _ => {
                // "overwrite"
                options.create(true).truncate(true);
            }
        }

        let mut file = options.open(&canonical_target).map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "file.write".to_string(),
                reason: format!("Failed to open file for writing: {e}"),
            }
        })?;

        file.write_all(content.as_bytes())
            .map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.write".to_string(),
                reason: format!("Failed to write content: {e}"),
            })?;

        file.flush()
            .map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.write".to_string(),
                reason: format!("Failed to flush file: {e}"),
            })?;

        Ok(serde_json::json!({
            "path": canonical_target.to_string_lossy(),
            "bytes_written": content.len(),
            "mode": mode,
            "created": !file_existed_before,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_file_write_skill_modes() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("output.txt");
        let target_str = target.to_str().unwrap();

        let skill = FileWriteSkill::new();

        // 1. Overwrite creates file
        let res1 = skill
            .execute(serde_json::json!({
                "path": target_str,
                "content": "First line\n",
                "mode": "overwrite"
            }))
            .unwrap();
        assert!(res1["created"].as_bool().unwrap());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "First line\n");

        // 2. Append adds to end
        let res2 = skill
            .execute(serde_json::json!({
                "path": target_str,
                "content": "Second line\n",
                "mode": "append"
            }))
            .unwrap();
        assert!(!res2["created"].as_bool().unwrap());
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "First line\nSecond line\n"
        );

        // 3. Create_new fails on existing
        let res3 = skill.execute(serde_json::json!({
            "path": target_str,
            "content": "Third line\n",
            "mode": "create_new"
        }));
        assert!(res3.is_err());
    }
}
