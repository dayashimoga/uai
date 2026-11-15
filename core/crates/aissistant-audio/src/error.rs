use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error taxonomy for audio buffer, resampling, and metering operations.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioError {
    #[error("Audio buffer overflow: requested {requested} samples, capacity is {capacity}")]
    BufferOverflow { capacity: usize, requested: usize },

    #[error("Audio buffer underflow: requested {requested} samples, only {available} available")]
    BufferUnderflow { requested: usize, available: usize },

    #[error("Invalid sample rate {rate} Hz: {reason}")]
    InvalidSampleRate { rate: u32, reason: String },

    #[error("Invalid channel count {channels}: {reason}")]
    InvalidChannelCount { channels: u16, reason: String },

    #[error("Resampling error: {0}")]
    ResamplingError(String),

    #[error("Audio parameter error: {0}")]
    ParameterError(String),

    #[error("Audio device not found: {0}")]
    DeviceNotFound(String),

    #[error("Audio device unavailable: {0}")]
    DeviceUnavailable(String),

    #[error("Audio stream build error: {0}")]
    StreamBuildError(String),

    #[error("Audio stream play error: {0}")]
    StreamPlayError(String),

    #[error("Audio backend error: {0}")]
    BackendError(String),
}
