use crate::error::SpeechError;
use serde::{Deserialize, Serialize};

/// Configuration for continuous offline wake-word spotting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeWordConfig {
    /// List of target keywords or keyphrases (e.g., `["hey assistant", "computer", "jarvis"]`).
    pub keywords: Vec<String>,

    /// Sensitivity detection threshold in range `[0.0, 1.0]`. Default: `0.60`.
    /// Higher values reduce false positives; lower values reduce false negatives.
    pub threshold: f32,

    /// Minimum duration (in milliseconds) between consecutive wake-word triggers to prevent double-firing.
    /// Default: `1500` ms.
    pub min_keyword_interval_ms: u32,

    /// Expected audio sample rate in Hz. Default: `16_000` (16 kHz mono).
    pub sample_rate: u32,

    /// Window size in samples for acoustic analysis frames. Default: `512` (32 ms @ 16 kHz).
    pub window_size: usize,

    /// Hop / stride step in samples between consecutive analysis frames. Default: `160` (10 ms @ 16 kHz).
    pub slide_step: usize,

    /// Minimum speech energy in dBFS required to activate keyword correlation matching.
    /// Rejects silence and quiet background noise before running pattern matcher.
    /// Default: `-45.0` dBFS.
    pub min_energy_dbfs: f32,
}

impl Default for WakeWordConfig {
    fn default() -> Self {
        Self {
            keywords: vec!["hey assistant".to_string()],
            threshold: 0.60,
            min_keyword_interval_ms: 1500,
            sample_rate: 16_000,
            window_size: 512,
            slide_step: 160,
            min_energy_dbfs: -45.0,
        }
    }
}

impl WakeWordConfig {
    /// Validates the configuration parameters.
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.keywords.is_empty() {
            return Err(SpeechError::ParameterError(
                "At least one keyword must be specified".to_string(),
            ));
        }
        for kw in &self.keywords {
            if kw.trim().is_empty() {
                return Err(SpeechError::ParameterError(
                    "Keyword cannot be empty or whitespace only".to_string(),
                ));
            }
        }
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err(SpeechError::ParameterError(format!(
                "Threshold {} is out of range [0.0, 1.0]",
                self.threshold
            )));
        }
        if self.sample_rate != 16_000 {
            return Err(SpeechError::InvalidAudioFormat(format!(
                "Wake-word engine requires 16000 Hz, got {}",
                self.sample_rate
            )));
        }
        if self.window_size == 0 || self.slide_step == 0 {
            return Err(SpeechError::ParameterError(
                "Window size and slide step must be greater than zero".to_string(),
            ));
        }
        if self.slide_step > self.window_size {
            return Err(SpeechError::ParameterError(
                "Slide step cannot be larger than window size".to_string(),
            ));
        }
        Ok(())
    }
}

/// Event emitted when a wake-word is detected in the streaming audio buffer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeWordEvent {
    /// The keyword or keyphrase that was spotted.
    pub keyword: String,

    /// Confidence score of the match in range `[0.0, 1.0]`.
    pub confidence: f32,

    /// Audio timestamp in milliseconds when the keyword trigger was detected.
    pub timestamp_ms: u64,

    /// Estimated start sample index in the continuous audio stream.
    pub start_sample: u64,

    /// Estimated end sample index in the continuous audio stream.
    pub end_sample: u64,
}

/// Abstract contract for offline wake-word and keyword spotting engines.
pub trait WakeWordSpotter: Send {
    /// Process streaming 16 kHz mono f32 audio samples.
    ///
    /// Returns `Some(WakeWordEvent)` if a target keyword triggered, or `None` if
    /// no keyword was identified in the recent audio window.
    fn process_samples(&mut self, samples: &[f32]) -> Result<Option<WakeWordEvent>, SpeechError>;

    /// Returns the list of currently monitored keywords.
    fn keywords(&self) -> &[String];

    /// Replaces the list of monitored keywords at runtime.
    fn set_keywords(&mut self, keywords: &[String]) -> Result<(), SpeechError>;

    /// Updates the sensitivity threshold `[0.0, 1.0]`.
    fn set_threshold(&mut self, threshold: f32) -> Result<(), SpeechError>;

    /// Clears internal sliding buffers, debounce timers, and stream counters.
    fn reset(&mut self);
}
