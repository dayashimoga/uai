use thiserror::Error;

/// Error taxonomy for the `aissistant-llm` crate.
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("Hardware probe failure: {0}")]
    HardwareProbeError(String),

    #[error("Unsupported platform for local LLM: {0}")]
    UnsupportedPlatform(String),

    #[error("Insufficient memory for model: required {required_bytes} bytes, available {available_bytes} bytes")]
    InsufficientMemory {
        required_bytes: u64,
        available_bytes: u64,
    },

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Checksum mismatch for model {model_id}: expected {expected}, calculated {actual}")]
    ChecksumMismatch {
        model_id: String,
        expected: String,
        actual: String,
    },

    #[error("GGUF metadata parsing error: {0}")]
    GgufParseError(String),

    #[error("llama.cpp inference runtime error: {0}")]
    InferenceError(String),

    #[error("GBNF grammar compilation error: {0}")]
    GrammarError(String),

    #[error("Embedding generation error: {0}")]
    EmbeddingError(String),

    #[error("Vision processing error: {0}")]
    VisionError(String),

    #[error("OCR extraction error: {0}")]
    OcrError(String),

    #[error("Translation error: {0}")]
    TranslationError(String),

    #[error("Inference cancellation requested")]
    CancellationRequested,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, LlmError>;
