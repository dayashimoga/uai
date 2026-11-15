use crate::error::SpeechError;
use serde::{Deserialize, Serialize};

/// Decoding search strategy for speech-to-text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DecodingMethod {
    #[default]
    GreedySearch,
    ModifiedBeamSearch,
}

/// Configuration parameters for streaming and chunked offline speech recognition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SttConfig {
    /// Target language code (e.g. `"en"`, `"es"`, `"de"`, `"fr"`, `"zh"`, `"ja"`, or `"auto"` for automatic detection).
    pub language: String,

    /// Expected audio sample rate in Hz. Must be 16,000 Hz.
    pub sample_rate: u32,

    /// Beam size when using `ModifiedBeamSearch`. Default: `4`.
    pub beam_size: usize,

    /// Decoding search algorithm. Default: `GreedySearch`.
    pub decoding_method: DecodingMethod,

    /// Trailing silence duration (in milliseconds) required to trigger automatic endpointing.
    /// Default: `800` ms.
    pub min_trailing_silence_ms: u32,

    /// Maximum utterance length (in milliseconds) before forcing an endpoint.
    /// Default: `20_000` ms (20 seconds).
    pub max_utterance_ms: u32,

    /// Hotwords / key vocabulary boosting phrases (e.g. `["assistant", "settings", "volume"]`).
    pub hotwords: Vec<String>,

    /// Score boost multiplier for hotwords. Default: `1.5`.
    pub hotwords_score: f32,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            sample_rate: 16_000,
            beam_size: 4,
            decoding_method: DecodingMethod::GreedySearch,
            min_trailing_silence_ms: 800,
            max_utterance_ms: 20_000,
            hotwords: Vec::new(),
            hotwords_score: 1.5,
        }
    }
}

impl SttConfig {
    /// Validates configuration parameters.
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.language.trim().is_empty() {
            return Err(SpeechError::ParameterError(
                "Language code cannot be empty".to_string(),
            ));
        }
        if self.sample_rate != 16_000 {
            return Err(SpeechError::InvalidAudioFormat(format!(
                "STT engine requires 16000 Hz audio, got {}",
                self.sample_rate
            )));
        }
        if self.beam_size == 0 {
            return Err(SpeechError::ParameterError(
                "beam_size must be greater than zero".to_string(),
            ));
        }
        if self.min_trailing_silence_ms == 0 {
            return Err(SpeechError::ParameterError(
                "min_trailing_silence_ms must be greater than zero".to_string(),
            ));
        }
        if self.max_utterance_ms <= self.min_trailing_silence_ms {
            return Err(SpeechError::ParameterError(
                "max_utterance_ms must be greater than min_trailing_silence_ms".to_string(),
            ));
        }
        Ok(())
    }
}

/// A transcribed speech hypothesis or completed sentence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// Decoded text.
    pub text: String,

    /// Resolved language code (e.g., `"en"`, `"es"`, `"de"`).
    pub language: String,

    /// Recognition confidence score in range `[0.0, 1.0]`.
    pub confidence: f32,

    /// Stream start timestamp in milliseconds.
    pub start_time_ms: u64,

    /// Stream end timestamp in milliseconds.
    pub end_time_ms: u64,

    /// Indicates whether this transcript is final (endpointed) or an interim partial hypothesis.
    pub is_final: bool,

    /// Individual decoded word/subword tokens.
    pub tokens: Vec<String>,
}

impl Default for TranscriptSegment {
    fn default() -> Self {
        Self {
            text: String::new(),
            language: "en".to_string(),
            confidence: 0.0,
            start_time_ms: 0,
            end_time_ms: 0,
            is_final: false,
            tokens: Vec::new(),
        }
    }
}

/// Events emitted during streaming speech recognition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StreamingTranscriptEvent {
    /// Interim partial transcript hypothesis while user is speaking.
    Partial(TranscriptSegment),

    /// Final, endpointed transcript at the conclusion of an utterance.
    Final(TranscriptSegment),

    /// Notification that endpoint silence was confirmed.
    EndpointDetected { timestamp_ms: u64 },

    /// Automatic language identification notification.
    LanguageIdentified { language: String, confidence: f32 },
}

/// Abstract contract for offline multilingual streaming and chunked STT engines.
pub trait SttEngine: Send {
    /// Accepts a slice of streaming 16 kHz mono f32 audio samples.
    fn accept_waveform(&mut self, samples: &[f32]) -> Result<(), SpeechError>;

    /// Executes one decoding step on buffered audio, returning an event if the hypothesis changed.
    fn decode_step(&mut self) -> Result<Option<StreamingTranscriptEvent>, SpeechError>;

    /// Returns the current full transcript hypothesis.
    fn get_result(&self) -> TranscriptSegment;

    /// Checks whether an utterance boundary / endpoint has been reached.
    fn is_endpoint(&self) -> bool;

    /// Resets the internal decoder stream state for a new utterance.
    fn reset(&mut self);

    /// Returns the active language code.
    fn language(&self) -> &str;

    /// Changes the active language code at runtime.
    fn set_language(&mut self, language: &str) -> Result<(), SpeechError>;
}
