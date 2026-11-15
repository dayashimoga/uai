//! High-level, safe FFI application programming interface and core state bridge.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use aissistant_core::{AuditEntryId, Capability, CapabilityRegistry, CapabilityStatus, SkillId};
use aissistant_security::{
    Actor, AeadKey, AuditEntry, AuditExecutionStatus, DeviceIdentity, PolicyDecision, PolicyEngine,
    GENESIS_PREV_HASH,
};
use aissistant_storage::{
    AuditStore, FileBoundKeystore, KeystoreProvider, SecretStore, SettingsStore, SqliteDb,
};
use serde::{Deserialize, Serialize};

use crate::device_bridge::{
    DeviceSupervisor, DiscoveredPeerItem, PairedDeviceItem, PairingSessionInfo,
};
use crate::error::{FfiError, FfiResult};
use crate::events::{BridgeEvent, EventStreamSink};
use crate::lifecycle::{
    CancellationToken, CoreRuntime, LifecycleState, MemoryPressureLevel, ShutdownReason,
};
use crate::model_bridge::{
    BenchmarkReport, HardwareOverview, ModelManagerSnapshot, ModelSupervisor,
};
use crate::voice_bridge::{VoiceListeningMode, VoicePipelineSupervisor};

/// Overall health tier of the runtime engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// All subsystems operational.
    Healthy,
    /// Non-fatal capability degradation or permission limitation.
    Degraded,
    /// Critical failure in database, keystore, or security integrity.
    Critical,
}

impl HealthStatus {
    /// Returns static string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Critical => "critical",
        }
    }
}

/// Comprehensive health and diagnostics report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    /// Current health tier.
    pub status: HealthStatus,
    /// Current lifecycle state.
    pub lifecycle_state: LifecycleState,
    /// Count of currently active background tasks.
    pub active_tasks: usize,
    /// SQLite engine operational.
    pub database_ok: bool,
    /// Security audit log integrity verified.
    pub audit_ok: bool,
    /// Keystore and AEAD encryption ready.
    pub keystore_ok: bool,
    /// Total recognized platform capabilities.
    pub total_capabilities: usize,
    /// Currently active and available capabilities.
    pub active_capabilities: usize,
    /// Number of committed audit log entries.
    pub audit_count: u64,
    /// Diagnostic details and degradation warnings.
    pub details: Vec<String>,
}

/// FFI view of a platform capability and its current runtime status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityItem {
    /// Canonical capability identifier (e.g. "local_llm").
    pub name: String,
    /// Associated risk tier or category.
    pub tier: String,
    /// Whether the capability is supported on this OS.
    pub is_supported: bool,
    /// Whether the capability is currently active and usable.
    pub is_active: bool,
    /// Availability state ("supported", "unsupported", "requires_permission", "degraded").
    pub status: String,
    /// Human-readable explanation if degraded or unsupported.
    pub degradation_reason: Option<String>,
    /// System permission name required if pending authorization.
    pub missing_permission: Option<String>,
}

/// FFI view of an immutable audit log entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditItem {
    /// Monotonic sequence number.
    pub sequence: u64,
    /// Unix millisecond timestamp.
    pub timestamp_ms: u64,
    /// Actor identifier.
    pub actor: String,
    /// Skill ID.
    pub skill_id: String,
    /// Redacted payload or argument summary.
    pub payload_summary: String,
    /// Policy decision applied.
    pub decision: String,
    /// Execution status outcome.
    pub execution_status: String,
    /// BLAKE3 digest of this entry in lowercase hex.
    pub entry_hash: String,
    /// BLAKE3 digest of preceding entry in lowercase hex.
    pub prev_hash: String,
}

/// Cryptographic audit log verification report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditIntegrityReport {
    /// Whether the entire chain is cryptographically intact.
    pub is_valid: bool,
    /// Total entries successfully verified.
    pub verified_count: u64,
    /// Tip entry hash in lowercase hex.
    pub head_hash: String,
    /// Error message if integrity was violated.
    pub error_message: Option<String>,
}

/// FFI view of an ingested document record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentRecordItem {
    /// Document unique identifier (`doc_<hex>`).
    pub id: String,
    /// Canonical resource path.
    pub path: String,
    /// Display file name.
    pub file_name: String,
    /// MIME type.
    pub mime_type: String,
    /// Content hash for change detection.
    pub content_hash: String,
    /// File size in bytes.
    pub file_size_bytes: u64,
    /// Number of semantic chunks.
    pub chunk_count: usize,
    /// Ingestion epoch timestamp in milliseconds.
    pub indexed_at_ms: i64,
    /// Last update epoch timestamp in milliseconds.
    pub updated_at_ms: i64,
}

/// FFI view of a grounded knowledge citation resulting from hybrid search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeCitationItem {
    /// Unique chunk identifier.
    pub chunk_id: String,
    /// Parent document identifier.
    pub document_id: String,
    /// Canonical resource path.
    pub path: String,
    /// Display file name.
    pub file_name: String,
    /// Chunk index within document.
    pub chunk_index: usize,
    /// Reciprocal Rank Fusion combined score.
    pub rrf_score: f64,
    /// BM25 rank (if matched).
    pub bm25_rank: Option<usize>,
    /// Dense vector rank (if matched).
    pub vector_rank: Option<usize>,
    /// Cosine similarity score (if matched).
    pub cosine_similarity: Option<f32>,
    /// Chunk text content.
    pub content: String,
    /// Snippet excerpt with highlight marks.
    pub snippet: String,
}

/// Core bridge orchestrating subsystem access and safe FFI dispatch.
pub struct CoreBridge {
    db: SqliteDb,
    audit_store: AuditStore,
    settings_store: SettingsStore,
    secret_store: SecretStore,
    capability_registry: RwLock<CapabilityRegistry>,
    policy_engine: RwLock<PolicyEngine>,
    event_sink: Arc<EventStreamSink>,
    runtime: Arc<CoreRuntime>,
    identity: DeviceIdentity,
    voice_supervisor: Arc<Mutex<VoicePipelineSupervisor>>,
    model_supervisor: Arc<Mutex<ModelSupervisor>>,
    device_supervisor: Arc<DeviceSupervisor>,
    data_dir: Option<PathBuf>,
}

impl std::fmt::Debug for CoreBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let dev_id = self
            .identity
            .to_device_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|_| "unknown_device".to_string());
        f.debug_struct("CoreBridge")
            .field("device_id", &dev_id)
            .field("lifecycle_state", &self.runtime.state())
            .field("data_dir", &self.data_dir)
            .finish()
    }
}

impl CoreBridge {
    /// Creates an in-memory bridge instance for testing or ephemeral runs.
    pub fn new_in_memory() -> FfiResult<Self> {
        let event_sink = Arc::new(EventStreamSink::new());
        let runtime = Arc::new(CoreRuntime::new(event_sink.clone()));
        runtime.transition_to(LifecycleState::Starting, Some("in_memory_init"))?;

        let db = SqliteDb::open_in_memory()?;
        let audit_store = AuditStore::new(db.clone());
        let settings_store = SettingsStore::new(db.clone());
        let master_key = AeadKey::generate();
        let secret_store = SecretStore::new(db.clone(), master_key);
        let capability_registry = RwLock::new(CapabilityRegistry::for_current_host());
        let policy_engine = RwLock::new(PolicyEngine::new());
        let identity = DeviceIdentity::generate();
        let voice_supervisor = Arc::new(Mutex::new(VoicePipelineSupervisor::new(
            event_sink.clone(),
        )?));
        let model_supervisor = Arc::new(Mutex::new(ModelSupervisor::new(event_sink.clone())));
        let device_supervisor = Arc::new(DeviceSupervisor::new(
            Arc::new(DeviceIdentity::from_bytes(&identity.secret_bytes())),
            db.clone(),
            event_sink.clone(),
        ));

        runtime.transition_to(LifecycleState::Running, Some("ready"))?;

        Ok(Self {
            db,
            audit_store,
            settings_store,
            secret_store,
            capability_registry,
            policy_engine,
            event_sink,
            runtime,
            identity,
            voice_supervisor,
            model_supervisor,
            device_supervisor,
            data_dir: None,
        })
    }

    /// Creates a directory-persisted bridge instance with SQLite database and encrypted keystore.
    pub fn new_with_path(data_dir: impl AsRef<Path>) -> FfiResult<Self> {
        let path = data_dir.as_ref();
        if !path.exists() {
            std::fs::create_dir_all(path).map_err(|e| FfiError::Storage(e.into()))?;
        }

        let event_sink = Arc::new(EventStreamSink::new());
        let runtime = Arc::new(CoreRuntime::new(event_sink.clone()));
        runtime.transition_to(LifecycleState::Starting, Some("disk_init"))?;

        let db_path = path.join("aissistant.db");
        let db = SqliteDb::open(&db_path)?;
        let audit_store = AuditStore::new(db.clone());
        let settings_store = SettingsStore::new(db.clone());

        // Master AEAD key management via FileBoundKeystore
        let keystore = FileBoundKeystore::new(path)?;
        let master_key = match keystore.get_master_key("master_key")? {
            Some(k) => k,
            None => {
                let k = AeadKey::generate();
                keystore.set_master_key("master_key", &k)?;
                k
            }
        };
        let secret_store = SecretStore::new(db.clone(), master_key);

        let capability_registry = RwLock::new(CapabilityRegistry::for_current_host());
        let policy_engine = RwLock::new(PolicyEngine::new());
        let identity = DeviceIdentity::generate();
        let voice_supervisor = Arc::new(Mutex::new(VoicePipelineSupervisor::new(
            event_sink.clone(),
        )?));
        let model_supervisor = Arc::new(Mutex::new(ModelSupervisor::new(event_sink.clone())));
        let device_supervisor = Arc::new(DeviceSupervisor::new(
            Arc::new(DeviceIdentity::from_bytes(&identity.secret_bytes())),
            db.clone(),
            event_sink.clone(),
        ));

        runtime.transition_to(LifecycleState::Running, Some("ready"))?;

        Ok(Self {
            db,
            audit_store,
            settings_store,
            secret_store,
            capability_registry,
            policy_engine,
            event_sink,
            runtime,
            identity,
            voice_supervisor,
            model_supervisor,
            device_supervisor,
            data_dir: Some(path.to_path_buf()),
        })
    }

    /// Returns a comprehensive system health report.
    pub fn health(&self) -> HealthReport {
        let mut details = Vec::new();
        let mut database_ok = false;
        let mut audit_ok = false;
        let keystore_ok = true;

        // 1. Verify SQLite connectivity
        if self.db.with_conn(|_| Ok(())).is_ok() {
            database_ok = true;
        } else {
            details.push("Database connection failed or inaccessible".to_string());
        }

        // 2. Verify Audit Log Integrity
        let audit_count = self.audit_store.count().unwrap_or(0);
        match self.audit_store.verify_integrity() {
            Ok(_) => {
                audit_ok = true;
            }
            Err(err) => {
                details.push(format!("Audit chain integrity compromised: {err}"));
            }
        }

        // 3. Evaluate capability status
        let (total_caps, active_caps) = if let Ok(reg) = self.capability_registry.read() {
            let statuses = reg.all_statuses();
            let total = statuses.len();
            let mut active = 0;
            for (cap, status) in statuses {
                if status.is_available() {
                    active += 1;
                } else if let CapabilityStatus::RequiresPermission { permission, .. } = status {
                    details.push(format!(
                        "Capability '{cap}' requires permission '{permission}'"
                    ));
                } else if let CapabilityStatus::Degraded { limitation } = status {
                    details.push(format!("Capability '{cap}' degraded: {limitation}"));
                }
            }
            (total, active)
        } else {
            details.push("Failed to acquire capability registry lock".to_string());
            (0, 0)
        };

        // Determine final health status
        let status = if !database_ok || !audit_ok || !keystore_ok {
            HealthStatus::Critical
        } else if !details.is_empty() {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let lifecycle_state = self.runtime.state();
        let active_tasks = self.runtime.active_task_count();

        HealthReport {
            status,
            lifecycle_state,
            active_tasks,
            database_ok,
            audit_ok,
            keystore_ok,
            total_capabilities: total_caps,
            active_capabilities: active_caps,
            audit_count,
            details,
        }
    }

    /// Returns the live platform capability matrix.
    pub fn capabilities(&self) -> Vec<CapabilityItem> {
        let registry = match self.capability_registry.read() {
            Ok(guard) => guard,
            Err(_) => return Vec::new(),
        };

        let mut items = Vec::new();
        for &cap in Capability::all() {
            let status = registry.status(cap);
            let is_supported = !matches!(status, CapabilityStatus::Unsupported { .. });
            let is_active = status.is_available();

            let (status_str, degradation_reason, missing_permission) = match &status {
                CapabilityStatus::Supported => ("supported".to_string(), None, None),
                CapabilityStatus::Unsupported { reason } => {
                    ("unsupported".to_string(), Some(reason.clone()), None)
                }
                CapabilityStatus::RequiresPermission {
                    permission,
                    granted,
                } => {
                    let st = if *granted {
                        "supported".to_string()
                    } else {
                        "requires_permission".to_string()
                    };
                    (
                        st,
                        None,
                        if *granted {
                            None
                        } else {
                            Some(permission.clone())
                        },
                    )
                }
                CapabilityStatus::Degraded { limitation } => {
                    ("degraded".to_string(), Some(limitation.clone()), None)
                }
            };

            items.push(CapabilityItem {
                name: cap.as_str().to_string(),
                tier: "standard".to_string(),
                is_supported,
                is_active,
                status: status_str,
                degradation_reason,
                missing_permission,
            });
        }

        items
    }

    /// Updates dynamic permission status for a capability and broadcasts the event.
    pub fn update_permission(
        &self,
        capability_name: &str,
        granted: bool,
    ) -> FfiResult<CapabilityItem> {
        let cap = Capability::from_str(capability_name)?;

        let (updated_status, reason, permission) = {
            let mut reg = self.capability_registry.write().map_err(|e| {
                FfiError::LockPoisoned(format!("Capability registry lock error: {e}"))
            })?;

            if let CapabilityStatus::RequiresPermission { ref permission, .. } = reg.status(cap) {
                let perm_name = permission.clone();
                reg.set_permission_granted(cap, &perm_name, granted);
            } else if granted {
                reg.set_status(cap, CapabilityStatus::Supported);
            } else {
                reg.set_status(
                    cap,
                    CapabilityStatus::Degraded {
                        limitation: "Capability disabled by runtime policy".to_string(),
                    },
                );
            }

            let new_st = reg.status(cap);
            let (st_str, reason_str, perm_str) = match &new_st {
                CapabilityStatus::Supported => ("supported".to_string(), None, None),
                CapabilityStatus::Unsupported { reason } => {
                    ("unsupported".to_string(), Some(reason.clone()), None)
                }
                CapabilityStatus::RequiresPermission {
                    permission,
                    granted,
                } => {
                    let s = if *granted {
                        "supported".to_string()
                    } else {
                        "requires_permission".to_string()
                    };
                    (
                        s,
                        None,
                        if *granted {
                            None
                        } else {
                            Some(permission.clone())
                        },
                    )
                }
                CapabilityStatus::Degraded { limitation } => {
                    ("degraded".to_string(), Some(limitation.clone()), None)
                }
            };

            (st_str, reason_str, perm_str)
        };

        // Broadcast event
        self.event_sink.broadcast(BridgeEvent::CapabilityChanged {
            capability: cap.as_str().to_string(),
            status: updated_status.clone(),
            reason: reason.clone().or_else(|| permission.clone()),
        });

        let is_supported = updated_status != "unsupported";
        let is_active = updated_status == "supported";

        Ok(CapabilityItem {
            name: cap.as_str().to_string(),
            tier: "standard".to_string(),
            is_supported,
            is_active,
            status: updated_status,
            degradation_reason: reason,
            missing_permission: permission,
        })
    }

    /// Queries historical audit entries in sequential order.
    pub fn query_audit_log(&self, limit: usize, offset: usize) -> FfiResult<Vec<AuditItem>> {
        let entries = self.audit_store.get_entries(limit, offset)?;
        let mut items = Vec::with_capacity(entries.len());

        for (idx, entry) in entries.into_iter().enumerate() {
            items.push(AuditItem {
                sequence: (offset + idx + 1) as u64,
                timestamp_ms: entry.timestamp_ms,
                actor: entry.actor.to_string(),
                skill_id: entry.skill_id.as_str().to_string(),
                payload_summary: entry.arguments_redacted_json,
                decision: entry.policy_decision.as_str().to_string(),
                execution_status: entry.execution_status.as_str().to_string(),
                entry_hash: hex_encode(&entry.entry_hash),
                prev_hash: hex_encode(&entry.prev_hash),
            });
        }

        Ok(items)
    }

    /// Cryptographically verifies the integrity of the audit log chain.
    pub fn verify_audit_integrity(&self) -> FfiResult<AuditIntegrityReport> {
        let count = self.audit_store.count()?;
        match self.audit_store.verify_integrity() {
            Ok(head_hash) => Ok(AuditIntegrityReport {
                is_valid: true,
                verified_count: count,
                head_hash: hex_encode(&head_hash),
                error_message: None,
            }),
            Err(err) => Ok(AuditIntegrityReport {
                is_valid: false,
                verified_count: count,
                head_hash: hex_encode(&[0u8; 32]),
                error_message: Some(err.to_string()),
            }),
        }
    }

    /// Appends a new audit record to the persistent log and broadcasts the event.
    pub fn append_audit_entry(
        &self,
        actor: Actor,
        skill_id: SkillId,
        payload_summary: &str,
        decision: PolicyDecision,
        status: AuditExecutionStatus,
    ) -> FfiResult<AuditItem> {
        let prev_hash = self.audit_store.latest_hash().unwrap_or(GENESIS_PREV_HASH);
        let count = self.audit_store.count()?;
        let sequence = count + 1;
        let entry_id = AuditEntryId::from_sequence(sequence);
        let timestamp_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            Ok(d) => d.as_millis() as u64,
            Err(_) => 0,
        };

        let entry = AuditEntry::create(
            entry_id,
            timestamp_ms,
            actor.clone(),
            skill_id.clone(),
            payload_summary,
            decision.clone(),
            status.clone(),
            prev_hash,
        );

        self.audit_store.append(&entry)?;

        let entry_hash_hex = hex_encode(&entry.entry_hash);
        let prev_hash_hex = hex_encode(&entry.prev_hash);

        // Broadcast event
        self.event_sink.broadcast(BridgeEvent::AuditLogAppended {
            sequence,
            actor: actor.to_string(),
            skill_id: skill_id.as_str().to_string(),
            decision: decision.as_str().to_string(),
            execution_status: status.as_str().to_string(),
            entry_hash: entry_hash_hex.clone(),
        });

        Ok(AuditItem {
            sequence,
            timestamp_ms,
            actor: actor.to_string(),
            skill_id: skill_id.as_str().to_string(),
            payload_summary: payload_summary.to_string(),
            decision: decision.as_str().to_string(),
            execution_status: status.as_str().to_string(),
            entry_hash: entry_hash_hex,
            prev_hash: prev_hash_hex,
        })
    }

    /// Stores a persistent configuration setting as JSON.
    pub fn set_setting(&self, key: &str, value_json: &str) -> FfiResult<()> {
        let parsed: serde_json::Value = serde_json::from_str(value_json)?;
        self.settings_store.set(key, &parsed)?;

        // Broadcast event
        self.event_sink.broadcast(BridgeEvent::SettingChanged {
            key: key.to_string(),
            value_preview: Some(value_json.chars().take(64).collect()),
        });

        Ok(())
    }

    /// Retrieves a persistent configuration setting formatted as JSON string.
    pub fn get_setting(&self, key: &str) -> FfiResult<Option<String>> {
        let val: Option<serde_json::Value> = self.settings_store.get(key)?;
        match val {
            Some(v) => Ok(Some(serde_json::to_string(&v)?)),
            None => Ok(None),
        }
    }

    /// Stores an encrypted secret in the secure store.
    pub fn set_secret(&self, key: &str, secret_value: &str) -> FfiResult<()> {
        self.secret_store.set(key, &secret_value.to_string())?;
        Ok(())
    }

    /// Evaluates a skill execution request against the security policy engine.
    pub fn evaluate_policy(
        &self,
        skill_id: &SkillId,
        tier: aissistant_security::RiskTier,
        context: &aissistant_security::PolicyContext,
    ) -> FfiResult<PolicyDecision> {
        let engine = self
            .policy_engine
            .read()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        Ok(engine.evaluate(skill_id, tier, context))
    }

    /// Returns a reference to the policy engine lock.
    #[must_use]
    pub fn policy_engine(&self) -> &RwLock<PolicyEngine> {
        &self.policy_engine
    }

    /// Retrieves and decrypts a secret from the secure store.
    pub fn get_secret(&self, key: &str) -> FfiResult<Option<String>> {
        let val: Option<String> = self.secret_store.get(key)?;
        Ok(val)
    }

    /// Returns a reference to the event stream sink for subscription.
    #[must_use]
    pub fn event_sink(&self) -> Arc<EventStreamSink> {
        self.event_sink.clone()
    }

    /// Returns a reference to the underlying SQLite database handle.
    #[must_use]
    pub fn db(&self) -> &SqliteDb {
        &self.db
    }

    /// Returns a reference to the core lifecycle runtime supervisor.
    #[must_use]
    pub fn runtime(&self) -> &Arc<CoreRuntime> {
        &self.runtime
    }

    /// Returns the active lifecycle state of the core runtime.
    #[must_use]
    pub fn lifecycle_state(&self) -> LifecycleState {
        self.runtime.state()
    }

    /// Spawns a background task tracked by the lifecycle supervisor.
    pub fn spawn_task(&self, name: &str) -> FfiResult<(u64, CancellationToken)> {
        self.runtime.spawn_task(name)
    }

    /// Marks a tracked background task as finished.
    pub fn finish_task(&self, task_id: u64) {
        self.runtime.finish_task(task_id);
    }

    /// Signals the core runtime to execute a graceful two-phase shutdown.
    pub fn request_shutdown(&self, reason: ShutdownReason, timeout: Duration) -> FfiResult<()> {
        self.runtime
            .request_shutdown(reason, timeout, Some(&self.db))
    }

    /// Delivers an operating system memory pressure notification.
    pub fn notify_memory_pressure(&self, level: MemoryPressureLevel) -> FfiResult<()> {
        if let Ok(mut sup) = self.model_supervisor.lock() {
            sup.on_memory_pressure(matches!(level, MemoryPressureLevel::Critical));
        }
        self.runtime.notify_memory_pressure(level, Some(&self.db))
    }

    /// Returns the device ID of this core instance.
    #[must_use]
    pub fn device_id(&self) -> String {
        self.identity
            .to_device_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|_| "unknown_device".to_string())
    }

    /// Starts voice listening in the requested mode (PushToTalk or WakeWord).
    pub fn voice_start_listening(&self, mode: VoiceListeningMode) -> FfiResult<()> {
        let mut supervisor = self
            .voice_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        supervisor.start_listening(mode)
    }

    /// Stops voice listening and returns to idle.
    pub fn voice_stop_listening(&self) -> FfiResult<()> {
        let mut supervisor = self
            .voice_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        supervisor.stop_listening()
    }

    /// Sets the active voice listening mode.
    pub fn voice_set_mode(&self, mode: VoiceListeningMode) -> FfiResult<()> {
        let mut supervisor = self
            .voice_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        supervisor.set_mode(mode)
    }

    /// Updates active wake-word keywords.
    pub fn voice_set_keywords(&self, keywords: &[String]) -> FfiResult<()> {
        let mut supervisor = self
            .voice_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        supervisor.set_keywords(keywords)
    }

    /// Ingests streaming audio samples into the voice pipeline.
    pub fn voice_inject_audio(&self, samples: &[f32]) -> FfiResult<()> {
        let mut supervisor = self
            .voice_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        supervisor.process_audio_chunk(samples)
    }

    /// Returns whether the voice pipeline is actively listening.
    pub fn voice_is_listening(&self) -> bool {
        if let Ok(supervisor) = self.voice_supervisor.lock() {
            supervisor.is_listening()
        } else {
            false
        }
    }

    /// Returns the active voice listening mode.
    pub fn voice_mode(&self) -> VoiceListeningMode {
        if let Ok(supervisor) = self.voice_supervisor.lock() {
            supervisor.mode()
        } else {
            VoiceListeningMode::PushToTalk
        }
    }

    /// Returns a live model catalog snapshot and detected hardware parameters.
    pub fn model_snapshot(&self) -> ModelManagerSnapshot {
        if let Ok(sup) = self.model_supervisor.lock() {
            sup.snapshot()
        } else {
            ModelManagerSnapshot {
                hardware: HardwareOverview {
                    cpu_architecture: "unknown".to_string(),
                    cpu_cores: 1,
                    simd_features: vec![],
                    total_ram_bytes: 0,
                    usable_budget_bytes: 0,
                    recommended_tier: "Unsupported".to_string(),
                    recommended_context: 0,
                    recommended_threads: 1,
                },
                active_model_id: None,
                models: vec![],
            }
        }
    }

    /// Loads a model into memory by ID.
    pub fn model_load(&self, model_id: &str) -> FfiResult<()> {
        let mut sup = self
            .model_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        sup.load_model(model_id)
    }

    /// Unloads the active model from memory.
    pub fn model_unload(&self) -> FfiResult<()> {
        let mut sup = self
            .model_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        sup.unload_model()
    }

    /// Executes an on-device inference benchmark.
    pub fn model_benchmark_run(
        &self,
        prompt_tokens: usize,
        generate_tokens: usize,
    ) -> FfiResult<BenchmarkReport> {
        let mut sup = self
            .model_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        sup.run_benchmark(prompt_tokens, generate_tokens)
    }

    /// Returns the device supervisor.
    #[must_use]
    pub fn device_supervisor(&self) -> Arc<DeviceSupervisor> {
        self.device_supervisor.clone()
    }

    /// Returns discovered local network peers.
    #[must_use]
    pub fn device_discovered_peers(&self) -> Vec<DiscoveredPeerItem> {
        self.device_supervisor.discovered_peers()
    }

    /// Returns paired devices and their assigned permissions.
    pub fn device_paired_list(&self) -> FfiResult<Vec<PairedDeviceItem>> {
        self.device_supervisor.paired_devices()
    }

    /// Initiates a pairing ceremony with a discovered peer.
    pub fn device_start_pairing(&self, peer_instance: &str) -> FfiResult<PairingSessionInfo> {
        self.device_supervisor.start_pairing(peer_instance)
    }

    /// Confirms SAS code and persists peer into DeviceRegistry.
    pub fn device_confirm_pairing(&self, sas_code: &str) -> FfiResult<PairedDeviceItem> {
        self.device_supervisor.confirm_pairing(sas_code)
    }

    /// Cancels active pairing ceremony.
    pub fn device_cancel_pairing(&self) -> FfiResult<()> {
        self.device_supervisor.cancel_pairing()
    }

    /// Sets or updates a capability permission level for a paired device.
    pub fn device_set_permission(
        &self,
        device_id: &str,
        capability: &str,
        level: &str,
    ) -> FfiResult<()> {
        self.device_supervisor
            .set_peer_permission(device_id, capability, level)
    }

    /// Unpairs a device and revokes its permissions.
    pub fn device_unpair(&self, device_id: &str) -> FfiResult<()> {
        self.device_supervisor.unpair_device(device_id)
    }

    /// Injects a discovered peer (useful for UI testing).
    pub fn device_simulate_peer(&self, item: DiscoveredPeerItem) -> FfiResult<()> {
        self.device_supervisor.simulate_discovered_peer(item)
    }

    /// Returns all indexed documents ordered by updated timestamp descending.
    pub fn knowledge_list_documents(&self) -> FfiResult<Vec<DocumentRecordItem>> {
        let store = aissistant_storage::DocumentStore::new(self.db.clone());
        let docs = store.list_documents().map_err(FfiError::Storage)?;
        Ok(docs
            .into_iter()
            .map(|d| DocumentRecordItem {
                id: d.id,
                path: d.path,
                file_name: d.file_name,
                mime_type: d.mime_type,
                content_hash: d.content_hash,
                file_size_bytes: d.file_size_bytes,
                chunk_count: d.chunk_count,
                indexed_at_ms: d.indexed_at_ms,
                updated_at_ms: d.updated_at_ms,
            })
            .collect())
    }

    /// Ingests and chunks a local document text.
    pub fn knowledge_index_document(
        &self,
        path: &str,
        file_name: &str,
        mime_type: &str,
        content: &str,
    ) -> FfiResult<String> {
        let store = aissistant_storage::DocumentStore::new(self.db.clone());
        let status = store
            .index_document(
                path,
                file_name,
                mime_type,
                content,
                None::<fn(&str) -> Result<Vec<f32>, aissistant_storage::StorageError>>,
            )
            .map_err(FfiError::Storage)?;

        let doc_id = match status {
            aissistant_storage::IndexingStatus::Indexed { document_id, .. }
            | aissistant_storage::IndexingStatus::Updated { document_id, .. }
            | aissistant_storage::IndexingStatus::SkippedUnchanged { document_id } => document_id,
        };
        Ok(doc_id)
    }

    /// Deletes an indexed document and its associated chunks and embeddings by ID.
    pub fn knowledge_delete_document(&self, doc_id: &str) -> FfiResult<bool> {
        let store = aissistant_storage::DocumentStore::new(self.db.clone());
        store.delete_document(doc_id).map_err(FfiError::Storage)
    }

    /// Performs hybrid search over document chunks and returns grounded citations.
    pub fn knowledge_search(
        &self,
        query: &str,
        limit: usize,
    ) -> FfiResult<Vec<KnowledgeCitationItem>> {
        let doc_store = aissistant_storage::DocumentStore::new(self.db.clone());
        let vec_store = aissistant_storage::VectorMemoryStore::new(self.db.clone());
        let retriever = aissistant_storage::KnowledgeRetriever::new(doc_store, vec_store);
        let citations = retriever
            .search(query, None, limit, None)
            .map_err(FfiError::Storage)?;
        Ok(citations
            .into_iter()
            .map(|c| KnowledgeCitationItem {
                chunk_id: c.chunk_id,
                document_id: c.document_id,
                path: c.path,
                file_name: c.file_name,
                chunk_index: c.chunk_index,
                rrf_score: c.rrf_score,
                bm25_rank: c.bm25_rank,
                vector_rank: c.vector_rank,
                cosine_similarity: c.cosine_similarity,
                content: c.content,
                snippet: c.snippet,
            })
            .collect())
    }
}

// Global Singleton Bridge Management

static GLOBAL_BRIDGE: OnceLock<RwLock<Option<Arc<CoreBridge>>>> = OnceLock::new();

fn get_bridge_slot() -> &'static RwLock<Option<Arc<CoreBridge>>> {
    GLOBAL_BRIDGE.get_or_init(|| RwLock::new(None))
}

/// Initializes the global core bridge singleton with an optional storage directory.
pub fn init_bridge(data_dir: Option<&str>) -> FfiResult<()> {
    let slot = get_bridge_slot();
    let mut guard = slot
        .write()
        .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;

    if guard.is_some() {
        return Ok(()); // Idempotent initialization
    }

    let bridge = match data_dir {
        Some(path) => CoreBridge::new_with_path(Path::new(path))?,
        None => CoreBridge::new_in_memory()?,
    };

    *guard = Some(Arc::new(bridge));
    Ok(())
}

/// Retrieves the global core bridge instance.
pub fn get_bridge() -> FfiResult<Arc<CoreBridge>> {
    let slot = get_bridge_slot();
    let guard = slot
        .read()
        .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;

    match &*guard {
        Some(b) => Ok(b.clone()),
        None => Err(FfiError::NotInitialized),
    }
}

/// Shuts down and drops the global core bridge singleton with graceful two-phase draining.
pub fn shutdown_bridge() -> FfiResult<()> {
    let slot = get_bridge_slot();
    let mut guard = slot
        .write()
        .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;

    if let Some(ref bridge) = *guard {
        let _ = bridge.request_shutdown(ShutdownReason::UserRequested, Duration::from_millis(1500));
    }

    *guard = None;
    Ok(())
}

/// Convenience entry point: returns health report from the global bridge.
pub fn bridge_get_health() -> FfiResult<HealthReport> {
    let bridge = get_bridge()?;
    Ok(bridge.health())
}

/// Convenience entry point: returns capability matrix from the global bridge.
pub fn bridge_get_capabilities() -> FfiResult<Vec<CapabilityItem>> {
    let bridge = get_bridge()?;
    Ok(bridge.capabilities())
}

/// Convenience entry point: updates capability permission on the global bridge.
pub fn bridge_update_permission(capability: &str, granted: bool) -> FfiResult<CapabilityItem> {
    let bridge = get_bridge()?;
    bridge.update_permission(capability, granted)
}

/// Convenience entry point: queries audit log from the global bridge.
pub fn bridge_query_audit(limit: usize, offset: usize) -> FfiResult<Vec<AuditItem>> {
    let bridge = get_bridge()?;
    bridge.query_audit_log(limit, offset)
}

/// Convenience entry point: verifies audit chain integrity on the global bridge.
pub fn bridge_verify_audit() -> FfiResult<AuditIntegrityReport> {
    let bridge = get_bridge()?;
    bridge.verify_audit_integrity()
}

/// Convenience entry point: retrieves setting value from the global bridge.
pub fn bridge_get_setting(key: &str) -> FfiResult<Option<String>> {
    let bridge = get_bridge()?;
    bridge.get_setting(key)
}

/// Convenience entry point: saves setting value on the global bridge.
pub fn bridge_set_setting(key: &str, value_json: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.set_setting(key, value_json)
}

/// Convenience entry point: queries active lifecycle state on the global bridge.
pub fn bridge_get_lifecycle_state() -> FfiResult<LifecycleState> {
    let bridge = get_bridge()?;
    Ok(bridge.lifecycle_state())
}

/// Convenience entry point: requests graceful shutdown on the global bridge.
pub fn bridge_request_shutdown(reason: ShutdownReason) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.request_shutdown(reason, Duration::from_millis(2000))
}

/// Convenience entry point: delivers memory pressure notification to the global bridge.
pub fn bridge_notify_memory_pressure(level: MemoryPressureLevel) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.notify_memory_pressure(level)
}

/// Convenience entry point: starts voice listening on the global bridge.
pub fn bridge_voice_start_listening(mode: u32) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.voice_start_listening(VoiceListeningMode::from(mode))
}

/// Convenience entry point: stops voice listening on the global bridge.
pub fn bridge_voice_stop_listening() -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.voice_stop_listening()
}

/// Convenience entry point: sets voice listening mode on the global bridge.
pub fn bridge_voice_set_mode(mode: u32) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.voice_set_mode(VoiceListeningMode::from(mode))
}

/// Convenience entry point: sets active wake-word keywords from a JSON string on the global bridge.
pub fn bridge_voice_set_keywords(keywords_json: &str) -> FfiResult<()> {
    let keywords: Vec<String> = serde_json::from_str(keywords_json)
        .map_err(|e| FfiError::Serialization(format!("Invalid keywords JSON: {e}")))?;
    let bridge = get_bridge()?;
    bridge.voice_set_keywords(&keywords)
}

/// Convenience entry point: injects audio samples into the global bridge voice pipeline.
pub fn bridge_voice_inject_audio(samples: &[f32]) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.voice_inject_audio(samples)
}

/// Convenience entry point: returns a snapshot of model catalog and hardware capabilities.
pub fn bridge_model_snapshot() -> FfiResult<ModelManagerSnapshot> {
    let bridge = get_bridge()?;
    Ok(bridge.model_snapshot())
}

/// Convenience entry point: loads a model by ID on the global bridge.
pub fn bridge_model_load(model_id: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.model_load(model_id)
}

/// Convenience entry point: unloads the active model on the global bridge.
pub fn bridge_model_unload() -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.model_unload()
}

/// Convenience entry point: runs a benchmark on the global bridge.
pub fn bridge_model_benchmark_run(
    prompt_tokens: usize,
    generate_tokens: usize,
) -> FfiResult<BenchmarkReport> {
    let bridge = get_bridge()?;
    bridge.model_benchmark_run(prompt_tokens, generate_tokens)
}

/// Convenience entry point: returns discovered network peers.
pub fn bridge_device_discovered_peers() -> FfiResult<Vec<DiscoveredPeerItem>> {
    let bridge = get_bridge()?;
    Ok(bridge.device_discovered_peers())
}

/// Convenience entry point: returns paired devices.
pub fn bridge_device_paired_list() -> FfiResult<Vec<PairedDeviceItem>> {
    let bridge = get_bridge()?;
    bridge.device_paired_list()
}

/// Convenience entry point: initiates pairing with a peer.
pub fn bridge_device_start_pairing(peer_instance: &str) -> FfiResult<PairingSessionInfo> {
    let bridge = get_bridge()?;
    bridge.device_start_pairing(peer_instance)
}

/// Convenience entry point: confirms pairing with the 6-digit SAS code.
pub fn bridge_device_confirm_pairing(sas_code: &str) -> FfiResult<PairedDeviceItem> {
    let bridge = get_bridge()?;
    bridge.device_confirm_pairing(sas_code)
}

/// Convenience entry point: cancels active pairing ceremony.
pub fn bridge_device_cancel_pairing() -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.device_cancel_pairing()
}

/// Convenience entry point: updates peer capability permission level.
pub fn bridge_device_set_permission(
    device_id: &str,
    capability: &str,
    level: &str,
) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.device_set_permission(device_id, capability, level)
}

/// Convenience entry point: unpairs a device.
pub fn bridge_device_unpair(device_id: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.device_unpair(device_id)
}

/// Convenience entry point: simulates a discovered peer.
pub fn bridge_device_simulate_peer(item: DiscoveredPeerItem) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.device_simulate_peer(item)
}

/// Convenience entry point: lists all indexed documents.
pub fn bridge_knowledge_list_documents() -> FfiResult<Vec<DocumentRecordItem>> {
    let bridge = get_bridge()?;
    bridge.knowledge_list_documents()
}

/// Convenience entry point: indexes a document text.
pub fn bridge_knowledge_index_document(
    path: &str,
    file_name: &str,
    mime_type: &str,
    content: &str,
) -> FfiResult<String> {
    let bridge = get_bridge()?;
    bridge.knowledge_index_document(path, file_name, mime_type, content)
}

/// Convenience entry point: deletes an indexed document by ID.
pub fn bridge_knowledge_delete_document(doc_id: &str) -> FfiResult<bool> {
    let bridge = get_bridge()?;
    bridge.knowledge_delete_document(doc_id)
}

/// Convenience entry point: performs hybrid knowledge search.
pub fn bridge_knowledge_search(query: &str, limit: usize) -> FfiResult<Vec<KnowledgeCitationItem>> {
    let bridge = get_bridge()?;
    bridge.knowledge_search(query, limit)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}
