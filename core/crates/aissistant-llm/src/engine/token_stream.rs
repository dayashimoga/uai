use serde::{Deserialize, Serialize};

/// Termination reason for token generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinishReason {
    /// Reached an end-of-sequence token or stop sequence.
    Stop,
    /// Reached `max_tokens` ceiling.
    Length,
    /// Interrupted by cooperative cancellation (e.g. user barge-in).
    Cancelled,
}

/// A streaming token increment emitted during autoregressive inference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenChunk {
    /// Decoded UTF-8 token text increment.
    pub token: String,
    /// Numerical token ID in model vocabulary.
    pub token_id: i32,
    /// Whether this chunk marks the completion of the turn.
    pub is_final: bool,
    /// Termination reason if `is_final` is true.
    pub finish_reason: Option<FinishReason>,
}

impl TokenChunk {
    /// Creates an intermediate token chunk.
    pub fn intermediate(token: impl Into<String>, token_id: i32) -> Self {
        Self {
            token: token.into(),
            token_id,
            is_final: false,
            finish_reason: None,
        }
    }

    /// Creates a final token chunk concluding the turn.
    pub fn final_chunk(token: impl Into<String>, token_id: i32, reason: FinishReason) -> Self {
        Self {
            token: token.into(),
            token_id,
            is_final: true,
            finish_reason: Some(reason),
        }
    }
}

/// Generation performance and token usage metrics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceMetrics {
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
    pub elapsed_ms: u64,
    pub tokens_per_second: f32,
}

impl InferenceMetrics {
    pub fn new(prompt_tokens: usize, generated_tokens: usize, elapsed_ms: u64) -> Self {
        let tokens_per_second = if elapsed_ms > 0 {
            (generated_tokens as f32 / elapsed_ms as f32) * 1000.0
        } else {
            0.0
        };

        Self {
            prompt_tokens,
            generated_tokens,
            elapsed_ms,
            tokens_per_second,
        }
    }
}
