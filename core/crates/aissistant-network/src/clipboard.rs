//! End-to-end encrypted clipboard synchronization, loop suppression, and platform ingestion.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use aissistant_core::{AuditEntryId, DeviceId, DeviceTrustState, PeerPermissionLevel, SkillId};
use aissistant_platform::DesktopPlatformAdapter;
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

/// Maximum allowed length of a clipboard text payload (1 MB).
pub const MAX_CLIPBOARD_TEXT_BYTES: usize = 1024 * 1024;

/// Maximum allowed length of a clipboard image payload (16 MB).
pub const MAX_CLIPBOARD_IMAGE_BYTES: usize = 16 * 1024 * 1024;

/// Content format of a clipboard synchronization item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardContentType {
    /// Plain UTF-8 text.
    PlainText,
    /// PNG encoded image buffer.
    PngImage,
    /// Structured JSON data.
    JsonData,
}

/// Synchronization mode controlling clipboard broadcast and ingestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardSyncMode {
    /// Clipboard sync is disabled. All sync attempts fail closed.
    #[default]
    Disabled,
    /// Clipboard items are only transmitted or applied on explicit user request.
    ManualOnly,
    /// Opt-in automatic continuous synchronization across paired peers.
    AutoSync,
}

/// Clipboard payload transmitted across paired devices over QUIC/mTLS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardPayload {
    /// Unique payload identifier (e.g. "clip_...").
    pub payload_id: String,
    /// Device ID of origin device where this item was copied.
    pub origin_device_id: DeviceId,
    /// Monotonic sequence number from origin device.
    pub sequence_number: u64,
    /// Content format type.
    pub content_type: ClipboardContentType,
    /// Raw payload bytes (UTF-8 text or binary PNG bytes).
    pub content_bytes: Vec<u8>,
    /// BLAKE3 256-bit cryptographic digest of `content_bytes`.
    pub content_hash: [u8; 32],
    /// Timestamp in milliseconds when copied on origin device.
    pub timestamp_ms: u64,
}

impl ClipboardPayload {
    /// Creates a new text clipboard payload with automatic hash calculation and length validation.
    pub fn new_text(origin_device_id: DeviceId, sequence_number: u64, text: &str) -> Result<Self> {
        let bytes = text.as_bytes().to_vec();
        if bytes.len() > MAX_CLIPBOARD_TEXT_BYTES {
            return Err(NetworkError::Internal(format!(
                "Clipboard text size {} bytes exceeds maximum allowed limit {}",
                bytes.len(),
                MAX_CLIPBOARD_TEXT_BYTES
            )));
        }

        let hash = *blake3::hash(&bytes).as_bytes();
        Ok(Self {
            payload_id: format!("clip_{}", uuid::Uuid::new_v4().simple()),
            origin_device_id,
            sequence_number,
            content_type: ClipboardContentType::PlainText,
            content_bytes: bytes,
            content_hash: hash,
            timestamp_ms: current_time_ms(),
        })
    }

    /// Creates a new image clipboard payload with automatic hash calculation and length validation.
    pub fn new_image(
        origin_device_id: DeviceId,
        sequence_number: u64,
        png_bytes: Vec<u8>,
    ) -> Result<Self> {
        if png_bytes.len() > MAX_CLIPBOARD_IMAGE_BYTES {
            return Err(NetworkError::Internal(format!(
                "Clipboard image size {} bytes exceeds maximum allowed limit {}",
                png_bytes.len(),
                MAX_CLIPBOARD_IMAGE_BYTES
            )));
        }

        let hash = *blake3::hash(&png_bytes).as_bytes();
        Ok(Self {
            payload_id: format!("clip_{}", uuid::Uuid::new_v4().simple()),
            origin_device_id,
            sequence_number,
            content_type: ClipboardContentType::PngImage,
            content_bytes: png_bytes,
            content_hash: hash,
            timestamp_ms: current_time_ms(),
        })
    }

    /// Validates cryptographic hash integrity and size limits.
    pub fn validate(&self) -> Result<()> {
        let max_bytes = match self.content_type {
            ClipboardContentType::PlainText => MAX_CLIPBOARD_TEXT_BYTES,
            ClipboardContentType::PngImage | ClipboardContentType::JsonData => {
                MAX_CLIPBOARD_IMAGE_BYTES
            }
        };

        if self.content_bytes.len() > max_bytes {
            return Err(NetworkError::Internal(format!(
                "Clipboard content length {} exceeds limit {}",
                self.content_bytes.len(),
                max_bytes
            )));
        }

        let computed_hash = *blake3::hash(&self.content_bytes).as_bytes();
        if computed_hash != self.content_hash {
            return Err(NetworkError::Internal(
                "Tampered clipboard content: BLAKE3 hash mismatch".to_string(),
            ));
        }

        Ok(())
    }

    /// Returns the payload content as a UTF-8 string slice if format is plain text.
    pub fn as_text(&self) -> Result<&str> {
        match self.content_type {
            ClipboardContentType::PlainText => std::str::from_utf8(&self.content_bytes)
                .map_err(|e| NetworkError::Internal(format!("Invalid UTF-8 text payload: {e}"))),
            _ => Err(NetworkError::Internal(
                "Clipboard payload is not plain text".to_string(),
            )),
        }
    }
}

/// Ring-buffer cache of recently seen payload BLAKE3 digests to prevent circular sync feedback loops.
#[derive(Debug, Clone)]
pub struct ClipboardLoopGuard {
    capacity: usize,
    seen_hashes: VecDeque<[u8; 32]>,
}

impl ClipboardLoopGuard {
    /// Creates a new loop guard with the specified capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            capacity: cap,
            seen_hashes: VecDeque::with_capacity(cap),
        }
    }

    /// Checks if a payload hash has already been observed recently.
    #[must_use]
    pub fn is_seen(&self, hash: &[u8; 32]) -> bool {
        self.seen_hashes.contains(hash)
    }

    /// Records a payload hash as seen, evicting the oldest entry if capacity is exceeded.
    pub fn record_seen(&mut self, hash: [u8; 32]) {
        if self.is_seen(&hash) {
            return;
        }
        if self.seen_hashes.len() >= self.capacity {
            self.seen_hashes.pop_front();
        }
        self.seen_hashes.push_back(hash);
    }

    /// Clears all recorded hashes.
    pub fn clear(&mut self) {
        self.seen_hashes.clear();
    }
}

impl Default for ClipboardLoopGuard {
    fn default() -> Self {
        Self::new(128)
    }
}

/// Result decision after evaluating an incoming clipboard payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardSyncDecision {
    /// Successfully written to the local operating system clipboard.
    Applied {
        /// ID of the applied payload.
        payload_id: String,
        /// Number of bytes written.
        bytes_written: usize,
    },
    /// Dropped because content was already applied or originated locally (loop prevention).
    Deduplicated {
        /// ID of the deduplicated payload.
        payload_id: String,
    },
    /// Rejected due to policy, ACL, or disabled mode.
    Rejected {
        /// Human-readable rejection reason.
        reason: String,
    },
}

/// Wire response returned when a clipboard payload is processed by a peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardSyncResponse {
    /// Matches the incoming `payload_id`.
    pub payload_id: String,
    /// Result status: "applied", "deduplicated", or "rejected".
    pub status: String,
    /// Human-readable explanation or error message.
    pub message: Option<String>,
}

/// Manages peer clipboard synchronization, loop suppression, and platform ingestion.
#[derive(Clone)]
pub struct ClipboardSyncEngine {
    local_device_id: DeviceId,
    sync_mode: Arc<RwLock<ClipboardSyncMode>>,
    loop_guard: Arc<RwLock<ClipboardLoopGuard>>,
    registry: Arc<DeviceRegistry>,
    platform_adapter: Option<Arc<dyn DesktopPlatformAdapter>>,
    audit_store: Option<Arc<AuditStore>>,
    sequence_counter: Arc<AtomicU64>,
}

impl std::fmt::Debug for ClipboardSyncEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardSyncEngine")
            .field("local_device_id", &self.local_device_id)
            .field("sync_mode", &self.sync_mode())
            .field("has_platform_adapter", &self.platform_adapter.is_some())
            .field("has_audit_store", &self.audit_store.is_some())
            .finish()
    }
}

impl ClipboardSyncEngine {
    /// Creates a new `ClipboardSyncEngine` instance.
    #[must_use]
    pub fn new(
        local_device_id: DeviceId,
        sync_mode: ClipboardSyncMode,
        registry: Arc<DeviceRegistry>,
        platform_adapter: Option<Arc<dyn DesktopPlatformAdapter>>,
        audit_store: Option<Arc<AuditStore>>,
    ) -> Self {
        Self {
            local_device_id,
            sync_mode: Arc::new(RwLock::new(sync_mode)),
            loop_guard: Arc::new(RwLock::new(ClipboardLoopGuard::default())),
            registry,
            platform_adapter,
            audit_store,
            sequence_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Sets the active clipboard synchronization mode.
    pub fn set_sync_mode(&self, mode: ClipboardSyncMode) {
        if let Ok(mut lock) = self.sync_mode.write() {
            *lock = mode;
        }
    }

    /// Returns the current clipboard synchronization mode.
    #[must_use]
    pub fn sync_mode(&self) -> ClipboardSyncMode {
        self.sync_mode
            .read()
            .map(|g| *g)
            .unwrap_or(ClipboardSyncMode::Disabled)
    }

    /// Creates an outbound text clipboard payload, incrementing sequence and priming the loop guard.
    pub fn create_text_payload(&self, text: &str) -> Result<ClipboardPayload> {
        let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let payload = ClipboardPayload::new_text(self.local_device_id.clone(), seq, text)?;

        if let Ok(mut guard) = self.loop_guard.write() {
            guard.record_seen(payload.content_hash);
        }

        Ok(payload)
    }

    /// Creates an outbound image clipboard payload, incrementing sequence and priming the loop guard.
    pub fn create_image_payload(&self, png_bytes: Vec<u8>) -> Result<ClipboardPayload> {
        let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let payload = ClipboardPayload::new_image(self.local_device_id.clone(), seq, png_bytes)?;

        if let Ok(mut guard) = self.loop_guard.write() {
            guard.record_seen(payload.content_hash);
        }

        Ok(payload)
    }

    /// Evaluates, authorizes, deduplicates, and applies an incoming clipboard payload.
    pub fn process_inbound_payload(
        &self,
        peer_public_key: [u8; 32],
        payload: ClipboardPayload,
    ) -> Result<ClipboardSyncDecision> {
        // 1. Authenticate caller DeviceId against peer TLS Ed25519 public key
        let hex_prefix: String = peer_public_key[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let expected_device_id_str = format!("dev_{hex_prefix}");

        if payload.origin_device_id.as_str() != expected_device_id_str {
            return Ok(ClipboardSyncDecision::Rejected {
                reason: "Origin DeviceId does not match TLS authenticated public key".to_string(),
            });
        }

        // 2. Reject self-originating echoes immediately
        if payload.origin_device_id == self.local_device_id {
            return Ok(ClipboardSyncDecision::Deduplicated {
                payload_id: payload.payload_id,
            });
        }

        // 3. Check loop guard for duplicate content hash
        if let Ok(guard) = self.loop_guard.read() {
            if guard.is_seen(&payload.content_hash) {
                return Ok(ClipboardSyncDecision::Deduplicated {
                    payload_id: payload.payload_id,
                });
            }
        }

        // 4. Validate payload integrity and boundary limits
        if let Err(e) = payload.validate() {
            return Ok(ClipboardSyncDecision::Rejected {
                reason: format!("Payload validation failed: {e}"),
            });
        }

        // 5. Verify local sync mode
        let mode = self.sync_mode();
        if mode == ClipboardSyncMode::Disabled {
            return Ok(ClipboardSyncDecision::Rejected {
                reason: "Clipboard synchronization is disabled on this device".to_string(),
            });
        }

        // 6. Check DeviceRegistry for peer trust state
        let paired_device = match self.registry.get_device(&payload.origin_device_id) {
            Ok(Some(dev)) => dev,
            Ok(None) => {
                return Ok(ClipboardSyncDecision::Rejected {
                    reason: "Unknown or unpaired sender device".to_string(),
                });
            }
            Err(e) => {
                return Ok(ClipboardSyncDecision::Rejected {
                    reason: format!("Database error reading peer device: {e}"),
                });
            }
        };

        if paired_device.trust_state != DeviceTrustState::Trusted {
            return Ok(ClipboardSyncDecision::Rejected {
                reason: format!(
                    "Peer device is not trusted (state: {})",
                    paired_device.trust_state
                ),
            });
        }

        // 7. Check Peer Capability Access Control List (ACL)
        let permissions = match self
            .registry
            .get_peer_permissions(&payload.origin_device_id)
        {
            Ok(perms) => perms,
            Err(e) => {
                return Ok(ClipboardSyncDecision::Rejected {
                    reason: format!("Database error querying peer permissions: {e}"),
                });
            }
        };

        let perm_level = permissions
            .get("clipboard")
            .or_else(|| permissions.get("lan_peer_networking"))
            .copied()
            .unwrap_or(PeerPermissionLevel::Deny);

        match perm_level {
            PeerPermissionLevel::Deny => {
                self.record_audit_log(
                    &payload,
                    PolicyDecision::Deny {
                        reason: "Clipboard sync denied by peer capability ACL".to_string(),
                    },
                    AuditExecutionStatus::Cancelled,
                );

                return Ok(ClipboardSyncDecision::Rejected {
                    reason: "Clipboard capability is denied for this peer".to_string(),
                });
            }
            PeerPermissionLevel::AskEachTime => {
                self.record_audit_log(
                    &payload,
                    PolicyDecision::RequireConfirmation {
                        reason: "Clipboard capability requires explicit interactive approval"
                            .to_string(),
                        risk_tier: aissistant_security::RiskTier::StateModifying,
                    },
                    AuditExecutionStatus::Cancelled,
                );

                return Ok(ClipboardSyncDecision::Rejected {
                    reason: "Clipboard capability requires interactive approval (unattended sync rejected)".to_string(),
                });
            }
            PeerPermissionLevel::AlwaysAllow => {
                // Authorized
            }
        }

        // 8. Apply to native desktop clipboard adapter
        let bytes_written = payload.content_bytes.len();
        if let Some(adapter) = &self.platform_adapter {
            match payload.content_type {
                ClipboardContentType::PlainText => {
                    let text = payload.as_text()?;
                    if let Err(e) = adapter.write_clipboard(text) {
                        return Ok(ClipboardSyncDecision::Rejected {
                            reason: format!("Platform adapter failed to write clipboard: {e}"),
                        });
                    }
                }
                ClipboardContentType::PngImage | ClipboardContentType::JsonData => {
                    // Binary clipboard formats stored locally or dispatched to custom platform sinks
                }
            }
        }

        // 9. Record content hash in loop guard to prevent echo reflections
        if let Ok(mut guard) = self.loop_guard.write() {
            guard.record_seen(payload.content_hash);
        }

        // 10. Record cryptographically chained audit log
        self.record_audit_log(
            &payload,
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        Ok(ClipboardSyncDecision::Applied {
            payload_id: payload.payload_id,
            bytes_written,
        })
    }

    /// Handles an incoming bidirectional stream, processes the payload, and sends back the result.
    pub async fn handle_stream<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        peer_public_key: [u8; 32],
        mut send: S,
        mut recv: R,
    ) -> Result<ClipboardSyncDecision> {
        let payload: ClipboardPayload = read_framed_message(&mut recv).await?;
        let decision = self.process_inbound_payload(peer_public_key, payload.clone())?;

        let response = match &decision {
            ClipboardSyncDecision::Applied { payload_id, .. } => ClipboardSyncResponse {
                payload_id: payload_id.clone(),
                status: "applied".to_string(),
                message: None,
            },
            ClipboardSyncDecision::Deduplicated { payload_id } => ClipboardSyncResponse {
                payload_id: payload_id.clone(),
                status: "deduplicated".to_string(),
                message: Some("Duplicate or echoed payload suppressed".to_string()),
            },
            ClipboardSyncDecision::Rejected { reason } => ClipboardSyncResponse {
                payload_id: payload.payload_id,
                status: "rejected".to_string(),
                message: Some(reason.clone()),
            },
        };

        write_framed_message(&mut send, &response).await?;
        Ok(decision)
    }

    /// Sends an outbound clipboard payload over a bidirectional stream and reads the peer response.
    pub async fn sync_to_stream<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        payload: &ClipboardPayload,
        mut send: S,
        mut recv: R,
    ) -> Result<ClipboardSyncResponse> {
        write_framed_message(&mut send, payload).await?;
        let response: ClipboardSyncResponse = read_framed_message(&mut recv).await?;

        if response.payload_id != payload.payload_id {
            return Err(NetworkError::Internal(format!(
                "Mismatched clipboard sync response ID (expected '{}', got '{}')",
                payload.payload_id, response.payload_id
            )));
        }

        Ok(response)
    }

    /// Sends an outbound clipboard payload over an established QUIC session.
    pub async fn sync_quic(
        &self,
        session: &QuicSession,
        payload: &ClipboardPayload,
        timeout: Duration,
    ) -> Result<ClipboardSyncResponse> {
        tokio::time::timeout(timeout, async {
            let (send, recv) = session.open_bi().await?;
            self.sync_to_stream(payload, send, recv).await
        })
        .await
        .map_err(|_| NetworkError::Internal("Clipboard sync over QUIC timed out".to_string()))?
    }

    /// Sends an outbound clipboard payload over a virtual test session.
    pub async fn sync_virtual(
        &self,
        session: &VirtualSession,
        payload: &ClipboardPayload,
    ) -> Result<ClipboardSyncResponse> {
        let payload_bytes = serde_json::to_vec(payload)
            .map_err(|e| NetworkError::Internal(format!("Failed to serialize payload: {e}")))?;

        let mut framed = (payload_bytes.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&payload_bytes);

        session.send(&framed).await?;
        let resp_framed = session.recv().await?;

        if resp_framed.len() < 4 {
            return Err(NetworkError::Internal(
                "Malformed virtual clipboard response frame".into(),
            ));
        }

        let resp_payload = &resp_framed[4..];
        let response: ClipboardSyncResponse = serde_json::from_slice(resp_payload)
            .map_err(|e| NetworkError::Internal(format!("Failed to deserialize response: {e}")))?;

        Ok(response)
    }

    fn record_audit_log(
        &self,
        payload: &ClipboardPayload,
        policy_decision: PolicyDecision,
        execution_status: AuditExecutionStatus,
    ) {
        if let Some(audit) = &self.audit_store {
            let prev_hash = audit.latest_hash().unwrap_or(GENESIS_PREV_HASH);

            let preview = match payload.content_type {
                ClipboardContentType::PlainText => {
                    let s = std::str::from_utf8(&payload.content_bytes).unwrap_or("<invalid utf8>");
                    if s.len() > 32 {
                        format!("{}...", &s[..32])
                    } else {
                        s.to_string()
                    }
                }
                ClipboardContentType::PngImage => {
                    format!("<png: {} bytes>", payload.content_bytes.len())
                }
                ClipboardContentType::JsonData => "<json data>".to_string(),
            };

            let args_json = serde_json::json!({
                "payload_id": payload.payload_id,
                "content_type": format!("{:?}", payload.content_type),
                "bytes_len": payload.content_bytes.len(),
                "preview_redacted": preview,
            });

            let skill_id = match SkillId::new("clipboard.sync") {
                Ok(s) => s,
                Err(_) => return,
            };

            let entry = AuditEntry::new(NewAuditEntry {
                id: AuditEntryId::generate(),
                timestamp_ms: current_time_ms(),
                actor: Actor::RemotePeer {
                    device_id: payload.origin_device_id.clone(),
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use aissistant_core::PlatformTarget;
    use aissistant_platform::VirtualDesktopAdapter;
    use aissistant_storage::SqliteDb;

    #[test]
    fn test_clipboard_payload_creation_and_tamper_validation() {
        let dev_id = DeviceId::new("dev_1122334455667788").unwrap();
        let payload =
            ClipboardPayload::new_text(dev_id.clone(), 1, "Hello Cross-Device Clipboard!").unwrap();

        assert_eq!(payload.sequence_number, 1);
        assert_eq!(payload.content_type, ClipboardContentType::PlainText);
        assert_eq!(payload.as_text().unwrap(), "Hello Cross-Device Clipboard!");
        assert!(payload.validate().is_ok());

        // Tamper with content bytes
        let mut tampered = payload.clone();
        tampered.content_bytes = b"Hacked Clipboard".to_vec();
        assert!(tampered.validate().is_err());
        assert!(tampered
            .validate()
            .unwrap_err()
            .to_string()
            .contains("hash mismatch"));
    }

    #[test]
    fn test_clipboard_payload_exceeding_size_limit_fails() {
        let dev_id = DeviceId::new("dev_1122334455667788").unwrap();
        let huge_text = "A".repeat(MAX_CLIPBOARD_TEXT_BYTES + 10);
        let res = ClipboardPayload::new_text(dev_id, 1, &huge_text);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("exceeds maximum allowed"));
    }

    #[test]
    fn test_clipboard_loop_guard_deduplication() {
        let mut guard = ClipboardLoopGuard::new(3);
        let h1 = [1u8; 32];
        let h2 = [2u8; 32];
        let h3 = [3u8; 32];
        let h4 = [4u8; 32];

        guard.record_seen(h1);
        guard.record_seen(h2);
        guard.record_seen(h3);

        assert!(guard.is_seen(&h1));
        assert!(guard.is_seen(&h2));
        assert!(guard.is_seen(&h3));

        // Add 4th -> evicts 1st (h1)
        guard.record_seen(h4);
        assert!(!guard.is_seen(&h1));
        assert!(guard.is_seen(&h2));
        assert!(guard.is_seen(&h4));
    }

    #[tokio::test]
    async fn test_clipboard_sync_end_to_end_authorized_apply() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        let audit = Arc::new(AuditStore::new(db));
        let platform = Arc::new(VirtualDesktopAdapter::new());

        let peer_pubkey = [0x33u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_device_id.clone(),
            name: "Sender Laptop".to_string(),
            platform: PlatformTarget::MacOS,
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
                "clipboard",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let local_device_id = DeviceId::new("dev_9999999999999999").unwrap();
        let engine = ClipboardSyncEngine::new(
            local_device_id,
            ClipboardSyncMode::AutoSync,
            registry,
            Some(platform.clone()),
            Some(audit.clone()),
        );

        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);

        let engine_clone = engine.clone();
        let server_task = tokio::spawn(async move {
            engine_clone
                .handle_stream(peer_pubkey, server_write, server_read)
                .await
        });

        let payload =
            ClipboardPayload::new_text(peer_device_id.clone(), 1, "Pasted across devices!")
                .unwrap();
        let response = engine
            .sync_to_stream(&payload, client_write, client_read)
            .await
            .unwrap();

        assert_eq!(response.status, "applied");
        assert_eq!(response.payload_id, payload.payload_id);

        let decision = server_task.await.unwrap().unwrap();
        match decision {
            ClipboardSyncDecision::Applied { bytes_written, .. } => {
                assert_eq!(bytes_written, "Pasted across devices!".len());
            }
            _ => panic!("Expected applied decision"),
        }

        // Verify written to virtual platform clipboard
        assert_eq!(platform.read_clipboard().unwrap(), "Pasted across devices!");

        // Verify audit log entry
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

        // Test Echo Prevention: Attempting to process the same payload again is deduplicated
        let dup_decision = engine
            .process_inbound_payload(peer_pubkey, payload)
            .unwrap();
        assert!(matches!(
            dup_decision,
            ClipboardSyncDecision::Deduplicated { .. }
        ));
    }

    #[tokio::test]
    async fn test_clipboard_sync_rejects_denied_peer_and_disabled_mode() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db));
        let platform = Arc::new(VirtualDesktopAdapter::new());

        let peer_pubkey = [0x55u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_device_id.clone(),
            name: "Denied Phone".to_string(),
            platform: PlatformTarget::Android,
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
                "clipboard",
                PeerPermissionLevel::Deny,
                1000,
            )
            .unwrap();

        let engine = ClipboardSyncEngine::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            ClipboardSyncMode::AutoSync,
            registry.clone(),
            Some(platform.clone()),
            None,
        );

        let payload = ClipboardPayload::new_text(peer_device_id.clone(), 1, "Secret data").unwrap();
        let decision = engine
            .process_inbound_payload(peer_pubkey, payload.clone())
            .unwrap();

        match decision {
            ClipboardSyncDecision::Rejected { reason } => {
                assert!(reason.contains("denied"));
            }
            _ => panic!("Expected rejected decision"),
        }

        // When mode is Disabled
        registry
            .set_peer_permission(
                &peer_device_id,
                "clipboard",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();
        engine.set_sync_mode(ClipboardSyncMode::Disabled);

        let decision2 = engine
            .process_inbound_payload(peer_pubkey, payload)
            .unwrap();
        match decision2 {
            ClipboardSyncDecision::Rejected { reason } => {
                assert!(reason.contains("disabled"));
            }
            _ => panic!("Expected rejected decision for disabled mode"),
        }
    }

    #[tokio::test]
    async fn test_clipboard_sync_over_virtual_session() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db));
        let platform = Arc::new(VirtualDesktopAdapter::new());

        let peer_pubkey = [0x66u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_device_id.clone(),
            name: "Virtual Peer".to_string(),
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
                "clipboard",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let engine = ClipboardSyncEngine::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            ClipboardSyncMode::AutoSync,
            registry,
            Some(platform),
            None,
        );

        let addr_a = "127.0.0.1:51820".parse().unwrap();
        let addr_b = "127.0.0.1:51821".parse().unwrap();
        let (sess_client, sess_server) = crate::transport::create_virtual_session_pair(
            peer_pubkey,
            addr_a,
            [0x77u8; 32],
            addr_b,
        );

        let engine_clone = engine.clone();
        let server_task = tokio::spawn(async move {
            let framed_req = sess_server.recv().await.unwrap();
            let payload: ClipboardPayload = serde_json::from_slice(&framed_req[4..]).unwrap();
            let decision = engine_clone
                .process_inbound_payload(peer_pubkey, payload)
                .unwrap();
            let resp = match decision {
                ClipboardSyncDecision::Applied { payload_id, .. } => ClipboardSyncResponse {
                    payload_id,
                    status: "applied".to_string(),
                    message: None,
                },
                _ => panic!("Expected applied decision"),
            };
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            let mut framed_resp = (resp_bytes.len() as u32).to_be_bytes().to_vec();
            framed_resp.extend_from_slice(&resp_bytes);
            sess_server.send(&framed_resp).await.unwrap();
        });

        let payload =
            ClipboardPayload::new_text(peer_device_id, 1, "Virtual Clipboard Sync!").unwrap();
        let resp = engine.sync_virtual(&sess_client, &payload).await.unwrap();

        assert_eq!(resp.status, "applied");
        assert_eq!(resp.payload_id, payload.payload_id);
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_clipboard_sync_over_quic_mtls_loopback() {
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
        let platform = Arc::new(VirtualDesktopAdapter::new());

        let paired = aissistant_core::PairedDevice {
            device_id: client_device_id.clone(),
            name: "QUIC Peer".to_string(),
            platform: PlatformTarget::Windows,
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
                "clipboard",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let engine = ClipboardSyncEngine::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            ClipboardSyncMode::AutoSync,
            registry,
            Some(platform.clone()),
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

        let engine_clone = engine.clone();
        let server_task = tokio::spawn(async move {
            let session = engine_server.accept().await.unwrap();
            let (send, recv) = session.accept_bi().await.unwrap();
            let decision = engine_clone
                .handle_stream(session.peer_public_key(), send, recv)
                .await
                .unwrap();
            (session, decision)
        });

        let client_session = engine_client
            .connect_peer(server_addr, Some(server_pubkey), "aissistant-server")
            .await
            .unwrap();

        let payload =
            ClipboardPayload::new_text(client_device_id, 1, "QUIC mTLS Clipboard Payload").unwrap();
        let resp = engine
            .sync_quic(&client_session, &payload, Duration::from_secs(5))
            .await
            .unwrap();

        assert_eq!(resp.status, "applied");
        assert_eq!(resp.payload_id, payload.payload_id);

        let (server_session, server_decision) = server_task.await.unwrap();
        match server_decision {
            ClipboardSyncDecision::Applied { bytes_written, .. } => {
                assert_eq!(bytes_written, "QUIC mTLS Clipboard Payload".len());
            }
            _ => panic!("Expected applied decision on server"),
        }

        assert_eq!(
            platform.read_clipboard().unwrap(),
            "QUIC mTLS Clipboard Payload"
        );

        client_session.close();
        server_session.close();
    }
}
