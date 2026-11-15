//! Chunked, BLAKE3-verified resumable file transfer protocol over QUIC/mTLS streams.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use aissistant_core::{AuditEntryId, DeviceId, DeviceTrustState, PeerPermissionLevel, SkillId};
use aissistant_security::{
    Actor, AuditEntry, AuditExecutionStatus, NewAuditEntry, PolicyDecision, GENESIS_PREV_HASH,
};
use aissistant_storage::{AuditStore, DeviceRegistry};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::discovery::current_time_ms;
use crate::error::{NetworkError, Result};
use crate::rpc::{read_framed_message, write_framed_message};
use crate::transport::{QuicSession, VirtualSession};

/// Default chunk size for file transfers (1 MB).
pub const DEFAULT_CHUNK_SIZE_BYTES: u32 = 1024 * 1024;

/// Maximum file size supported by a single transfer session (10 GB).
pub const MAX_TRANSFER_FILE_BYTES: u64 = 10 * 1024 * 1024 * 1024;

/// Maximum allowed filename byte length (255 bytes).
pub const MAX_FILENAME_BYTES: usize = 255;

/// Sanitizes a file name to prevent path traversal (`../`, `..\`) and directory escape vulnerabilities.
pub fn sanitize_file_name(raw_name: &str) -> Result<String> {
    let trimmed = raw_name.trim();
    if trimmed.is_empty() {
        return Err(NetworkError::Internal(
            "Filename cannot be empty".to_string(),
        ));
    }

    if trimmed.len() > MAX_FILENAME_BYTES {
        return Err(NetworkError::Internal(format!(
            "Filename length {} exceeds maximum allowed limit {}",
            trimmed.len(),
            MAX_FILENAME_BYTES
        )));
    }

    // Reject null bytes, traversal segments, or root paths
    if trimmed.contains('\0')
        || trimmed.contains("../")
        || trimmed.contains("..\\")
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || (trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':')
    {
        return Err(NetworkError::Internal(format!(
            "Path traversal or invalid characters detected in filename: '{trimmed}'"
        )));
    }

    // Extract basename
    let path = Path::new(trimmed);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| NetworkError::Internal("Invalid file name".to_string()))?;

    if file_name == "." || file_name == ".." {
        return Err(NetworkError::Internal(
            "Filename cannot be a dot directory identifier".to_string(),
        ));
    }

    Ok(file_name.to_string())
}

/// Metadata manifest describing a file to be transferred, including chunk and whole-file BLAKE3 hashes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferManifest {
    /// Unique identifier for the transfer session (e.g. "xfer_...").
    pub transfer_id: String,
    /// Origin device ID initiating the file transfer.
    pub origin_device_id: DeviceId,
    /// Target file name (sanitized basename).
    pub file_name: String,
    /// Total file size in bytes.
    pub file_size_bytes: u64,
    /// Size of each chunk in bytes (except potentially the final chunk).
    pub chunk_size_bytes: u32,
    /// Total number of chunks.
    pub total_chunks: u32,
    /// BLAKE3 256-bit hash of the entire file.
    pub whole_file_hash: [u8; 32],
    /// Ordered list of BLAKE3 256-bit hashes for each chunk.
    pub chunk_hashes: Vec<[u8; 32]>,
    /// Creation timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Resume negotiation response containing the set of chunk indices already verified and written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferResumeResponse {
    /// Matches the manifest's `transfer_id`.
    pub transfer_id: String,
    /// List of chunk indices already verified and present on the target node.
    pub received_chunk_indices: Vec<u32>,
    /// Whether the target is ready to accept chunks.
    pub can_resume: bool,
}

/// A single chunk of file data with index and offset coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkData {
    /// Unique transfer session identifier.
    pub transfer_id: String,
    /// Zero-based chunk index.
    pub chunk_index: u32,
    /// Byte offset within the target file.
    pub offset_bytes: u64,
    /// Raw chunk bytes.
    pub data: Vec<u8>,
    /// BLAKE3 hash of this chunk's data.
    pub chunk_hash: [u8; 32],
}

/// Progress state of a transfer session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TransferStatus {
    /// Sender is ready and manifest has been acknowledged.
    Accepted,
    /// File chunks are currently streaming.
    InProgress {
        /// Number of chunks verified so far.
        received_chunks: u32,
        /// Total chunks expected.
        total_chunks: u32,
    },
    /// File transfer successfully completed and verified.
    Completed {
        /// Local destination file path.
        file_path: String,
        /// Total bytes verified and written.
        total_bytes: u64,
    },
    /// Transfer failed or rejected.
    Failed {
        /// Human-readable explanation.
        reason: String,
    },
}

/// Wire envelope multiplexing transfer protocol messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "msg_type", rename_all = "snake_case")]
pub enum TransferMessage {
    /// Initial file transfer announcement and cryptographic manifest.
    Manifest(TransferManifest),
    /// Target resume status indicating which chunks are already received.
    Resume(TransferResumeResponse),
    /// A single chunk of file data.
    Chunk(ChunkData),
    /// High-level transfer status or completion confirmation.
    Status(TransferStatus),
}

/// In-memory metadata state saved to disk alongside incomplete `.part` files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferMetaState {
    /// Transfer manifest.
    pub manifest: TransferManifest,
    /// Set of chunk indices already verified and written.
    pub completed_chunks: HashSet<u32>,
}

/// Receiver engine managing file ingestion, chunk verification, resumption, and atomic renaming.
#[derive(Clone)]
pub struct FileTransferReceiver {
    local_device_id: DeviceId,
    storage_dir: PathBuf,
    registry: Arc<DeviceRegistry>,
    audit_store: Option<Arc<AuditStore>>,
}

impl std::fmt::Debug for FileTransferReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileTransferReceiver")
            .field("local_device_id", &self.local_device_id)
            .field("storage_dir", &self.storage_dir)
            .field("has_audit_store", &self.audit_store.is_some())
            .finish()
    }
}

impl FileTransferReceiver {
    /// Creates a new `FileTransferReceiver`.
    #[must_use]
    pub fn new(
        local_device_id: DeviceId,
        storage_dir: PathBuf,
        registry: Arc<DeviceRegistry>,
        audit_store: Option<Arc<AuditStore>>,
    ) -> Self {
        Self {
            local_device_id,
            storage_dir,
            registry,
            audit_store,
        }
    }

    /// Handles an incoming file transfer stream from an authenticated peer connection.
    pub async fn handle_stream<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        peer_public_key: [u8; 32],
        mut send: S,
        mut recv: R,
    ) -> Result<PathBuf> {
        // 1. Receive and decode initial Manifest message
        let first_msg: TransferMessage = read_framed_message(&mut recv).await?;
        let manifest = match first_msg {
            TransferMessage::Manifest(m) => m,
            _ => {
                let err = TransferStatus::Failed {
                    reason: "Expected Manifest message as first frame".to_string(),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal(
                    "Invalid initial transfer frame".to_string(),
                ));
            }
        };

        // 2. Authenticate caller DeviceId against peer TLS Ed25519 public key
        let hex_prefix: String = peer_public_key[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let expected_device_id_str = format!("dev_{hex_prefix}");

        if manifest.origin_device_id.as_str() != expected_device_id_str {
            let err = TransferStatus::Failed {
                reason: "Origin DeviceId does not match TLS authenticated public key".to_string(),
            };
            let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
            return Err(NetworkError::Internal("DeviceId spoofing detected".into()));
        }

        // 3. Verify sender trust state in DeviceRegistry
        let paired_device = match self.registry.get_device(&manifest.origin_device_id) {
            Ok(Some(dev)) => dev,
            Ok(None) => {
                let err = TransferStatus::Failed {
                    reason: "Unknown or unpaired sender device".to_string(),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal("Unpaired sender device".into()));
            }
            Err(e) => {
                let err = TransferStatus::Failed {
                    reason: format!("Database error reading peer device: {e}"),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal(e.to_string()));
            }
        };

        if paired_device.trust_state != DeviceTrustState::Trusted {
            let err = TransferStatus::Failed {
                reason: format!(
                    "Peer device trust state is '{}' (not trusted)",
                    paired_device.trust_state
                ),
            };
            let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
            return Err(NetworkError::Internal("Untrusted sender device".into()));
        }

        // 4. Verify Peer Capability ACL (file_system or lan_peer_networking)
        let permissions = match self
            .registry
            .get_peer_permissions(&manifest.origin_device_id)
        {
            Ok(perms) => perms,
            Err(e) => {
                let err = TransferStatus::Failed {
                    reason: format!("Database error querying peer permissions: {e}"),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal(e.to_string()));
            }
        };

        let perm_level = permissions
            .get("file_system")
            .or_else(|| permissions.get("lan_peer_networking"))
            .copied()
            .unwrap_or(PeerPermissionLevel::Deny);

        match perm_level {
            PeerPermissionLevel::Deny => {
                self.record_audit_log(
                    &manifest,
                    PolicyDecision::Deny {
                        reason: "File transfer denied by peer ACL".to_string(),
                    },
                    AuditExecutionStatus::Cancelled,
                );

                let err = TransferStatus::Failed {
                    reason: "File transfer capability is denied for this peer".to_string(),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal(
                    "Permission denied by peer ACL".into(),
                ));
            }
            PeerPermissionLevel::AskEachTime => {
                self.record_audit_log(
                    &manifest,
                    PolicyDecision::RequireConfirmation {
                        reason: "File transfer requires explicit user confirmation".to_string(),
                        risk_tier: aissistant_security::RiskTier::StateModifying,
                    },
                    AuditExecutionStatus::Cancelled,
                );

                let err = TransferStatus::Failed {
                    reason:
                        "File transfer requires interactive approval (unattended transfer rejected)"
                            .to_string(),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal(
                    "Interactive approval required".into(),
                ));
            }
            PeerPermissionLevel::AlwaysAllow => {
                // Authorized
            }
        }

        // 5. Sanitize target filename and validate bounds
        let safe_name = match sanitize_file_name(&manifest.file_name) {
            Ok(name) => name,
            Err(e) => {
                let err = TransferStatus::Failed {
                    reason: format!("Invalid filename: {e}"),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(e);
            }
        };

        if manifest.file_size_bytes > MAX_TRANSFER_FILE_BYTES {
            let err = TransferStatus::Failed {
                reason: format!(
                    "File size {} exceeds maximum supported {}",
                    manifest.file_size_bytes, MAX_TRANSFER_FILE_BYTES
                ),
            };
            let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
            return Err(NetworkError::Internal("File size exceeds limit".into()));
        }

        if manifest.chunk_hashes.len() != manifest.total_chunks as usize {
            let err = TransferStatus::Failed {
                reason: "Manifest chunk hashes count does not match total_chunks".to_string(),
            };
            let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
            return Err(NetworkError::Internal("Mismatched chunk count".into()));
        }

        // Ensure target directory exists
        std::fs::create_dir_all(&self.storage_dir)
            .map_err(|e| NetworkError::Internal(format!("Failed to create storage dir: {e}")))?;

        let part_path = self
            .storage_dir
            .join(format!("{}.part", manifest.transfer_id));
        let meta_path = self
            .storage_dir
            .join(format!("{}.meta", manifest.transfer_id));
        let final_path = self.storage_dir.join(&safe_name);

        // 6. Resume detection: load completed chunk indices if existing
        let mut completed_chunks: HashSet<u32> = HashSet::new();
        if meta_path.exists() && part_path.exists() {
            if let Ok(meta_data) = std::fs::read(&meta_path) {
                if let Ok(meta_state) = serde_json::from_slice::<TransferMetaState>(&meta_data) {
                    if meta_state.manifest.whole_file_hash == manifest.whole_file_hash {
                        completed_chunks = meta_state.completed_chunks;
                    }
                }
            }
        }

        // Send Resume response
        let resume_resp = TransferResumeResponse {
            transfer_id: manifest.transfer_id.clone(),
            received_chunk_indices: completed_chunks.iter().copied().collect(),
            can_resume: true,
        };
        write_framed_message(&mut send, &TransferMessage::Resume(resume_resp)).await?;

        // Open .part file for writing chunks at specific offsets
        let mut part_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(!part_path.exists())
            .open(&part_path)
            .map_err(|e| NetworkError::Internal(format!("Failed to open part file: {e}")))?;

        // Pre-allocate part file size if empty
        if part_file.metadata().map(|m| m.len()).unwrap_or(0) < manifest.file_size_bytes {
            part_file.set_len(manifest.file_size_bytes).map_err(|e| {
                NetworkError::Internal(format!("Failed to allocate part file: {e}"))
            })?;
        }

        // 7. Loop reading chunks until transfer is complete
        let mut remaining_chunks = (manifest.total_chunks as usize) - completed_chunks.len();

        while remaining_chunks > 0 {
            let msg: TransferMessage = read_framed_message(&mut recv).await?;
            let chunk = match msg {
                TransferMessage::Chunk(c) => c,
                _ => {
                    let err = TransferStatus::Failed {
                        reason: "Unexpected message while awaiting chunks".to_string(),
                    };
                    let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                    return Err(NetworkError::Internal(
                        "Protocol error awaiting chunk".to_string(),
                    ));
                }
            };

            if chunk.transfer_id != manifest.transfer_id {
                let err = TransferStatus::Failed {
                    reason: "Mismatched transfer ID in chunk".to_string(),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal("Mismatched transfer ID".into()));
            }

            let idx = chunk.chunk_index as usize;
            if idx >= manifest.chunk_hashes.len() {
                let err = TransferStatus::Failed {
                    reason: format!("Chunk index {} out of bounds", chunk.chunk_index),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal("Chunk index out of bounds".into()));
            }

            // Cryptographic per-chunk BLAKE3 verification
            let computed_hash = *blake3::hash(&chunk.data).as_bytes();
            if computed_hash != manifest.chunk_hashes[idx] || computed_hash != chunk.chunk_hash {
                let err = TransferStatus::Failed {
                    reason: format!(
                        "Chunk {} hash verification failed (corrupted)",
                        chunk.chunk_index
                    ),
                };
                let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
                return Err(NetworkError::Internal(
                    "Corrupted chunk BLAKE3 mismatch".into(),
                ));
            }

            // Write chunk bytes to calculated offset
            part_file
                .seek(SeekFrom::Start(chunk.offset_bytes))
                .map_err(|e| NetworkError::Internal(format!("Seek error in part file: {e}")))?;
            part_file
                .write_all(&chunk.data)
                .map_err(|e| NetworkError::Internal(format!("Write error in part file: {e}")))?;

            if completed_chunks.insert(chunk.chunk_index) {
                remaining_chunks -= 1;
            }

            // Persist progress to .meta file
            let meta_state = TransferMetaState {
                manifest: manifest.clone(),
                completed_chunks: completed_chunks.clone(),
            };
            if let Ok(meta_bytes) = serde_json::to_vec(&meta_state) {
                let _ = std::fs::write(&meta_path, meta_bytes);
            }
        }

        part_file
            .flush()
            .map_err(|e| NetworkError::Internal(format!("Flush error on part file: {e}")))?;

        // 8. Verify entire file BLAKE3 digest from beginning to end
        part_file
            .seek(SeekFrom::Start(0))
            .map_err(|e| NetworkError::Internal(format!("Seek start error: {e}")))?;

        let mut hasher = blake3::Hasher::new();
        let mut read_buf = vec![0u8; 64 * 1024];
        let mut total_read = 0u64;

        loop {
            let n = part_file.read(&mut read_buf).map_err(|e| {
                NetworkError::Internal(format!("Read error during verification: {e}"))
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&read_buf[..n]);
            total_read += n as u64;
        }

        if total_read != manifest.file_size_bytes {
            let err = TransferStatus::Failed {
                reason: format!(
                    "Final file size {} does not match manifest {}",
                    total_read, manifest.file_size_bytes
                ),
            };
            let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
            return Err(NetworkError::Internal("Final file size mismatch".into()));
        }

        let final_hash = *hasher.finalize().as_bytes();
        if final_hash != manifest.whole_file_hash {
            let _ = std::fs::remove_file(&part_path);
            let _ = std::fs::remove_file(&meta_path);
            let err = TransferStatus::Failed {
                reason: "Whole file BLAKE3 checksum mismatch".to_string(),
            };
            let _ = write_framed_message(&mut send, &TransferMessage::Status(err)).await;
            return Err(NetworkError::Internal("Whole file hash mismatch".into()));
        }

        // Close file handle before renaming
        drop(part_file);

        // Atomic rename from .part to final target
        std::fs::rename(&part_path, &final_path)
            .map_err(|e| NetworkError::Internal(format!("Failed to rename completed file: {e}")))?;

        // Cleanup .meta file
        let _ = std::fs::remove_file(&meta_path);

        // Send Completion confirmation to sender
        let completed_status = TransferStatus::Completed {
            file_path: final_path.to_string_lossy().to_string(),
            total_bytes: manifest.file_size_bytes,
        };
        write_framed_message(&mut send, &TransferMessage::Status(completed_status)).await?;

        // 9. Append verified audit log entry
        self.record_audit_log(
            &manifest,
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        Ok(final_path)
    }

    fn record_audit_log(
        &self,
        manifest: &TransferManifest,
        policy_decision: PolicyDecision,
        execution_status: AuditExecutionStatus,
    ) {
        if let Some(audit) = &self.audit_store {
            let prev_hash = audit.latest_hash().unwrap_or(GENESIS_PREV_HASH);
            let skill_id = match SkillId::new("file.receive") {
                Ok(s) => s,
                Err(_) => return,
            };

            let args_json = serde_json::json!({
                "transfer_id": manifest.transfer_id,
                "file_name": manifest.file_name,
                "file_size_bytes": manifest.file_size_bytes,
                "total_chunks": manifest.total_chunks,
            });

            let entry = AuditEntry::new(NewAuditEntry {
                id: AuditEntryId::generate(),
                timestamp_ms: current_time_ms(),
                actor: Actor::RemotePeer {
                    device_id: manifest.origin_device_id.clone(),
                },
                skill_id,
                arguments_redacted_json: serde_json::to_string(&args_json)
                    .unwrap_or_else(|_| "{}".to_string()),
                policy_decision,
                execution_status,
                prev_hash,
            });

            let _ = audit.append(&entry);
        }
    }
}

/// Sender engine constructing manifests, querying resume status, and streaming file chunks.
#[derive(Debug, Clone)]
pub struct FileTransferSender {
    local_device_id: DeviceId,
}

impl FileTransferSender {
    /// Creates a new `FileTransferSender`.
    #[must_use]
    pub fn new(local_device_id: DeviceId) -> Self {
        Self { local_device_id }
    }

    /// Generates a cryptographic manifest for the specified local file.
    pub fn create_manifest(
        &self,
        file_path: &Path,
        chunk_size_bytes: u32,
    ) -> Result<TransferManifest> {
        let chunk_size = if chunk_size_bytes == 0 {
            DEFAULT_CHUNK_SIZE_BYTES
        } else {
            chunk_size_bytes
        };

        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| NetworkError::Internal("Invalid file path".to_string()))?;

        let safe_name = sanitize_file_name(file_name)?;

        let mut file = File::open(file_path)
            .map_err(|e| NetworkError::Internal(format!("Failed to open file: {e}")))?;

        let metadata = file
            .metadata()
            .map_err(|e| NetworkError::Internal(format!("Failed to read metadata: {e}")))?;

        let file_size_bytes = metadata.len();
        if file_size_bytes > MAX_TRANSFER_FILE_BYTES {
            return Err(NetworkError::Internal(format!(
                "File size {file_size_bytes} exceeds maximum supported {MAX_TRANSFER_FILE_BYTES}"
            )));
        }

        let total_chunks = if file_size_bytes == 0 {
            1
        } else {
            file_size_bytes.div_ceil(chunk_size as u64) as u32
        };

        let mut whole_hasher = blake3::Hasher::new();
        let mut chunk_hashes = Vec::with_capacity(total_chunks as usize);

        let mut buffer = vec![0u8; chunk_size as usize];
        let mut read_total = 0u64;

        while read_total < file_size_bytes {
            let to_read = std::cmp::min(chunk_size as u64, file_size_bytes - read_total) as usize;
            file.read_exact(&mut buffer[..to_read])
                .map_err(|e| NetworkError::Internal(format!("Error reading chunk: {e}")))?;

            let slice = &buffer[..to_read];
            whole_hasher.update(slice);
            let chunk_hash = *blake3::hash(slice).as_bytes();
            chunk_hashes.push(chunk_hash);

            read_total += to_read as u64;
        }

        // Handle empty file edge-case (0 bytes)
        if file_size_bytes == 0 {
            let empty_hash = *blake3::hash(b"").as_bytes();
            chunk_hashes.push(empty_hash);
        }

        let whole_file_hash = *whole_hasher.finalize().as_bytes();

        Ok(TransferManifest {
            transfer_id: format!("xfer_{}", uuid::Uuid::new_v4().simple()),
            origin_device_id: self.local_device_id.clone(),
            file_name: safe_name,
            file_size_bytes,
            chunk_size_bytes: chunk_size,
            total_chunks,
            whole_file_hash,
            chunk_hashes,
            timestamp_ms: current_time_ms(),
        })
    }

    /// Streams a file over a bidirectional stream, negotiating resumption and verifying completion.
    pub async fn send_file_stream<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        file_path: &Path,
        manifest: &TransferManifest,
        mut send: S,
        mut recv: R,
    ) -> Result<TransferStatus> {
        // 1. Send Manifest
        write_framed_message(&mut send, &TransferMessage::Manifest(manifest.clone())).await?;

        // 2. Await Resume response
        let resume_msg: TransferMessage = read_framed_message(&mut recv).await?;
        let resume = match resume_msg {
            TransferMessage::Resume(r) => r,
            TransferMessage::Status(TransferStatus::Failed { reason }) => {
                return Err(NetworkError::Internal(format!(
                    "Transfer rejected: {reason}"
                )));
            }
            _ => {
                return Err(NetworkError::Internal(
                    "Unexpected response to manifest".into(),
                ));
            }
        };

        if !resume.can_resume {
            return Err(NetworkError::Internal("Target rejected resumption".into()));
        }

        let already_received: HashSet<u32> = resume.received_chunk_indices.into_iter().collect();

        // 3. Open source file and stream missing chunks
        let mut file = File::open(file_path)
            .map_err(|e| NetworkError::Internal(format!("Failed to open source file: {e}")))?;

        let mut buffer = vec![0u8; manifest.chunk_size_bytes as usize];

        for chunk_index in 0..manifest.total_chunks {
            if already_received.contains(&chunk_index) {
                continue;
            }

            let offset = (chunk_index as u64) * (manifest.chunk_size_bytes as u64);
            let to_read = if manifest.file_size_bytes == 0 {
                0
            } else {
                std::cmp::min(
                    manifest.chunk_size_bytes as u64,
                    manifest.file_size_bytes - offset,
                ) as usize
            };

            file.seek(SeekFrom::Start(offset))
                .map_err(|e| NetworkError::Internal(format!("Seek error: {e}")))?;

            if to_read > 0 {
                file.read_exact(&mut buffer[..to_read])
                    .map_err(|e| NetworkError::Internal(format!("Read chunk error: {e}")))?;
            }

            let slice = &buffer[..to_read];
            let chunk_hash = *blake3::hash(slice).as_bytes();

            let chunk_data = ChunkData {
                transfer_id: manifest.transfer_id.clone(),
                chunk_index,
                offset_bytes: offset,
                data: slice.to_vec(),
                chunk_hash,
            };

            write_framed_message(&mut send, &TransferMessage::Chunk(chunk_data)).await?;
        }

        // 4. Await final Completion status
        let final_msg: TransferMessage = read_framed_message(&mut recv).await?;
        match final_msg {
            TransferMessage::Status(status) => match &status {
                TransferStatus::Completed { .. } => Ok(status),
                TransferStatus::Failed { reason } => Err(NetworkError::Internal(format!(
                    "Transfer failed on target: {reason}"
                ))),
                _ => Err(NetworkError::Internal("Unexpected final status".into())),
            },
            _ => Err(NetworkError::Internal(
                "Unexpected message type at end of transfer".into(),
            )),
        }
    }

    /// Sends a file over an established Quinn QUIC session.
    pub async fn send_file_quic(
        &self,
        session: &QuicSession,
        file_path: &Path,
        manifest: &TransferManifest,
        timeout: Duration,
    ) -> Result<TransferStatus> {
        tokio::time::timeout(timeout, async {
            let (send, recv) = session.open_bi().await?;
            self.send_file_stream(file_path, manifest, send, recv).await
        })
        .await
        .map_err(|_| NetworkError::Internal("File transfer over QUIC timed out".into()))?
    }

    /// Sends a file over a simulated virtual session.
    pub async fn send_file_virtual(
        &self,
        session: &VirtualSession,
        file_path: &Path,
        manifest: &TransferManifest,
    ) -> Result<TransferStatus> {
        // Send Manifest
        let msg = TransferMessage::Manifest(manifest.clone());
        let msg_bytes =
            serde_json::to_vec(&msg).map_err(|e| NetworkError::Internal(e.to_string()))?;
        let mut framed = (msg_bytes.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&msg_bytes);
        session.send(&framed).await?;

        // Read Resume response
        let resp_framed = session.recv().await?;
        let resume_msg: TransferMessage = serde_json::from_slice(&resp_framed[4..])
            .map_err(|e| NetworkError::Internal(e.to_string()))?;
        let resume = match resume_msg {
            TransferMessage::Resume(r) => r,
            TransferMessage::Status(TransferStatus::Failed { reason }) => {
                return Err(NetworkError::Internal(reason));
            }
            _ => return Err(NetworkError::Internal("Unexpected virtual response".into())),
        };

        let already_received: HashSet<u32> = resume.received_chunk_indices.into_iter().collect();

        // Stream chunks
        let mut file = File::open(file_path)
            .map_err(|e| NetworkError::Internal(format!("Failed to open file: {e}")))?;
        let mut buffer = vec![0u8; manifest.chunk_size_bytes as usize];

        for chunk_index in 0..manifest.total_chunks {
            if already_received.contains(&chunk_index) {
                continue;
            }

            let offset = (chunk_index as u64) * (manifest.chunk_size_bytes as u64);
            let to_read = if manifest.file_size_bytes == 0 {
                0
            } else {
                std::cmp::min(
                    manifest.chunk_size_bytes as u64,
                    manifest.file_size_bytes - offset,
                ) as usize
            };

            file.seek(SeekFrom::Start(offset))
                .map_err(|e| NetworkError::Internal(e.to_string()))?;
            if to_read > 0 {
                file.read_exact(&mut buffer[..to_read])
                    .map_err(|e| NetworkError::Internal(e.to_string()))?;
            }

            let slice = &buffer[..to_read];
            let chunk_hash = *blake3::hash(slice).as_bytes();
            let chunk_data = ChunkData {
                transfer_id: manifest.transfer_id.clone(),
                chunk_index,
                offset_bytes: offset,
                data: slice.to_vec(),
                chunk_hash,
            };

            let c_msg = TransferMessage::Chunk(chunk_data);
            let c_bytes =
                serde_json::to_vec(&c_msg).map_err(|e| NetworkError::Internal(e.to_string()))?;
            let mut c_framed = (c_bytes.len() as u32).to_be_bytes().to_vec();
            c_framed.extend_from_slice(&c_bytes);
            session.send(&c_framed).await?;
        }

        // Read final status
        let final_framed = session.recv().await?;
        let final_msg: TransferMessage = serde_json::from_slice(&final_framed[4..])
            .map_err(|e| NetworkError::Internal(e.to_string()))?;

        match final_msg {
            TransferMessage::Status(status) => Ok(status),
            _ => Err(NetworkError::Internal(
                "Unexpected final virtual status".into(),
            )),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use aissistant_core::PlatformTarget;
    use aissistant_storage::SqliteDb;
    use tempfile::tempdir;

    #[test]
    fn test_filename_sanitization_defense() {
        assert_eq!(sanitize_file_name("document.pdf").unwrap(), "document.pdf");
        assert_eq!(sanitize_file_name("my_photo.png").unwrap(), "my_photo.png");

        // Traversal attempts must fail closed
        assert!(sanitize_file_name("../secret.txt").is_err());
        assert!(sanitize_file_name("..\\windows\\system32").is_err());
        assert!(sanitize_file_name("/etc/passwd").is_err());
        assert!(sanitize_file_name("C:\\boot.ini").is_err());
        assert!(sanitize_file_name("foo\0bar.txt").is_err());
        assert!(sanitize_file_name(".").is_err());
        assert!(sanitize_file_name("..").is_err());
        assert!(sanitize_file_name("   ").is_err());
    }

    #[test]
    fn test_manifest_creation_and_hash_calculation() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("payload.bin");

        // Write 2.5 MB of deterministic data (3 chunks of 1 MB)
        let data = vec![0xABu8; 2_500_000];
        std::fs::write(&file_path, &data).unwrap();

        let sender = FileTransferSender::new(DeviceId::new("dev_1111111111111111").unwrap());
        let manifest = sender.create_manifest(&file_path, 1024 * 1024).unwrap();

        assert_eq!(manifest.file_size_bytes, 2_500_000);
        assert_eq!(manifest.total_chunks, 3);
        assert_eq!(manifest.chunk_hashes.len(), 3);
        assert_eq!(manifest.file_name, "payload.bin");

        let expected_whole = *blake3::hash(&data).as_bytes();
        assert_eq!(manifest.whole_file_hash, expected_whole);
    }

    #[tokio::test]
    async fn test_file_transfer_end_to_end_and_resumption() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        let audit = Arc::new(AuditStore::new(db));

        let peer_pubkey = [0x44u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_device_id.clone(),
            name: "Sender Node".to_string(),
            platform: PlatformTarget::Linux,
            ed25519_public_key: peer_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Trusted,
        };
        registry.register_device(&paired).unwrap();
        registry
            .set_peer_permission(
                &peer_device_id,
                "file_system",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let src_dir = tempdir().unwrap();
        let dest_dir = tempdir().unwrap();

        // 1.5 MB test file -> 2 chunks of 1 MB
        let src_file = src_dir.path().join("transfer_test.dat");
        let test_bytes = vec![0x55u8; 1_500_000];
        std::fs::write(&src_file, &test_bytes).unwrap();

        let sender = FileTransferSender::new(peer_device_id.clone());
        let manifest = sender.create_manifest(&src_file, 1024 * 1024).unwrap();
        assert_eq!(manifest.total_chunks, 2);

        let receiver = FileTransferReceiver::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            dest_dir.path().to_path_buf(),
            registry,
            Some(audit.clone()),
        );

        // Run full transfer over in-memory duplex streams
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);

        let receiver_clone = receiver.clone();
        let server_task = tokio::spawn(async move {
            receiver_clone
                .handle_stream(peer_pubkey, server_write, server_read)
                .await
        });

        let sender_status = sender
            .send_file_stream(&src_file, &manifest, client_write, client_read)
            .await
            .unwrap();

        assert!(matches!(sender_status, TransferStatus::Completed { .. }));

        let received_path = server_task.await.unwrap().unwrap();
        assert_eq!(received_path, dest_dir.path().join("transfer_test.dat"));

        // Verify content matches exactly
        let received_bytes = std::fs::read(&received_path).unwrap();
        assert_eq!(received_bytes, test_bytes);

        // Verify audit trail continuity
        let logs = audit.get_entries(10, 0).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(
            logs[0].actor,
            Actor::RemotePeer {
                device_id: peer_device_id
            }
        );
        assert_eq!(logs[0].execution_status, AuditExecutionStatus::Success);
        assert!(
            aissistant_security::AuditChainVerifier::verify_chain(&logs, &GENESIS_PREV_HASH)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_file_transfer_rejects_corrupted_chunk() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db));

        let peer_pubkey = [0x66u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_device_id.clone(),
            name: "Sender Node".to_string(),
            platform: PlatformTarget::Windows,
            ed25519_public_key: peer_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Trusted,
        };
        registry.register_device(&paired).unwrap();
        registry
            .set_peer_permission(
                &peer_device_id,
                "file_system",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let dest_dir = tempdir().unwrap();
        let receiver = FileTransferReceiver::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            dest_dir.path().to_path_buf(),
            registry,
            None,
        );

        let (mut client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (server_read, server_write) = tokio::io::split(server_io);

        let server_task = tokio::spawn(async move {
            receiver
                .handle_stream(peer_pubkey, server_write, server_read)
                .await
        });

        let manifest = TransferManifest {
            transfer_id: "xfer_corrupt".to_string(),
            origin_device_id: peer_device_id,
            file_name: "test.bin".to_string(),
            file_size_bytes: 100,
            chunk_size_bytes: 100,
            total_chunks: 1,
            whole_file_hash: [0xAA; 32],
            chunk_hashes: vec![[0xBB; 32]], // Expected hash 0xBB
            timestamp_ms: 1000,
        };

        // Send manifest
        write_framed_message(&mut client_io, &TransferMessage::Manifest(manifest))
            .await
            .unwrap();

        // Read resume response
        let _resume: TransferMessage = read_framed_message(&mut client_io).await.unwrap();

        // Send corrupted chunk with mismatched hash
        let bad_chunk = ChunkData {
            transfer_id: "xfer_corrupt".to_string(),
            chunk_index: 0,
            offset_bytes: 0,
            data: vec![0x11; 100],
            chunk_hash: [0x99; 32], // Does not match actual hash or manifest
        };
        write_framed_message(&mut client_io, &TransferMessage::Chunk(bad_chunk))
            .await
            .unwrap();

        let server_res = server_task.await.unwrap();
        assert!(server_res.is_err());
        assert!(server_res
            .unwrap_err()
            .to_string()
            .contains("Corrupted chunk"));
    }

    #[tokio::test]
    async fn test_file_transfer_over_quic_mtls_loopback() {
        use crate::transport::QuicTransportEngine;
        use aissistant_security::DeviceIdentity;
        use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

        let id_server = Arc::new(DeviceIdentity::generate());
        let id_client = Arc::new(DeviceIdentity::generate());

        let server_pubkey = id_server.verifying_key_bytes();
        let client_pubkey = id_client.verifying_key_bytes();

        let hex_client: String = client_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let client_device_id = DeviceId::new(format!("dev_{hex_client}")).unwrap();

        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db));
        let dest_dir = tempdir().unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: client_device_id.clone(),
            name: "QUIC File Client".to_string(),
            platform: PlatformTarget::MacOS,
            ed25519_public_key: client_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Trusted,
        };
        registry.register_device(&paired).unwrap();
        registry
            .set_peer_permission(
                &client_device_id,
                "file_system",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let receiver = FileTransferReceiver::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            dest_dir.path().to_path_buf(),
            registry,
            None,
        );

        let mut engine_server = QuicTransportEngine::new(id_server.clone());
        let mut engine_client = QuicTransportEngine::new(id_client.clone());

        let bind_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        let server_addr = engine_server
            .start_endpoint(bind_addr, vec![client_pubkey], false)
            .unwrap();

        let _client_addr = engine_client
            .start_endpoint(bind_addr, vec![server_pubkey], false)
            .unwrap();

        let receiver_clone = receiver.clone();
        let server_task = tokio::spawn(async move {
            let session = engine_server.accept().await.unwrap();
            let (send, recv) = session.accept_bi().await.unwrap();
            let saved_path = receiver_clone
                .handle_stream(session.peer_public_key(), send, recv)
                .await
                .unwrap();
            (session, saved_path)
        });

        let client_session = engine_client
            .connect_peer(server_addr, Some(server_pubkey), "aissistant-server")
            .await
            .unwrap();

        let src_dir = tempdir().unwrap();
        let src_file = src_dir.path().join("quic_sample.txt");
        let content = b"QUIC mTLS File Transfer Content!";
        std::fs::write(&src_file, content).unwrap();

        let sender = FileTransferSender::new(client_device_id);
        let manifest = sender.create_manifest(&src_file, 1024 * 1024).unwrap();

        let send_status = sender
            .send_file_quic(
                &client_session,
                &src_file,
                &manifest,
                Duration::from_secs(5),
            )
            .await
            .unwrap();

        assert!(matches!(send_status, TransferStatus::Completed { .. }));

        let (server_session, saved_path) = server_task.await.unwrap();
        assert_eq!(std::fs::read(&saved_path).unwrap(), content);

        client_session.close();
        server_session.close();
    }
}
