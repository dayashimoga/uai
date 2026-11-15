//! Permission-gated remote pointer and keyboard input control over QUIC and virtual streams.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
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
use crate::transport::QuicSession;

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    /// Primary / left mouse button.
    Left,
    /// Secondary / right mouse button.
    Right,
    /// Tertiary / middle mouse button.
    Middle,
    /// Auxiliary button identifier.
    Other(u8),
}

/// Keyboard modifier flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct KeyModifiers {
    /// Shift key pressed.
    pub shift: bool,
    /// Control key pressed.
    pub ctrl: bool,
    /// Alt / Option key pressed.
    pub alt: bool,
    /// Meta / Windows / Command key pressed.
    pub meta: bool,
}

/// Remote input event variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RemoteInputEvent {
    /// Pointer cursor moved to normalized coordinates (0.0 to 1.0).
    MouseMove { x: f32, y: f32 },
    /// Mouse button pressed at normalized coordinates.
    MouseDown { button: MouseButton, x: f32, y: f32 },
    /// Mouse button released at normalized coordinates.
    MouseUp { button: MouseButton, x: f32, y: f32 },
    /// Mouse scroll wheel delta.
    MouseWheel { delta_x: f32, delta_y: f32 },
    /// Keyboard key pressed.
    KeyDown {
        key_code: u32,
        key_text: Option<String>,
        modifiers: KeyModifiers,
    },
    /// Keyboard key released.
    KeyUp {
        key_code: u32,
        modifiers: KeyModifiers,
    },
    /// Direct text snippet entry.
    TextInput { text: String },
}

impl RemoteInputEvent {
    /// Sanitizes and clamps coordinates if the event is a pointer event.
    #[must_use]
    pub fn sanitized(self) -> Self {
        match self {
            Self::MouseMove { x, y } => Self::MouseMove {
                x: x.clamp(0.0, 1.0),
                y: y.clamp(0.0, 1.0),
            },
            Self::MouseDown { button, x, y } => Self::MouseDown {
                button,
                x: x.clamp(0.0, 1.0),
                y: y.clamp(0.0, 1.0),
            },
            Self::MouseUp { button, x, y } => Self::MouseUp {
                button,
                x: x.clamp(0.0, 1.0),
                y: y.clamp(0.0, 1.0),
            },
            other => other,
        }
    }
}

/// Session control messages for remote input injection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RemoteInputSessionControl {
    /// Client requests input control session.
    RequestControl {
        session_id: String,
        caller_device_id: DeviceId,
    },
    /// Host grants input control session.
    ControlGranted { session_id: String },
    /// Host rejects input control session.
    ControlRejected { session_id: String, reason: String },
    /// Client voluntarily releases input control.
    ReleaseControl { session_id: String },
    /// Host or client abruptly terminates session (e.g. host user pressed Esc).
    ControlTerminated { session_id: String, reason: String },
}

/// Envelope for length-delimited remote input protocol wire messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemoteInputWireMessage {
    /// Session lifecycle control message.
    Control(RemoteInputSessionControl),
    /// Batch of input events to be injected.
    Events { events: Vec<RemoteInputEvent> },
}

/// Trait for platform or synthetic remote input event injectors.
pub trait InputInjector: Send + Sync {
    /// Injects a single input event.
    fn inject_event(&self, event: &RemoteInputEvent) -> Result<()>;

    /// Injects a batch of input events sequentially.
    fn inject_batch(&self, events: &[RemoteInputEvent]) -> Result<()> {
        for event in events {
            self.inject_event(event)?;
        }
        Ok(())
    }
}

/// Hermetic in-memory input injector for testing and headless CI.
#[derive(Debug, Default)]
pub struct VirtualInputInjector {
    events: RwLock<Vec<RemoteInputEvent>>,
}

impl VirtualInputInjector {
    /// Creates a new virtual input injector.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a copy of all injected events recorded so far.
    #[must_use]
    pub fn events(&self) -> Vec<RemoteInputEvent> {
        self.events.read().map(|g| g.clone()).unwrap_or_default()
    }

    /// Returns the number of events injected.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Clears recorded events.
    pub fn clear(&self) {
        if let Ok(mut g) = self.events.write() {
            g.clear();
        }
    }
}

impl InputInjector for VirtualInputInjector {
    fn inject_event(&self, event: &RemoteInputEvent) -> Result<()> {
        if let Ok(mut g) = self.events.write() {
            g.push(event.clone().sanitized());
        }
        Ok(())
    }
}

/// Active session tracking on host.
#[derive(Debug, Clone)]
pub struct ActiveInputSession {
    /// Unique session identifier.
    pub session_id: String,
    /// Paired controller DeviceId.
    pub caller_device_id: DeviceId,
    /// Timestamp when session was granted.
    pub started_at_ms: u64,
    /// Monotonic count of events successfully injected in this session.
    pub events_injected: Arc<AtomicUsize>,
}

/// Host remote input controller enforcing TLS caller authentication,
/// capability ACLs, active session state, emergency escape, and audit logging.
#[derive(Clone)]
pub struct RemoteInputHost {
    local_device_id: DeviceId,
    registry: Arc<DeviceRegistry>,
    audit_store: Option<Arc<AuditStore>>,
    injector: Arc<dyn InputInjector>,
    active_session: Arc<RwLock<Option<ActiveInputSession>>>,
    emergency_stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for RemoteInputHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteInputHost")
            .field("local_device_id", &self.local_device_id)
            .field("is_control_active", &self.is_control_active())
            .field("has_audit_store", &self.audit_store.is_some())
            .finish()
    }
}

impl RemoteInputHost {
    /// Creates a new `RemoteInputHost`.
    #[must_use]
    pub fn new(
        local_device_id: DeviceId,
        registry: Arc<DeviceRegistry>,
        audit_store: Option<Arc<AuditStore>>,
        injector: Arc<dyn InputInjector>,
    ) -> Self {
        Self {
            local_device_id,
            registry,
            audit_store,
            injector,
            active_session: Arc::new(RwLock::new(None)),
            emergency_stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns `true` if a remote control session is currently active and not terminated.
    #[must_use]
    pub fn is_control_active(&self) -> bool {
        if self.emergency_stop.load(Ordering::SeqCst) {
            return false;
        }
        self.active_session
            .read()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }

    /// Returns the active session ID, if one is running.
    #[must_use]
    pub fn active_session_id(&self) -> Option<String> {
        if self.emergency_stop.load(Ordering::SeqCst) {
            None
        } else {
            self.active_session
                .read()
                .ok()
                .and_then(|g| g.as_ref().map(|s| s.session_id.clone()))
        }
    }

    /// Returns the caller DeviceId of the active controller, if active.
    #[must_use]
    pub fn active_caller_id(&self) -> Option<DeviceId> {
        if self.emergency_stop.load(Ordering::SeqCst) {
            None
        } else {
            self.active_session
                .read()
                .ok()
                .and_then(|g| g.as_ref().map(|s| s.caller_device_id.clone()))
        }
    }

    /// Host emergency escape trigger: immediately revokes remote control.
    /// Can be called from local key listeners (Escape key) or host UI Stop buttons.
    pub fn terminate_session(&self, reason: &str) -> bool {
        self.emergency_stop.store(true, Ordering::SeqCst);
        let session = if let Ok(mut guard) = self.active_session.write() {
            guard.take()
        } else {
            None
        };
        if let Some(session) = session {
            let count = session.events_injected.load(Ordering::SeqCst);
            self.record_audit_log(
                &session.caller_device_id,
                "input.terminate",
                PolicyDecision::Allow,
                AuditExecutionStatus::Success,
                Some(&format!(
                    "Session '{}' terminated by host user: {reason}. Total events: {count}",
                    session.session_id
                )),
            );
            true
        } else {
            false
        }
    }

    /// Handles an incoming input control stream from an authenticated peer.
    pub async fn handle_stream<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        peer_public_key: [u8; 32],
        mut send: S,
        mut recv: R,
    ) -> Result<()> {
        // 1. Await RequestControl message
        let initial_msg: RemoteInputWireMessage = read_framed_message(&mut recv).await?;
        let (session_id, caller_device_id) = match initial_msg {
            RemoteInputWireMessage::Control(RemoteInputSessionControl::RequestControl {
                session_id,
                caller_device_id,
            }) => (session_id, caller_device_id),
            _ => {
                let err = RemoteInputSessionControl::ControlRejected {
                    session_id: "unknown".to_string(),
                    reason: "Expected RequestControl as first message".to_string(),
                };
                let _ =
                    write_framed_message(&mut send, &RemoteInputWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(
                    "Invalid initial remote input message".to_string(),
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
            let err = RemoteInputSessionControl::ControlRejected {
                session_id: session_id.clone(),
                reason: "Origin DeviceId does not match TLS authenticated public key".to_string(),
            };
            let _ = write_framed_message(&mut send, &RemoteInputWireMessage::Control(err)).await;
            return Err(NetworkError::Internal("DeviceId spoofing detected".into()));
        }

        // 3. Verify sender trust state in DeviceRegistry
        let paired_device = match self.registry.get_device(&caller_device_id) {
            Ok(Some(dev)) => dev,
            Ok(None) => {
                let err = RemoteInputSessionControl::ControlRejected {
                    session_id: session_id.clone(),
                    reason: "Unknown or unpaired controller device".to_string(),
                };
                let _ =
                    write_framed_message(&mut send, &RemoteInputWireMessage::Control(err)).await;
                return Err(NetworkError::Internal("Unpaired controller device".into()));
            }
            Err(e) => {
                let err = RemoteInputSessionControl::ControlRejected {
                    session_id: session_id.clone(),
                    reason: format!("Database error reading peer device: {e}"),
                };
                let _ =
                    write_framed_message(&mut send, &RemoteInputWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(e.to_string()));
            }
        };

        if paired_device.trust_state != DeviceTrustState::Trusted {
            let err = RemoteInputSessionControl::ControlRejected {
                session_id: session_id.clone(),
                reason: format!(
                    "Peer device trust state is '{}' (not trusted)",
                    paired_device.trust_state
                ),
            };
            let _ = write_framed_message(&mut send, &RemoteInputWireMessage::Control(err)).await;
            return Err(NetworkError::Internal("Untrusted controller device".into()));
        }

        // 4. Verify Peer Capability ACL (remote_input_injection_host)
        let permissions = match self.registry.get_peer_permissions(&caller_device_id) {
            Ok(perms) => perms,
            Err(e) => {
                let err = RemoteInputSessionControl::ControlRejected {
                    session_id: session_id.clone(),
                    reason: format!("Database error querying peer permissions: {e}"),
                };
                let _ =
                    write_framed_message(&mut send, &RemoteInputWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(e.to_string()));
            }
        };

        let perm_level = permissions
            .get("remote_input_injection_host")
            .copied()
            .unwrap_or(PeerPermissionLevel::Deny);

        match perm_level {
            PeerPermissionLevel::Deny => {
                self.record_audit_log(
                    &caller_device_id,
                    "input.request",
                    PolicyDecision::Deny {
                        reason: "Remote input injection denied by peer capability ACL".to_string(),
                    },
                    AuditExecutionStatus::Cancelled,
                    None,
                );
                let err = RemoteInputSessionControl::ControlRejected {
                    session_id: session_id.clone(),
                    reason: "Remote input injection capability is denied for this peer".to_string(),
                };
                let _ =
                    write_framed_message(&mut send, &RemoteInputWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(
                    "Permission denied by input ACL".into(),
                ));
            }
            PeerPermissionLevel::AskEachTime => {
                self.record_audit_log(
                    &caller_device_id,
                    "input.request",
                    PolicyDecision::RequireConfirmation {
                        reason: "Remote input injection requires explicit user approval"
                            .to_string(),
                        risk_tier: aissistant_security::RiskTier::PrivilegedSystem,
                    },
                    AuditExecutionStatus::Cancelled,
                    None,
                );
                let err = RemoteInputSessionControl::ControlRejected {
                    session_id: session_id.clone(),
                    reason: "Remote input injection requires interactive approval (unattended injection rejected)"
                        .to_string(),
                };
                let _ =
                    write_framed_message(&mut send, &RemoteInputWireMessage::Control(err)).await;
                return Err(NetworkError::Internal(
                    "Interactive approval required".into(),
                ));
            }
            PeerPermissionLevel::AlwaysAllow => {
                // Authorized
            }
        }

        // Reset emergency stop flag for the new session
        self.emergency_stop.store(false, Ordering::SeqCst);

        // Set active session
        let events_injected = Arc::new(AtomicUsize::new(0));
        if let Ok(mut guard) = self.active_session.write() {
            *guard = Some(ActiveInputSession {
                session_id: session_id.clone(),
                caller_device_id: caller_device_id.clone(),
                started_at_ms: current_time_ms(),
                events_injected: Arc::clone(&events_injected),
            });
        }

        // Acknowledge session grant
        let grant = RemoteInputSessionControl::ControlGranted {
            session_id: session_id.clone(),
        };
        write_framed_message(&mut send, &RemoteInputWireMessage::Control(grant)).await?;

        self.record_audit_log(
            &caller_device_id,
            "input.grant",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
            Some(&format!(
                "Granted remote input control session '{session_id}'"
            )),
        );

        // Event loop
        loop {
            // Check if host emergency stop was activated
            if self.emergency_stop.load(Ordering::SeqCst) {
                let term = RemoteInputSessionControl::ControlTerminated {
                    session_id: session_id.clone(),
                    reason: "Emergency stop activated by host user".to_string(),
                };
                let _ =
                    write_framed_message(&mut send, &RemoteInputWireMessage::Control(term)).await;
                break;
            }

            // Read next wire message
            let msg: RemoteInputWireMessage = match read_framed_message(&mut recv).await {
                Ok(m) => m,
                Err(_) => {
                    // Client disconnected or stream closed
                    break;
                }
            };

            // Re-check emergency stop right after read
            if self.emergency_stop.load(Ordering::SeqCst) {
                let term = RemoteInputSessionControl::ControlTerminated {
                    session_id: session_id.clone(),
                    reason: "Emergency stop activated by host user".to_string(),
                };
                let _ =
                    write_framed_message(&mut send, &RemoteInputWireMessage::Control(term)).await;
                break;
            }

            match msg {
                RemoteInputWireMessage::Control(ctrl) => match ctrl {
                    RemoteInputSessionControl::ReleaseControl { session_id: rel_id } => {
                        if rel_id == session_id {
                            let term = RemoteInputSessionControl::ControlTerminated {
                                session_id: session_id.clone(),
                                reason: "Session released by client".to_string(),
                            };
                            let _ = write_framed_message(
                                &mut send,
                                &RemoteInputWireMessage::Control(term),
                            )
                            .await;
                        }
                        break;
                    }
                    RemoteInputSessionControl::ControlTerminated { .. } => {
                        break;
                    }
                    _ => {}
                },
                RemoteInputWireMessage::Events { events } => {
                    if !events.is_empty() {
                        self.injector.inject_batch(&events)?;
                        events_injected.fetch_add(events.len(), Ordering::SeqCst);
                    }
                }
            }
        }

        // Clean up active session
        let total_injected = events_injected.load(Ordering::SeqCst);
        if let Ok(mut guard) = self.active_session.write() {
            if let Some(s) = guard.as_ref() {
                if s.session_id == session_id {
                    *guard = None;
                }
            }
        }

        self.record_audit_log(
            &caller_device_id,
            "input.terminate",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
            Some(&format!(
                "Session '{session_id}' completed. Total events injected: {total_injected}"
            )),
        );

        Ok(())
    }

    fn record_audit_log(
        &self,
        peer_device_id: &DeviceId,
        action: &str,
        policy_decision: PolicyDecision,
        execution_status: AuditExecutionStatus,
        details: Option<&str>,
    ) {
        if let Some(audit) = &self.audit_store {
            let prev_hash = audit.latest_hash().unwrap_or(GENESIS_PREV_HASH);
            let skill_id = match SkillId::new(action) {
                Ok(s) => s,
                Err(_) => return,
            };

            let args_json = if let Some(msg) = details {
                serde_json::json!({ "details": msg }).to_string()
            } else {
                "{}".to_string()
            };

            let entry = AuditEntry::new(NewAuditEntry {
                id: AuditEntryId::generate(),
                timestamp_ms: current_time_ms(),
                actor: Actor::RemotePeer {
                    device_id: peer_device_id.clone(),
                },
                skill_id,
                arguments_redacted_json: args_json,
                policy_decision,
                execution_status,
                prev_hash,
            });

            let _ = audit.append(&entry);
        }
    }
}

/// Controller client managing remote input session and event dispatch.
#[derive(Debug, Clone)]
pub struct RemoteInputClient {
    local_device_id: DeviceId,
}

impl RemoteInputClient {
    /// Creates a new `RemoteInputClient`.
    #[must_use]
    pub fn new(local_device_id: DeviceId) -> Self {
        Self { local_device_id }
    }

    /// Initiates a remote control session on the host.
    pub async fn request_control<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        session_id: &str,
        send: &mut S,
        recv: &mut R,
    ) -> Result<()> {
        let req = RemoteInputWireMessage::Control(RemoteInputSessionControl::RequestControl {
            session_id: session_id.to_string(),
            caller_device_id: self.local_device_id.clone(),
        });
        write_framed_message(send, &req).await?;

        let resp: RemoteInputWireMessage = read_framed_message(recv).await?;
        match resp {
            RemoteInputWireMessage::Control(RemoteInputSessionControl::ControlGranted {
                session_id: resp_id,
            }) => {
                if resp_id == session_id {
                    Ok(())
                } else {
                    Err(NetworkError::Internal(format!(
                        "Session ID mismatch: expected {session_id}, got {resp_id}"
                    )))
                }
            }
            RemoteInputWireMessage::Control(RemoteInputSessionControl::ControlRejected {
                reason,
                ..
            }) => Err(NetworkError::Internal(format!(
                "Remote control session rejected: {reason}"
            ))),
            other => Err(NetworkError::Internal(format!(
                "Unexpected initial session control response: {other:?}"
            ))),
        }
    }

    /// Sends a batch of remote input events to the host.
    pub async fn send_events<S: AsyncWrite + Unpin>(
        &self,
        send: &mut S,
        events: Vec<RemoteInputEvent>,
    ) -> Result<()> {
        let msg = RemoteInputWireMessage::Events { events };
        write_framed_message(send, &msg).await
    }

    /// Voluntarily releases control of the host session.
    pub async fn release_control<S: AsyncWrite + Unpin>(
        &self,
        session_id: &str,
        send: &mut S,
    ) -> Result<()> {
        let msg = RemoteInputWireMessage::Control(RemoteInputSessionControl::ReleaseControl {
            session_id: session_id.to_string(),
        });
        write_framed_message(send, &msg).await
    }

    /// Sends events over an established QUIC session.
    pub async fn execute_quic_stream(
        &self,
        session: &QuicSession,
        session_id: &str,
        events: Vec<RemoteInputEvent>,
        timeout: Duration,
    ) -> Result<()> {
        tokio::time::timeout(timeout, async {
            let (mut send, mut recv) = session.open_bi().await?;
            self.request_control(session_id, &mut send, &mut recv)
                .await?;
            self.send_events(&mut send, events).await?;
            self.release_control(session_id, &mut send).await?;
            Ok(())
        })
        .await
        .map_err(|_| NetworkError::Internal("Remote input QUIC stream timed out".into()))?
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

    use aissistant_core::{PairedDevice, PlatformTarget};
    use aissistant_security::DeviceIdentity;
    use aissistant_storage::SqliteDb;

    use super::*;
    use crate::transport::QuicTransportEngine;

    fn setup_test_registry_and_audit() -> (Arc<DeviceRegistry>, Arc<AuditStore>) {
        let db = SqliteDb::open_in_memory().unwrap();
        let reg = Arc::new(DeviceRegistry::new(db.clone()));
        let audit = Arc::new(AuditStore::new(db));
        (reg, audit)
    }

    #[test]
    fn test_coordinate_clamping_and_sanitization() {
        let over_move = RemoteInputEvent::MouseMove { x: 1.5, y: -0.2 };
        assert_eq!(
            over_move.sanitized(),
            RemoteInputEvent::MouseMove { x: 1.0, y: 0.0 }
        );

        let over_down = RemoteInputEvent::MouseDown {
            button: MouseButton::Left,
            x: -1.0,
            y: 2.0,
        };
        assert_eq!(
            over_down.sanitized(),
            RemoteInputEvent::MouseDown {
                button: MouseButton::Left,
                x: 0.0,
                y: 1.0,
            }
        );

        let key = RemoteInputEvent::KeyDown {
            key_code: 42,
            key_text: Some("x".to_string()),
            modifiers: KeyModifiers {
                shift: true,
                ..Default::default()
            },
        };
        assert_eq!(key.clone().sanitized(), key);
    }

    #[tokio::test]
    async fn test_remote_input_rejects_denied_peer() {
        let (reg, audit) = setup_test_registry_and_audit();
        let injector = Arc::new(VirtualInputInjector::new());

        let host_device_id = DeviceId::new("dev_host00000000001").unwrap();
        let host = RemoteInputHost::new(
            host_device_id,
            Arc::clone(&reg),
            Some(Arc::clone(&audit)),
            injector,
        );

        let peer_pubkey = [0x55u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let caller_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = PairedDevice {
            device_id: caller_device_id.clone(),
            name: "Denied Phone".to_string(),
            platform: PlatformTarget::Android,
            ed25519_public_key: peer_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Trusted,
        };
        reg.register_device(&paired).unwrap();
        reg.set_peer_permission(
            &caller_device_id,
            "remote_input_injection_host",
            PeerPermissionLevel::Deny,
            1000,
        )
        .unwrap();

        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);

        let host_task = tokio::spawn(async move {
            host.handle_stream(peer_pubkey, server_write, server_read)
                .await
        });

        let client = RemoteInputClient::new(caller_device_id);
        let mut client_write = client_write;
        let mut client_read = client_read;

        let res = client
            .request_control("input_session_1", &mut client_write, &mut client_read)
            .await;

        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("denied"));

        let _ = host_task.await;
    }

    #[tokio::test]
    async fn test_remote_input_session_lifecycle_and_virtual_injection() {
        let (reg, audit) = setup_test_registry_and_audit();
        let injector = Arc::new(VirtualInputInjector::new());

        let host_device_id = DeviceId::new("dev_host00000000001").unwrap();
        let host = RemoteInputHost::new(
            host_device_id,
            Arc::clone(&reg),
            Some(Arc::clone(&audit)),
            Arc::clone(&injector) as Arc<dyn InputInjector>,
        );

        let peer_pubkey = [0x77u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let caller_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = PairedDevice {
            device_id: caller_device_id.clone(),
            name: "Authorized Tablet".to_string(),
            platform: PlatformTarget::MacOS,
            ed25519_public_key: peer_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Trusted,
        };
        reg.register_device(&paired).unwrap();
        reg.set_peer_permission(
            &caller_device_id,
            "remote_input_injection_host",
            PeerPermissionLevel::AlwaysAllow,
            1000,
        )
        .unwrap();

        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);

        let host_clone = host.clone();
        let host_task = tokio::spawn(async move {
            host_clone
                .handle_stream(peer_pubkey, server_write, server_read)
                .await
        });

        let client = RemoteInputClient::new(caller_device_id);
        let mut client_write = client_write;
        let mut client_read = client_read;

        assert!(!host.is_control_active());

        // 1. Request control
        client
            .request_control("session_abc", &mut client_write, &mut client_read)
            .await
            .expect("session request should succeed");

        assert!(host.is_control_active());
        assert_eq!(host.active_session_id().as_deref(), Some("session_abc"));

        // 2. Send events batch
        let events = vec![
            RemoteInputEvent::MouseMove { x: 0.25, y: 0.75 },
            RemoteInputEvent::MouseDown {
                button: MouseButton::Left,
                x: 0.25,
                y: 0.75,
            },
            RemoteInputEvent::MouseUp {
                button: MouseButton::Left,
                x: 0.25,
                y: 0.75,
            },
            RemoteInputEvent::TextInput {
                text: "Hello Rust!".to_string(),
            },
        ];

        client
            .send_events(&mut client_write, events)
            .await
            .expect("events dispatch should succeed");

        // Small yield to let injector process
        tokio::task::yield_now().await;
        assert_eq!(injector.event_count(), 4);

        // 3. Release control
        client
            .release_control("session_abc", &mut client_write)
            .await
            .expect("release should succeed");

        let _ = host_task.await;

        assert!(!host.is_control_active());
        assert_eq!(host.active_session_id(), None);
    }

    #[tokio::test]
    async fn test_remote_input_host_emergency_escape() {
        let (reg, audit) = setup_test_registry_and_audit();
        let injector = Arc::new(VirtualInputInjector::new());

        let host_device_id = DeviceId::new("dev_host00000000001").unwrap();
        let host = RemoteInputHost::new(
            host_device_id,
            Arc::clone(&reg),
            Some(Arc::clone(&audit)),
            Arc::clone(&injector) as Arc<dyn InputInjector>,
        );

        let peer_pubkey = [0x88u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let caller_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = PairedDevice {
            device_id: caller_device_id.clone(),
            name: "Test Controller".to_string(),
            platform: PlatformTarget::Linux,
            ed25519_public_key: peer_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Trusted,
        };
        reg.register_device(&paired).unwrap();
        reg.set_peer_permission(
            &caller_device_id,
            "remote_input_injection_host",
            PeerPermissionLevel::AlwaysAllow,
            1000,
        )
        .unwrap();

        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);

        let host_clone = host.clone();
        let host_task = tokio::spawn(async move {
            host_clone
                .handle_stream(peer_pubkey, server_write, server_read)
                .await
        });

        let client = RemoteInputClient::new(caller_device_id);
        let mut client_write = client_write;
        let mut client_read = client_read;

        client
            .request_control("session_esc_test", &mut client_write, &mut client_read)
            .await
            .expect("request control should succeed");

        assert!(host.is_control_active());

        // Host emergency escape (simulating Escape key or overlay stop button)
        let terminated = host.terminate_session("User pressed Escape");
        assert!(terminated);
        assert!(!host.is_control_active());

        // Client attempts to send event
        let _ = client
            .send_events(
                &mut client_write,
                vec![RemoteInputEvent::MouseMove { x: 0.5, y: 0.5 }],
            )
            .await;

        // Small yield
        tokio::task::yield_now().await;

        let _ = host_task.await;
    }

    #[tokio::test]
    async fn test_remote_input_over_quic_mtls_loopback() {
        let (reg, audit) = setup_test_registry_and_audit();
        let injector = Arc::new(VirtualInputInjector::new());

        let host_device_id = DeviceId::new("dev_host00000000001").unwrap();
        let host = RemoteInputHost::new(
            host_device_id,
            Arc::clone(&reg),
            Some(Arc::clone(&audit)),
            Arc::clone(&injector) as Arc<dyn InputInjector>,
        );

        let id_server = Arc::new(DeviceIdentity::generate());
        let server_pubkey = id_server.verifying_key_bytes();
        let id_client = Arc::new(DeviceIdentity::generate());
        let client_pubkey = id_client.verifying_key_bytes();

        let hex_prefix: String = client_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let client_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = PairedDevice {
            device_id: client_device_id.clone(),
            name: "QUIC Client Controller".to_string(),
            platform: PlatformTarget::Windows,
            ed25519_public_key: client_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Trusted,
        };
        reg.register_device(&paired).unwrap();
        reg.set_peer_permission(
            &client_device_id,
            "remote_input_injection_host",
            PeerPermissionLevel::AlwaysAllow,
            1000,
        )
        .unwrap();

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
            host_clone
                .handle_stream(session.peer_public_key(), send, recv)
                .await
                .unwrap();
            session
        });

        let client_session = engine_client
            .connect_peer(server_addr, Some(server_pubkey), "aissistant-server")
            .await
            .unwrap();

        let client = RemoteInputClient::new(client_device_id);
        let events = vec![
            RemoteInputEvent::MouseMove { x: 0.1, y: 0.2 },
            RemoteInputEvent::MouseDown {
                button: MouseButton::Right,
                x: 0.1,
                y: 0.2,
            },
            RemoteInputEvent::MouseUp {
                button: MouseButton::Right,
                x: 0.1,
                y: 0.2,
            },
        ];

        client
            .execute_quic_stream(
                &client_session,
                "quic_input_session",
                events,
                Duration::from_secs(5),
            )
            .await
            .unwrap();

        let _ = server_task.await;

        assert_eq!(injector.event_count(), 3);
    }
}
