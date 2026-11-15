use serde::{Deserialize, Serialize};

use crate::error::{LlmError, Result};

/// Hyperparameters controlling token sampling, temperature, and length bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamplingParams {
    /// Softmax temperature: 0.0 is greedy deterministic, higher is more creative.
    pub temperature: f32,
    /// Nucleus sampling probability cutoff (0.0 to 1.0).
    pub top_p: f32,
    /// Top-K token candidates limit.
    pub top_k: u32,
    /// Penalty applied to previously generated tokens to prevent repetitive loops.
    pub repetition_penalty: f32,
    /// Maximum new tokens to generate before stopping.
    pub max_tokens: usize,
    /// String sequences that trigger immediate turn termination when generated.
    pub stop_sequences: Vec<String>,
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repetition_penalty: 1.1,
            max_tokens: 1024,
            stop_sequences: vec![
                "<|im_end|>".to_string(),
                "<|end_of_text|>".to_string(),
                "<|eot_id|>".to_string(),
                "\nUser:".to_string(),
            ],
        }
    }
}

impl SamplingParams {
    /// Validates parameter ranges and bounds.
    pub fn validate(&self) -> Result<()> {
        if self.temperature < 0.0 || self.temperature > 2.0 {
            return Err(LlmError::InferenceError(format!(
                "Invalid temperature {:.2}; must be between 0.0 and 2.0",
                self.temperature
            )));
        }
        if self.top_p <= 0.0 || self.top_p > 1.0 {
            return Err(LlmError::InferenceError(format!(
                "Invalid top_p {:.2}; must be in (0.0, 1.0]",
                self.top_p
            )));
        }
        if self.top_k == 0 {
            return Err(LlmError::InferenceError(
                "Invalid top_k 0; must be >= 1".to_string(),
            ));
        }
        if self.repetition_penalty < 0.5 || self.repetition_penalty > 3.0 {
            return Err(LlmError::InferenceError(format!(
                "Invalid repetition_penalty {:.2}; must be in [0.5, 3.0]",
                self.repetition_penalty
            )));
        }
        if self.max_tokens == 0 {
            return Err(LlmError::InferenceError(
                "max_tokens must be >= 1".to_string(),
            ));
        }

        Ok(())
    }
}
