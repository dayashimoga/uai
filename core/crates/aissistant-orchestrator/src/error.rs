//! Domain error taxonomy for the AI Orchestrator crate.

use aissistant_core::CoreError;
use thiserror::Error;

/// Domain errors occurring within the conversational turn state machine and orchestrator.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    /// An illegal turn state transition was attempted.
    #[error("Invalid turn state transition from '{from}' to '{to}': {reason}")]
    InvalidStateTransition {
        from: String,
        to: String,
        reason: String,
    },

    /// Specified conversation session was not found.
    #[error("Conversation not found: {0}")]
    ConversationNotFound(String),

    /// Turn execution was interrupted by user barge-in or cancellation.
    #[error("Turn execution interrupted: {0}")]
    Interrupted(String),

    /// Underlying storage or persistence failure.
    #[error("Storage error: {0}")]
    Storage(#[from] aissistant_storage::StorageError),

    /// Local LLM inference or model execution error.
    #[error("LLM error: {0}")]
    Llm(#[from] aissistant_llm::LlmError),

    /// Speech recognition or synthesis error.
    #[error("Speech error: {0}")]
    Speech(#[from] aissistant_speech::SpeechError),

    /// Audio capture or playback error.
    #[error("Audio error: {0}")]
    Audio(#[from] aissistant_audio::AudioError),

    /// Core domain validation error.
    #[error("Core error: {0}")]
    Core(#[from] aissistant_core::CoreError),

    /// JSON serialization or deserialization failure.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// General internal orchestrator failure.
    #[error("Internal orchestrator error: {0}")]
    Internal(String),

    /// Requested skill or tool was not found in the dispatcher registry.
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Execution of a tool handler failed.
    #[error("Tool '{tool_name}' execution failed: {reason}")]
    ToolExecutionFailed { tool_name: String, reason: String },

    /// Specified pending confirmation request was not found.
    #[error("Confirmation not found: {0}")]
    ConfirmationNotFound(String),

    /// Tool execution was strictly denied by security policy.
    #[error("Tool '{tool_name}' execution denied by policy: {reason}")]
    ToolPolicyDenied { tool_name: String, reason: String },

    /// Arguments provided for tool execution do not match parameter schema.
    #[error("Invalid arguments for tool '{tool_name}': {reason}")]
    InvalidToolArguments { tool_name: String, reason: String },
}

impl OrchestratorError {
    /// Maps this orchestrator error into the canonical `CoreError` taxonomy.
    #[must_use]
    pub fn to_core_error(&self) -> CoreError {
        match self {
            Self::Core(err) => err.clone(),
            Self::Storage(err) => err.to_core_error(),
            Self::InvalidStateTransition { from, to, reason } => CoreError::ConfigValidation {
                field: format!("state_transition_{from}_to_{to}"),
                reason: reason.clone(),
            },
            Self::ConversationNotFound(id) => CoreError::NotFound {
                entity: "Conversation",
                id: id.clone(),
            },
            Self::ToolNotFound(id) => CoreError::NotFound {
                entity: "Tool",
                id: id.clone(),
            },
            Self::ConfirmationNotFound(id) => CoreError::NotFound {
                entity: "Confirmation",
                id: id.clone(),
            },
            Self::ToolPolicyDenied { tool_name, reason } => CoreError::PermissionDenied {
                action: format!("tool_execution:{tool_name}"),
                reason: reason.clone(),
            },
            Self::InvalidToolArguments { tool_name, reason } => CoreError::ConfigValidation {
                field: format!("tool_args:{tool_name}"),
                reason: reason.clone(),
            },
            Self::ToolExecutionFailed { tool_name, reason } => CoreError::Internal {
                message: format!("Tool '{tool_name}' execution failed: {reason}"),
            },
            Self::Interrupted(msg) => CoreError::Internal {
                message: format!("Turn interrupted: {msg}"),
            },
            Self::Llm(err) => CoreError::Internal {
                message: format!("LLM failure: {err}"),
            },
            Self::Speech(err) => CoreError::Internal {
                message: format!("Speech failure: {err}"),
            },
            Self::Audio(err) => CoreError::Internal {
                message: format!("Audio failure: {err}"),
            },
            Self::Serialization(err) => CoreError::Serialization {
                message: err.to_string(),
            },
            Self::Internal(msg) => CoreError::Internal {
                message: msg.clone(),
            },
        }
    }
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_core::ErrorCategory;

    #[test]
    fn test_error_mapping_to_core() {
        let invalid = OrchestratorError::InvalidStateTransition {
            from: "Idle".to_string(),
            to: "SynthesizingSpeech".to_string(),
            reason: "Must receive utterance first".to_string(),
        };
        assert_eq!(
            invalid.to_core_error().category(),
            ErrorCategory::Configuration
        );

        let not_found = OrchestratorError::ConversationNotFound("conv_123".to_string());
        assert_eq!(
            not_found.to_core_error().category(),
            ErrorCategory::NotFound
        );

        let interrupted = OrchestratorError::Interrupted("barge_in".to_string());
        assert_eq!(
            interrupted.to_core_error().category(),
            ErrorCategory::Internal
        );
    }
}
