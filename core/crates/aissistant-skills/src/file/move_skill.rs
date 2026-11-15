//! Safe file moving and renaming skill (`file.move`).

use std::fs;
use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

use super::jail::PathJail;

/// Safe file moving and renaming skill.
#[derive(Debug, Clone)]
pub struct FileMoveSkill {
    jail: Arc<PathJail>,
}

impl Default for FileMoveSkill {
    fn default() -> Self {
        Self {
            jail: Arc::new(PathJail::default()),
        }
    }
}

impl FileMoveSkill {
    /// Creates a new `FileMoveSkill` with default jail constraints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `FileMoveSkill` with a customized path jail.
    #[must_use]
    pub fn with_jail(jail: Arc<PathJail>) -> Self {
        Self { jail }
    }
}

impl ToolHandler for FileMoveSkill {
    fn name(&self) -> &str {
        "file.move"
    }

    fn description(&self) -> &str {
        "Moves or renames a source file or directory to a destination path."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "Path of the existing file or directory to move."
                },
                "destination_path": {
                    "type": "string",
                    "description": "Target destination path."
                },
                "overwrite": {
                    "type": "boolean",
                    "description": "Whether to overwrite an existing destination file (default false)."
                }
            },
            "required": ["source_path", "destination_path"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::FileAndApp
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let raw_src = arguments
            .get("source_path")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.move".to_string(),
                reason: "Missing required 'source_path' argument".to_string(),
            })?;

        let raw_dst = arguments
            .get("destination_path")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.move".to_string(),
                reason: "Missing required 'destination_path' argument".to_string(),
            })?;

        let overwrite = arguments
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let canonical_src = self
            .jail
            .validate_existing(raw_src)
            .map_err(|err| err.to_orchestrator_error())?;

        let canonical_dst = self
            .jail
            .validate_for_write(raw_dst)
            .map_err(|err| err.to_orchestrator_error())?;

        if canonical_dst.exists() && !overwrite {
            return Err(OrchestratorError::InvalidToolArguments {
                tool_name: "file.move".to_string(),
                reason: format!(
                    "Destination already exists and 'overwrite' is false: '{}'",
                    canonical_dst.display()
                ),
            });
        }

        let size_bytes = fs::metadata(&canonical_src).map(|m| m.len()).unwrap_or(0);

        fs::rename(&canonical_src, &canonical_dst).map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "file.move".to_string(),
                reason: format!("Failed to move/rename file: {e}"),
            }
        })?;

        Ok(serde_json::json!({
            "source_path": canonical_src.to_string_lossy(),
            "destination_path": canonical_dst.to_string_lossy(),
            "size_bytes": size_bytes,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_file_move_skill() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("source.txt");
        let dst = dir.path().join("dest.txt");

        fs::write(&src, b"movable data").unwrap();

        let skill = FileMoveSkill::new();
        let res = skill
            .execute(serde_json::json!({
                "source_path": src.to_str().unwrap(),
                "destination_path": dst.to_str().unwrap()
            }))
            .unwrap();

        assert!(!src.exists());
        assert!(dst.exists());
        assert_eq!(res["size_bytes"], 12);
    }
}
