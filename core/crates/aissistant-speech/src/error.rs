use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error taxonomy for speech processing, VAD, and wake-word operations.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeechError {
    #[error("Invalid audio format: {0}")]
    InvalidAudioFormat(String),

    #[error("VAD processing error: {0}")]
    VadError(String),

    #[error("Speech model error: {0}")]
    ModelError(String),

    #[error("Buffer error: {0}")]
    BufferError(String),

    #[error("Speech parameter error: {0}")]
    ParameterError(String),

    #[error("Wake-word detection error: {0}")]
    WakeWordError(String),

    #[error("STT transcription error: {0}")]
    SttError(String),
}
