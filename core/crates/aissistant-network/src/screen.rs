//! Low-latency cross-device screen capture and frame streaming over QUIC/mTLS streams.

use std::sync::atomic::{AtomicBool, Ordering};
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

/// Pixel and compression format of screen frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenPixelFormat {
    /// 24-bit RGB raw uncompressed pixels.
    Rgb8,
    /// 32-bit RGBA raw uncompressed pixels.
    Rgba8,
    /// 32-bit BGRA raw uncompressed pixels (Windows native).
    Bgra8,
    /// JPEG encoded compressed frame.
    Jpeg,
    /// H.264 / AVC video keyframe or slice.
    H264Keyframe,
}

/// Configuration settings for a screen streaming session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenStreamConfig {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Target frame rate in frames per second (15–60 FPS).
    pub target_fps: u32,
    /// Target bit rate in kilobits per second.
    pub bitrate_kbps: u32,
    /// Desired pixel / compression format.
    pub pixel_format: ScreenPixelFormat,
}

impl Default for ScreenStreamConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            target_fps: 30,
            bitrate_kbps: 2500,
            pixel_format: ScreenPixelFormat::Jpeg,
        }
    }
}

/// Metadata header describing an individual screen frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenFrameHeader {
    /// Matches the active stream `session_id`.
    pub session_id: String,
    /// Monotonically increasing frame index.
    pub frame_index: u64,
    /// Capture timestamp in microseconds.
    pub timestamp_us: u64,
    /// Whether this frame is an intra-coded keyframe.
    pub is_keyframe: bool,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Format of data payload.
    pub pixel_format: ScreenPixelFormat,
    /// Byte length of data buffer.
    pub data_len: u32,
    /// BLAKE3 256-bit cryptographic digest of frame bytes.
    pub frame_hash: [u8; 32],
}

/// A complete screen frame consisting of metadata header and raw frame bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenFrame {
    /// Frame header metadata.
    pub header: ScreenFrameHeader,
    /// Compressed or raw frame buffer bytes.
    pub data: Vec<u8>,
}

impl ScreenFrame {
    /// Creates a new screen frame, automatically computing its BLAKE3 cryptographic hash.
    #[must_use]
    pub fn new(
        session_id: String,
        frame_index: u64,
        timestamp_us: u64,
        is_keyframe: bool,
        dimensions: (u32, u32),
        pixel_format: ScreenPixelFormat,
        data: Vec<u8>,
    ) -> Self {
        let (width, height) = dimensions;
        let data_len = data.len() as u32;
        let frame_hash = *blake3::hash(&data).as_bytes();

        Self {
            header: ScreenFrameHeader {
                session_id,
                frame_index,
                timestamp_us,
                is_keyframe,
                width,
                height,
                pixel_format,
                data_len,
                frame_hash,
            },
            data,
        }
    }

    /// Validates frame buffer boundary integrity and cryptographic hash.
    pub fn validate(&self) -> Result<()> {
        if self.data.len() != self.header.data_len as usize {
            return Err(NetworkError::Internal(format!(
                "Frame data length {} does not match header data_len {}",
                self.data.len(),
                self.header.data_len
            )));
        }

        let computed_hash = *blake3::hash(&self.data).as_bytes();
        if computed_hash != self.header.frame_hash {
            return Err(NetworkError::Internal(
                "Tampered frame: BLAKE3 hash mismatch".to_string(),
            ));
        }

        Ok(())
    }
}

/// Session control messages exchanged between stream host and viewer client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "control_type", rename_all = "snake_case")]
pub enum ScreenStreamControl {
    /// Viewer requests initiation of a screen stream with desired configuration.
    StartSession {
        /// Viewer's authenticated DeviceId.
        caller_device_id: DeviceId,
        /// Desired streaming configuration.
        config: ScreenStreamConfig,
    },
    /// Host acknowledges session startup.
    SessionStarted {
        /// Unique session identifier.
        session_id: String,
        /// Agreed configuration.
        config: ScreenStreamConfig,
    },
    /// Viewer requests an immediate intra-refresh keyframe (e.g. after frame loss).
    RequestKeyframe {
        /// Active session identifier.
        session_id: String,
        /// Last frame successfully received.
        last_received_frame: u64,
    },
    /// Viewer or host terminates the streaming session.
    StopSession {
        /// Active session identifier.
        session_id: String,
    },
    /// Acknowledgment of session shutdown.
    SessionStopped {
        /// Active session identifier.
        session_id: String,
        /// Optional reason for termination.
        reason: Option<String>,
    },
    /// Host rejects session request due to ACL, policy, or busy state.
    SessionRejected {
        /// Human-readable explanation.
        reason: String,
    },
}

/// Wire envelope multiplexing control and frame messages across the stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "wire_type", rename_all = "snake_case")]
pub enum ScreenWireMessage {
    /// Control signal.
    Control(ScreenStreamControl),
    /// Frame payload.
    Frame(ScreenFrame),
}

/// Trait defining the platform or virtual screen capture provider.
pub trait ScreenCaptureProvider: Send + Sync {
    /// Returns the native capture resolution `(width, height)`.
    fn resolution(&self) -> (u32, u32);

    /// Captures a single frame.
    fn capture_frame(
        &mut self,
        session_id: &str,
        frame_index: u64,
        is_keyframe: bool,
    ) -> Result<ScreenFrame>;
}

/// Hermetic synthetic screen capture provider for testing and headless CI.
#[derive(Debug, Clone)]
pub struct VirtualScreenCaptureProvider {
    width: u32,
    height: u32,
    pixel_format: ScreenPixelFormat,
}

impl VirtualScreenCaptureProvider {
    /// Creates a new virtual capture provider with specified dimensions and format.
    #[must_use]
    pub fn new(width: u32, height: u32, pixel_format: ScreenPixelFormat) -> Self {
        Self {
            width,
            height,
            pixel_format,
        }
    }
}

impl ScreenCaptureProvider for VirtualScreenCaptureProvider {
    fn resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn capture_frame(
        &mut self,
        session_id: &str,
        frame_index: u64,
        is_keyframe: bool,
    ) -> Result<ScreenFrame> {
        // Generate deterministic test pattern based on frame index
        let pattern_byte = ((frame_index * 17) % 256) as u8;
        let data_size = match self.pixel_format {
            ScreenPixelFormat::Jpeg | ScreenPixelFormat::H264Keyframe => 1024,
            ScreenPixelFormat::Rgb8 => (self.width * self.height * 3).min(64 * 1024) as usize,
            ScreenPixelFormat::Rgba8 | ScreenPixelFormat::Bgra8 => {
                (self.width * self.height * 4).min(64 * 1024) as usize
            }
        };

        let mut data = vec![pattern_byte; data_size];
        // Embed frame index in first 8 bytes for traceability
        data[..8.min(data_size)].copy_from_slice(&frame_index.to_be_bytes()[..8.min(data_size)]);

        let now_us = (current_time_ms() * 1000) + (frame_index * 33_333);

        Ok(ScreenFrame::new(
            session_id.to_string(),
            frame_index,
            now_us,
            is_keyframe,
            (self.width, self.height),
            self.pixel_format,
            data,
        ))
    }
}

/// Host screen streaming engine enforcing policy gating, active session state, and frame dispatch.
#[derive(Clone)]
pub struct ScreenStreamingHost {
    local_device_id: DeviceId,
    registry: Arc<DeviceRegistry>,
    audit_store: Option<Arc<AuditStore>>,
    is_streaming_active: Arc<AtomicBool>,
}

impl std::fmt::Debug for ScreenStreamingHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScreenStreamingHost")
            .field("local_device_id", &self.local_device_id)
            .field("is_streaming_active", &self.is_streaming_active())
            .field("has_audit_store", &self.audit_store.is_some())
            .finish()
    }
}

impl ScreenStreamingHost {
    /// Creates a new `ScreenStreamingHost`.
    #[must_use]
    pub fn new(
        local_device_id: DeviceId,
        registry: Arc<DeviceRegistry>,
        audit_store: Option<Arc<AuditStore>>,
    ) -> Self {
        Self {
            local_device_id,
            registry,
            audit_store,
            is_streaming_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns `true` if a screen streaming session is currently active.
    #[must_use]
    pub fn is_streaming_active(&self) -> bool {
        self.is_streaming_active.load(Ordering::SeqCst)
    }

    /// Handles an incoming streaming session connection from an authenticated peer.
    pub async fn handle_stream<
        S: AsyncWrite + Unpin,
        R: AsyncRead + Unpin,
        P: ScreenCaptureProvider,
    >(
        &self,
        peer_public_key: [u8; 32],
        mut send: S,
        mut recv: R,
        mut provider: P,
        max_frames: Option<u64>,
    ) -> Result<()> {
        // 1. Await StartSession control message
        let initial_msg: ScreenWireMessage = read_framed_message(&mut recv).await?;
        let (caller_device_id, config) = match initial_msg {
            ScreenWireMessage::Control(ScreenStreamControl::StartSession {
                caller_device_id,
                config,
            }) => (caller_device_id, config),
            _ => {
                let err = ScreenStreamControl::SessionRejected {
                    reason: "Expected StartSession control message as first frame".to_string(),
                };
                let _ = write_framed_message(&mut send, &ScreenWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(
                    "Invalid initial screen stream frame".to_string(),
                ));
            }
        };

        // 2. Authenticate caller DeviceId against peer TLS Ed25519 public key
        let hex_prefix: String = peer_public_key[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let expected_device_id_str = format!("dev_{hex_prefix}");

        if caller_device_id.as_str() != expected_device_id_str {
            let err = ScreenStreamControl::SessionRejected {
                reason: "Origin DeviceId does not match TLS authenticated public key".to_string(),
            };
            let _ = write_framed_message(&mut send, &ScreenWireMessage::Control(err)).await;
            return Err(NetworkError::Internal("DeviceId spoofing detected".into()));
        }

        // 3. Verify sender trust state in DeviceRegistry
        let paired_device = match self.registry.get_device(&caller_device_id) {
            Ok(Some(dev)) => dev,
            Ok(None) => {
                let err = ScreenStreamControl::SessionRejected {
                    reason: "Unknown or unpaired viewer device".to_string(),
                };
                let _ = write_framed_message(&mut send, &ScreenWireMessage::Control(err)).await;
                return Err(NetworkError::Internal("Unpaired viewer device".into()));
            }
            Err(e) => {
                let err = ScreenStreamControl::SessionRejected {
                    reason: format!("Database error reading peer device: {e}"),
                };
                let _ = write_framed_message(&mut send, &ScreenWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(e.to_string()));
            }
        };

        if paired_device.trust_state != DeviceTrustState::Trusted {
            let err = ScreenStreamControl::SessionRejected {
                reason: format!(
                    "Peer device trust state is '{}' (not trusted)",
                    paired_device.trust_state
                ),
            };
            let _ = write_framed_message(&mut send, &ScreenWireMessage::Control(err)).await;
            return Err(NetworkError::Internal("Untrusted viewer device".into()));
        }

        // 4. Verify Peer Capability ACL (screen_streaming_host or lan_peer_networking)
        let permissions = match self.registry.get_peer_permissions(&caller_device_id) {
            Ok(perms) => perms,
            Err(e) => {
                let err = ScreenStreamControl::SessionRejected {
                    reason: format!("Database error querying peer permissions: {e}"),
                };
                let _ = write_framed_message(&mut send, &ScreenWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(e.to_string()));
            }
        };

        let perm_level = permissions
            .get("screen_streaming_host")
            .or_else(|| permissions.get("lan_peer_networking"))
            .copied()
            .unwrap_or(PeerPermissionLevel::Deny);

        match perm_level {
            PeerPermissionLevel::Deny => {
                self.record_audit_log(
                    &caller_device_id,
                    "screen.stream",
                    PolicyDecision::Deny {
                        reason: "Screen streaming denied by peer capability ACL".to_string(),
                    },
                    AuditExecutionStatus::Cancelled,
                );

                let err = ScreenStreamControl::SessionRejected {
                    reason: "Screen streaming capability is denied for this peer".to_string(),
                };
                let _ = write_framed_message(&mut send, &ScreenWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(
                    "Permission denied by screen ACL".into(),
                ));
            }
            PeerPermissionLevel::AskEachTime => {
                self.record_audit_log(
                    &caller_device_id,
                    "screen.stream",
                    PolicyDecision::RequireConfirmation {
                        reason: "Screen streaming requires explicit user approval".to_string(),
                        risk_tier: aissistant_security::RiskTier::PrivilegedSystem,
                    },
                    AuditExecutionStatus::Cancelled,
                );

                let err = ScreenStreamControl::SessionRejected {
                    reason: "Screen streaming requires interactive approval (unattended streaming rejected)".to_string(),
                };
                let _ = write_framed_message(&mut send, &ScreenWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(
                    "Interactive approval required".into(),
                ));
            }
            PeerPermissionLevel::AlwaysAllow => {
                // Authorized
            }
        }

        // 5. Initialize active streaming session
        let session_id = format!("screen_{}", uuid::Uuid::new_v4().simple());
        self.is_streaming_active.store(true, Ordering::SeqCst);

        let started_msg = ScreenWireMessage::Control(ScreenStreamControl::SessionStarted {
            session_id: session_id.clone(),
            config: config.clone(),
        });
        write_framed_message(&mut send, &started_msg).await?;

        // Record session initiation audit log
        self.record_audit_log(
            &caller_device_id,
            "screen.stream_start",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        // 6. Frame dispatch loop
        let mut frame_index: u64 = 0;
        let limit = max_frames.unwrap_or(u64::MAX);

        while frame_index < limit {
            let is_keyframe = frame_index % 30 == 0;
            let frame = provider.capture_frame(&session_id, frame_index, is_keyframe)?;

            write_framed_message(&mut send, &ScreenWireMessage::Frame(frame)).await?;
            frame_index += 1;
        }

        // 7. Graceful session termination
        self.is_streaming_active.store(false, Ordering::SeqCst);
        let stop_msg = ScreenWireMessage::Control(ScreenStreamControl::SessionStopped {
            session_id,
            reason: Some("Stream finished".to_string()),
        });
        write_framed_message(&mut send, &stop_msg).await?;

        self.record_audit_log(
            &caller_device_id,
            "screen.stream_stop",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        Ok(())
    }

    fn record_audit_log(
        &self,
        peer_device_id: &DeviceId,
        action: &str,
        policy_decision: PolicyDecision,
        execution_status: AuditExecutionStatus,
    ) {
        if let Some(audit) = &self.audit_store {
            let prev_hash = audit.latest_hash().unwrap_or(GENESIS_PREV_HASH);
            let skill_id = match SkillId::new(action) {
                Ok(s) => s,
                Err(_) => return,
            };

            let entry = AuditEntry::new(NewAuditEntry {
                id: AuditEntryId::generate(),
                timestamp_ms: current_time_ms(),
                actor: Actor::RemotePeer {
                    device_id: peer_device_id.clone(),
                },
                skill_id,
                arguments_redacted_json: "{}".to_string(),
                policy_decision,
                execution_status,
                prev_hash,
            });

            let _ = audit.append(&entry);
        }
    }
}

/// Viewer client engine initiating sessions and receiving streaming frames.
#[derive(Debug, Clone)]
pub struct ScreenStreamReceiver {
    local_device_id: DeviceId,
}

impl ScreenStreamReceiver {
    /// Creates a new `ScreenStreamReceiver`.
    #[must_use]
    pub fn new(local_device_id: DeviceId) -> Self {
        Self { local_device_id }
    }

    /// Initiates a screen streaming session with the remote host.
    pub async fn start_session<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        config: ScreenStreamConfig,
        mut send: S,
        mut recv: R,
    ) -> Result<(String, ScreenStreamConfig)> {
        let req = ScreenWireMessage::Control(ScreenStreamControl::StartSession {
            caller_device_id: self.local_device_id.clone(),
            config,
        });

        write_framed_message(&mut send, &req).await?;
        let resp_msg: ScreenWireMessage = read_framed_message(&mut recv).await?;

        match resp_msg {
            ScreenWireMessage::Control(ScreenStreamControl::SessionStarted {
                session_id,
                config,
            }) => Ok((session_id, config)),
            ScreenWireMessage::Control(ScreenStreamControl::SessionRejected { reason }) => Err(
                NetworkError::Internal(format!("Host rejected session: {reason}")),
            ),
            _ => Err(NetworkError::Internal(
                "Unexpected response to StartSession".into(),
            )),
        }
    }

    /// Reads the next frame or control message from the active streaming stream.
    pub async fn read_message<R: AsyncRead + Unpin>(
        &self,
        mut recv: R,
    ) -> Result<ScreenWireMessage> {
        read_framed_message(&mut recv).await
    }

    /// Streams screen frames over an established QUIC session.
    pub async fn receive_quic_stream(
        &self,
        session: &QuicSession,
        config: ScreenStreamConfig,
        max_frames: u64,
        timeout: Duration,
    ) -> Result<Vec<ScreenFrame>> {
        tokio::time::timeout(timeout, async {
            let (mut send, mut recv) = session.open_bi().await?;
            let (_session_id, _conf) = self.start_session(config, &mut send, &mut recv).await?;

            let mut frames = Vec::new();
            while (frames.len() as u64) < max_frames {
                let msg = self.read_message(&mut recv).await?;
                match msg {
                    ScreenWireMessage::Frame(frame) => {
                        frame.validate()?;
                        frames.push(frame);
                    }
                    ScreenWireMessage::Control(ScreenStreamControl::SessionStopped { .. }) => {
                        break;
                    }
                    _ => {}
                }
            }

            Ok(frames)
        })
        .await
        .map_err(|_| NetworkError::Internal("Screen stream over QUIC timed out".into()))?
    }

    /// Streams screen frames over a virtual session pair.
    pub async fn receive_virtual_stream(
        &self,
        session: &VirtualSession,
        config: ScreenStreamConfig,
        max_frames: u64,
    ) -> Result<Vec<ScreenFrame>> {
        // Send StartSession
        let req = ScreenWireMessage::Control(ScreenStreamControl::StartSession {
            caller_device_id: self.local_device_id.clone(),
            config,
        });
        let req_bytes =
            serde_json::to_vec(&req).map_err(|e| NetworkError::Internal(e.to_string()))?;
        let mut framed = (req_bytes.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&req_bytes);
        session.send(&framed).await?;

        // Read SessionStarted
        let resp_framed = session.recv().await?;
        let resp_msg: ScreenWireMessage = serde_json::from_slice(&resp_framed[4..])
            .map_err(|e| NetworkError::Internal(e.to_string()))?;

        match resp_msg {
            ScreenWireMessage::Control(ScreenStreamControl::SessionStarted { .. }) => {}
            ScreenWireMessage::Control(ScreenStreamControl::SessionRejected { reason }) => {
                return Err(NetworkError::Internal(reason));
            }
            _ => return Err(NetworkError::Internal("Unexpected virtual response".into())),
        }

        let mut frames = Vec::new();
        while (frames.len() as u64) < max_frames {
            let frame_raw = session.recv().await?;
            let msg: ScreenWireMessage = serde_json::from_slice(&frame_raw[4..])
                .map_err(|e| NetworkError::Internal(e.to_string()))?;
            match msg {
                ScreenWireMessage::Frame(frame) => {
                    frame.validate()?;
                    frames.push(frame);
                }
                ScreenWireMessage::Control(ScreenStreamControl::SessionStopped { .. }) => {
                    break;
                }
                _ => {}
            }
        }

        Ok(frames)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use aissistant_core::PlatformTarget;
    use aissistant_storage::SqliteDb;

    #[test]
    fn test_virtual_screen_frame_generation_and_validation() {
        let mut provider = VirtualScreenCaptureProvider::new(1280, 720, ScreenPixelFormat::Jpeg);
        let frame = provider.capture_frame("sess_123", 0, true).unwrap();

        assert_eq!(frame.header.frame_index, 0);
        assert!(frame.header.is_keyframe);
        assert_eq!(frame.header.width, 1280);
        assert_eq!(frame.header.height, 720);
        assert!(frame.validate().is_ok());

        // Tamper with frame data
        let mut tampered = frame.clone();
        tampered.data[10] ^= 0xFF;
        assert!(tampered.validate().is_err());
        assert!(tampered
            .validate()
            .unwrap_err()
            .to_string()
            .contains("hash mismatch"));
    }

    #[tokio::test]
    async fn test_screen_stream_end_to_end_and_active_indicator() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        let audit = Arc::new(AuditStore::new(db));

        let peer_pubkey = [0x55u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_device_id.clone(),
            name: "Viewer Laptop".to_string(),
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
                "screen_streaming_host",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let host = ScreenStreamingHost::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            registry,
            Some(audit.clone()),
        );

        assert!(!host.is_streaming_active());

        let receiver = ScreenStreamReceiver::new(peer_device_id.clone());

        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);

        let host_clone = host.clone();
        let provider = VirtualScreenCaptureProvider::new(1280, 720, ScreenPixelFormat::Jpeg);

        let server_task = tokio::spawn(async move {
            host_clone
                .handle_stream(peer_pubkey, server_write, server_read, provider, Some(5))
                .await
        });

        let (session_id, _conf) = receiver
            .start_session(ScreenStreamConfig::default(), client_write, client_read)
            .await
            .unwrap();

        assert!(session_id.starts_with("screen_"));

        server_task.await.unwrap().unwrap();
        assert!(!host.is_streaming_active());

        // Verify audit records
        let logs = audit.get_entries(10, 0).unwrap();
        assert!(logs.len() >= 2);
        assert_eq!(
            logs[1].actor,
            Actor::RemotePeer {
                device_id: peer_device_id
            }
        );
        assert!(
            aissistant_security::AuditChainVerifier::verify_chain(&logs, &GENESIS_PREV_HASH)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_screen_stream_rejects_denied_peer() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db));

        let peer_pubkey = [0x77u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_device_id.clone(),
            name: "Denied Peer".to_string(),
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
                "screen_streaming_host",
                PeerPermissionLevel::Deny,
                1000,
            )
            .unwrap();

        let host = ScreenStreamingHost::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            registry,
            None,
        );

        let receiver = ScreenStreamReceiver::new(peer_device_id);

        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);

        let provider = VirtualScreenCaptureProvider::new(1280, 720, ScreenPixelFormat::Jpeg);
        let server_task = tokio::spawn(async move {
            host.handle_stream(peer_pubkey, server_write, server_read, provider, Some(1))
                .await
        });

        let client_res = receiver
            .start_session(ScreenStreamConfig::default(), client_write, client_read)
            .await;

        assert!(client_res.is_err());
        assert!(client_res.unwrap_err().to_string().contains("denied"));

        let server_res = server_task.await.unwrap();
        assert!(server_res.is_err());
    }

    #[tokio::test]
    async fn test_screen_stream_over_quic_mtls_loopback() {
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

        let paired = aissistant_core::PairedDevice {
            device_id: client_device_id.clone(),
            name: "QUIC Viewer".to_string(),
            platform: PlatformTarget::Linux,
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
                "screen_streaming_host",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let host = ScreenStreamingHost::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
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

        let host_clone = host.clone();
        let server_task = tokio::spawn(async move {
            let session = engine_server.accept().await.unwrap();
            let (send, recv) = session.accept_bi().await.unwrap();
            let provider = VirtualScreenCaptureProvider::new(1280, 720, ScreenPixelFormat::Jpeg);
            host_clone
                .handle_stream(session.peer_public_key(), send, recv, provider, Some(3))
                .await
                .unwrap();
            session
        });

        let client_session = engine_client
            .connect_peer(server_addr, Some(server_pubkey), "aissistant-server")
            .await
            .unwrap();

        let receiver = ScreenStreamReceiver::new(client_device_id);
        let frames = receiver
            .receive_quic_stream(
                &client_session,
                ScreenStreamConfig::default(),
                3,
                Duration::from_secs(5),
            )
            .await
            .unwrap();

        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].header.frame_index, 0);
        assert_eq!(frames[1].header.frame_index, 1);
        assert_eq!(frames[2].header.frame_index, 2);

        let server_session = server_task.await.unwrap();
        client_session.close();
        server_session.close();
    }
}
