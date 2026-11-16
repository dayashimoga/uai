use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::catalog::{ModelDescriptor, ModelLifecycleState};
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
    pub speed_bps: u64,
    pub percentage: f32,
    pub status: DownloadStatus,
}

impl DownloadProgress {
    pub fn new(model_id: &str, downloaded: u64, total: u64, status: DownloadStatus) -> Self {
        Self::with_speed(model_id, downloaded, total, 0, status)
    }

    pub fn with_speed(
        model_id: &str,
        downloaded: u64,
        total: u64,
        speed_bps: u64,
        status: DownloadStatus,
    ) -> Self {
        let percentage = if total > 0 {
            ((downloaded as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
        } else {
            0.0
        };

        Self {
            model_id: model_id.to_string(),
            downloaded_bytes: downloaded,
            total_bytes: total,
            speed_bps,
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

/// Asynchronous, resumable HTTP streaming download manager with SHA-256 verification and Range support.
#[derive(Debug, Clone)]
pub struct ModelDownloadManager {
    models_dir: PathBuf,
    active_cancels: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl ModelDownloadManager {
    /// Creates a new download manager rooted in the provided models directory.
    pub fn new<P: Into<PathBuf>>(models_dir: P) -> Self {
        Self {
            models_dir: models_dir.into(),
            active_cancels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Base directory where models are stored.
    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Checks if a model download is actively running.
    pub fn is_downloading(&self, model_id: &str) -> bool {
        if let Ok(guard) = self.active_cancels.lock() {
            guard.contains_key(model_id)
        } else {
            false
        }
    }

    /// Signals cancellation of an active download by model ID.
    pub fn cancel_download(&self, model_id: &str) -> bool {
        if let Ok(mut guard) = self.active_cancels.lock() {
            if let Some(cancel_flag) = guard.remove(model_id) {
                cancel_flag.store(true, Ordering::SeqCst);
                return true;
            }
        }
        false
    }

    /// Streams model weights from the model's HuggingFace or direct URL with Range resumption.
    pub async fn download_model<F>(
        &self,
        model: &ModelDescriptor,
        mut progress_cb: F,
    ) -> Result<PathBuf>
    where
        F: FnMut(DownloadProgress) + Send + 'static,
    {
        fs::create_dir_all(&self.models_dir)?;
        let final_path = self.models_dir.join(format!("{}.gguf", model.id));
        let part_path = self.models_dir.join(format!("{}.gguf.part", model.id));

        // If verified target file already exists on disk, complete immediately
        if final_path.exists() {
            if let Ok(hash) = Sha256Verifier::compute_file_hash(&final_path) {
                if hash.eq_ignore_ascii_case(&model.sha256) {
                    progress_cb(DownloadProgress::new(
                        &model.id,
                        model.file_size_bytes,
                        model.file_size_bytes,
                        DownloadStatus::Completed,
                    ));
                    return Ok(final_path);
                }
            }
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut guard = self
                .active_cancels
                .lock()
                .map_err(|e| LlmError::InferenceError(e.to_string()))?;
            guard.insert(model.id.clone(), cancel_flag.clone());
        }

        let url = model.download_urls.first().ok_or_else(|| {
            LlmError::InferenceError("No download URL provided for model".to_string())
        })?;

        // Determine existing byte offset from .part file for HTTP Range resumption
        let resume_offset = if part_path.exists() {
            fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let client = reqwest::Client::new();
        let mut request = client.get(url);
        if resume_offset > 0 {
            request = request.header("Range", format!("bytes={resume_offset}-"));
        }

        let response = match request.send().await {
            Ok(resp) => resp,
            Err(e) => {
                if let Ok(mut guard) = self.active_cancels.lock() {
                    guard.remove(&model.id);
                }
                return Err(LlmError::InferenceError(format!(
                    "HTTP request failed: {e}"
                )));
            }
        };

        let status = response.status();
        let (mut current_offset, total_size) = if status == reqwest::StatusCode::PARTIAL_CONTENT {
            let total = response
                .content_length()
                .map(|len| resume_offset + len)
                .unwrap_or(model.file_size_bytes);
            (resume_offset, total)
        } else if status.is_success() {
            let total = response.content_length().unwrap_or(model.file_size_bytes);
            (0, total)
        } else {
            if let Ok(mut guard) = self.active_cancels.lock() {
                guard.remove(&model.id);
            }
            return Err(LlmError::InferenceError(format!(
                "HTTP download error: status {status}"
            )));
        };

        let mut file = if current_offset > 0 {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&part_path)?
        } else {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&part_path)?
        };

        let mut verifier = Sha256Verifier::new();
        if current_offset > 0 {
            if let Ok(mut existing_file) = fs::File::open(&part_path) {
                let mut buf = vec![0u8; 64 * 1024];
                let mut hashed_bytes = 0;
                while hashed_bytes < current_offset {
                    let to_read = (current_offset - hashed_bytes).min(buf.len() as u64) as usize;
                    let n = existing_file.read(&mut buf[..to_read])?;
                    if n == 0 {
                        break;
                    }
                    verifier.update(&buf[..n]);
                    hashed_bytes += n as u64;
                }
            }
        }

        let mut last_progress_report = Instant::now();
        let mut response_stream = response;

        while let Some(chunk) = response_stream
            .chunk()
            .await
            .map_err(|e| LlmError::InferenceError(e.to_string()))?
        {
            if cancel_flag.load(Ordering::SeqCst) {
                let _ = file.flush();
                if let Ok(mut guard) = self.active_cancels.lock() {
                    guard.remove(&model.id);
                }
                return Err(LlmError::InferenceError(
                    "Download cancelled by user".to_string(),
                ));
            }

            file.write_all(&chunk)?;
            verifier.update(&chunk);
            current_offset += chunk.len() as u64;

            let now = Instant::now();
            let elapsed = now.duration_since(last_progress_report);
            if elapsed.as_millis() >= 200 {
                let speed_bps = if elapsed.as_secs_f64() > 0.0 {
                    ((chunk.len() as f64) / elapsed.as_secs_f64()) as u64
                } else {
                    0
                };
                last_progress_report = now;
                progress_cb(DownloadProgress::with_speed(
                    &model.id,
                    current_offset,
                    total_size,
                    speed_bps,
                    DownloadStatus::Downloading,
                ));
            }
        }

        file.flush()?;
        drop(file);

        // Verification phase
        progress_cb(DownloadProgress::new(
            &model.id,
            current_offset,
            total_size,
            DownloadStatus::Verifying,
        ));

        let computed_hash = verifier.finalize_hex();
        let expected_hash = model.sha256.trim().to_ascii_lowercase();

        if !computed_hash.eq_ignore_ascii_case(&expected_hash) {
            let _ = fs::remove_file(&part_path);
            if let Ok(mut guard) = self.active_cancels.lock() {
                guard.remove(&model.id);
            }
            return Err(LlmError::ChecksumMismatch {
                model_id: model.id.clone(),
                expected: expected_hash,
                actual: computed_hash,
            });
        }

        fs::rename(&part_path, &final_path)?;

        if let Ok(mut guard) = self.active_cancels.lock() {
            guard.remove(&model.id);
        }

        progress_cb(DownloadProgress::new(
            &model.id,
            current_offset,
            total_size,
            DownloadStatus::Completed,
        ));

        Ok(final_path)
    }
}

/// Evaluates the real on-disk presence and runtime residency of a model catalog descriptor.
pub fn query_model_lifecycle(
    models_dir: &Path,
    model: &ModelDescriptor,
    is_active: bool,
    is_downloading: bool,
) -> ModelLifecycleState {
    if is_active {
        return ModelLifecycleState::LoadedActive {
            memory_footprint_bytes: model.file_size_bytes + (256 * 1024 * 1024),
        };
    }

    if is_downloading {
        let part_path = models_dir.join(format!("{}.gguf.part", model.id));
        let downloaded = if part_path.exists() {
            fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        return ModelLifecycleState::Downloading {
            downloaded_bytes: downloaded,
            total_bytes: model.file_size_bytes,
            speed_bps: 0,
        };
    }

    // Check if final .gguf file exists in models directory
    let final_path = models_dir.join(format!("{}.gguf", model.id));
    if final_path.exists() {
        let size = fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
        let verified = size > 0 && (size == model.file_size_bytes || model.sha256.is_empty());
        return ModelLifecycleState::DownloadedOnDisk {
            local_path: final_path.to_string_lossy().to_string(),
            sha256_verified: verified,
        };
    }

    // Check custom imported local paths
    if let Some(first_url) = model.download_urls.first() {
        let custom_path = PathBuf::from(first_url);
        if custom_path.exists() {
            return ModelLifecycleState::DownloadedOnDisk {
                local_path: custom_path.to_string_lossy().to_string(),
                sha256_verified: true,
            };
        }
    }

    // Check for paused / partial download .part file
    let part_path = models_dir.join(format!("{}.gguf.part", model.id));
    if part_path.exists() {
        let part_len = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);
        if part_len > 0 {
            return ModelLifecycleState::Downloading {
                downloaded_bytes: part_len,
                total_bytes: model.file_size_bytes,
                speed_bps: 0,
            };
        }
    }

    ModelLifecycleState::NotDownloaded {
        size_bytes: model.file_size_bytes,
        hf_url: model.download_urls.first().cloned().unwrap_or_default(),
        expected_sha256: model.sha256.clone(),
    }
}
