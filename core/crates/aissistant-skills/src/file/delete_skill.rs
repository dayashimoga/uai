//! Safe file deletion skill (`file.delete`).

use std::fs;
use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

use super::jail::PathJail;

/// Irreversible file and directory deletion skill.
#[derive(Debug, Clone)]
pub struct FileDeleteSkill {
    jail: Arc<PathJail>,
}

impl Default for FileDeleteSkill {
    fn default() -> Self {
        Self {
            jail: Arc::new(PathJail::default()),
        }
    }
}

impl FileDeleteSkill {
    /// Creates a new `FileDeleteSkill` with default jail constraints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `FileDeleteSkill` with a customized path jail.
    #[must_use]
    pub fn with_jail(jail: Arc<PathJail>) -> Self {
        Self { jail }
    }
}

impl ToolHandler for FileDeleteSkill {
    fn name(&self) -> &str {
        "file.delete"
    }

    fn description(&self) -> &str {
        "Permanently deletes a file or directory within permitted boundaries (destructive operation)."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path of the file or directory to delete permanently."
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to delete directories recursively (default false; required if directory is not empty)."
                }
            },
            "required": ["path"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::Destructive
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let raw_path = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.delete".to_string(),
                reason: "Missing required 'path' argument".to_string(),
            })?;

        let recursive = arguments
            .get("recursive")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let canonical_path = self
            .jail
            .validate_existing(raw_path)
            .map_err(|err| err.to_orchestrator_error())?;

        let is_dir = canonical_path.is_dir();

        if is_dir {
            if recursive {
                fs::remove_dir_all(&canonical_path).map_err(|e| {
                    OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.delete".to_string(),
                        reason: format!("Failed to recursively remove directory: {e}"),
                    }
                })?;
            } else {
                fs::remove_dir(&canonical_path).map_err(|e| {
                    OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.delete".to_string(),
                        reason: format!("Failed to remove directory (may be non-empty, set 'recursive: true'): {e}"),
                    }
                })?;
            }
        } else {
            fs::remove_file(&canonical_path).map_err(|e| {
                OrchestratorError::ToolExecutionFailed {
                    tool_name: "file.delete".to_string(),
                    reason: format!("Failed to remove file: {e}"),
                }
            })?;
        }

        Ok(serde_json::json!({
            "path": canonical_path.to_string_lossy(),
            "is_dir": is_dir,
            "deleted": true,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_file_delete_skill() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("to_delete.txt");
        fs::write(&target, b"delete me").unwrap();

        let skill = FileDeleteSkill::new();
        assert_eq!(skill.risk_tier(), RiskTier::Destructive);

        let res = skill
            .execute(serde_json::json!({
                "path": target.to_str().unwrap()
            }))
            .unwrap();

        assert!(!target.exists());
        assert!(res["deleted"].as_bool().unwrap());
    }
}
