use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::catalog::ModelDescriptor;
use super::verifier::Sha256Verifier;
use crate::error::{LlmError, Result};

/// Status states for model downloading and integrity verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Idle,
    Downloading,
    Verifying,
    Completed,
    Paused,
    Failed(String),
}

/// Download progress snapshot reportable to the UI and logging subsystem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f32,
    pub status: DownloadStatus,
}

impl DownloadProgress {
    pub fn new(model_id: &str, downloaded: u64, total: u64, status: DownloadStatus) -> Self {
        let percentage = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
        } else {
            0.0
        };

        Self {
            model_id: model_id.to_string(),
            downloaded_bytes: downloaded,
            total_bytes: total,
            percentage,
            status,
        }
    }
}

/// Abstract model download coordinator managing chunking, resumability, and verification.
pub trait ModelDownloader: Send + Sync {
    /// Computes the existing byte length of a partial download file for Range resume.
    fn compute_resume_offset(&self, part_path: &Path) -> u64 {
        if part_path.exists() {
            fs::metadata(part_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        }
    }

    /// Ingests a model from chunks or source data, verifying SHA-256 integrity upon completion.
    fn ingest_model(
        &self,
        model: &ModelDescriptor,
        target_dir: &Path,
        source_data: &[u8],
        progress_cb: Option<&mut dyn FnMut(DownloadProgress)>,
    ) -> Result<PathBuf>;
}

/// Robust local model download and atomic staging engine.
#[derive(Debug, Default)]
pub struct LocalModelDownloader;

impl LocalModelDownloader {
    pub fn new() -> Self {
        Self
    }
}

impl ModelDownloader for LocalModelDownloader {
    fn ingest_model(
        &self,
        model: &ModelDescriptor,
        target_dir: &Path,
        source_data: &[u8],
        mut progress_cb: Option<&mut dyn FnMut(DownloadProgress)>,
    ) -> Result<PathBuf> {
        fs::create_dir_all(target_dir)?;

        let final_path = target_dir.join(format!("{}.gguf", model.id));
        let part_path = target_dir.join(format!("{}.gguf.part", model.id));

        let total_bytes = source_data.len() as u64;

        if let Some(ref mut cb) = progress_cb {
            cb(DownloadProgress::new(
                &model.id,
                0,
                total_bytes,
                DownloadStatus::Downloading,
            ));
        }

        // Open or resume .part file
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&part_path)?;

        // Stream and write in 64 KB chunks
        let chunk_size = 64 * 1024;
        let mut written: u64 = 0;
        let mut verifier = Sha256Verifier::new();

        for chunk in source_data.chunks(chunk_size) {
            file.write_all(chunk)?;
            verifier.update(chunk);
            written += chunk.len() as u64;

            if let Some(ref mut cb) = progress_cb {
                cb(DownloadProgress::new(
                    &model.id,
                    written,
                    total_bytes,
                    DownloadStatus::Downloading,
                ));
            }
        }

        file.flush()?;
        drop(file);

        // Verification phase
        if let Some(ref mut cb) = progress_cb {
            cb(DownloadProgress::new(
                &model.id,
                written,
                total_bytes,
                DownloadStatus::Verifying,
            ));
        }

        let computed_hash = verifier.finalize_hex();
        let expected_hash = model.sha256.trim().to_ascii_lowercase();

        if !computed_hash.eq_ignore_ascii_case(&expected_hash) {
            // Delete corrupt .part file to prevent using corrupted artifacts
            let _ = fs::remove_file(&part_path);
            let err_msg = format!(
                "Checksum mismatch for {}: expected {}, computed {}",
                model.id, expected_hash, computed_hash
            );
            if let Some(ref mut cb) = progress_cb {
                cb(DownloadProgress::new(
                    &model.id,
                    written,
                    total_bytes,
                    DownloadStatus::Failed(err_msg),
                ));
            }
            return Err(LlmError::ChecksumMismatch {
                model_id: model.id.clone(),
                expected: expected_hash,
                actual: computed_hash,
            });
        }

        // Atomic rename from .part to final .gguf
        fs::rename(&part_path, &final_path)?;

        if let Some(ref mut cb) = progress_cb {
            cb(DownloadProgress::new(
                &model.id,
                written,
                total_bytes,
                DownloadStatus::Completed,
            ));
        }

        Ok(final_path)
    }
}
