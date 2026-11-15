//! Safe archive skill (`file.archive`).

use std::fs::{self, File};
use std::path::Path;
use std::sync::Arc;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::jail::PathJail;

/// Safe ZIP archive creation, extraction, and inspection skill with zip-slip protection.
#[derive(Debug, Clone)]
pub struct FileArchiveSkill {
    jail: Arc<PathJail>,
}

impl Default for FileArchiveSkill {
    fn default() -> Self {
        Self {
            jail: Arc::new(PathJail::default()),
        }
    }
}

impl FileArchiveSkill {
    /// Creates a new `FileArchiveSkill` with default jail constraints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `FileArchiveSkill` with a customized path jail.
    #[must_use]
    pub fn with_jail(jail: Arc<PathJail>) -> Self {
        Self { jail }
    }
}

impl ToolHandler for FileArchiveSkill {
    fn name(&self) -> &str {
        "file.archive"
    }

    fn description(&self) -> &str {
        "Safely creates ZIP archives, inspects contents, or extracts files with zip-slip traversal protection."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["create", "extract", "list"],
                    "description": "Archive operation: 'create' (compress), 'extract' (decompress), or 'list' (inspect)."
                },
                "archive_path": {
                    "type": "string",
                    "description": "Path to the ZIP archive file."
                },
                "target_path": {
                    "type": "string",
                    "description": "Directory or file to compress (for 'create') or destination directory to extract into (for 'extract')."
                }
            },
            "required": ["action", "archive_path"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::FileAndApp
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let action = arguments
            .get("action")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.archive".to_string(),
                reason: "Missing required 'action' argument".to_string(),
            })?;

        let raw_archive = arguments
            .get("archive_path")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "file.archive".to_string(),
                reason: "Missing required 'archive_path' argument".to_string(),
            })?;

        match action {
            "list" => self.execute_list(raw_archive),
            "create" => {
                let raw_target = arguments
                    .get("target_path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                        tool_name: "file.archive".to_string(),
                        reason: "Missing required 'target_path' argument for create action"
                            .to_string(),
                    })?;
                self.execute_create(raw_archive, raw_target)
            }
            "extract" => {
                let raw_target = arguments
                    .get("target_path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                        tool_name: "file.archive".to_string(),
                        reason: "Missing required 'target_path' argument for extract action"
                            .to_string(),
                    })?;
                self.execute_extract(raw_archive, raw_target)
            }
            other => Err(OrchestratorError::InvalidToolArguments {
                tool_name: "file.archive".to_string(),
                reason: format!(
                    "Unrecognized action '{other}', expected 'create', 'extract', or 'list'"
                ),
            }),
        }
    }
}

impl FileArchiveSkill {
    fn execute_list(&self, raw_archive: &str) -> Result<Value, OrchestratorError> {
        let canonical_archive = self
            .jail
            .validate_existing(raw_archive)
            .map_err(|err| err.to_orchestrator_error())?;

        let file =
            File::open(&canonical_archive).map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.archive".to_string(),
                reason: format!("Failed to open archive: {e}"),
            })?;

        let mut archive =
            ZipArchive::new(file).map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.archive".to_string(),
                reason: format!("Failed to parse ZIP archive: {e}"),
            })?;

        let mut entries = Vec::new();
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                entries.push(serde_json::json!({
                    "name": entry.name(),
                    "size_bytes": entry.size(),
                    "compressed_bytes": entry.compressed_size(),
                    "is_dir": entry.is_dir(),
                }));
            }
        }

        Ok(serde_json::json!({
            "archive_path": canonical_archive.to_string_lossy(),
            "action": "list",
            "entries_count": entries.len(),
            "entries": entries,
        }))
    }

    fn execute_create(
        &self,
        raw_archive: &str,
        raw_target: &str,
    ) -> Result<Value, OrchestratorError> {
        let canonical_target = self
            .jail
            .validate_existing(raw_target)
            .map_err(|err| err.to_orchestrator_error())?;

        let canonical_archive = self
            .jail
            .validate_for_write(raw_archive)
            .map_err(|err| err.to_orchestrator_error())?;

        let file = File::create(&canonical_archive).map_err(|e| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "file.archive".to_string(),
                reason: format!("Failed to create archive file: {e}"),
            }
        })?;

        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        let mut added_count = 0;

        if canonical_target.is_file() {
            let file_name = canonical_target
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");

            zip.start_file(file_name, options).map_err(|e| {
                OrchestratorError::ToolExecutionFailed {
                    tool_name: "file.archive".to_string(),
                    reason: format!("Failed to add file to archive: {e}"),
                }
            })?;

            let mut f = File::open(&canonical_target).map_err(|e| {
                OrchestratorError::ToolExecutionFailed {
                    tool_name: "file.archive".to_string(),
                    reason: format!("Failed to read source file: {e}"),
                }
            })?;
            std::io::copy(&mut f, &mut zip).map_err(|e| {
                OrchestratorError::ToolExecutionFailed {
                    tool_name: "file.archive".to_string(),
                    reason: format!("Failed to write data to archive: {e}"),
                }
            })?;
            added_count += 1;
        } else {
            self.add_directory_to_zip(
                &canonical_target,
                &canonical_target,
                &mut zip,
                options,
                &mut added_count,
            )?;
        }

        zip.finish()
            .map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.archive".to_string(),
                reason: format!("Failed to finalize archive: {e}"),
            })?;

        Ok(serde_json::json!({
            "archive_path": canonical_archive.to_string_lossy(),
            "target_path": canonical_target.to_string_lossy(),
            "action": "create",
            "entries_count": added_count,
        }))
    }

    fn add_directory_to_zip(
        &self,
        base_dir: &Path,
        current_dir: &Path,
        zip: &mut ZipWriter<File>,
        options: SimpleFileOptions,
        added_count: &mut usize,
    ) -> Result<(), OrchestratorError> {
        let entries =
            fs::read_dir(current_dir).map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.archive".to_string(),
                reason: format!("Failed to read directory: {e}"),
            })?;

        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path.strip_prefix(base_dir).unwrap_or(&path);
            let relative_str = relative.to_string_lossy().replace('\\', "/");

            if path.is_dir() {
                zip.add_directory(format!("{relative_str}/"), options)
                    .map_err(|e| OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.archive".to_string(),
                        reason: format!("Failed to add directory entry: {e}"),
                    })?;
                self.add_directory_to_zip(base_dir, &path, zip, options, added_count)?;
            } else {
                zip.start_file(&relative_str, options).map_err(|e| {
                    OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.archive".to_string(),
                        reason: format!("Failed to start file in zip: {e}"),
                    }
                })?;

                let mut f =
                    File::open(&path).map_err(|e| OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.archive".to_string(),
                        reason: format!("Failed to read file: {e}"),
                    })?;
                std::io::copy(&mut f, zip).map_err(|e| OrchestratorError::ToolExecutionFailed {
                    tool_name: "file.archive".to_string(),
                    reason: format!("Failed to write file to zip: {e}"),
                })?;
                *added_count += 1;
            }
        }

        Ok(())
    }

    fn execute_extract(
        &self,
        raw_archive: &str,
        raw_target: &str,
    ) -> Result<Value, OrchestratorError> {
        let canonical_archive = self
            .jail
            .validate_existing(raw_archive)
            .map_err(|err| err.to_orchestrator_error())?;

        let canonical_dest = self
            .jail
            .validate_existing(raw_target)
            .map_err(|err| err.to_orchestrator_error())?;

        if !canonical_dest.is_dir() {
            return Err(OrchestratorError::InvalidToolArguments {
                tool_name: "file.archive".to_string(),
                reason: format!("Extraction target is not a directory: '{raw_target}'"),
            });
        }

        let file =
            File::open(&canonical_archive).map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.archive".to_string(),
                reason: format!("Failed to open archive: {e}"),
            })?;

        let mut archive =
            ZipArchive::new(file).map_err(|e| OrchestratorError::ToolExecutionFailed {
                tool_name: "file.archive".to_string(),
                reason: format!("Failed to read ZIP archive: {e}"),
            })?;

        let mut extracted_count = 0;
        let mut extracted_names = Vec::new();

        for i in 0..archive.len() {
            let mut file =
                archive
                    .by_index(i)
                    .map_err(|e| OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.archive".to_string(),
                        reason: format!("Failed to read entry {i}: {e}"),
                    })?;

            let entry_name = file.name().to_string();

            // Zip-slip security validation
            let dest_path = self
                .jail
                .sanitize_archive_entry_path(&entry_name, &canonical_dest)
                .map_err(|err| err.to_orchestrator_error())?;

            if file.is_dir() {
                fs::create_dir_all(&dest_path).map_err(|e| {
                    OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.archive".to_string(),
                        reason: format!("Failed to create extracted directory: {e}"),
                    }
                })?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        OrchestratorError::ToolExecutionFailed {
                            tool_name: "file.archive".to_string(),
                            reason: format!("Failed to create parent directory: {e}"),
                        }
                    })?;
                }

                let mut outfile = File::create(&dest_path).map_err(|e| {
                    OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.archive".to_string(),
                        reason: format!("Failed to create extracted file: {e}"),
                    }
                })?;

                std::io::copy(&mut file, &mut outfile).map_err(|e| {
                    OrchestratorError::ToolExecutionFailed {
                        tool_name: "file.archive".to_string(),
                        reason: format!("Failed to write extracted file: {e}"),
                    }
                })?;

                extracted_count += 1;
                extracted_names.push(entry_name);
            }
        }

        Ok(serde_json::json!({
            "archive_path": canonical_archive.to_string_lossy(),
            "destination_dir": canonical_dest.to_string_lossy(),
            "action": "extract",
            "extracted_count": extracted_count,
            "extracted_entries": extracted_names,
        }))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_file_archive_skill_create_list_extract() {
        let src_dir = tempdir().unwrap();
        let dst_dir = tempdir().unwrap();
        let archive_dir = tempdir().unwrap();

        let doc1 = src_dir.path().join("doc1.txt");
        let doc2 = src_dir.path().join("doc2.txt");
        fs::write(&doc1, b"Hello inside zip!").unwrap();
        fs::write(&doc2, b"Second file inside zip!").unwrap();

        let archive_path = archive_dir.path().join("test.zip");
        let archive_str = archive_path.to_str().unwrap();

        let skill = FileArchiveSkill::new();

        // 1. Create
        let res_create = skill
            .execute(serde_json::json!({
                "action": "create",
                "archive_path": archive_str,
                "target_path": src_dir.path().to_str().unwrap()
            }))
            .unwrap();
        assert_eq!(res_create["entries_count"], 2);

        // 2. List
        let res_list = skill
            .execute(serde_json::json!({
                "action": "list",
                "archive_path": archive_str
            }))
            .unwrap();
        assert_eq!(res_list["entries_count"], 2);

        // 3. Extract
        let res_extract = skill
            .execute(serde_json::json!({
                "action": "extract",
                "archive_path": archive_str,
                "target_path": dst_dir.path().to_str().unwrap()
            }))
            .unwrap();
        assert_eq!(res_extract["extracted_count"], 2);

        assert_eq!(
            fs::read_to_string(dst_dir.path().join("doc1.txt")).unwrap(),
            "Hello inside zip!"
        );
        assert_eq!(
            fs::read_to_string(dst_dir.path().join("doc2.txt")).unwrap(),
            "Second file inside zip!"
        );
    }
}
