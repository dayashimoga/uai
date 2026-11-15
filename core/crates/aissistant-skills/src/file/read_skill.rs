//! Safe file reading skill (`file.read`).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

use super::jail::PathJail;

const DEFAULT_READ_BYTES: usize = 64 * 1024; // 64 KB
const MAX_READ_BYTES: usize = 1024 * 1024; // 1 MB cap to prevent OOM

/// Safe file reader skill.
#[derive(Debug, Clone)]
pub struct FileReadSkill {
    jail: Arc<PathJail>,
}

impl Default for FileReadSkill {
    fn default() -> Self {
        Self {
            jail: Arc::new(PathJail::default()),
        }
    }
}

impl FileReadSkill {
    /// Creates a new `FileReadSkill` using default jail constraints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `FileReadSkill` with customized path jail.
    #[must_use]
    pub fn with_jail(jail: Arc<PathJail>) -> Self {
        Self { jail }
    }
}

impl ToolHandler for FileReadSkill {
    fn name(&self) -> &str {
        "file.read"
    }

    fn description(&self) -> &str {
        "Reads file contents safely with bounded byte buffers (maximum 1 MB per call) and optional pagination offsets."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path of the file to read."
                },
                "offset": {
                    "type": "integer",
                    "description": "Byte offset to begin reading from (defaults to 0)."
                },
                "max_bytes": {
                    "type": "integer",
                    "description": "Maximum number of bytes to read (default 65536, maximum 1048576)."
                },
                "encoding": {
                    "type": "string",
                    "enum": ["utf8", "hex"],
                    "description": "Encoding format for returned content (defaults to 'utf8')."
                }
            },
            "required": ["path"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let raw_path = arguments
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.read".to_string(),
                reason: "Missing required 'path' argument".to_string(),
            })?;

        let offset = arguments.get("offset").and_then(Value::as_u64).unwrap_or(0);

        let max_bytes = arguments
            .get("max_bytes")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_READ_BYTES, |v| (v as usize).min(MAX_READ_BYTES));

        let encoding = arguments
            .get("encoding")
            .and_then(Value::as_str)
            .unwrap_or("utf8");

        let canonical_path = self
            .jail
            .validate_existing(raw_path)
            .map_err(|err| err.to_orchestrator_error())?;

        if canonical_path.is_dir() {
            return Err(OrchestratorError::InvalidToolArguments {
                tool_name: "file.read".to_string(),
                reason: format!("Target path is a directory, not a file: '{raw_path}'"),
            });
        }

        let mut file =
            File::open(&canonical_path).map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.read".to_string(),
                reason: format!("Failed to open file: {e}"),
            })?;

        let total_size = file.metadata().map(|m| m.len()).unwrap_or(0);

        if offset < total_size {
            file.seek(SeekFrom::Start(offset)).map_err(|e| {
                OrchestratorError::ToolExecutionFailed {
                    tool_name: "file.read".to_string(),
                    reason: format!("Seek failed: {e}"),
                }
            })?;
        }

        let mut buffer = vec![0u8; max_bytes];
        let bytes_read =
            file.read(&mut buffer)
                .map_err(|e| OrchestratorError::ToolExecutionFailed {
                    tool_name: "file.read".to_string(),
                    reason: format!("Read failed: {e}"),
                })?;

        buffer.truncate(bytes_read);
        let is_eof = (offset + bytes_read as u64) >= total_size;

        let content_str = match encoding {
            "hex" => buffer
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            _ => String::from_utf8_lossy(&buffer).to_string(),
        };

        Ok(serde_json::json!({
            "path": canonical_path.to_string_lossy(),
            "total_size_bytes": total_size,
            "offset": offset,
            "bytes_read": bytes_read,
            "is_eof": is_eof,
            "content": content_str,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_file_read_skill_basic_and_pagination() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"Hello world from safe file reader!")
            .unwrap();

        let skill = FileReadSkill::new();
        let res = skill
            .execute(serde_json::json!({
                "path": temp.path().to_str().unwrap(),
                "offset": 0,
                "max_bytes": 11
            }))
            .unwrap();

        assert_eq!(res["bytes_read"], 11);
        assert_eq!(res["content"], "Hello world");
        assert!(!res["is_eof"].as_bool().unwrap());

        // Read remaining
        let res2 = skill
            .execute(serde_json::json!({
                "path": temp.path().to_str().unwrap(),
                "offset": 11,
                "max_bytes": 100
            }))
            .unwrap();

        assert_eq!(res2["content"], " from safe file reader!");
        assert!(res2["is_eof"].as_bool().unwrap());
    }
}
