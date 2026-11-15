//! Canonical error types and FFI-safe error descriptors.

use aissistant_core::{CoreError, ErrorCategory, ErrorCode, ErrorDescriptor};
use aissistant_storage::StorageError;
use thiserror::Error;

/// Error types emitted across the FFI bridge boundary.
#[derive(Debug, Error)]
pub enum FfiError {
    /// Failure within core domain logic.
    #[error("Core domain error: {0}")]
    Core(#[from] CoreError),

    /// Failure within SQLite persistence or keystore operations.
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    /// Security policy violation or verification failure.
    #[error("Security policy error: {0}")]
    Security(String),

    /// Serialization or deserialization error crossing FFI.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Invalid parameter supplied over FFI.
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    /// Bridge has not been initialized.
    #[error("Core bridge has not been initialized")]
    NotInitialized,

    /// Bridge has already been initialized.
    #[error("Core bridge has already been initialized")]
    AlreadyInitialized,

    /// Synchronization primitive lock was poisoned.
    #[error("Internal synchronization lock poisoned: {0}")]
    LockPoisoned(String),

    /// Audio or voice processing failure.
    #[error("Voice processing error: {0}")]
    Voice(String),

    /// Local LLM inference or model management failure.
    #[error("Local LLM model error: {0}")]
    Model(String),

    /// Caught an unexpected panic during FFI execution.
    #[error("Unexpected panic caught across FFI: {0}")]
    PanicCaught(String),

    /// Invalid lifecycle state transition attempted.
    #[error("Invalid lifecycle state transition from {from} to {to}")]
    InvalidStateTransition {
        /// Current state.
        from: String,
        /// Target state.
        to: String,
    },

    /// Operation rejected because runtime is shutting down or terminated.
    #[error("Operation rejected: runtime is shutting down or terminated")]
    ShuttingDown,
}

impl From<serde_json::Error> for FfiError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<FfiError> for ErrorDescriptor {
    fn from(err: FfiError) -> Self {
        match err {
            FfiError::Core(core_err) => core_err.to_descriptor(),
            FfiError::Storage(storage_err) => match storage_err {
                StorageError::Core(c) => c.to_descriptor(),
                StorageError::NotFound { key } => ErrorDescriptor::new(
                    ErrorCode::CoreNotFound,
                    ErrorCategory::NotFound,
                    format!("Storage item not found: {key}"),
                    false,
                ),
                StorageError::Serialization { message } => ErrorDescriptor::new(
                    ErrorCode::CoreSerializationFailed,
                    ErrorCategory::Storage,
                    message,
                    false,
                ),
                StorageError::CorruptAuditChain { reason } => ErrorDescriptor::new(
                    ErrorCode::CorePermissionDenied,
                    ErrorCategory::Security,
                    format!("Audit chain integrity violation: {reason}"),
                    false,
                ),
                StorageError::Sqlite(sqlite_err) => ErrorDescriptor::new(
                    ErrorCode::CoreInternalError,
                    ErrorCategory::Storage,
                    sqlite_err.to_string(),
                    true,
                ),
                _ => ErrorDescriptor::new(
                    ErrorCode::CoreInternalError,
                    ErrorCategory::Storage,
                    storage_err.to_string(),
                    false,
                ),
            },
            FfiError::Security(msg) => ErrorDescriptor::new(
                ErrorCode::CorePermissionDenied,
                ErrorCategory::Security,
                msg,
                false,
            ),
            FfiError::Serialization(msg) => ErrorDescriptor::new(
                ErrorCode::CoreSerializationFailed,
                ErrorCategory::Validation,
                msg,
                false,
            ),
            FfiError::InvalidParameter(msg) => ErrorDescriptor::new(
                ErrorCode::CoreConfigInvalid,
                ErrorCategory::Validation,
                msg,
                false,
            ),
            FfiError::NotInitialized
            | FfiError::AlreadyInitialized
            | FfiError::InvalidStateTransition { .. }
            | FfiError::ShuttingDown => ErrorDescriptor::new(
                ErrorCode::CoreConfigInvalid,
                ErrorCategory::Configuration,
                err.to_string(),
                false,
            ),
            FfiError::LockPoisoned(msg)
            | FfiError::PanicCaught(msg)
            | FfiError::Voice(msg)
            | FfiError::Model(msg) => ErrorDescriptor::new(
                ErrorCode::CoreInternalError,
                ErrorCategory::Internal,
                msg,
                false,
            ),
        }
    }
}

/// Convenience alias for bridge results.
pub type FfiResult<T> = Result<T, FfiError>;
