//! Safe file search skill (`file.search`).

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

use super::jail::PathJail;

/// Skill for safely searching files and directories.
#[derive(Debug, Clone)]
pub struct FileSearchSkill {
    jail: Arc<PathJail>,
}

impl Default for FileSearchSkill {
    fn default() -> Self {
        Self {
            jail: Arc::new(PathJail::default()),
        }
    }
}

impl FileSearchSkill {
    /// Creates a new `FileSearchSkill` using the default jail.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `FileSearchSkill` with a customized path jail.
    #[must_use]
    pub fn with_jail(jail: Arc<PathJail>) -> Self {
        Self { jail }
    }
}

impl ToolHandler for FileSearchSkill {
    fn name(&self) -> &str {
        "file.search"
    }

    fn description(&self) -> &str {
        "Searches for files and directories matching a query pattern within a validated directory path."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Root directory path to begin the search (defaults to current directory '.')."
                },
                "query": {
                    "type": "string",
                    "description": "Substring or filename pattern to search for (case-insensitive)."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of search results to return (default 50, maximum 200)."
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to search recursively through subdirectories (default true)."
                },
                "include_hidden": {
                    "type": "boolean",
                    "description": "Whether to include hidden files or folders starting with '.' (default false)."
                }
            },
            "required": ["query"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let query = arguments
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.search".to_string(),
                reason: "Missing required 'query' argument".to_string(),
            })?
            .to_lowercase();

        let raw_root = arguments.get("path").and_then(Value::as_str).unwrap_or(".");

        let max_results = arguments
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(50)
            .min(200) as usize;

        let recursive = arguments
            .get("recursive")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        let include_hidden = arguments
            .get("include_hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let canonical_root = self
            .jail
            .validate_existing(raw_root)
            .map_err(|err| err.to_orchestrator_error())?;

        if !canonical_root.is_dir() {
            return Err(OrchestratorError::InvalidToolArguments {
                tool_name: "file.search".to_string(),
                reason: format!("Target path is not a directory: '{raw_root}'"),
            });
        }

        let mut matches = Vec::new();
        let mut truncated = false;

        let criteria = SearchCriteria {
            query: &query,
            recursive,
            include_hidden,
            max_results,
        };

        self.walk_dir(&canonical_root, criteria, &mut matches, &mut truncated)?;

        Ok(serde_json::json!({
            "root_path": canonical_root.to_string_lossy(),
            "query": query,
            "count": matches.len(),
            "truncated": truncated,
            "matches": matches,
        }))
    }
}

#[derive(Debug, Clone, Copy)]
struct SearchCriteria<'a> {
    query: &'a str,
    recursive: bool,
    include_hidden: bool,
    max_results: usize,
}

impl FileSearchSkill {
    fn walk_dir(
        &self,
        dir: &Path,
        criteria: SearchCriteria<'_>,
        matches: &mut Vec<Value>,
        truncated: &mut bool,
    ) -> Result<(), OrchestratorError> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()), // Skip unreadable directories
        };

        for entry in entries.flatten() {
            if matches.len() >= criteria.max_results {
                *truncated = true;
                return Ok(());
            }

            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();

            if !criteria.include_hidden && name_str.starts_with('.') {
                continue;
            }

            let path = entry.path();
            let is_dir = path.is_dir();

            if name_str.to_lowercase().contains(criteria.query) {
                let metadata = entry.metadata().ok();
                let size_bytes = metadata.as_ref().map_or(0, |m| m.len());
                let modified_ms = metadata
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis() as i64);

                matches.push(serde_json::json!({
                    "name": name_str,
                    "path": path.to_string_lossy(),
                    "is_dir": is_dir,
                    "size_bytes": size_bytes,
                    "modified_epoch_ms": modified_ms,
                }));
            }

            if is_dir && criteria.recursive {
                self.walk_dir(&path, criteria, matches, truncated)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_file_search_skill_matching() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("report_q1.txt");
        let f2 = dir.path().join("report_q2.pdf");
        let f3 = dir.path().join("other.log");
        let sub = dir.path().join("archive");
        fs::create_dir(&sub).unwrap();
        let f4 = sub.join("report_final.txt");

        fs::write(&f1, b"q1").unwrap();
        fs::write(&f2, b"q2").unwrap();
        fs::write(&f3, b"other").unwrap();
        fs::write(&f4, b"final").unwrap();

        let skill = FileSearchSkill::new();
        let res = skill
            .execute(serde_json::json!({
                "path": dir.path().to_str().unwrap(),
                "query": "report",
                "recursive": true
            }))
            .unwrap();

        assert_eq!(res["count"], 3);
        assert!(!res["truncated"].as_bool().unwrap());
    }
}
