//! Asynchronous event broadcasting and reactive stream sink across the FFI boundary.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

/// Events emitted by the core engine across the FFI bridge to Flutter or native runners.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum BridgeEvent {
    /// System health status updated (e.g. database error, storage restored).
    HealthStatusChanged {
        /// Health state indicator ("healthy", "degraded", "critical").
        status: String,
        /// Diagnostic detail messages.
        details: Vec<String>,
    },

    /// Runtime capability state or permission grant changed.
    CapabilityChanged {
        /// Canonical capability identifier (e.g. "local_llm", "offline_speech").
        capability: String,
        /// Current availability status ("supported", "unsupported", "requires_permission", "degraded").
        status: String,
        /// Optional human-readable reason or required permission name.
        reason: Option<String>,
    },

    /// New security audit entry committed to the hash-chained log.
    AuditLogAppended {
        /// Monotonically increasing sequence number.
        sequence: u64,
        /// Actor that initiated the action.
        actor: String,
        /// Skill or subsystem invoked.
        skill_id: String,
        /// Policy decision applied ("allow", "require_confirmation", "deny").
        decision: String,
        /// Execution status outcome.
        execution_status: String,
        /// BLAKE3 entry hash in lowercase hexadecimal.
        entry_hash: String,
    },

    /// Persistent setting was modified or removed.
    SettingChanged {
        /// Setting key identifier.
        key: String,
        /// Sanitized preview of the updated value, if available.
        value_preview: Option<String>,
    },

    /// Structured error encountered by a background task.
    ErrorOccurred {
        /// Stable error code string.
        code: String,
        /// Human-readable description.
        message: String,
        /// Whether the operation is safe to retry.
        recoverable: bool,
    },

    /// Runtime lifecycle state changed (e.g. running -> shutting_down).
    LifecycleStateChanged {
        /// Previous lifecycle state string.
        previous: String,
        /// New lifecycle state string.
        current: String,
        /// Optional reason for transition.
        reason: Option<String>,
    },

    /// Host OS memory pressure warning received.
    MemoryPressure {
        /// Severity level ("moderate", "critical").
        level: String,
    },

    /// Voice audio level meter update (RMS & peak in dBFS).
    VoiceLevelUpdated {
        /// RMS energy in dBFS.
        rms_dbfs: f32,
        /// Peak amplitude in dBFS.
        peak_dbfs: f32,
    },

    /// Wake-word triggered in streaming audio.
    WakeWordTriggered {
        /// Spotted keyword (e.g. "hey assistant").
        keyword: String,
        /// Confidence score [0.0, 1.0].
        confidence: f32,
    },

    /// Speech-to-text transcript update.
    TranscriptUpdated {
        /// Transcript text.
        text: String,
        /// Whether the transcript is finalized (endpointed) or interim partial.
        is_final: bool,
        /// Language tag ("en", "es", etc.).
        language: String,
        /// Confidence score [0.0, 1.0].
        confidence: f32,
    },

    /// Voice listening state or mode changed.
    VoiceStateChanged {
        /// Whether microphone is actively listening.
        is_listening: bool,
        /// Active listening mode ("push_to_talk", "wake_word", "idle").
        mode: String,
    },

    /// Local LLM model loaded into memory.
    ModelLoaded {
        /// Loaded model identifier.
        model_id: String,
    },

    /// Local LLM model unloaded from memory.
    ModelUnloaded {
        /// Unloaded model identifier.
        model_id: String,
    },

    /// Local LLM model download progress update.
    ModelDownloadProgress {
        /// Downloading model identifier.
        model_id: String,
        /// Cumulative bytes downloaded so far.
        downloaded_bytes: u64,
        /// Total file size in bytes.
        total_bytes: u64,
        /// Instantaneous download speed in bytes per second.
        speed_bps: u64,
    },

    /// Local LLM model download finished and verified.
    ModelDownloadCompleted {
        /// Downloaded model identifier.
        model_id: String,
        /// Path on local disk.
        local_path: String,
        /// Whether SHA-256 matched the expected hash.
        sha256_verified: bool,
    },

    /// Local LLM model download failed or was cancelled.
    ModelDownloadFailed {
        /// Target model identifier.
        model_id: String,
        /// Human-readable error or cancellation reason.
        error: String,
    },

    /// On-device LLM benchmark finished.
    BenchmarkCompleted {
        /// Serialized benchmark metrics.
        metrics_json: String,
    },

    /// Discovered peer updated on local network.
    DeviceDiscovered {
        /// Peer instance name.
        instance_name: String,
        /// Platform.
        platform: String,
        /// Endpoint address.
        endpoint: Option<String>,
        /// Public key fingerprint.
        fingerprint: String,
    },

    /// Active pairing ceremony state updated.
    PairingStateChanged {
        /// Target peer instance name.
        peer_instance: String,
        /// Status: "started", "confirmed", "failed", "cancelled".
        status: String,
        /// Short Authentication String (if active).
        sas_code: Option<String>,
        /// QR payload string (if active).
        qr_payload: Option<String>,
        /// Error message if failed.
        error: Option<String>,
    },

    /// Master AEAD key rotated across the encrypted secret store.
    KeyRotated {
        /// Epoch timestamp in milliseconds.
        timestamp_ms: u64,
    },

    /// Cryptographic trust revoked for a paired peer device.
    DeviceRevoked {
        /// Device identifier of the revoked peer.
        device_id: String,
    },

    /// Emergency lockdown triggered or lifted.
    EmergencyLockdown {
        /// Whether lockdown is currently engaged.
        active: bool,
        /// Reason or trigger description.
        reason: String,
    },

    /// Performance metrics snapshot updated or benchmark completed.
    PerformanceMetricsUpdated {
        /// Cold start latency in milliseconds.
        cold_start_ms: u64,
        /// Wake-word processing time in microseconds.
        wake_word_frame_micros: u64,
        /// STT latency in milliseconds.
        stt_latency_ms: u64,
        /// TTS time to first audio in milliseconds.
        tts_first_audio_ms: u64,
        /// Resident memory in bytes.
        resident_memory_bytes: u64,
        /// Target idle budget in bytes.
        idle_budget_bytes: u64,
        /// Whether budget is exceeded.
        memory_budget_exceeded: bool,
    },

    /// Host OS power state transition (sleep/wake/suspend/resume).
    PowerTransition {
        /// The event name ("suspend" or "resume").
        event: String,
    },

    /// A single token streamed from the active LLM session.
    ChatTokenDelta {
        /// Turn identifier.
        turn_id: String,
        /// Incremental token content.
        token: String,
    },

    /// Chat turn completed and persisted.
    ChatTurnComplete {
        /// Turn identifier.
        turn_id: String,
        /// Citations or memory excerpt snippets used in the turn.
        citations: Vec<String>,
    },

    /// Security policy requires explicit user confirmation before executing a tool.
    ToolConfirmationRequired {
        /// Turn identifier.
        turn_id: String,
        /// Tool or skill name.
        tool_name: String,
        /// Risk classification tier.
        risk_tier: String,
        /// Formatted arguments JSON.
        params_json: String,
        /// Whether turn input was tainted by prompt injection.
        taint_status: bool,
    },

    /// Tool confirmation request resolved.
    ToolConfirmationResolved {
        /// Confirmation identifier.
        confirmation_id: String,
        /// Whether execution was approved by user.
        approved: bool,
    },

    /// Offline speech models download/readiness progress update.
    SpeechModelProgress {
        /// Current state ("downloading", "ready", "failed").
        status: String,
        /// Downloaded bytes.
        downloaded_bytes: u64,
        /// Total bytes.
        total_bytes: u64,
        /// Percentage (0 to 100).
        percent: u32,
    },
}

type CallbackFn = Box<dyn Fn(&BridgeEvent) + Send + Sync + 'static>;

/// Thread-safe event broadcaster supporting concurrent channel receivers and callbacks.
#[derive(Default)]
pub struct EventStreamSink {
    next_id: AtomicU64,
    channels: Mutex<HashMap<u64, SyncSender<BridgeEvent>>>,
    callbacks: Mutex<HashMap<u64, Arc<CallbackFn>>>,
}

impl std::fmt::Debug for EventStreamSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventStreamSink")
            .field("next_id", &self.next_id.load(Ordering::Relaxed))
            .finish()
    }
}

impl EventStreamSink {
    /// Creates a new empty event stream sink.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            channels: Mutex::new(HashMap::new()),
            callbacks: Mutex::new(HashMap::new()),
        }
    }

    /// Subscribes to the event stream using a bounded FIFO channel.
    /// Returns the subscription ID and the corresponding `Receiver`.
    #[must_use]
    pub fn subscribe(&self, buffer_size: usize) -> (u64, Receiver<BridgeEvent>) {
        let capacity = buffer_size.clamp(16, 2048);
        let (tx, rx) = sync_channel(capacity);
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);

        if let Ok(mut channels) = self.channels.lock() {
            channels.insert(id, tx);
        }

        (id, rx)
    }

    /// Cancels a channel subscription by ID.
    pub fn unsubscribe(&self, subscription_id: u64) {
        if let Ok(mut channels) = self.channels.lock() {
            channels.remove(&subscription_id);
        }
    }

    /// Registers a listener callback invoked synchronously on every broadcast.
    /// Returns a listener ID for unregistration.
    pub fn add_callback<F>(&self, callback: F) -> u64
    where
        F: Fn(&BridgeEvent) + Send + Sync + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let boxed: CallbackFn = Box::new(callback);

        if let Ok(mut callbacks) = self.callbacks.lock() {
            callbacks.insert(id, Arc::new(boxed));
        }

        id
    }

    /// Unregisters a listener callback by ID.
    pub fn remove_callback(&self, callback_id: u64) {
        if let Ok(mut callbacks) = self.callbacks.lock() {
            callbacks.remove(&callback_id);
        }
    }

    /// Broadcasts an event to all active channel subscribers and registered callbacks.
    /// Automatically purges disconnected channels.
    pub fn broadcast(&self, event: BridgeEvent) {
        // 1. Dispatch to channel subscribers (purging disconnected ones)
        if let Ok(mut channels) = self.channels.lock() {
            let mut dead_ids = Vec::new();
            for (&id, tx) in channels.iter() {
                match tx.try_send(event.clone()) {
                    Ok(()) | Err(TrySendError::Full(_)) => {
                        // Channel buffer accepted or full; keep subscription alive
                    }
                    Err(TrySendError::Disconnected(_)) => {
                        dead_ids.push(id);
                    }
                }
            }

            for id in dead_ids {
                channels.remove(&id);
            }
        }

        // 2. Dispatch to registered callbacks under panic shield
        let callbacks_snapshot = {
            if let Ok(callbacks) = self.callbacks.lock() {
                callbacks.values().cloned().collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        };

        for cb in callbacks_snapshot {
            let event_ref = &event;
            let _ = catch_unwind(AssertUnwindSafe(|| {
                cb(event_ref);
            }));
        }
    }

    /// Returns the total count of active subscribers (channels + callbacks).
    #[must_use]
    pub fn total_subscribers(&self) -> usize {
        let ch_count = self.channels.lock().map(|c| c.len()).unwrap_or(0);
        let cb_count = self.callbacks.lock().map(|c| c.len()).unwrap_or(0);
        ch_count + cb_count
    }
}
