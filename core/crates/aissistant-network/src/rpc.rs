//! Authenticated Remote Skill RPC protocol, length-delimited framing, and policy-enforcing server.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use aissistant_core::{AuditEntryId, DeviceId, DeviceTrustState, PeerPermissionLevel, SkillId};
use aissistant_orchestrator::tool_dispatcher::redact_sensitive_json;
use aissistant_orchestrator::ToolDispatcher;
use aissistant_security::{
    Actor, AuditEntry, AuditExecutionStatus, NewAuditEntry, PolicyContext, PolicyDecision,
    PolicyEngine, TaintState, GENESIS_PREV_HASH,
};
use aissistant_storage::{AuditStore, DeviceRegistry};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::discovery::current_time_ms;
use crate::error::{NetworkError, Result};
use crate::transport::{QuicSession, VirtualSession};

/// Maximum allowed length of a framed RPC message payload (4 MB).
pub const MAX_RPC_MESSAGE_BYTES: u32 = 4 * 1024 * 1024;

/// RPC request payload sent from initiator to remote peer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRpcRequest {
    /// Unique invocation identifier (e.g. "rpc_...").
    pub rpc_id: String,
    /// Authenticated device identifier of the caller.
    pub caller_device_id: DeviceId,
    /// Target skill identifier to invoke (e.g. "system.time", "file.read").
    pub skill_id: SkillId,
    /// JSON argument payload.
    pub params: serde_json::Value,
    /// Timestamp in milliseconds when request was sent.
    pub timestamp_ms: u64,
}

/// Execution status of a remote RPC call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteRpcStatus {
    /// Skill executed successfully and produced a result.
    Success,
    /// Rejected by local security policy engine or peer capability ACL.
    PermissionDenied,
    /// Execution failed within skill logic.
    SkillFailed,
    /// Target skill timed out during execution.
    TimedOut,
    /// Malformed request or unresolvable skill identifier.
    InvalidRequest,
}

/// RPC response payload sent back to initiator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteRpcResponse {
    /// Matches the `rpc_id` of the corresponding request.
    pub rpc_id: String,
    /// High-level execution status.
    pub status: RemoteRpcStatus,
    /// Output result on success.
    pub result: Option<serde_json::Value>,
    /// Human-readable error message on failure.
    pub error: Option<String>,
    /// Server-side execution duration in milliseconds.
    pub execution_ms: u64,
}

/// Encodes and writes a length-delimited JSON RPC message to an asynchronous writer.
pub async fn write_framed_message<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    message: &T,
) -> Result<()> {
    let payload = serde_json::to_vec(message)
        .map_err(|e| NetworkError::Internal(format!("Failed to serialize RPC message: {e}")))?;

    let len = u32::try_from(payload.len())
        .map_err(|_| NetworkError::Internal("RPC payload exceeds u32 max length".into()))?;

    if len > MAX_RPC_MESSAGE_BYTES {
        return Err(NetworkError::Internal(format!(
            "RPC payload length {} exceeds maximum allowed {}",
            len, MAX_RPC_MESSAGE_BYTES
        )));
    }

    writer
        .write_all(&len.to_be_bytes())
        .await
        .map_err(NetworkError::Io)?;
    writer.write_all(&payload).await.map_err(NetworkError::Io)?;
    writer.flush().await.map_err(NetworkError::Io)?;
    Ok(())
}

/// Reads and decodes a length-delimited JSON RPC message from an asynchronous reader.
pub async fn read_framed_message<R: AsyncRead + Unpin, T: for<'de> Deserialize<'de>>(
    reader: &mut R,
) -> Result<T> {
    let mut len_bytes = [0u8; 4];
    reader
        .read_exact(&mut len_bytes)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::UnexpectedEof => {
                NetworkError::ConnectionClosed("EOF before frame header".into())
            }
            _ => NetworkError::Io(e),
        })?;

    let len = u32::from_be_bytes(len_bytes);
    if len > MAX_RPC_MESSAGE_BYTES {
        return Err(NetworkError::Internal(format!(
            "Incoming RPC payload length {} exceeds maximum allowed {}",
            len, MAX_RPC_MESSAGE_BYTES
        )));
    }

    let mut buffer = vec![0u8; len as usize];
    reader
        .read_exact(&mut buffer)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::UnexpectedEof => {
                NetworkError::ConnectionClosed("EOF before frame body".into())
            }
            _ => NetworkError::Io(e),
        })?;

    let item: T = serde_json::from_slice(&buffer)
        .map_err(|e| NetworkError::Internal(format!("Failed to deserialize RPC payload: {e}")))?;

    Ok(item)
}

/// Resolves the capability category identifier corresponding to a skill name.
#[must_use]
pub fn resolve_capability_for_skill(skill_id: &str) -> &'static str {
    if skill_id.starts_with("file.") {
        "file_system"
    } else if skill_id.starts_with("app.")
        || skill_id.starts_with("process.")
        || skill_id.starts_with("window.")
    {
        "application_management"
    } else if skill_id.starts_with("volume.") || skill_id.starts_with("media.") {
        "system_media_control"
    } else if skill_id.starts_with("clipboard.") {
        "clipboard"
    } else if skill_id.starts_with("llm.") {
        "local_llm"
    } else if skill_id.starts_with("screen.") {
        "screen_streaming_host"
    } else if skill_id.starts_with("input.") {
        "remote_input_injection_host"
    } else {
        "lan_peer_networking"
    }
}

/// Server-side RPC handler validating peer identity, policy ACLs, sandboxing, and audit logs.
#[derive(Clone)]
pub struct RemoteSkillRpcServer {
    local_device_id: DeviceId,
    registry: Arc<DeviceRegistry>,
    dispatcher: Arc<ToolDispatcher>,
    policy_engine: Arc<RwLock<PolicyEngine>>,
    audit_store: Option<Arc<AuditStore>>,
}

impl std::fmt::Debug for RemoteSkillRpcServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSkillRpcServer")
            .field("local_device_id", &self.local_device_id)
            .field("has_audit_store", &self.audit_store.is_some())
            .finish()
    }
}

impl RemoteSkillRpcServer {
    /// Creates a new RPC server instance.
    #[must_use]
    pub fn new(
        local_device_id: DeviceId,
        registry: Arc<DeviceRegistry>,
        dispatcher: Arc<ToolDispatcher>,
        policy_engine: Arc<RwLock<PolicyEngine>>,
        audit_store: Option<Arc<AuditStore>>,
    ) -> Self {
        Self {
            local_device_id,
            registry,
            dispatcher,
            policy_engine,
            audit_store,
        }
    }

    /// Evaluates, authorizes, executes, and audits an incoming remote skill request.
    pub fn execute_request(
        &self,
        peer_public_key: [u8; 32],
        request: RemoteRpcRequest,
    ) -> RemoteRpcResponse {
        let rpc_id = request.rpc_id.clone();

        // 1. Authenticate caller DeviceId against peer Ed25519 public key
        let hex_prefix: String = peer_public_key[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let expected_device_id_str = format!("dev_{hex_prefix}");

        if request.caller_device_id.as_str() != expected_device_id_str {
            return RemoteRpcResponse {
                rpc_id,
                status: RemoteRpcStatus::PermissionDenied,
                result: None,
                error: Some(
                    "Caller DeviceId does not match TLS authenticated public key".to_string(),
                ),
                execution_ms: 0,
            };
        }

        // 2. Query DeviceRegistry for caller trust state
        let paired_device = match self.registry.get_device(&request.caller_device_id) {
            Ok(Some(dev)) => dev,
            Ok(None) => {
                return RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::PermissionDenied,
                    result: None,
                    error: Some("Unknown or unpaired caller device".to_string()),
                    execution_ms: 0,
                };
            }
            Err(e) => {
                return RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::PermissionDenied,
                    result: None,
                    error: Some(format!("Database error reading peer device: {e}")),
                    execution_ms: 0,
                };
            }
        };

        if paired_device.trust_state != DeviceTrustState::Trusted {
            return RemoteRpcResponse {
                rpc_id,
                status: RemoteRpcStatus::PermissionDenied,
                result: None,
                error: Some(format!(
                    "Peer device trust state is '{}' (not trusted)",
                    paired_device.trust_state
                )),
                execution_ms: 0,
            };
        }

        // 3. Check Peer Capability Access Control List (ACL)
        let capability_id = resolve_capability_for_skill(request.skill_id.as_str());
        let permissions = match self
            .registry
            .get_peer_permissions(&request.caller_device_id)
        {
            Ok(perms) => perms,
            Err(e) => {
                return RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::PermissionDenied,
                    result: None,
                    error: Some(format!("Database error querying peer permissions: {e}")),
                    execution_ms: 0,
                };
            }
        };

        let perm_level = permissions
            .get(capability_id)
            .copied()
            .unwrap_or(PeerPermissionLevel::Deny);

        match perm_level {
            PeerPermissionLevel::Deny => {
                self.record_audit_log(
                    &request,
                    PolicyDecision::Deny {
                        reason: format!("Capability '{capability_id}' denied by peer ACL"),
                    },
                    AuditExecutionStatus::Cancelled,
                );

                return RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::PermissionDenied,
                    result: None,
                    error: Some(format!(
                        "Access to capability '{capability_id}' is denied for this peer"
                    )),
                    execution_ms: 0,
                };
            }
            PeerPermissionLevel::AskEachTime => {
                self.record_audit_log(
                    &request,
                    PolicyDecision::RequireConfirmation {
                        reason: format!("Capability '{capability_id}' requires explicit interactive confirmation"),
                        risk_tier: aissistant_security::RiskTier::StateModifying,
                    },
                    AuditExecutionStatus::Cancelled,
                );

                return RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::PermissionDenied,
                    result: None,
                    error: Some(format!(
                        "Capability '{capability_id}' requires interactive local approval (unattended RPC rejected)"
                    )),
                    execution_ms: 0,
                };
            }
            PeerPermissionLevel::AlwaysAllow => {
                // Permitted by ACL
            }
        }

        // 4. Resolve skill tool handler from ToolDispatcher
        let tool = match self.dispatcher.get(request.skill_id.as_str()) {
            Ok(Some(t)) => t,
            Ok(None) => {
                return RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::InvalidRequest,
                    result: None,
                    error: Some(format!("Skill '{}' not found", request.skill_id)),
                    execution_ms: 0,
                };
            }
            Err(e) => {
                return RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::SkillFailed,
                    result: None,
                    error: Some(format!("Dispatcher error: {e}")),
                    execution_ms: 0,
                };
            }
        };

        // 5. Evaluate against PolicyEngine
        let policy_decision = {
            let policy = match self.policy_engine.read() {
                Ok(p) => p,
                Err(_) => {
                    return RemoteRpcResponse {
                        rpc_id,
                        status: RemoteRpcStatus::SkillFailed,
                        result: None,
                        error: Some("Policy engine lock poisoned".to_string()),
                        execution_ms: 0,
                    };
                }
            };

            let context =
                PolicyContext::remote_peer(request.caller_device_id.clone(), TaintState::Clean);
            policy.evaluate(&request.skill_id, tool.risk_tier(), &context)
        };

        if policy_decision.is_denied() {
            self.record_audit_log(
                &request,
                policy_decision.clone(),
                AuditExecutionStatus::Cancelled,
            );

            return RemoteRpcResponse {
                rpc_id,
                status: RemoteRpcStatus::PermissionDenied,
                result: None,
                error: Some(format!("Policy denial: {policy_decision:?}")),
                execution_ms: 0,
            };
        }

        if policy_decision.requires_confirmation() {
            self.record_audit_log(
                &request,
                policy_decision.clone(),
                AuditExecutionStatus::Cancelled,
            );

            return RemoteRpcResponse {
                rpc_id,
                status: RemoteRpcStatus::PermissionDenied,
                result: None,
                error: Some(
                    "Security policy requires local user confirmation for this skill tier"
                        .to_string(),
                ),
                execution_ms: 0,
            };
        }

        // 6. Execute skill handler with timed measurement
        let start_time = Instant::now();
        let exec_result = tool.execute(request.params.clone());
        let execution_ms = start_time.elapsed().as_millis() as u64;

        // 7. Record verified audit trail
        match &exec_result {
            Ok(_) => {
                self.record_audit_log(&request, policy_decision, AuditExecutionStatus::Success);

                RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::Success,
                    result: exec_result.ok(),
                    error: None,
                    execution_ms,
                }
            }
            Err(e) => {
                self.record_audit_log(
                    &request,
                    policy_decision,
                    AuditExecutionStatus::Failed {
                        reason: e.to_string(),
                    },
                );

                RemoteRpcResponse {
                    rpc_id,
                    status: RemoteRpcStatus::SkillFailed,
                    result: None,
                    error: Some(e.to_string()),
                    execution_ms,
                }
            }
        }
    }

    /// Handles an incoming bidirectional byte stream from an authenticated peer connection.
    pub async fn handle_stream<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        peer_public_key: [u8; 32],
        mut send: S,
        mut recv: R,
    ) -> Result<RemoteRpcResponse> {
        let request: RemoteRpcRequest = read_framed_message(&mut recv).await?;
        let response = self.execute_request(peer_public_key, request);
        write_framed_message(&mut send, &response).await?;
        Ok(response)
    }

    fn record_audit_log(
        &self,
        request: &RemoteRpcRequest,
        policy_decision: PolicyDecision,
        execution_status: AuditExecutionStatus,
    ) {
        if let Some(audit) = &self.audit_store {
            let prev_hash = audit.latest_hash().unwrap_or(GENESIS_PREV_HASH);
            let entry = AuditEntry::new(NewAuditEntry {
                id: AuditEntryId::generate(),
                timestamp_ms: current_time_ms(),
                actor: Actor::RemotePeer {
                    device_id: request.caller_device_id.clone(),
                },
                skill_id: request.skill_id.clone(),
                arguments_redacted_json: serde_json::to_string(&redact_sensitive_json(
                    &request.params,
                ))
                .unwrap_or_else(|_| "{}".to_string()),
                policy_decision,
                execution_status,
                prev_hash,
            });

            let _ = audit.append(&entry);
        }
    }
}

/// Client-side RPC caller formatting requests and streaming responses over QUIC.
#[derive(Debug, Clone, Default)]
pub struct RemoteSkillRpcClient;

impl RemoteSkillRpcClient {
    /// Creates a new RPC client instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Invokes a remote skill over bidirectional asynchronous streams.
    pub async fn invoke_stream<S: AsyncWrite + Unpin, R: AsyncRead + Unpin>(
        &self,
        caller_device_id: &DeviceId,
        skill_id: &SkillId,
        params: serde_json::Value,
        mut send: S,
        mut recv: R,
    ) -> Result<RemoteRpcResponse> {
        let rpc_id = format!("rpc_{}", uuid::Uuid::new_v4().simple());
        let request = RemoteRpcRequest {
            rpc_id: rpc_id.clone(),
            caller_device_id: caller_device_id.clone(),
            skill_id: skill_id.clone(),
            params,
            timestamp_ms: current_time_ms(),
        };

        write_framed_message(&mut send, &request).await?;
        let response: RemoteRpcResponse = read_framed_message(&mut recv).await?;

        if response.rpc_id != rpc_id {
            return Err(NetworkError::Internal(format!(
                "Mismatched RPC response ID (expected '{}', got '{}')",
                rpc_id, response.rpc_id
            )));
        }

        Ok(response)
    }

    /// Invokes a remote skill over an established Quinn QUIC session with a timeout.
    pub async fn invoke_quic(
        &self,
        session: &QuicSession,
        caller_device_id: &DeviceId,
        skill_id: &SkillId,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<RemoteRpcResponse> {
        tokio::time::timeout(timeout, async {
            let (send, recv) = session.open_bi().await?;
            self.invoke_stream(caller_device_id, skill_id, params, send, recv)
                .await
        })
        .await
        .map_err(|_| NetworkError::Internal("Remote RPC call timed out".to_string()))?
    }

    /// Invokes a remote skill over a virtual test session.
    pub async fn invoke_virtual(
        &self,
        session: &VirtualSession,
        caller_device_id: &DeviceId,
        skill_id: &SkillId,
        params: serde_json::Value,
    ) -> Result<RemoteRpcResponse> {
        let rpc_id = format!("rpc_{}", uuid::Uuid::new_v4().simple());
        let request = RemoteRpcRequest {
            rpc_id: rpc_id.clone(),
            caller_device_id: caller_device_id.clone(),
            skill_id: skill_id.clone(),
            params,
            timestamp_ms: current_time_ms(),
        };

        let request_bytes = serde_json::to_vec(&request)
            .map_err(|e| NetworkError::Internal(format!("Failed to serialize request: {e}")))?;

        let mut framed = (request_bytes.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(&request_bytes);

        session.send(&framed).await?;
        let resp_framed = session.recv().await?;

        if resp_framed.len() < 4 {
            return Err(NetworkError::Internal(
                "Malformed virtual response frame".into(),
            ));
        }

        let resp_payload = &resp_framed[4..];
        let response: RemoteRpcResponse = serde_json::from_slice(resp_payload)
            .map_err(|e| NetworkError::Internal(format!("Failed to deserialize response: {e}")))?;

        Ok(response)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use aissistant_core::PlatformTarget;
    use aissistant_orchestrator::ToolHandler;
    use aissistant_security::RiskTier;
    use aissistant_storage::SqliteDb;
    use serde_json::json;

    struct TestEchoSkill;
    impl ToolHandler for TestEchoSkill {
        fn name(&self) -> &str {
            "system.echo"
        }
        fn description(&self) -> &str {
            "Echoes input message back"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": { "msg": { "type": "string" } },
                "required": ["msg"]
            })
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::ReadOnly
        }
        fn execute(
            &self,
            arguments: serde_json::Value,
        ) -> aissistant_orchestrator::Result<serde_json::Value> {
            Ok(json!({ "echo": arguments["msg"] }))
        }
    }

    struct TestFailingSkill;
    impl ToolHandler for TestFailingSkill {
        fn name(&self) -> &str {
            "system.fail"
        }
        fn description(&self) -> &str {
            "Always fails"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            json!({})
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::ReadOnly
        }
        fn execute(
            &self,
            _arguments: serde_json::Value,
        ) -> aissistant_orchestrator::Result<serde_json::Value> {
            Err(aissistant_orchestrator::OrchestratorError::Internal(
                "Simulated skill failure".into(),
            ))
        }
    }

    #[tokio::test]
    async fn test_rpc_framing_roundtrip() {
        let (mut client_io, mut server_io) = tokio::io::duplex(1024);

        let request = RemoteRpcRequest {
            rpc_id: "rpc_test_123".to_string(),
            caller_device_id: DeviceId::new("dev_1122334455667788").unwrap(),
            skill_id: SkillId::new("system.time").unwrap(),
            params: json!({ "zone": "UTC" }),
            timestamp_ms: 123456789,
        };

        // Write on client, read on server
        write_framed_message(&mut client_io, &request)
            .await
            .unwrap();
        let decoded_req: RemoteRpcRequest = read_framed_message(&mut server_io).await.unwrap();
        assert_eq!(decoded_req, request);

        // Write response on server, read on client
        let response = RemoteRpcResponse {
            rpc_id: "rpc_test_123".to_string(),
            status: RemoteRpcStatus::Success,
            result: Some(json!({ "time": "2026-11-15T12:00:00Z" })),
            error: None,
            execution_ms: 5,
        };

        write_framed_message(&mut server_io, &response)
            .await
            .unwrap();
        let decoded_resp: RemoteRpcResponse = read_framed_message(&mut client_io).await.unwrap();
        assert_eq!(decoded_resp, response);
    }

    #[tokio::test]
    async fn test_rpc_end_to_end_authorized_skill_execution() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        let audit = Arc::new(AuditStore::new(db.clone()));

        // Register peer device
        let peer_pubkey = [0x42u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_device_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_device_id.clone(),
            name: "Test Client".to_string(),
            platform: PlatformTarget::Linux,
            ed25519_public_key: peer_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: Some("192.168.1.10:51820".to_string()),
            trust_state: DeviceTrustState::Trusted,
        };
        registry.register_device(&paired).unwrap();

        // Grant permission for lan_peer_networking
        registry
            .set_peer_permission(
                &peer_device_id,
                "lan_peer_networking",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        // Set up dispatcher with echo skill
        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(TestEchoSkill)).unwrap();

        let local_id = DeviceId::new("dev_0000000000000000").unwrap();
        let policy_engine = Arc::new(RwLock::new(PolicyEngine::new()));

        let server = RemoteSkillRpcServer::new(
            local_id,
            registry,
            dispatcher,
            policy_engine,
            Some(audit.clone()),
        );

        let client = RemoteSkillRpcClient::new();
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_read, client_write) = tokio::io::split(client_io);
        let (server_read, server_write) = tokio::io::split(server_io);

        // Spawn server handler task
        let server_clone = server.clone();
        let server_task = tokio::spawn(async move {
            server_clone
                .handle_stream(peer_pubkey, server_write, server_read)
                .await
        });

        // Client invokes echo skill
        let skill_id = SkillId::new("system.echo").unwrap();
        let resp = client
            .invoke_stream(
                &peer_device_id,
                &skill_id,
                json!({ "msg": "hello rust mesh" }),
                client_write,
                client_read,
            )
            .await
            .unwrap();

        assert_eq!(resp.status, RemoteRpcStatus::Success);
        assert_eq!(resp.result.unwrap()["echo"], "hello rust mesh");

        let server_resp = server_task.await.unwrap().unwrap();
        assert_eq!(server_resp.status, RemoteRpcStatus::Success);

        // Verify cryptographic audit log integrity
        let logs = audit.get_entries(10, 0).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(
            logs[0].actor,
            Actor::RemotePeer {
                device_id: peer_device_id
            }
        );
        assert_eq!(logs[0].skill_id, skill_id);
        assert_eq!(logs[0].execution_status, AuditExecutionStatus::Success);
        assert!(
            aissistant_security::AuditChainVerifier::verify_chain(&logs, &GENESIS_PREV_HASH)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_rpc_rejects_untrusted_and_denied_peers() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(TestEchoSkill)).unwrap();

        let local_id = DeviceId::new("dev_0000000000000000").unwrap();
        let policy_engine = Arc::new(RwLock::new(PolicyEngine::new()));

        let server =
            RemoteSkillRpcServer::new(local_id, registry.clone(), dispatcher, policy_engine, None);

        // Case 1: Unregistered caller
        let peer_pubkey = [0x55u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let req = RemoteRpcRequest {
            rpc_id: "rpc_1".to_string(),
            caller_device_id: peer_id.clone(),
            skill_id: SkillId::new("system.echo").unwrap(),
            params: json!({ "msg": "hi" }),
            timestamp_ms: 1000,
        };

        let resp = server.execute_request(peer_pubkey, req);
        assert_eq!(resp.status, RemoteRpcStatus::PermissionDenied);
        assert!(resp.error.unwrap().contains("Unknown"));

        // Case 2: Registered but Blocked peer
        let blocked_dev = aissistant_core::PairedDevice {
            device_id: peer_id.clone(),
            name: "Blocked Node".to_string(),
            platform: PlatformTarget::Windows,
            ed25519_public_key: peer_pubkey,
            paired_at_ms: 1000,
            last_seen_ms: 1000,
            last_endpoint: None,
            trust_state: DeviceTrustState::Blocked,
        };
        registry.register_device(&blocked_dev).unwrap();

        let req2 = RemoteRpcRequest {
            rpc_id: "rpc_2".to_string(),
            caller_device_id: peer_id.clone(),
            skill_id: SkillId::new("system.echo").unwrap(),
            params: json!({ "msg": "hi" }),
            timestamp_ms: 1000,
        };
        let resp2 = server.execute_request(peer_pubkey, req2);
        assert_eq!(resp2.status, RemoteRpcStatus::PermissionDenied);
        assert!(resp2.error.unwrap().contains("blocked"));

        // Case 3: Trusted peer but capability ACL is Deny
        registry
            .set_device_trust_state(&peer_id, DeviceTrustState::Trusted)
            .unwrap();
        registry
            .set_peer_permission(
                &peer_id,
                "lan_peer_networking",
                PeerPermissionLevel::Deny,
                1000,
            )
            .unwrap();

        let req3 = RemoteRpcRequest {
            rpc_id: "rpc_3".to_string(),
            caller_device_id: peer_id.clone(),
            skill_id: SkillId::new("system.echo").unwrap(),
            params: json!({ "msg": "hi" }),
            timestamp_ms: 1000,
        };
        let resp3 = server.execute_request(peer_pubkey, req3);
        assert_eq!(resp3.status, RemoteRpcStatus::PermissionDenied);
        assert!(resp3.error.unwrap().contains("denied"));

        // Case 4: Spoofed caller DeviceId
        let fake_id = DeviceId::new("dev_0011223344556677").unwrap();
        let req4 = RemoteRpcRequest {
            rpc_id: "rpc_4".to_string(),
            caller_device_id: fake_id,
            skill_id: SkillId::new("system.echo").unwrap(),
            params: json!({ "msg": "hi" }),
            timestamp_ms: 1000,
        };
        let resp4 = server.execute_request(peer_pubkey, req4);
        assert_eq!(resp4.status, RemoteRpcStatus::PermissionDenied);
        assert!(resp4.error.unwrap().contains("does not match"));
    }

    #[tokio::test]
    async fn test_rpc_handles_skill_failure_cleanly() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db));
        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(TestFailingSkill)).unwrap();

        let peer_pubkey = [0x77u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_id.clone(),
            name: "Failing Client".to_string(),
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
                &peer_id,
                "lan_peer_networking",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let server = RemoteSkillRpcServer::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            registry,
            dispatcher,
            Arc::new(RwLock::new(PolicyEngine::new())),
            None,
        );

        let req = RemoteRpcRequest {
            rpc_id: "rpc_fail".to_string(),
            caller_device_id: peer_id,
            skill_id: SkillId::new("system.fail").unwrap(),
            params: json!({}),
            timestamp_ms: 1000,
        };

        let resp = server.execute_request(peer_pubkey, req);
        assert_eq!(resp.status, RemoteRpcStatus::SkillFailed);
        assert!(resp.error.unwrap().contains("Simulated skill failure"));
    }

    #[tokio::test]
    async fn test_rpc_framing_payload_exceeds_max_bytes() {
        let (mut client_io, mut server_io) = tokio::io::duplex(1024);

        // Manually write a length header exceeding MAX_RPC_MESSAGE_BYTES
        let invalid_len = MAX_RPC_MESSAGE_BYTES + 10;
        client_io
            .write_all(&invalid_len.to_be_bytes())
            .await
            .unwrap();

        let result: Result<RemoteRpcRequest> = read_framed_message(&mut server_io).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exceeds maximum allowed"));
    }

    #[tokio::test]
    async fn test_rpc_virtual_session_invocation() {
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db));
        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(TestEchoSkill)).unwrap();

        let peer_pubkey = [0x88u8; 32];
        let hex_prefix: String = peer_pubkey[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let peer_id = DeviceId::new(format!("dev_{hex_prefix}")).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: peer_id.clone(),
            name: "Virtual Client".to_string(),
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
                &peer_id,
                "lan_peer_networking",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let server = RemoteSkillRpcServer::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            registry,
            dispatcher,
            Arc::new(RwLock::new(PolicyEngine::new())),
            None,
        );

        let addr_a = "127.0.0.1:51820".parse().unwrap();
        let addr_b = "127.0.0.1:51821".parse().unwrap();
        let (sess_client, sess_server) = crate::transport::create_virtual_session_pair(
            peer_pubkey,
            addr_a,
            [0x99u8; 32],
            addr_b,
        );

        // Server handler loop for virtual session
        let server_clone = server.clone();
        let server_task = tokio::spawn(async move {
            let framed_req = sess_server.recv().await.unwrap();
            let req: RemoteRpcRequest = serde_json::from_slice(&framed_req[4..]).unwrap();
            let resp = server_clone.execute_request(peer_pubkey, req);
            let resp_bytes = serde_json::to_vec(&resp).unwrap();
            let mut framed_resp = (resp_bytes.len() as u32).to_be_bytes().to_vec();
            framed_resp.extend_from_slice(&resp_bytes);
            sess_server.send(&framed_resp).await.unwrap();
        });

        let client = RemoteSkillRpcClient::new();
        let skill_id = SkillId::new("system.echo").unwrap();
        let resp = client
            .invoke_virtual(
                &sess_client,
                &peer_id,
                &skill_id,
                json!({ "msg": "via virtual session" }),
            )
            .await
            .unwrap();

        assert_eq!(resp.status, RemoteRpcStatus::Success);
        assert_eq!(resp.result.unwrap()["echo"], "via virtual session");
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn test_rpc_over_quic_mtls_loopback() {
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

        // Database & registry setup
        let db = SqliteDb::open_in_memory().unwrap();
        let registry = Arc::new(DeviceRegistry::new(db.clone()));
        let audit = Arc::new(AuditStore::new(db));
        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(TestEchoSkill)).unwrap();

        let paired = aissistant_core::PairedDevice {
            device_id: client_device_id.clone(),
            name: "Loopback QUIC Client".to_string(),
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
                "lan_peer_networking",
                PeerPermissionLevel::AlwaysAllow,
                1000,
            )
            .unwrap();

        let rpc_server = RemoteSkillRpcServer::new(
            DeviceId::new("dev_0000000000000000").unwrap(),
            registry,
            dispatcher,
            Arc::new(RwLock::new(PolicyEngine::new())),
            Some(audit),
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

        // Server accepts connection and handles incoming RPC stream
        let server_clone = rpc_server.clone();
        let server_task = tokio::spawn(async move {
            let session = engine_server.accept().await.unwrap();
            let (send, recv) = session.accept_bi().await.unwrap();
            let resp = server_clone
                .handle_stream(session.peer_public_key(), send, recv)
                .await
                .unwrap();
            (session, resp)
        });

        // Client connects over QUIC
        let client_session = engine_client
            .connect_peer(server_addr, Some(server_pubkey), "aissistant-server")
            .await
            .unwrap();

        let rpc_client = RemoteSkillRpcClient::new();
        let skill_id = SkillId::new("system.echo").unwrap();
        let resp = rpc_client
            .invoke_quic(
                &client_session,
                &client_device_id,
                &skill_id,
                json!({ "msg": "quic mTLS call" }),
                Duration::from_secs(5),
            )
            .await
            .unwrap();

        assert_eq!(resp.status, RemoteRpcStatus::Success);
        assert_eq!(resp.result.unwrap()["echo"], "quic mTLS call");

        let (server_session, server_resp) = server_task.await.unwrap();
        assert_eq!(server_resp.status, RemoteRpcStatus::Success);

        client_session.close();
        server_session.close();
    }
}
