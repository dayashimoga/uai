//! Application configuration schema, bounds validation, and offline-first defaults.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::CoreError;
use crate::ids::ModelId;

/// Voice capture, wake-word, and speech synthesis settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceConfig {
    /// Whether continuous background wake-word listening is active.
    pub wake_word_enabled: bool,
    /// Configured wake phrases (e.g. `["hey assistant"]`).
    pub wake_phrases: Vec<String>,
    /// Detection sensitivity threshold between 0.0 and 1.0.
    pub wake_word_sensitivity: f32,
    /// Trailing silence duration in milliseconds before marking end of speech.
    pub vad_silence_duration_ms: u32,
    /// Audio capture and playback sample rate in Hz (16000 or 48000).
    pub sample_rate: u32,
    /// Speech synthesis speed multiplier (0.25 to 4.0).
    pub tts_speaking_rate: f32,
    /// Optional default STT model identifier.
    pub stt_model_id: Option<ModelId>,
    /// Optional default TTS voice identifier.
    pub tts_voice_id: Option<String>,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            wake_word_enabled: false,
            wake_phrases: vec!["hey assistant".to_string()],
            wake_word_sensitivity: 0.6,
            vad_silence_duration_ms: 500,
            sample_rate: 16_000,
            tts_speaking_rate: 1.0,
            stt_model_id: None,
            tts_voice_id: None,
        }
    }
}

impl VoiceConfig {
    /// Validates voice settings against supported ranges.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.wake_word_enabled && self.wake_phrases.is_empty() {
            return Err(CoreError::ConfigValidation {
                field: "voice.wake_phrases".to_string(),
                reason: "wake word is enabled but no wake phrases are configured".to_string(),
            });
        }

        for phrase in &self.wake_phrases {
            if phrase.trim().is_empty() {
                return Err(CoreError::ConfigValidation {
                    field: "voice.wake_phrases".to_string(),
                    reason: "wake phrases cannot be empty or whitespace".to_string(),
                });
            }
        }

        if !(0.0..=1.0).contains(&self.wake_word_sensitivity) {
            return Err(CoreError::ConfigValidation {
                field: "voice.wake_word_sensitivity".to_string(),
                reason: format!(
                    "sensitivity must be between 0.0 and 1.0 (got {})",
                    self.wake_word_sensitivity
                ),
            });
        }

        if !(50..=5000).contains(&self.vad_silence_duration_ms) {
            return Err(CoreError::ConfigValidation {
                field: "voice.vad_silence_duration_ms".to_string(),
                reason: format!(
                    "VAD silence duration must be between 50 and 5000 ms (got {})",
                    self.vad_silence_duration_ms
                ),
            });
        }

        if self.sample_rate != 16_000 && self.sample_rate != 48_000 {
            return Err(CoreError::ConfigValidation {
                field: "voice.sample_rate".to_string(),
                reason: format!(
                    "sample rate must be 16000 or 48000 Hz (got {})",
                    self.sample_rate
                ),
            });
        }

        if !(0.25..=4.0).contains(&self.tts_speaking_rate) {
            return Err(CoreError::ConfigValidation {
                field: "voice.tts_speaking_rate".to_string(),
                reason: format!(
                    "TTS speaking rate must be between 0.25 and 4.0 (got {})",
                    self.tts_speaking_rate
                ),
            });
        }

        Ok(())
    }
}

/// Local LLM runtime and memory limit settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Default language model identifier to load on demand.
    pub default_model_id: Option<ModelId>,
    /// Base directory where verified model files are stored.
    pub models_dir: PathBuf,
    /// Maximum concurrent models resident in RAM/VRAM.
    pub max_loaded_models: usize,
    /// Maximum token context window.
    pub context_window_tokens: usize,
    /// Number of layers to offload to GPU (-1 for all, 0 for CPU-only).
    pub gpu_layers: i32,
    /// Number of CPU threads to allocate (0 for physical core detection).
    pub cpu_threads: u32,
    /// Sampling temperature (0.0 to 2.0).
    pub temperature: f32,
    /// Idle duration in seconds before automatically unloading the model from memory.
    pub auto_unload_timeout_secs: u64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            default_model_id: None,
            models_dir: PathBuf::from("models"),
            max_loaded_models: 1,
            context_window_tokens: 4096,
            gpu_layers: 0,
            cpu_threads: 0, // 0 = automatic detection based on physical cores
            temperature: 0.7,
            auto_unload_timeout_secs: 300,
        }
    }
}

impl ModelConfig {
    /// Validates model configuration bounds and memory safety constraints.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.max_loaded_models == 0 || self.max_loaded_models > 4 {
            return Err(CoreError::ConfigValidation {
                field: "model.max_loaded_models".to_string(),
                reason: format!(
                    "max loaded models must be between 1 and 4 (got {})",
                    self.max_loaded_models
                ),
            });
        }

        if !(512..=131_072).contains(&self.context_window_tokens) {
            return Err(CoreError::ConfigValidation {
                field: "model.context_window_tokens".to_string(),
                reason: format!(
                    "context window must be between 512 and 131072 tokens (got {})",
                    self.context_window_tokens
                ),
            });
        }

        if self.gpu_layers < -1 {
            return Err(CoreError::ConfigValidation {
                field: "model.gpu_layers".to_string(),
                reason: format!("gpu_layers must be >= -1 (got {})", self.gpu_layers),
            });
        }

        if self.cpu_threads > 128 {
            return Err(CoreError::ConfigValidation {
                field: "model.cpu_threads".to_string(),
                reason: format!("cpu_threads cannot exceed 128 (got {})", self.cpu_threads),
            });
        }

        if !(0.0..=2.0).contains(&self.temperature) {
            return Err(CoreError::ConfigValidation {
                field: "model.temperature".to_string(),
                reason: format!(
                    "temperature must be between 0.0 and 2.0 (got {})",
                    self.temperature
                ),
            });
        }

        Ok(())
    }
}

/// Security policies, authorization controls, and taint tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Whether device pairing requires explicit out-of-band SAS / QR confirmation.
    pub require_pairing_confirmation: bool,
    /// Whether security and skill audit logs are persisted to SQLite.
    pub audit_log_enabled: bool,
    /// Session duration before requiring authorization refresh.
    pub session_timeout_secs: u64,
    /// Whether conversation turns with untrusted data are marked tainted.
    pub taint_tracking_enabled: bool,
    /// Explicit filesystem directories allowed for file operations.
    pub allowed_storage_roots: Vec<PathBuf>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_pairing_confirmation: true,
            audit_log_enabled: true,
            session_timeout_secs: 3600,
            taint_tracking_enabled: true,
            allowed_storage_roots: Vec::new(),
        }
    }
}

impl SecurityConfig {
    /// Validates security policy constraints.
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(60..=86_400).contains(&self.session_timeout_secs) {
            return Err(CoreError::ConfigValidation {
                field: "security.session_timeout_secs".to_string(),
                reason: format!(
                    "session timeout must be between 60 and 86400 seconds (got {})",
                    self.session_timeout_secs
                ),
            });
        }

        Ok(())
    }
}

/// Local discovery and peer-to-peer transport settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Whether mDNS LAN peer discovery is enabled.
    pub mdns_discovery_enabled: bool,
    /// Local UDP listening port for QUIC transport.
    pub quic_port: u16,
    /// Maximum allowed incoming payload size in bytes.
    pub max_payload_bytes: usize,
    /// Maximum failed pairing attempts allowed before temporary lockout.
    pub rate_limit_pairing_attempts: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mdns_discovery_enabled: true,
            quic_port: 42_424,
            max_payload_bytes: 32 * 1024 * 1024, // 32 MiB default chunk/stream cap
            rate_limit_pairing_attempts: 5,
        }
    }
}

impl NetworkConfig {
    /// Validates network configuration parameters.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.quic_port < 1024 {
            return Err(CoreError::ConfigValidation {
                field: "network.quic_port".to_string(),
                reason: format!(
                    "quic_port must be in unprivileged range >= 1024 (got {})",
                    self.quic_port
                ),
            });
        }

        if self.max_payload_bytes < 64 * 1024 || self.max_payload_bytes > 1024 * 1024 * 1024 {
            return Err(CoreError::ConfigValidation {
                field: "network.max_payload_bytes".to_string(),
                reason: format!(
                    "max payload must be between 64 KiB and 1 GiB (got {} bytes)",
                    self.max_payload_bytes
                ),
            });
        }

        if self.rate_limit_pairing_attempts == 0 || self.rate_limit_pairing_attempts > 20 {
            return Err(CoreError::ConfigValidation {
                field: "network.rate_limit_pairing_attempts".to_string(),
                reason: format!(
                    "pairing attempts rate limit must be between 1 and 20 (got {})",
                    self.rate_limit_pairing_attempts
                ),
            });
        }

        Ok(())
    }
}

/// Top-level application configuration schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Display name of the application or assistant persona.
    pub app_name: String,
    /// Root data directory for persistence and caches.
    pub data_dir: PathBuf,
    /// Enforces zero outbound cloud API connections.
    pub offline_only: bool,
    /// Voice and speech pipeline configuration.
    pub voice: VoiceConfig,
    /// Local language model configuration.
    pub model: ModelConfig,
    /// Security, taint tracking, and permission settings.
    pub security: SecurityConfig,
    /// LAN discovery and QUIC networking settings.
    pub network: NetworkConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app_name: "Universal AI Assistant".to_string(),
            data_dir: PathBuf::from("aissistant_data"),
            offline_only: true, // Offline-first invariant by default
            voice: VoiceConfig::default(),
            model: ModelConfig::default(),
            security: SecurityConfig::default(),
            network: NetworkConfig::default(),
        }
    }
}

impl AppConfig {
    /// Validates the complete application configuration tree.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.app_name.trim().is_empty() {
            return Err(CoreError::ConfigValidation {
                field: "app_name".to_string(),
                reason: "app_name cannot be empty or whitespace".to_string(),
            });
        }

        self.voice.validate()?;
        self.model.validate()?;
        self.security.validate()?;
        self.network.validate()?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
        assert!(config.offline_only);
        assert_eq!(config.network.quic_port, 42_424);
    }

    #[test]
    fn test_app_name_validation() {
        let config = AppConfig {
            app_name: "   ".to_string(),
            ..AppConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_voice_config_validation() {
        let mut voice = VoiceConfig {
            wake_word_enabled: true,
            wake_phrases: vec![],
            ..VoiceConfig::default()
        };
        assert!(voice.validate().is_err());

        voice.wake_phrases = vec!["  ".to_string()];
        assert!(voice.validate().is_err());

        voice.wake_phrases = vec!["computer".to_string()];
        voice.wake_word_sensitivity = 1.5;
        assert!(voice.validate().is_err());

        voice.wake_word_sensitivity = 0.8;
        voice.vad_silence_duration_ms = 10;
        assert!(voice.validate().is_err());

        voice.vad_silence_duration_ms = 400;
        voice.sample_rate = 44100;
        assert!(voice.validate().is_err());

        voice.sample_rate = 16000;
        voice.tts_speaking_rate = 5.0;
        assert!(voice.validate().is_err());

        voice.tts_speaking_rate = 1.2;
        assert!(voice.validate().is_ok());
    }

    #[test]
    fn test_model_config_validation() {
        let mut model = ModelConfig {
            max_loaded_models: 0,
            ..ModelConfig::default()
        };
        assert!(model.validate().is_err());

        model.max_loaded_models = 5;
        assert!(model.validate().is_err());

        model.max_loaded_models = 2;
        model.context_window_tokens = 256;
        assert!(model.validate().is_err());

        model.context_window_tokens = 200_000;
        assert!(model.validate().is_err());

        model.context_window_tokens = 8192;
        model.gpu_layers = -2;
        assert!(model.validate().is_err());

        model.gpu_layers = 33;
        model.cpu_threads = 200;
        assert!(model.validate().is_err());

        model.cpu_threads = 8;
        model.temperature = 2.5;
        assert!(model.validate().is_err());

        model.temperature = -0.5;
        assert!(model.validate().is_err());

        model.temperature = 0.7;
        assert!(model.validate().is_ok());
    }

    #[test]
    fn test_security_and_network_validation() {
        let mut sec = SecurityConfig {
            session_timeout_secs: 10,
            ..SecurityConfig::default()
        };
        assert!(sec.validate().is_err());

        sec.session_timeout_secs = 100_000;
        assert!(sec.validate().is_err());

        sec.session_timeout_secs = 1800;
        assert!(sec.validate().is_ok());

        let mut net = NetworkConfig {
            quic_port: 80,
            ..NetworkConfig::default()
        };
        assert!(net.validate().is_err());

        net.quic_port = 50000;
        net.max_payload_bytes = 1000;
        assert!(net.validate().is_err());

        net.max_payload_bytes = 2 * 1024 * 1024 * 1024;
        assert!(net.validate().is_err());

        net.max_payload_bytes = 64 * 1024 * 1024;
        net.rate_limit_pairing_attempts = 0;
        assert!(net.validate().is_err());

        net.rate_limit_pairing_attempts = 25;
        assert!(net.validate().is_err());

        net.rate_limit_pairing_attempts = 3;
        assert!(net.validate().is_ok());
    }

    #[test]
    fn test_app_config_serde_roundtrip() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
