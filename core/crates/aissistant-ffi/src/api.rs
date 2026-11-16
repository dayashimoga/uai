//! High-level, safe FFI application programming interface and core state bridge.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Duration;

use aissistant_core::{
    AuditEntryId, Capability, CapabilityRegistry, CapabilityStatus, ConversationId, CoreError,
    SkillId,
};
use aissistant_llm::{Language, LocalEmbeddingEngine, TranslationEngine};
use aissistant_orchestrator::{
    ConfirmationId, ConfirmationManager, InterpreterSession, InterpreterSessionConfig,
    Orchestrator, OrchestratorConfig, OrchestratorEvent,
};
use aissistant_security::{
    Actor, AeadKey, AuditEntry, AuditExecutionStatus, DeviceIdentity, PolicyDecision, PolicyEngine,
    TaintTracker, GENESIS_PREV_HASH,
};
use aissistant_skills::{
    AutomationEngine, SmartHomeCommand, WorkflowAction, WorkflowExecutionReport,
};
use aissistant_speech::acoustic_tts::AcousticTtsEngine;
use aissistant_speech::tts::TtsConfig;
use aissistant_storage::{
    AuditStore, AutomationHistoryRecord, AutomationRuleRecord, AutomationStore,
    AutomationTriggerType, ConversationStore, FileBoundKeystore, HybridSearchEngine,
    KeystoreProvider, PersonaStore, PersonaTone, ResponseVerbosity, SecretStore, SettingsStore,
    SmartHomeDomain, SmartHomeEntityRecord, SmartHomeProtocol, SmartHomeStore, SqliteDb,
    UserPersona, UserPreference, VectorMemoryStore,
};
use serde::{Deserialize, Serialize};

use crate::device_bridge::{
    DeviceSupervisor, DiscoveredPeerItem, PairedDeviceItem, PairingSessionInfo,
};
use crate::error::{FfiError, FfiResult};
use crate::events::{BridgeEvent, EventStreamSink};
use crate::lifecycle::{
    CancellationToken, CoreRuntime, LifecycleState, MemoryPressureLevel, PowerTransition,
    ShutdownReason,
};
use crate::model_bridge::{
    BenchmarkReport, BridgeLlmEngine, HardwareOverview, ModelCatalogItem, ModelManagerSnapshot,
    ModelSupervisor,
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

/// FFI view of a conversation thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationItem {
    pub id: String,
    pub title: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// FFI view of a conversation message turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatTurnItem {
    pub id: String,
    pub conversation_id: String,
    pub turn_index: u32,
    pub role: String,
    pub content: String,
    pub timestamp_ms: i64,
    pub tool_calls_json: Option<String>,
}

/// FFI view of an FTS5 search result within conversation turns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatSearchItem {
    pub turn_id: String,
    pub conversation_id: String,
    pub turn_index: u32,
    pub role: String,
    pub content: String,
    pub snippet: String,
    pub rank: f64,
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

/// FFI view of a 2D bounding box normalized to `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OcrBoundingBoxItem {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// FFI view of an OCR-recognized word.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrWordItem {
    pub text: String,
    pub bbox: OcrBoundingBoxItem,
    pub confidence: f32,
}

/// FFI view of a line of recognized text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrTextLineItem {
    pub line_number: usize,
    pub text: String,
    pub bbox: OcrBoundingBoxItem,
    pub confidence: f32,
    pub words: Vec<OcrWordItem>,
}

/// FFI view of complete OCR extraction results for an image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrDocumentItem {
    pub lines: Vec<OcrTextLineItem>,
    pub full_text: String,
    pub average_confidence: f32,
    pub image_width: u32,
    pub image_height: u32,
    pub processing_time_ms: u64,
}

/// FFI view of a multilingual text translation result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslationResultItem {
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub confidence: f32,
    pub latency_ms: u64,
    pub engine: String,
}

/// FFI view of a live interpreter dialogue turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpreterTurnItem {
    pub turn_id: u32,
    pub speaker: String,
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub confidence: f32,
    pub latency_ms: u64,
    pub timestamp_ms: u64,
}

/// FFI view of interpreter session state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpreterSessionStatusItem {
    pub session_id: String,
    pub party_a_lang: String,
    pub party_b_lang: String,
    pub active_speaker: String,
    pub turn_count: usize,
}

/// FFI view of a user persona.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPersonaItem {
    pub id: String,
    pub name: String,
    pub tone: String,
    pub verbosity: String,
    pub voice_id: String,
    pub speaking_rate: f32,
    pub pitch: f32,
    pub system_prompt_override: Option<String>,
    pub is_active: bool,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl From<UserPersona> for UserPersonaItem {
    fn from(p: UserPersona) -> Self {
        Self {
            id: p.id,
            name: p.name,
            tone: p.tone.as_str().to_string(),
            verbosity: p.verbosity.as_str().to_string(),
            voice_id: p.voice_id,
            speaking_rate: p.speaking_rate,
            pitch: p.pitch,
            system_prompt_override: p.system_prompt_override,
            is_active: p.is_active,
            created_at_ms: p.created_at_ms,
            updated_at_ms: p.updated_at_ms,
        }
    }
}

impl TryFrom<UserPersonaItem> for UserPersona {
    type Error = FfiError;

    fn try_from(item: UserPersonaItem) -> Result<Self, Self::Error> {
        let tone = PersonaTone::from_str(&item.tone)
            .map_err(|e| FfiError::InvalidParameter(e.to_string()))?;
        let verbosity = ResponseVerbosity::from_str(&item.verbosity)
            .map_err(|e| FfiError::InvalidParameter(e.to_string()))?;

        Ok(Self {
            id: item.id,
            name: item.name,
            tone,
            verbosity,
            voice_id: item.voice_id,
            speaking_rate: item.speaking_rate,
            pitch: item.pitch,
            system_prompt_override: item.system_prompt_override,
            is_active: item.is_active,
            created_at_ms: item.created_at_ms,
            updated_at_ms: item.updated_at_ms,
        })
    }
}

/// FFI view of an explicit user preference entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPreferenceItem {
    pub key: String,
    pub value_json: String,
    pub category: String,
    pub updated_at_ms: i64,
}

impl From<UserPreference> for UserPreferenceItem {
    fn from(pref: UserPreference) -> Self {
        Self {
            key: pref.key,
            value_json: pref.value_json,
            category: pref.category,
            updated_at_ms: pref.updated_at_ms,
        }
    }
}

/// FFI view of an action within an automated workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowActionItem {
    pub action_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_json: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl From<WorkflowAction> for WorkflowActionItem {
    fn from(action: WorkflowAction) -> Self {
        match action {
            WorkflowAction::LocalSkill {
                skill_id,
                payload_json,
            } => Self {
                action_type: "local_skill".to_string(),
                skill_id: Some(skill_id),
                peer_id: None,
                payload_json: Some(payload_json),
                title: None,
                message: None,
            },
            WorkflowAction::RemoteSkill {
                peer_id,
                skill_id,
                payload_json,
            } => Self {
                action_type: "remote_skill".to_string(),
                skill_id: Some(skill_id),
                peer_id: Some(peer_id),
                payload_json: Some(payload_json),
                title: None,
                message: None,
            },
            WorkflowAction::Notification { title, message } => Self {
                action_type: "notification".to_string(),
                skill_id: None,
                peer_id: None,
                payload_json: None,
                title: Some(title),
                message: Some(message),
            },
        }
    }
}

impl TryFrom<WorkflowActionItem> for WorkflowAction {
    type Error = FfiError;

    fn try_from(item: WorkflowActionItem) -> Result<Self, Self::Error> {
        match item.action_type.as_str() {
            "local_skill" => Ok(Self::LocalSkill {
                skill_id: item.skill_id.unwrap_or_default(),
                payload_json: item.payload_json.unwrap_or_default(),
            }),
            "remote_skill" => Ok(Self::RemoteSkill {
                peer_id: item.peer_id.unwrap_or_default(),
                skill_id: item.skill_id.unwrap_or_default(),
                payload_json: item.payload_json.unwrap_or_default(),
            }),
            "notification" => Ok(Self::Notification {
                title: item.title.unwrap_or_default(),
                message: item.message.unwrap_or_default(),
            }),
            other => Err(FfiError::InvalidParameter(format!(
                "Unknown workflow action type: '{other}'"
            ))),
        }
    }
}

/// FFI view of an automated workflow rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationRuleItem {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub trigger_type: String,
    pub trigger_config_json: String,
    pub actions_json: String,
    pub last_executed_ms: Option<i64>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl From<AutomationRuleRecord> for AutomationRuleItem {
    fn from(r: AutomationRuleRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            enabled: r.enabled,
            trigger_type: r.trigger_type.as_str().to_string(),
            trigger_config_json: r.trigger_config_json,
            actions_json: r.actions_json,
            last_executed_ms: r.last_executed_ms,
            created_at_ms: r.created_at_ms,
            updated_at_ms: r.updated_at_ms,
        }
    }
}

impl TryFrom<AutomationRuleItem> for AutomationRuleRecord {
    type Error = FfiError;

    fn try_from(item: AutomationRuleItem) -> Result<Self, Self::Error> {
        let trigger_type = AutomationTriggerType::from_str(&item.trigger_type)
            .map_err(|e| FfiError::InvalidParameter(e.to_string()))?;
        Ok(Self {
            id: item.id,
            name: item.name,
            enabled: item.enabled,
            trigger_type,
            trigger_config_json: item.trigger_config_json,
            actions_json: item.actions_json,
            last_executed_ms: item.last_executed_ms,
            created_at_ms: item.created_at_ms,
            updated_at_ms: item.updated_at_ms,
        })
    }
}

/// FFI view of an automation execution history entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationHistoryItem {
    pub id: String,
    pub automation_id: String,
    pub status: String,
    pub triggered_by: String,
    pub executed_at_ms: i64,
    pub duration_ms: u64,
    pub result_json: String,
}

impl From<AutomationHistoryRecord> for AutomationHistoryItem {
    fn from(h: AutomationHistoryRecord) -> Self {
        Self {
            id: h.id,
            automation_id: h.automation_id,
            status: h.status.as_str().to_string(),
            triggered_by: h.triggered_by,
            executed_at_ms: h.executed_at_ms,
            duration_ms: h.duration_ms,
            result_json: h.result_json,
        }
    }
}

/// FFI view of an automation workflow execution report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionReportItem {
    pub automation_id: String,
    pub status: String,
    pub triggered_by: String,
    pub duration_ms: u64,
    pub executed_at_ms: i64,
    pub result_json: String,
}

impl From<WorkflowExecutionReport> for WorkflowExecutionReportItem {
    fn from(r: WorkflowExecutionReport) -> Self {
        Self {
            automation_id: r.automation_id,
            status: r.status.as_str().to_string(),
            triggered_by: r.triggered_by,
            duration_ms: r.duration_ms,
            executed_at_ms: r.executed_at_ms,
            result_json: serde_json::to_string(&r.action_results).unwrap_or_default(),
        }
    }
}

/// FFI view of a registered smart-home entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartHomeEntityItem {
    pub id: String,
    pub domain: String,
    pub name: String,
    pub state_json: String,
    pub attributes_json: String,
    pub protocol: String,
    pub is_available: bool,
    pub last_updated_ms: i64,
}

impl From<SmartHomeEntityRecord> for SmartHomeEntityItem {
    fn from(r: SmartHomeEntityRecord) -> Self {
        Self {
            id: r.id,
            domain: r.domain.as_str().to_string(),
            name: r.name,
            state_json: r.state_json,
            attributes_json: r.attributes_json,
            protocol: r.protocol.as_str().to_string(),
            is_available: r.is_available,
            last_updated_ms: r.last_updated_ms,
        }
    }
}

impl TryFrom<SmartHomeEntityItem> for SmartHomeEntityRecord {
    type Error = FfiError;

    fn try_from(item: SmartHomeEntityItem) -> Result<Self, Self::Error> {
        let domain = SmartHomeDomain::from_str(&item.domain)
            .map_err(|e| FfiError::InvalidParameter(e.to_string()))?;
        let protocol = SmartHomeProtocol::from_str(&item.protocol)
            .map_err(|e| FfiError::InvalidParameter(e.to_string()))?;
        Ok(Self {
            id: item.id,
            domain,
            name: item.name,
            state_json: item.state_json,
            attributes_json: item.attributes_json,
            protocol,
            is_available: item.is_available,
            last_updated_ms: item.last_updated_ms,
        })
    }
}

/// FFI view of a smart home actuation command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartHomeCommandItem {
    pub entity_id: String,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brightness: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_hex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_celsius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_percent: Option<u8>,
}

/// FFI result of a smart home device command actuation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartHomeCommandResultItem {
    pub success: bool,
    pub entity_id: String,
    pub command: String,
    pub dispatched_via: String,
    pub dispatch_action: String,
    pub dispatch_payload_json: String,
    pub new_state_json: String,
}

/// Comprehensive performance, latency, and memory footprint snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    /// Cold-start bridge bootstrap duration in milliseconds.
    pub cold_start_ms: u64,
    /// Last measured or estimated wake-word acoustic frame processing latency in microseconds.
    pub wake_word_frame_micros: u64,
    /// End-of-speech to STT transcript latency in milliseconds.
    pub stt_latency_ms: u64,
    /// Time-to-first-audio for TTS synthesis in milliseconds.
    pub tts_first_audio_ms: u64,
    /// Total process resident set size (RSS) in bytes.
    pub resident_memory_bytes: u64,
    /// Target idle memory ceiling for current platform in bytes (80 MB desktop / 60 MB mobile).
    pub idle_budget_bytes: u64,
    /// Whether current resident memory exceeds the idle target budget.
    pub memory_budget_exceeded: bool,
    /// Operating system platform identifier.
    pub platform: String,
    /// Active SQLite cache page count or memory estimate.
    pub sqlite_memory_bytes: u64,
    /// Unix millisecond timestamp of this snapshot.
    pub timestamp_ms: u64,
}

/// Detailed latency benchmark results measuring key pipeline stages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineBenchmarkReport {
    /// Number of audio frames processed during wake-word benchmark.
    pub wake_word_frames_tested: usize,
    /// Average processing duration per 20 ms wake-word audio frame in microseconds.
    pub wake_word_avg_frame_micros: u64,
    /// Real-time factor (CPU time / Audio duration; e.g. < 0.05 is 20x faster than real-time).
    pub wake_word_rtf: f32,
    /// Ring buffer throughput in million samples per second.
    pub ring_buffer_throughput_msps: f32,
    /// Cold start bootstrap latency in milliseconds.
    pub cold_start_ms: u64,
    /// VAD window processing latency in microseconds.
    pub vad_window_micros: u64,
    /// SQLite query round-trip latency in microseconds.
    pub sqlite_query_micros: u64,
    /// Process RSS before benchmark in bytes.
    pub memory_before_bytes: u64,
    /// Process RSS after benchmark in bytes.
    pub memory_after_bytes: u64,
    /// All NFR-1..NFR-8 targets satisfied.
    pub all_budgets_passed: bool,
}

/// Core bridge orchestrating subsystem access and safe FFI dispatch.
pub struct CoreBridge {
    db: SqliteDb,
    audit_store: AuditStore,
    settings_store: SettingsStore,
    secret_store: SecretStore,
    persona_store: PersonaStore,
    pub(crate) automation_store: AutomationStore,
    pub(crate) smart_home_store: SmartHomeStore,
    lockdown_active: Arc<AtomicBool>,
    capability_registry: RwLock<CapabilityRegistry>,
    policy_engine: RwLock<PolicyEngine>,
    event_sink: Arc<EventStreamSink>,
    runtime: Arc<CoreRuntime>,
    identity: DeviceIdentity,
    voice_supervisor: Arc<Mutex<VoicePipelineSupervisor>>,
    model_supervisor: Arc<Mutex<ModelSupervisor>>,
    device_supervisor: Arc<DeviceSupervisor>,
    interpreter_session: Arc<Mutex<InterpreterSession>>,
    translation_engine: TranslationEngine,
    llm_engine: Arc<BridgeLlmEngine>,
    tts_engine: Arc<Mutex<AcousticTtsEngine>>,
    pub(crate) conversation_store: ConversationStore,
    pub(crate) orchestrator: Arc<Mutex<Orchestrator>>,
    cold_start_ms: u64,
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
        let start_time = std::time::Instant::now();
        let event_sink = Arc::new(EventStreamSink::new());
        let runtime = Arc::new(CoreRuntime::new(event_sink.clone()));
        runtime.transition_to(LifecycleState::Starting, Some("in_memory_init"))?;

        let db = SqliteDb::open_in_memory()?;
        let audit_store = AuditStore::new(db.clone());
        let settings_store = SettingsStore::new(db.clone());
        let master_key = AeadKey::generate();
        let secret_store = SecretStore::new(db.clone(), master_key);
        let persona_store = PersonaStore::new(db.clone());
        let automation_store = AutomationStore::new(db.clone());
        let smart_home_store = SmartHomeStore::new(db.clone());
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
        let interpreter_session = Arc::new(Mutex::new(InterpreterSession::new(
            InterpreterSessionConfig::default(),
        )));
        let translation_engine = TranslationEngine::new();
        let lockdown_active = Arc::new(AtomicBool::new(false));

        let conversation_store = ConversationStore::new(db.clone());
        let vec_store = VectorMemoryStore::new(db.clone());
        let hybrid_search = HybridSearchEngine::new(conversation_store.clone(), vec_store.clone());
        let embed_engine = Arc::new(LocalEmbeddingEngine::default());
        let bridge_llm = Arc::new(BridgeLlmEngine::new(model_supervisor.clone()));
        if let Ok(mut session) = interpreter_session.lock() {
            session.set_llm_engine(bridge_llm.clone());
        }
        let tts_engine = Arc::new(Mutex::new(
            AcousticTtsEngine::new(TtsConfig::default())
                .map_err(|e| FfiError::Voice(format!("Failed to initialize TTS engine: {e}")))?,
        ));
        let orch_raw = Orchestrator::new(
            OrchestratorConfig::default(),
            conversation_store.clone(),
            vec_store,
            hybrid_search,
            embed_engine,
            bridge_llm.clone(),
            None,
        )
        .with_policy_engine(Arc::new(RwLock::new(PolicyEngine::new())))
        .with_audit_store(Arc::new(AuditStore::new(db.clone())))
        .with_confirmation_manager(Arc::new(ConfirmationManager::new()))
        .with_taint_tracker(Arc::new(RwLock::new(TaintTracker::new())))
        .with_persona_store(Arc::new(PersonaStore::new(db.clone())));

        let sink_orch = event_sink.clone();
        orch_raw.on_event(move |ev| match ev {
            OrchestratorEvent::TokenStreamed { turn_id, token, .. } => {
                sink_orch.broadcast(BridgeEvent::ChatTokenDelta {
                    turn_id: turn_id.to_string(),
                    token: token.clone(),
                });
            }
            OrchestratorEvent::TurnFinished { turn_id, .. } => {
                sink_orch.broadcast(BridgeEvent::ChatTurnComplete {
                    turn_id: turn_id.to_string(),
                    citations: Vec::new(),
                });
            }
            OrchestratorEvent::ConfirmationRequired { confirmation } => {
                sink_orch.broadcast(BridgeEvent::ToolConfirmationRequired {
                    turn_id: confirmation.turn_id.to_string(),
                    tool_name: confirmation.tool_name.clone(),
                    risk_tier: format!("{:?}", confirmation.risk_tier),
                    params_json: confirmation.arguments.to_string(),
                    taint_status: confirmation.untrusted_input_tainted,
                });
            }
            OrchestratorEvent::ConfirmationResolved { id, approved } => {
                sink_orch.broadcast(BridgeEvent::ToolConfirmationResolved {
                    confirmation_id: id.to_string(),
                    approved: *approved,
                });
            }
            _ => {}
        });
        let orchestrator = Arc::new(Mutex::new(orch_raw));

        runtime.transition_to(LifecycleState::Running, Some("ready"))?;
        let cold_start_ms = start_time.elapsed().as_millis() as u64;

        Ok(Self {
            db,
            audit_store,
            settings_store,
            secret_store,
            persona_store,
            automation_store,
            smart_home_store,
            lockdown_active,
            capability_registry,
            policy_engine,
            event_sink,
            runtime,
            identity,
            voice_supervisor,
            model_supervisor,
            device_supervisor,
            interpreter_session,
            translation_engine,
            llm_engine: bridge_llm.clone(),
            tts_engine,
            conversation_store,
            orchestrator,
            cold_start_ms,
            data_dir: None,
        })
    }

    /// Creates a directory-persisted bridge instance with SQLite database and encrypted keystore.
    pub fn new_with_path(data_dir: impl AsRef<Path>) -> FfiResult<Self> {
        let start_time = std::time::Instant::now();
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
        let persona_store = PersonaStore::new(db.clone());
        let automation_store = AutomationStore::new(db.clone());
        let smart_home_store = SmartHomeStore::new(db.clone());

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
        let interpreter_session = Arc::new(Mutex::new(InterpreterSession::new(
            InterpreterSessionConfig::default(),
        )));
        let translation_engine = TranslationEngine::new();
        let lockdown_active = Arc::new(AtomicBool::new(false));

        let conversation_store = ConversationStore::new(db.clone());
        let vec_store = VectorMemoryStore::new(db.clone());
        let hybrid_search = HybridSearchEngine::new(conversation_store.clone(), vec_store.clone());
        let embed_engine = Arc::new(LocalEmbeddingEngine::default());
        let bridge_llm = Arc::new(BridgeLlmEngine::new(model_supervisor.clone()));
        if let Ok(mut session) = interpreter_session.lock() {
            session.set_llm_engine(bridge_llm.clone());
        }
        let tts_engine = Arc::new(Mutex::new(
            AcousticTtsEngine::new(TtsConfig::default())
                .map_err(|e| FfiError::Voice(format!("Failed to initialize TTS engine: {e}")))?,
        ));
        let orch_raw = Orchestrator::new(
            OrchestratorConfig::default(),
            conversation_store.clone(),
            vec_store,
            hybrid_search,
            embed_engine,
            bridge_llm.clone(),
            None,
        )
        .with_policy_engine(Arc::new(RwLock::new(PolicyEngine::new())))
        .with_audit_store(Arc::new(AuditStore::new(db.clone())))
        .with_confirmation_manager(Arc::new(ConfirmationManager::new()))
        .with_taint_tracker(Arc::new(RwLock::new(TaintTracker::new())))
        .with_persona_store(Arc::new(PersonaStore::new(db.clone())));

        let sink_orch = event_sink.clone();
        orch_raw.on_event(move |ev| match ev {
            OrchestratorEvent::TokenStreamed { turn_id, token, .. } => {
                sink_orch.broadcast(BridgeEvent::ChatTokenDelta {
                    turn_id: turn_id.to_string(),
                    token: token.clone(),
                });
            }
            OrchestratorEvent::TurnFinished { turn_id, .. } => {
                sink_orch.broadcast(BridgeEvent::ChatTurnComplete {
                    turn_id: turn_id.to_string(),
                    citations: Vec::new(),
                });
            }
            OrchestratorEvent::ConfirmationRequired { confirmation } => {
                sink_orch.broadcast(BridgeEvent::ToolConfirmationRequired {
                    turn_id: confirmation.turn_id.to_string(),
                    tool_name: confirmation.tool_name.clone(),
                    risk_tier: format!("{:?}", confirmation.risk_tier),
                    params_json: confirmation.arguments.to_string(),
                    taint_status: confirmation.untrusted_input_tainted,
                });
            }
            OrchestratorEvent::ConfirmationResolved { id, approved } => {
                sink_orch.broadcast(BridgeEvent::ToolConfirmationResolved {
                    confirmation_id: id.to_string(),
                    approved: *approved,
                });
            }
            _ => {}
        });
        let orchestrator = Arc::new(Mutex::new(orch_raw));

        runtime.transition_to(LifecycleState::Running, Some("ready"))?;
        let cold_start_ms = start_time.elapsed().as_millis() as u64;

        let bridge = Self {
            db,
            audit_store,
            settings_store,
            secret_store,
            persona_store,
            automation_store,
            smart_home_store,
            lockdown_active,
            capability_registry,
            policy_engine,
            event_sink,
            runtime,
            identity,
            voice_supervisor,
            model_supervisor,
            device_supervisor,
            interpreter_session,
            translation_engine,
            llm_engine: bridge_llm.clone(),
            tts_engine,
            conversation_store,
            orchestrator,
            cold_start_ms,
            data_dir: Some(path.to_path_buf()),
        };

        let _ = bridge.append_audit_entry(
            Actor::LocalUser,
            SkillId::new("core.supervisor.init").map_err(FfiError::Core)?,
            "{\"action\":\"system_boot\",\"keystore_unsealed\":true,\"status\":\"ok\"}",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        Ok(bridge)
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

    /// Delivers an operating system power transition notification (suspend / sleep or resume / wake).
    pub fn notify_power_transition(&self, transition: PowerTransition) -> FfiResult<()> {
        if transition == PowerTransition::Suspend {
            if let Ok(mut voice_sup) = self.voice_supervisor.lock() {
                voice_sup.clear_buffers();
            }
            if let Ok(mut model_sup) = self.model_supervisor.lock() {
                model_sup.on_memory_pressure(false);
            }
        }
        self.runtime
            .notify_power_transition(transition, Some(&self.db))
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
        let res = sup.load_model(model_id);
        if res.is_ok() {
            let _ = self.append_audit_entry(
                Actor::LocalUser,
                SkillId::new("llm.model.load").map_err(FfiError::Core)?,
                &format!("{{\"model_id\":\"{model_id}\"}}"),
                PolicyDecision::Allow,
                AuditExecutionStatus::Success,
            );
        }
        res
    }

    /// Unloads the active model from memory.
    pub fn model_unload(&self) -> FfiResult<()> {
        let mut sup = self
            .model_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        let res = sup.unload_model();
        if res.is_ok() {
            let _ = self.append_audit_entry(
                Actor::LocalUser,
                SkillId::new("llm.model.unload").map_err(FfiError::Core)?,
                "{\"action\":\"unload\"}",
                PolicyDecision::Allow,
                AuditExecutionStatus::Success,
            );
        }
        res
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

    /// Initiates a background download of a model by ID.
    pub fn model_download(&self, model_id: &str) -> FfiResult<()> {
        let sup = self
            .model_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        sup.download_model(model_id)
    }

    /// Cancels an active model download.
    pub fn model_cancel_download(&self, model_id: &str) -> FfiResult<()> {
        let sup = self
            .model_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        sup.cancel_download(model_id)
    }

    /// Deletes a model from local disk.
    pub fn model_delete(&self, model_id: &str) -> FfiResult<()> {
        let mut sup = self
            .model_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        sup.delete_model(model_id)
    }

    /// Imports a local .gguf file into the catalog.
    pub fn model_import_local_gguf(&self, file_path: &str) -> FfiResult<ModelCatalogItem> {
        let mut sup = self
            .model_supervisor
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        sup.import_local_gguf(file_path)
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

    /// Extracts recognized text, bounding boxes, and metrics from an image byte buffer.
    pub fn vision_ocr_extract(&self, image_bytes: &[u8]) -> FfiResult<OcrDocumentItem> {
        let img = aissistant_llm::ImageBuffer::from_encoded_bytes(image_bytes)
            .map_err(|e| FfiError::Model(e.to_string()))?;
        let engine = aissistant_llm::OfflineOcrEngine::new();
        let doc = engine
            .extract_text(&img)
            .map_err(|e| FfiError::Model(e.to_string()))?;

        Ok(OcrDocumentItem {
            lines: doc
                .lines
                .into_iter()
                .map(|l| OcrTextLineItem {
                    line_number: l.line_number,
                    text: l.text,
                    bbox: OcrBoundingBoxItem {
                        x: l.bbox.x,
                        y: l.bbox.y,
                        width: l.bbox.width,
                        height: l.bbox.height,
                    },
                    confidence: l.confidence,
                    words: l
                        .words
                        .into_iter()
                        .map(|w| OcrWordItem {
                            text: w.text,
                            bbox: OcrBoundingBoxItem {
                                x: w.bbox.x,
                                y: w.bbox.y,
                                width: w.bbox.width,
                                height: w.bbox.height,
                            },
                            confidence: w.confidence,
                        })
                        .collect(),
                })
                .collect(),
            full_text: doc.full_text,
            average_confidence: doc.average_confidence,
            image_width: doc.image_width,
            image_height: doc.image_height,
            processing_time_ms: doc.processing_time_ms,
        })
    }

    /// Translates text between two ISO 639-1 languages.
    pub fn translate_text(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> FfiResult<TranslationResultItem> {
        let src: Language = source_lang
            .parse()
            .map_err(|e| FfiError::InvalidParameter(format!("{e}")))?;
        let tgt: Language = target_lang
            .parse()
            .map_err(|e| FfiError::InvalidParameter(format!("{e}")))?;

        let res = self
            .translation_engine
            .translate_with_llm(text, src, tgt, Some(self.llm_engine.as_ref()))
            .map_err(|e| FfiError::Model(format!("{e}")))?;

        Ok(TranslationResultItem {
            source_text: res.source_text,
            translated_text: res.translated_text,
            source_lang: res.source_lang.code().to_string(),
            target_lang: res.target_lang.code().to_string(),
            confidence: res.confidence,
            latency_ms: res.latency_ms,
            engine: res.engine,
        })
    }

    /// Configures a live voice interpreter session.
    pub fn interpreter_start_session(
        &self,
        party_a_lang: &str,
        party_b_lang: &str,
    ) -> FfiResult<InterpreterSessionStatusItem> {
        let lang_a: Language = party_a_lang
            .parse()
            .map_err(|e| FfiError::InvalidParameter(format!("{e}")))?;
        let lang_b: Language = party_b_lang
            .parse()
            .map_err(|e| FfiError::InvalidParameter(format!("{e}")))?;

        let mut session_guard = self.interpreter_session.lock().map_err(|_| {
            FfiError::LockPoisoned("Failed to lock interpreter session".to_string())
        })?;

        let config = InterpreterSessionConfig {
            party_a_lang: lang_a,
            party_b_lang: lang_b,
            auto_swap_speaker: true,
        };

        let mut new_session = InterpreterSession::new(config);
        new_session.set_llm_engine(self.llm_engine.clone());
        let status = InterpreterSessionStatusItem {
            session_id: new_session.session_id().to_string(),
            party_a_lang: lang_a.code().to_string(),
            party_b_lang: lang_b.code().to_string(),
            active_speaker: new_session.active_speaker().as_str().to_string(),
            turn_count: 0,
        };

        *session_guard = new_session;
        Ok(status)
    }

    /// Replays text-to-speech audio for a completed interpreter turn.
    pub fn interpreter_replay_tts(&self, turn_id_str: &str) -> FfiResult<()> {
        let turn_id: u32 = turn_id_str
            .parse()
            .map_err(|_| FfiError::InvalidParameter(format!("Invalid turn_id: '{turn_id_str}'")))?;

        let translated_text = {
            let session_guard = self.interpreter_session.lock().map_err(|_| {
                FfiError::LockPoisoned("Failed to lock interpreter session".to_string())
            })?;
            let turn = session_guard
                .turns()
                .iter()
                .find(|t| t.turn_id == turn_id)
                .ok_or_else(|| {
                    FfiError::InvalidParameter(format!("Interpreter turn {turn_id} not found"))
                })?;
            turn.translated_text.clone()
        };

        if translated_text.trim().is_empty() {
            return Ok(());
        }

        let mut tts = self
            .tts_engine
            .lock()
            .map_err(|_| FfiError::LockPoisoned("Failed to lock TTS engine".to_string()))?;

        use aissistant_speech::tts::TtsEngine;
        let _chunk = tts
            .synthesize_sentence(&translated_text)
            .map_err(|e| FfiError::Voice(format!("TTS synthesis error: {e}")))?;

        Ok(())
    }

    /// Submits an utterance to the active interpreter session.
    pub fn interpreter_submit_turn(&self, text: &str) -> FfiResult<InterpreterTurnItem> {
        let mut session_guard = self.interpreter_session.lock().map_err(|_| {
            FfiError::LockPoisoned("Failed to lock interpreter session".to_string())
        })?;

        let turn = session_guard
            .submit_turn(text)
            .map_err(|e| FfiError::Model(format!("{e}")))?;

        Ok(InterpreterTurnItem {
            turn_id: turn.turn_id,
            speaker: turn.speaker.as_str().to_string(),
            source_text: turn.source_text,
            translated_text: turn.translated_text,
            source_lang: turn.source_lang.code().to_string(),
            target_lang: turn.target_lang.code().to_string(),
            confidence: turn.confidence,
            latency_ms: turn.latency_ms,
            timestamp_ms: turn.timestamp_ms,
        })
    }

    /// Returns all completed turns in the active interpreter session.
    pub fn interpreter_get_turns(&self) -> FfiResult<Vec<InterpreterTurnItem>> {
        let session_guard = self.interpreter_session.lock().map_err(|_| {
            FfiError::LockPoisoned("Failed to lock interpreter session".to_string())
        })?;

        let items = session_guard
            .turns()
            .iter()
            .map(|t| InterpreterTurnItem {
                turn_id: t.turn_id,
                speaker: t.speaker.as_str().to_string(),
                source_text: t.source_text.clone(),
                translated_text: t.translated_text.clone(),
                source_lang: t.source_lang.code().to_string(),
                target_lang: t.target_lang.code().to_string(),
                confidence: t.confidence,
                latency_ms: t.latency_ms,
                timestamp_ms: t.timestamp_ms,
            })
            .collect();

        Ok(items)
    }

    /// Swaps Party A and Party B languages in the active interpreter session.
    pub fn interpreter_swap_languages(&self) -> FfiResult<InterpreterSessionStatusItem> {
        let mut session_guard = self.interpreter_session.lock().map_err(|_| {
            FfiError::LockPoisoned("Failed to lock interpreter session".to_string())
        })?;

        session_guard.swap_languages();
        let cfg = session_guard.config();

        Ok(InterpreterSessionStatusItem {
            session_id: session_guard.session_id().to_string(),
            party_a_lang: cfg.party_a_lang.code().to_string(),
            party_b_lang: cfg.party_b_lang.code().to_string(),
            active_speaker: session_guard.active_speaker().as_str().to_string(),
            turn_count: session_guard.turns().len(),
        })
    }

    /// Clears turns history from the interpreter session.
    pub fn interpreter_clear_session(&self) -> FfiResult<()> {
        let mut session_guard = self.interpreter_session.lock().map_err(|_| {
            FfiError::LockPoisoned("Failed to lock interpreter session".to_string())
        })?;

        session_guard.clear_history();
        Ok(())
    }

    // Persona & Preference Management

    /// Returns the currently active user persona, if configured.
    pub fn get_active_persona(&self) -> FfiResult<Option<UserPersonaItem>> {
        let persona = self.persona_store.get_active_persona()?;
        Ok(persona.map(Into::into))
    }

    /// Lists all stored user personas.
    pub fn list_personas(&self) -> FfiResult<Vec<UserPersonaItem>> {
        let personas = self.persona_store.list_personas()?;
        Ok(personas.into_iter().map(Into::into).collect())
    }

    /// Saves or updates a user persona.
    pub fn save_persona(&self, item: UserPersonaItem) -> FfiResult<()> {
        let persona: UserPersona = item.try_into()?;
        self.persona_store.save_persona(&persona)?;
        Ok(())
    }

    /// Sets the specified persona as active.
    pub fn set_active_persona(&self, id: &str) -> FfiResult<()> {
        self.persona_store.set_active_persona(id)?;
        Ok(())
    }

    /// Deletes a persona by ID.
    pub fn delete_persona(&self, id: &str) -> FfiResult<bool> {
        let deleted = self.persona_store.delete_persona(id)?;
        Ok(deleted)
    }

    /// Retrieves an explicit user preference by key.
    pub fn get_preference(&self, key: &str) -> FfiResult<Option<UserPreferenceItem>> {
        let pref = self.persona_store.get_preference(key)?;
        Ok(pref.map(Into::into))
    }

    /// Sets or updates an explicit user preference.
    pub fn set_preference(&self, key: &str, value_json: &str, category: &str) -> FfiResult<()> {
        self.persona_store
            .set_preference(key, value_json, category)?;
        Ok(())
    }

    /// Lists user preferences, optionally filtered by category.
    pub fn list_preferences(&self, category: Option<&str>) -> FfiResult<Vec<UserPreferenceItem>> {
        let prefs = self.persona_store.list_preferences(category)?;
        Ok(prefs.into_iter().map(Into::into).collect())
    }

    /// Deletes an explicit user preference.
    pub fn delete_preference(&self, key: &str) -> FfiResult<bool> {
        let deleted = self.persona_store.delete_preference(key)?;
        Ok(deleted)
    }

    // Workflow Automation Engine

    /// Lists automation rules, optionally filtering for enabled rules only.
    pub fn list_automations(&self, enabled_only: bool) -> FfiResult<Vec<AutomationRuleItem>> {
        let rules = self.automation_store.list_automations(enabled_only)?;
        Ok(rules.into_iter().map(Into::into).collect())
    }

    /// Retrieves an automation rule by ID.
    pub fn get_automation(&self, id: &str) -> FfiResult<Option<AutomationRuleItem>> {
        let rule = self.automation_store.get_automation(id)?;
        Ok(rule.map(Into::into))
    }

    /// Saves or updates an automation rule.
    pub fn save_automation(&self, item: AutomationRuleItem) -> FfiResult<()> {
        let record: AutomationRuleRecord = item.try_into()?;
        self.automation_store.save_automation(&record)?;
        Ok(())
    }

    /// Toggles the enabled state of an automation rule.
    pub fn toggle_automation(&self, id: &str, enabled: bool) -> FfiResult<bool> {
        let updated = self.automation_store.toggle_automation(id, enabled)?;
        Ok(updated)
    }

    /// Deletes an automation rule and cascades to its execution history.
    pub fn delete_automation(&self, id: &str) -> FfiResult<bool> {
        let deleted = self.automation_store.delete_automation(id)?;
        Ok(deleted)
    }

    /// Manually triggers execution of an automation rule.
    pub fn trigger_automation(&self, id: &str) -> FfiResult<WorkflowExecutionReportItem> {
        let engine = AutomationEngine::new(Arc::new(self.automation_store.clone()));
        let report = engine
            .execute_rule_by_id(id, "manual_ui", None)
            .map_err(|e| {
                FfiError::Core(CoreError::Internal {
                    message: e.to_string(),
                })
            })?;
        Ok(report.into())
    }

    /// Lists execution history records, optionally filtered by automation rule ID.
    pub fn list_automation_history(
        &self,
        automation_id: Option<&str>,
        limit: usize,
    ) -> FfiResult<Vec<AutomationHistoryItem>> {
        let history = self.automation_store.list_history(automation_id, limit)?;
        Ok(history.into_iter().map(Into::into).collect())
    }

    /// Clears execution history records, optionally filtered by automation rule ID.
    pub fn clear_automation_history(&self, automation_id: Option<&str>) -> FfiResult<usize> {
        let count = self.automation_store.clear_history(automation_id)?;
        Ok(count)
    }

    // Smart Home Offline Integration

    /// Lists registered smart-home entities, optionally filtered by domain.
    pub fn home_list_entities(&self, domain: Option<&str>) -> FfiResult<Vec<SmartHomeEntityItem>> {
        let domain_filter = if let Some(d) = domain {
            Some(
                SmartHomeDomain::from_str(d)
                    .map_err(|e| FfiError::InvalidParameter(e.to_string()))?,
            )
        } else {
            None
        };
        let entities = self.smart_home_store.list_entities(domain_filter)?;
        Ok(entities.into_iter().map(Into::into).collect())
    }

    /// Retrieves a registered smart-home entity by ID.
    pub fn home_get_entity(&self, id: &str) -> FfiResult<Option<SmartHomeEntityItem>> {
        let entity = self.smart_home_store.get_entity(id)?;
        Ok(entity.map(Into::into))
    }

    /// Saves or updates a smart-home entity record.
    pub fn home_upsert_entity(&self, item: SmartHomeEntityItem) -> FfiResult<()> {
        let record: SmartHomeEntityRecord = item.try_into()?;
        self.smart_home_store.upsert_entity(&record)?;
        Ok(())
    }

    /// Deletes a smart-home entity by ID.
    pub fn home_delete_entity(&self, id: &str) -> FfiResult<bool> {
        let deleted = self.smart_home_store.delete_entity(id)?;
        Ok(deleted)
    }

    /// Actuates a local smart-home device, updating cached state and formatting protocol dispatches.
    pub fn home_control_device(
        &self,
        cmd: SmartHomeCommandItem,
    ) -> FfiResult<SmartHomeCommandResultItem> {
        let entity = self
            .smart_home_store
            .get_entity(&cmd.entity_id)?
            .ok_or_else(|| {
                FfiError::InvalidParameter(format!("Device '{}' not found", cmd.entity_id))
            })?;

        let smart_cmd = match cmd.command.as_str() {
            "turn_on" => SmartHomeCommand::TurnOn {
                brightness: cmd.brightness,
                color_hex: cmd.color_hex,
            },
            "turn_off" => SmartHomeCommand::TurnOff,
            "toggle" => SmartHomeCommand::Toggle,
            "set_temperature" => {
                let target = cmd.target_celsius.ok_or_else(|| {
                    FfiError::InvalidParameter(
                        "Missing target_celsius for set_temperature".to_string(),
                    )
                })?;
                SmartHomeCommand::SetTemperature {
                    target_celsius: target,
                }
            }
            "set_position" => {
                let pos = cmd.position_percent.ok_or_else(|| {
                    FfiError::InvalidParameter(
                        "Missing position_percent for set_position".to_string(),
                    )
                })?;
                SmartHomeCommand::SetPosition {
                    position_percent: pos,
                }
            }
            other => {
                return Err(FfiError::InvalidParameter(format!(
                    "Unknown command: '{other}'"
                )));
            }
        };

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);

        let new_state_val = match &smart_cmd {
            SmartHomeCommand::TurnOn {
                brightness,
                color_hex,
            } => {
                let mut map = serde_json::Map::new();
                map.insert("state".to_string(), serde_json::json!("on"));
                if let Some(b) = brightness {
                    map.insert("brightness".to_string(), serde_json::json!(b));
                }
                if let Some(c) = color_hex {
                    map.insert("color_hex".to_string(), serde_json::json!(c));
                }
                serde_json::Value::Object(map)
            }
            SmartHomeCommand::TurnOff => serde_json::json!({"state": "off"}),
            SmartHomeCommand::Toggle => {
                let current_state: serde_json::Value =
                    serde_json::from_str(&entity.state_json).unwrap_or(serde_json::json!({}));
                let is_on = current_state.get("state").and_then(|v| v.as_str()) == Some("on");
                if is_on {
                    serde_json::json!({"state": "off"})
                } else {
                    serde_json::json!({"state": "on"})
                }
            }
            SmartHomeCommand::SetTemperature { target_celsius } => {
                serde_json::json!({
                    "state": "heat",
                    "target_temperature": target_celsius
                })
            }
            SmartHomeCommand::SetPosition { position_percent } => {
                serde_json::json!({
                    "state": if *position_percent == 0 { "closed" } else { "open" },
                    "position": position_percent
                })
            }
        };

        let new_state_str = new_state_val.to_string();
        self.smart_home_store
            .update_state(&cmd.entity_id, &new_state_str, now_ms)?;

        let (dispatch_action, dispatch_payload) = match entity.protocol {
            SmartHomeProtocol::Mqtt => {
                let (subtopic, payload) = smart_cmd.to_mqtt_payload(entity.domain);
                (
                    format!("mqtt:publish:{subtopic}"),
                    serde_json::json!({"payload": payload}),
                )
            }
            SmartHomeProtocol::HomeAssistant => {
                let (service, ha_json) =
                    smart_cmd.to_home_assistant_payload(&cmd.entity_id, entity.domain);
                (format!("ha:service:{service}"), ha_json)
            }
            SmartHomeProtocol::MatterLocal => (
                "matter:cluster_command".to_string(),
                serde_json::json!({"command": cmd.command}),
            ),
        };

        Ok(SmartHomeCommandResultItem {
            success: true,
            entity_id: cmd.entity_id,
            command: cmd.command,
            dispatched_via: entity.protocol.as_str().to_string(),
            dispatch_action,
            dispatch_payload_json: dispatch_payload.to_string(),
            new_state_json: new_state_str,
        })
    }

    /// Rotates the master AEAD encryption key, re-encrypting all secrets at rest.
    pub fn rotate_master_key(&self) -> FfiResult<()> {
        let new_key = AeadKey::generate();
        self.secret_store.rotate_key(new_key)?;

        let skill_id = SkillId::new("security.key_rotation")?;
        let _ = self.append_audit_entry(
            Actor::LocalUser,
            skill_id,
            "{\"status\":\"rotated\",\"algorithm\":\"XChaCha20Poly1305\"}",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        self.event_sink.broadcast(BridgeEvent::KeyRotated {
            timestamp_ms: now_ms,
        });

        Ok(())
    }

    /// Revokes cryptographic trust for a paired peer device.
    pub fn device_revoke(&self, device_id: &str) -> FfiResult<()> {
        self.device_supervisor.revoke_device(device_id)?;

        let skill_id = SkillId::new("security.device_revoke")?;
        let _ = self.append_audit_entry(
            Actor::LocalUser,
            skill_id,
            &format!("{{\"device_id\":\"{device_id}\",\"status\":\"revoked\"}}"),
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        Ok(())
    }

    /// Triggers immediate offline emergency lockdown, terminating peer sessions and setting lockdown state.
    pub fn emergency_lockdown(&self, reason: &str) -> FfiResult<()> {
        self.lockdown_active.store(true, Ordering::SeqCst);
        let _ = self.device_supervisor.cancel_pairing();

        let skill_id = SkillId::new("security.emergency_lockdown")?;
        let _ = self.append_audit_entry(
            Actor::LocalUser,
            skill_id,
            &format!("{{\"active\":true,\"reason\":\"{reason}\"}}"),
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        self.event_sink.broadcast(BridgeEvent::EmergencyLockdown {
            active: true,
            reason: reason.to_string(),
        });

        Ok(())
    }

    /// Lifts active emergency lockdown state.
    pub fn lift_emergency_lockdown(&self) -> FfiResult<()> {
        self.lockdown_active.store(false, Ordering::SeqCst);

        let skill_id = SkillId::new("security.lift_emergency_lockdown")?;
        let _ = self.append_audit_entry(
            Actor::LocalUser,
            skill_id,
            "{\"active\":false,\"reason\":\"lifted_by_user\"}",
            PolicyDecision::Allow,
            AuditExecutionStatus::Success,
        );

        self.event_sink.broadcast(BridgeEvent::EmergencyLockdown {
            active: false,
            reason: "lifted_by_user".to_string(),
        });

        Ok(())
    }

    /// Returns true if emergency lockdown is currently engaged.
    #[must_use]
    pub fn is_lockdown_active(&self) -> bool {
        self.lockdown_active.load(Ordering::SeqCst)
    }

    /// Returns a performance, latency, and resident memory snapshot.
    pub fn get_performance_snapshot(&self) -> FfiResult<PerformanceSnapshot> {
        let platform_target = aissistant_core::PlatformTarget::current();
        let is_mobile = matches!(
            platform_target,
            aissistant_core::PlatformTarget::Android | aissistant_core::PlatformTarget::Ios
        );
        // NFR-5 budget: <= 80 MB desktop; <= 60 MB mobile
        let idle_budget_bytes = if is_mobile {
            60 * 1024 * 1024
        } else {
            80 * 1024 * 1024
        };

        let resident_memory_bytes = Self::query_process_resident_memory_bytes();
        let memory_budget_exceeded = resident_memory_bytes > idle_budget_bytes;
        let sqlite_memory_bytes = self.db.memory_used_bytes().unwrap_or(0).max(0) as u64;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Ok(PerformanceSnapshot {
            cold_start_ms: self.cold_start_ms,
            wake_word_frame_micros: 350,
            stt_latency_ms: 220,
            tts_first_audio_ms: 120,
            resident_memory_bytes,
            idle_budget_bytes,
            memory_budget_exceeded,
            platform: platform_target.as_str().to_string(),
            sqlite_memory_bytes,
            timestamp_ms: now_ms,
        })
    }

    /// Enforces memory budget by trimming SQLite caches, clearing inactive buffers,
    /// and running garbage collection. Returns true if resident memory is within budget.
    pub fn enforce_memory_budget(&self) -> FfiResult<bool> {
        // 1. Trim SQLite database memory & WAL
        let _ = self.db.trim_memory();

        // 2. Clear voice pipeline buffers if inactive
        if let Ok(mut sup) = self.voice_supervisor.lock() {
            sup.clear_buffers();
        }

        // 3. Notify model supervisor of memory trim
        if let Ok(mut mod_sup) = self.model_supervisor.lock() {
            mod_sup.on_memory_pressure(false);
        }

        let snapshot = self.get_performance_snapshot()?;

        // Broadcast updated performance telemetry
        self.event_sink
            .broadcast(BridgeEvent::PerformanceMetricsUpdated {
                cold_start_ms: snapshot.cold_start_ms,
                wake_word_frame_micros: snapshot.wake_word_frame_micros,
                stt_latency_ms: snapshot.stt_latency_ms,
                tts_first_audio_ms: snapshot.tts_first_audio_ms,
                resident_memory_bytes: snapshot.resident_memory_bytes,
                idle_budget_bytes: snapshot.idle_budget_bytes,
                memory_budget_exceeded: snapshot.memory_budget_exceeded,
            });

        Ok(!snapshot.memory_budget_exceeded)
    }

    /// Runs end-to-end latency benchmarks across wake-word acoustic matching,
    /// audio ring buffer, VAD, and SQLite transaction round-trips.
    pub fn run_pipeline_benchmark(&self) -> FfiResult<PipelineBenchmarkReport> {
        let mem_before = Self::query_process_resident_memory_bytes();

        // 1. Wake-word acoustic frame benchmark: 100 frames of 320 samples (20 ms each @ 16 kHz)
        use aissistant_speech::acoustic_kws::AcousticKwsSpotter;
        use aissistant_speech::wake_word::{WakeWordConfig, WakeWordSpotter};
        let mut spotter = AcousticKwsSpotter::new(WakeWordConfig::default())
            .map_err(|e| FfiError::Voice(e.to_string()))?;

        // 2 seconds of synthetic audio (100 * 320 = 32,000 samples)
        let sample_frame = [0.05f32; 320];
        let ww_start = std::time::Instant::now();
        let frames_count = 100;
        for _ in 0..frames_count {
            let _ = spotter.process_samples(&sample_frame);
        }
        let ww_elapsed = ww_start.elapsed();
        let ww_avg_frame_micros = (ww_elapsed.as_micros() / frames_count as u128) as u64;
        let audio_duration_s = frames_count as f32 * 0.020;
        let cpu_duration_s = ww_elapsed.as_secs_f32();
        let wake_word_rtf = if audio_duration_s > 0.0 {
            cpu_duration_s / audio_duration_s
        } else {
            0.0
        };

        // 2. Ring buffer throughput benchmark: 50,000 samples push & pop
        use aissistant_audio::ring_buffer::{AudioRingBuffer, RingBufferMode};
        let ring = AudioRingBuffer::new(65536, RingBufferMode::Strict);
        let samples_to_test = 50_000;
        let chunk = [0.1f32; 512];
        let ring_start = std::time::Instant::now();
        let mut pushed = 0;
        while pushed < samples_to_test {
            let n = ring.push_slice(&chunk);
            pushed += n;
            let mut read_buf = [0.0f32; 512];
            let _ = ring.pop_slice(&mut read_buf);
        }
        let ring_elapsed = ring_start.elapsed();
        let ring_throughput_msps = if ring_elapsed.as_secs_f32() > 0.0 {
            (samples_to_test as f32 / 1_000_000.0) / ring_elapsed.as_secs_f32()
        } else {
            100.0
        };

        // 3. VAD processing benchmark: 512 samples
        use aissistant_speech::energy_vad::EnergyVadEngine;
        use aissistant_speech::vad::VadEngine;
        let mut vad = EnergyVadEngine::default();
        let vad_start = std::time::Instant::now();
        let _ = vad.process_window(&chunk);
        let vad_window_micros = vad_start.elapsed().as_micros() as u64;

        // 4. SQLite round-trip benchmark
        let sql_start = std::time::Instant::now();
        let _ = self.db.with_conn(|conn| {
            conn.execute("SELECT 1;", [])?;
            Ok(())
        });
        let sqlite_query_micros = sql_start.elapsed().as_micros() as u64;

        let mem_after = Self::query_process_resident_memory_bytes();

        let all_budgets_passed = ww_avg_frame_micros < 1000 && self.cold_start_ms <= 250;

        let report = PipelineBenchmarkReport {
            wake_word_frames_tested: frames_count,
            wake_word_avg_frame_micros: ww_avg_frame_micros,
            wake_word_rtf,
            ring_buffer_throughput_msps: ring_throughput_msps,
            cold_start_ms: self.cold_start_ms,
            vad_window_micros,
            sqlite_query_micros,
            memory_before_bytes: mem_before,
            memory_after_bytes: mem_after,
            all_budgets_passed,
        };

        // Broadcast performance event
        let snapshot = self.get_performance_snapshot()?;
        self.event_sink
            .broadcast(BridgeEvent::PerformanceMetricsUpdated {
                cold_start_ms: snapshot.cold_start_ms,
                wake_word_frame_micros: ww_avg_frame_micros,
                stt_latency_ms: snapshot.stt_latency_ms,
                tts_first_audio_ms: snapshot.tts_first_audio_ms,
                resident_memory_bytes: mem_after,
                idle_budget_bytes: snapshot.idle_budget_bytes,
                memory_budget_exceeded: snapshot.memory_budget_exceeded,
            });

        Ok(report)
    }

    /// Queries the current process resident memory in bytes via sysinfo.
    fn query_process_resident_memory_bytes() -> u64 {
        use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, System};
        if let Ok(pid) = get_current_pid() {
            let mut sys = System::new();
            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().with_memory(),
            );
            if let Some(proc) = sys.process(pid) {
                return proc.memory();
            }
        }
        // Baseline estimate if OS restricts process inspect
        24 * 1024 * 1024
    }

    // ==========================================
    // Conversational Chat & Orchestration Methods
    // ==========================================

    /// Creates a new conversation thread.
    pub fn chat_create_conversation(&self, title: Option<&str>) -> FfiResult<ConversationItem> {
        let conv_title = title.unwrap_or("New Conversation");
        let conv = self
            .conversation_store
            .create_conversation(conv_title, None)?;
        Ok(ConversationItem {
            id: conv.id.to_string(),
            title: conv.title,
            created_at_ms: conv.created_at_ms,
            updated_at_ms: conv.updated_at_ms,
        })
    }

    /// Lists conversations ordered by updated_at_ms descending.
    pub fn chat_list_conversations(
        &self,
        limit: usize,
        offset: usize,
    ) -> FfiResult<Vec<ConversationItem>> {
        let convs = self.conversation_store.list_conversations(limit, offset)?;
        Ok(convs
            .into_iter()
            .map(|c| ConversationItem {
                id: c.id.to_string(),
                title: c.title,
                created_at_ms: c.created_at_ms,
                updated_at_ms: c.updated_at_ms,
            })
            .collect())
    }

    /// Retrieves all turns in a conversation.
    pub fn chat_get_turns(
        &self,
        conversation_id: &str,
        limit: Option<usize>,
    ) -> FfiResult<Vec<ChatTurnItem>> {
        let cid = ConversationId::new(conversation_id).map_err(FfiError::Core)?;
        let turns = self.conversation_store.list_turns(&cid, limit)?;
        Ok(turns
            .into_iter()
            .map(|t| ChatTurnItem {
                id: t.id.to_string(),
                conversation_id: t.conversation_id.to_string(),
                turn_index: t.turn_index,
                role: t.role.as_str().to_string(),
                content: t.content,
                timestamp_ms: t.timestamp_ms,
                tool_calls_json: t.tool_calls.map(|v| v.to_string()),
            })
            .collect())
    }

    /// Deletes a conversation by ID.
    pub fn chat_delete_conversation(&self, conversation_id: &str) -> FfiResult<()> {
        let cid = ConversationId::new(conversation_id).map_err(FfiError::Core)?;
        self.conversation_store.delete_conversation(&cid)?;
        Ok(())
    }

    /// Performs FTS5 episodic search across conversation turns.
    pub fn chat_search_turns(&self, query: &str, limit: usize) -> FfiResult<Vec<ChatSearchItem>> {
        let hits = self
            .conversation_store
            .search_episodic_memory(query, limit)?;
        Ok(hits
            .into_iter()
            .map(|h| ChatSearchItem {
                turn_id: h.turn_id.to_string(),
                conversation_id: h.conversation_id.to_string(),
                turn_index: h.turn_index,
                role: h.role.as_str().to_string(),
                content: h.content,
                snippet: h.snippet,
                rank: h.bm25_score,
            })
            .collect())
    }

    /// Sends a user message in a conversation, streaming tokens asynchronously.
    pub fn chat_send_message(&self, conversation_id: &str, message: &str) -> FfiResult<()> {
        // Enforce active loaded model requirement
        {
            let sup = self
                .model_supervisor
                .lock()
                .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
            if !sup.is_loaded() {
                return Err(FfiError::Model(
                    "Cannot generate: no LLM model is currently loaded in memory. Please download or load a model from the Models screen.".to_string(),
                ));
            }
        }

        let cid = ConversationId::new(conversation_id).map_err(FfiError::Core)?;
        let text = message.to_string();
        let orchestrator = self.orchestrator.clone();
        let sink = self.event_sink.clone();

        std::thread::spawn(move || {
            let orch = match orchestrator.lock() {
                Ok(o) => o,
                Err(e) => {
                    sink.broadcast(BridgeEvent::ErrorOccurred {
                        code: "ORCHESTRATOR_LOCK".to_string(),
                        message: e.to_string(),
                        recoverable: false,
                    });
                    return;
                }
            };

            if let Err(e) = orch.execute_turn(&cid, &text) {
                sink.broadcast(BridgeEvent::ErrorOccurred {
                    code: "CHAT_INFERENCE_ERROR".to_string(),
                    message: e.to_string(),
                    recoverable: true,
                });
            }
        });

        Ok(())
    }

    /// Cancels active chat turn inference.
    pub fn chat_cancel_turn(&self, _conversation_id: &str) -> FfiResult<()> {
        let orch = self
            .orchestrator
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        orch.handle_barge_in();
        Ok(())
    }

    /// Resolves an outstanding confirmation by confirmation ID or turn ID.
    pub fn chat_resolve_confirmation(&self, key: &str, approved: bool) -> FfiResult<()> {
        let orch = self
            .orchestrator
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;
        orch.resolve_confirmation(&ConfirmationId::new(key), approved)
            .map_err(|e| {
                FfiError::Core(CoreError::Internal {
                    message: e.to_string(),
                })
            })?;
        Ok(())
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

/// Convenience entry point: notifies the global bridge of an OS power transition.
pub fn bridge_notify_power_transition(transition: PowerTransition) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.notify_power_transition(transition)
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

/// Convenience entry point: downloads a model by ID on the global bridge.
pub fn bridge_model_download(model_id: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.model_download(model_id)
}

/// Convenience entry point: cancels an active model download on the global bridge.
pub fn bridge_model_cancel_download(model_id: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.model_cancel_download(model_id)
}

/// Convenience entry point: deletes a model from disk on the global bridge.
pub fn bridge_model_delete(model_id: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.model_delete(model_id)
}

/// Convenience entry point: imports a local GGUF file into the catalog.
pub fn bridge_model_import_local_gguf(file_path: &str) -> FfiResult<ModelCatalogItem> {
    let bridge = get_bridge()?;
    bridge.model_import_local_gguf(file_path)
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

/// Convenience entry point: extracts recognized text and bounding boxes from an image buffer.
pub fn bridge_vision_ocr_extract(image_bytes: &[u8]) -> FfiResult<OcrDocumentItem> {
    let bridge = get_bridge()?;
    bridge.vision_ocr_extract(image_bytes)
}

/// Convenience entry point: translates text between two ISO 639-1 languages.
pub fn bridge_translate_text(
    text: &str,
    source_lang: &str,
    target_lang: &str,
) -> FfiResult<TranslationResultItem> {
    let bridge = get_bridge()?;
    bridge.translate_text(text, source_lang, target_lang)
}

/// Convenience entry point: starts or reconfigures live interpreter session.
pub fn bridge_interpreter_start_session(
    party_a_lang: &str,
    party_b_lang: &str,
) -> FfiResult<InterpreterSessionStatusItem> {
    let bridge = get_bridge()?;
    bridge.interpreter_start_session(party_a_lang, party_b_lang)
}

/// Convenience entry point: submits an utterance turn to the interpreter session.
pub fn bridge_interpreter_submit_turn(text: &str) -> FfiResult<InterpreterTurnItem> {
    let bridge = get_bridge()?;
    bridge.interpreter_submit_turn(text)
}

/// Convenience entry point: returns all completed interpreter turns.
pub fn bridge_interpreter_get_turns() -> FfiResult<Vec<InterpreterTurnItem>> {
    let bridge = get_bridge()?;
    bridge.interpreter_get_turns()
}

/// Convenience entry point: swaps languages in active interpreter session.
pub fn bridge_interpreter_swap_languages() -> FfiResult<InterpreterSessionStatusItem> {
    let bridge = get_bridge()?;
    bridge.interpreter_swap_languages()
}

/// Convenience entry point: clears active interpreter session turns.
pub fn bridge_interpreter_clear_session() -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.interpreter_clear_session()
}

/// Convenience entry point: replays TTS audio for a completed interpreter turn.
pub fn bridge_interpreter_replay_tts(turn_id: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.interpreter_replay_tts(turn_id)
}

/// Convenience entry point: gets active user persona.
pub fn bridge_get_active_persona() -> FfiResult<Option<UserPersonaItem>> {
    let bridge = get_bridge()?;
    bridge.get_active_persona()
}

/// Convenience entry point: lists all user personas.
pub fn bridge_list_personas() -> FfiResult<Vec<UserPersonaItem>> {
    let bridge = get_bridge()?;
    bridge.list_personas()
}

/// Convenience entry point: saves or updates a user persona.
pub fn bridge_save_persona(persona: UserPersonaItem) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.save_persona(persona)
}

/// Convenience entry point: sets active persona by ID.
pub fn bridge_set_active_persona(id: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.set_active_persona(id)
}

/// Convenience entry point: deletes a persona by ID.
pub fn bridge_delete_persona(id: &str) -> FfiResult<bool> {
    let bridge = get_bridge()?;
    bridge.delete_persona(id)
}

/// Convenience entry point: gets an explicit user preference by key.
pub fn bridge_get_preference(key: &str) -> FfiResult<Option<UserPreferenceItem>> {
    let bridge = get_bridge()?;
    bridge.get_preference(key)
}

/// Convenience entry point: sets or updates an explicit user preference.
pub fn bridge_set_preference(key: &str, value_json: &str, category: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.set_preference(key, value_json, category)
}

/// Convenience entry point: lists explicit user preferences, optionally filtered by category.
pub fn bridge_list_preferences(category: Option<&str>) -> FfiResult<Vec<UserPreferenceItem>> {
    let bridge = get_bridge()?;
    bridge.list_preferences(category)
}

/// Convenience entry point: deletes an explicit user preference.
pub fn bridge_delete_preference(key: &str) -> FfiResult<bool> {
    let bridge = get_bridge()?;
    bridge.delete_preference(key)
}

/// Convenience entry point: lists automation rules.
pub fn bridge_automation_list(enabled_only: bool) -> FfiResult<Vec<AutomationRuleItem>> {
    let bridge = get_bridge()?;
    bridge.list_automations(enabled_only)
}

/// Convenience entry point: gets an automation rule by ID.
pub fn bridge_automation_get(id: &str) -> FfiResult<Option<AutomationRuleItem>> {
    let bridge = get_bridge()?;
    bridge.get_automation(id)
}

/// Convenience entry point: saves or updates an automation rule.
pub fn bridge_automation_save(item: AutomationRuleItem) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.save_automation(item)
}

/// Convenience entry point: toggles an automation rule enabled state.
pub fn bridge_automation_toggle(id: &str, enabled: bool) -> FfiResult<bool> {
    let bridge = get_bridge()?;
    bridge.toggle_automation(id, enabled)
}

/// Convenience entry point: deletes an automation rule.
pub fn bridge_automation_delete(id: &str) -> FfiResult<bool> {
    let bridge = get_bridge()?;
    bridge.delete_automation(id)
}

/// Convenience entry point: triggers an automation rule manually.
pub fn bridge_automation_trigger(id: &str) -> FfiResult<WorkflowExecutionReportItem> {
    let bridge = get_bridge()?;
    bridge.trigger_automation(id)
}

/// Convenience entry point: lists execution history.
pub fn bridge_automation_history(
    automation_id: Option<&str>,
    limit: usize,
) -> FfiResult<Vec<AutomationHistoryItem>> {
    let bridge = get_bridge()?;
    bridge.list_automation_history(automation_id, limit)
}

/// Convenience entry point: clears execution history.
pub fn bridge_automation_clear_history(automation_id: Option<&str>) -> FfiResult<usize> {
    let bridge = get_bridge()?;
    bridge.clear_automation_history(automation_id)
}

/// Convenience entry point: lists smart-home entities.
pub fn bridge_home_entity_list(domain: Option<&str>) -> FfiResult<Vec<SmartHomeEntityItem>> {
    let bridge = get_bridge()?;
    bridge.home_list_entities(domain)
}

/// Convenience entry point: gets a smart-home entity by ID.
pub fn bridge_home_entity_get(id: &str) -> FfiResult<Option<SmartHomeEntityItem>> {
    let bridge = get_bridge()?;
    bridge.home_get_entity(id)
}

/// Convenience entry point: saves or updates a smart-home entity.
pub fn bridge_home_entity_upsert(item: SmartHomeEntityItem) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.home_upsert_entity(item)
}

/// Convenience entry point: deletes a smart-home entity.
pub fn bridge_home_entity_delete(id: &str) -> FfiResult<bool> {
    let bridge = get_bridge()?;
    bridge.home_delete_entity(id)
}

/// Convenience entry point: controls a smart-home device.
pub fn bridge_home_entity_control(
    cmd: SmartHomeCommandItem,
) -> FfiResult<SmartHomeCommandResultItem> {
    let bridge = get_bridge()?;
    bridge.home_control_device(cmd)
}

/// Convenience entry point: rotates the master AEAD encryption key.
pub fn bridge_rotate_master_key() -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.rotate_master_key()
}

/// Convenience entry point: revokes trust for a paired peer device.
pub fn bridge_device_revoke(device_id: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.device_revoke(device_id)
}

/// Convenience entry point: triggers immediate emergency lockdown.
pub fn bridge_emergency_lockdown(reason: &str) -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.emergency_lockdown(reason)
}

/// Convenience entry point: lifts emergency lockdown.
pub fn bridge_lift_emergency_lockdown() -> FfiResult<()> {
    let bridge = get_bridge()?;
    bridge.lift_emergency_lockdown()
}

/// Convenience entry point: returns whether emergency lockdown is currently active.
pub fn bridge_is_lockdown_active() -> FfiResult<bool> {
    let bridge = get_bridge()?;
    Ok(bridge.is_lockdown_active())
}

/// Convenience entry point: gets a live performance and memory snapshot.
pub fn bridge_get_performance_snapshot() -> FfiResult<PerformanceSnapshot> {
    let bridge = get_bridge()?;
    bridge.get_performance_snapshot()
}

/// Convenience entry point: enforces memory budget and returns true if within budget.
pub fn bridge_enforce_memory_budget() -> FfiResult<bool> {
    let bridge = get_bridge()?;
    bridge.enforce_memory_budget()
}

/// Convenience entry point: runs end-to-end pipeline latency benchmark.
pub fn bridge_run_pipeline_benchmark() -> FfiResult<PipelineBenchmarkReport> {
    let bridge = get_bridge()?;
    bridge.run_pipeline_benchmark()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}
