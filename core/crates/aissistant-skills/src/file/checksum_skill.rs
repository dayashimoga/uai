//! Safe file checksum calculation skill (`file.checksum`).

use std::fs::File;
use std::io::Read;
use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::jail::PathJail;

const BUFFER_SIZE: usize = 64 * 1024; // 64 KB read buffer

/// Cryptographic checksum calculation skill (BLAKE3 and SHA-256).
#[derive(Debug, Clone)]
pub struct FileChecksumSkill {
    jail: Arc<PathJail>,
}

impl Default for FileChecksumSkill {
    fn default() -> Self {
        Self {
            jail: Arc::new(PathJail::default()),
        }
    }
}

impl FileChecksumSkill {
    /// Creates a new `FileChecksumSkill` with default jail constraints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `FileChecksumSkill` with a customized path jail.
    #[must_use]
    pub fn with_jail(jail: Arc<PathJail>) -> Self {
        Self { jail }
    }
}

impl ToolHandler for FileChecksumSkill {
    fn name(&self) -> &str {
        "file.checksum"
    }

    fn description(&self) -> &str {
        "Computes cryptographic hash digests (BLAKE3 or SHA-256) for verified file integrity."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the target file."
                },
                "algorithm": {
                    "type": "string",
                    "enum": ["blake3", "sha256"],
                    "description": "Cryptographic hash algorithm: 'blake3' (fast, default) or 'sha256'."
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
                tool_name: "file.checksum".to_string(),
                reason: "Missing required 'path' argument".to_string(),
            })?;

        let algorithm = arguments
            .get("algorithm")
            .and_then(Value::as_str)
            .unwrap_or("blake3");

        let canonical_path = self
            .jail
            .validate_existing(raw_path)
            .map_err(|err| err.to_orchestrator_error())?;

        if canonical_path.is_dir() {
            return Err(OrchestratorError::InvalidToolArguments {
                tool_name: "file.checksum".to_string(),
                reason: format!("Target path is a directory, not a file: '{raw_path}'"),
            });
        }

        let mut file =
            File::open(&canonical_path).map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.checksum".to_string(),
                reason: format!("Failed to open file: {e}"),
            })?;

        let total_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut buffer = [0u8; BUFFER_SIZE];

        let digest_hex = match algorithm {
            "sha256" => {
                let mut hasher = Sha256::new();
                loop {
                    let bytes = file.read(&mut buffer).map_err(|e| {
                        OrchestratorError::ToolExecutionFailed {
                            tool_name: "file.checksum".to_string(),
                            reason: format!("Read failure: {e}"),
                        }
                    })?;
                    if bytes == 0 {
                        break;
                    }
                    hasher.update(&buffer[..bytes]);
                }
                format!("{:x}", hasher.finalize())
            }
            _ => {
                // "blake3"
                let mut hasher = blake3::Hasher::new();
                loop {
                    let bytes = file.read(&mut buffer).map_err(|e| {
                        OrchestratorError::ToolExecutionFailed {
                            tool_name: "file.checksum".to_string(),
                            reason: format!("Read failure: {e}"),
                        }
                    })?;
                    if bytes == 0 {
                        break;
                    }
                    hasher.update(&buffer[..bytes]);
                }
                hasher.finalize().to_hex().to_string()
            }
        };

        Ok(serde_json::json!({
            "path": canonical_path.to_string_lossy(),
            "algorithm": algorithm,
            "digest_hex": digest_hex,
            "file_size_bytes": total_size,
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
    fn test_file_checksum_blake3_and_sha256() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"Hello checksum!").unwrap();

        let skill = FileChecksumSkill::new();

        // BLAKE3
        let res_blake3 = skill
            .execute(serde_json::json!({
                "path": temp.path().to_str().unwrap(),
                "algorithm": "blake3"
            }))
            .unwrap();

        let expected_blake3 = blake3::hash(b"Hello checksum!").to_hex().to_string();
        assert_eq!(res_blake3["digest_hex"], expected_blake3);

        // SHA-256
        let res_sha256 = skill
            .execute(serde_json::json!({
                "path": temp.path().to_str().unwrap(),
                "algorithm": "sha256"
            }))
            .unwrap();

        let mut hasher = Sha256::new();
        hasher.update(b"Hello checksum!");
        let expected_sha256 = format!("{:x}", hasher.finalize());
        assert_eq!(res_sha256["digest_hex"], expected_sha256);
    }
}
