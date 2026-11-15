//! Foundational domain types, strongly typed identifiers, configuration schemas,
//! and error taxonomy for the Universal AI Assistant.

#![forbid(unsafe_code)]

pub mod config;
pub mod error;
pub mod ids;

pub use config::{AppConfig, ModelConfig, NetworkConfig, SecurityConfig, VoiceConfig};
pub use error::{CoreError, ErrorCategory, ErrorCode, ErrorDescriptor};
pub use ids::{AuditEntryId, ConversationId, DeviceId, ModelId, SkillId, TurnId};

/// Constant library version for telemetry and handshake verification.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_version_present() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_end_to_end_core_contracts() {
        let device_id = DeviceId::generate();
        let conv_id = ConversationId::generate();
        let turn_id = TurnId::from_sequence(1);
        let skill_id = SkillId::new("device.telemetry").unwrap();
        let model_id = ModelId::new("phi-3.5-mini-instruct").unwrap();

        let mut config = AppConfig::default();
        config.model.default_model_id = Some(model_id);
        assert!(config.validate().is_ok());

        assert_eq!(device_id.as_str().len(), 36);
        assert_eq!(conv_id.as_str().len(), 36);
        assert_eq!(turn_id.as_str(), "turn_000001");
        assert_eq!(skill_id.as_str(), "device.telemetry");
    }
}
