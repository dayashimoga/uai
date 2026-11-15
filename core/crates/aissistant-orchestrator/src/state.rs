//! Conversation turn states, inputs, and orchestrator event definitions.

use std::fmt;

use aissistant_core::{ConversationId, TurnId};
use serde::{Deserialize, Serialize};

/// Discrete states in the conversational interaction turn lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    /// Resting state, awaiting new user voice or text input.
    Idle,
    /// Actively recording microphone audio stream.
    Listening,
    /// Live speech-to-text decoding partial transcripts.
    Transcribing,
    /// Complete user message (text or finalized STT) received.
    UtteranceReceived,
    /// Querying SQLite FTS5 BM25 and vector stores via Reciprocal Rank Fusion.
    RetrievingMemory,
    /// Assembling prompt context, memory excerpts, and skill tool schemas.
    PromptAssembly,
    /// Local LLM token-by-token generation.
    Inferring,
    /// A structured tool call invocation was emitted by the model.
    ToolInvocationDetected,
    /// Waiting for policy authorization or external tool execution.
    AwaitingToolExecution,
    /// Neural TTS speech synthesis and sentence chunk streaming.
    SynthesizingSpeech,
    /// Turn completed, persisted in SQLite, and embedded in vector store.
    TurnCompleted,
    /// Turn was interrupted by user barge-in or cancellation.
    Interrupted,
    /// An unrecoverable error occurred during the turn.
    Failed,
}

impl TurnState {
    /// Returns the static string representation of this state.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Listening => "listening",
            Self::Transcribing => "transcribing",
            Self::UtteranceReceived => "utterance_received",
            Self::RetrievingMemory => "retrieving_memory",
            Self::PromptAssembly => "prompt_assembly",
            Self::Inferring => "inferring",
            Self::ToolInvocationDetected => "tool_invocation_detected",
            Self::AwaitingToolExecution => "awaiting_tool_execution",
            Self::SynthesizingSpeech => "synthesizing_speech",
            Self::TurnCompleted => "turn_completed",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
        }
    }

    /// Evaluates whether a transition from `self` to `target` is allowed.
    #[must_use]
    pub fn can_transition_to(&self, target: Self) -> bool {
        if *self == target {
            return true;
        }

        match self {
            Self::Idle => matches!(
                target,
                Self::Listening | Self::Transcribing | Self::UtteranceReceived | Self::Failed
            ),
            Self::Listening => matches!(
                target,
                Self::Transcribing
                    | Self::UtteranceReceived
                    | Self::Idle
                    | Self::Interrupted
                    | Self::Failed
            ),
            Self::Transcribing => matches!(
                target,
                Self::UtteranceReceived
                    | Self::Listening
                    | Self::Idle
                    | Self::Interrupted
                    | Self::Failed
            ),
            Self::UtteranceReceived => matches!(
                target,
                Self::RetrievingMemory
                    | Self::PromptAssembly
                    | Self::Inferring
                    | Self::Interrupted
                    | Self::Failed
            ),
            Self::RetrievingMemory => matches!(
                target,
                Self::PromptAssembly | Self::Inferring | Self::Interrupted | Self::Failed
            ),
            Self::PromptAssembly => {
                matches!(target, Self::Inferring | Self::Interrupted | Self::Failed)
            }
            Self::Inferring => matches!(
                target,
                Self::ToolInvocationDetected
                    | Self::SynthesizingSpeech
                    | Self::TurnCompleted
                    | Self::Interrupted
                    | Self::Failed
            ),
            Self::ToolInvocationDetected => matches!(
                target,
                Self::AwaitingToolExecution | Self::Inferring | Self::Interrupted | Self::Failed
            ),
            Self::AwaitingToolExecution => matches!(
                target,
                Self::Inferring
                    | Self::SynthesizingSpeech
                    | Self::TurnCompleted
                    | Self::Interrupted
                    | Self::Failed
            ),
            Self::SynthesizingSpeech => {
                matches!(
                    target,
                    Self::TurnCompleted | Self::Interrupted | Self::Failed
                )
            }
            Self::TurnCompleted => matches!(target, Self::Idle | Self::Listening),
            Self::Interrupted => {
                matches!(
                    target,
                    Self::Idle | Self::Listening | Self::UtteranceReceived
                )
            }
            Self::Failed => matches!(target, Self::Idle | Self::Listening),
        }
    }
}

impl fmt::Display for TurnState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The origin or modality of a user turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputMode {
    /// Natural-language text prompt entered via chat UI.
    Text,
    /// Spoken natural-language utterance transcribed via streaming STT.
    Voice,
}

/// Input payload initiating a turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnInput {
    /// Target conversation session.
    pub conversation_id: ConversationId,
    /// Transcribed or typed message content.
    pub content: String,
    /// Input modality.
    pub mode: InputMode,
    /// Unix millisecond timestamp when the input was received.
    pub timestamp_ms: i64,
}

/// Event emitted by the orchestrator turn state machine for UI/FFI consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum OrchestratorEvent {
    /// Turn state machine moved to a new state.
    StateChanged {
        from: TurnState,
        to: TurnState,
        timestamp_ms: i64,
    },
    /// Hybrid memory retrieval finished.
    MemoryRetrieved { count: usize, duration_ms: u64 },
    /// A single token was generated by the local LLM.
    TokenStreamed { token: String, index: usize },
    /// A sentence was synthesized by TTS.
    SentenceSynthesized {
        sentence: String,
        audio_samples: usize,
    },
    /// A structured tool call was emitted by the model.
    ToolCallDetected { tool_name: String },
    /// User barge-in speech or explicit cancellation triggered.
    BargeInTriggered { timestamp_ms: i64 },
    /// A conversation turn finished executing.
    TurnFinished {
        turn_id: TurnId,
        duration_ms: u64,
        token_count: usize,
        interrupted: bool,
    },
    /// An error occurred during turn processing.
    ErrorOccurred { error: String },
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_turn_state_transitions() {
        assert!(TurnState::Idle.can_transition_to(TurnState::UtteranceReceived));
        assert!(TurnState::UtteranceReceived.can_transition_to(TurnState::RetrievingMemory));
        assert!(TurnState::RetrievingMemory.can_transition_to(TurnState::PromptAssembly));
        assert!(TurnState::PromptAssembly.can_transition_to(TurnState::Inferring));
        assert!(TurnState::Inferring.can_transition_to(TurnState::SynthesizingSpeech));
        assert!(TurnState::SynthesizingSpeech.can_transition_to(TurnState::TurnCompleted));
        assert!(TurnState::TurnCompleted.can_transition_to(TurnState::Idle));

        // Self-transition is always valid
        assert!(TurnState::Inferring.can_transition_to(TurnState::Inferring));

        // Interruption allowed during generation and speech
        assert!(TurnState::Inferring.can_transition_to(TurnState::Interrupted));
        assert!(TurnState::SynthesizingSpeech.can_transition_to(TurnState::Interrupted));
        assert!(TurnState::Interrupted.can_transition_to(TurnState::Idle));
    }

    #[test]
    fn test_invalid_turn_state_transitions_rejected() {
        // Cannot jump directly from Idle to SynthesizingSpeech
        assert!(!TurnState::Idle.can_transition_to(TurnState::SynthesizingSpeech));
        // Cannot jump directly from RetrievingMemory to TurnCompleted
        assert!(!TurnState::RetrievingMemory.can_transition_to(TurnState::TurnCompleted));
        // Cannot jump directly from SynthesizingSpeech to Inferring
        assert!(!TurnState::SynthesizingSpeech.can_transition_to(TurnState::Inferring));
    }
}
