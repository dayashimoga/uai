//! Error taxonomy for desktop and mobile platform operations.

use aissistant_core::CoreError;
use thiserror::Error;

/// Platform operation and automation errors.
#[derive(Debug, Error)]
pub enum PlatformError {
    /// Process with the given PID was not found.
    #[error("Process not found for PID {pid}")]
    ProcessNotFound {
        /// Process identifier.
        pid: u32,
    },

    /// Attempted termination of a protected critical system process.
    #[error("Termination of critical system process '{name}' (PID {pid}) rejected")]
    CriticalProcessProtected {
        /// Process identifier.
        pid: u32,
        /// Executable or process name.
        name: String,
    },

    /// Window with the specified handle or ID was not found.
    #[error("Window not found for ID {window_id}")]
    WindowNotFound {
        /// Window identifier or handle.
        window_id: u64,
    },

    /// Application launch failed.
    #[error("Failed to launch application '{executable}': {reason}")]
    LaunchFailed {
        /// Executable name or path.
        executable: String,
        /// Reason description.
        reason: String,
    },

    /// Command injection or forbidden shell invocation rejected.
    #[error("Command injection rejected in '{command}': {reason}")]
    CommandInjectionRejected {
        /// The rejected command string.
        command: String,
        /// Reason for rejection.
        reason: String,
    },

    /// Operation not supported on current operating system or environment.
    #[error("Platform operation not supported on current host: {0}")]
    UnsupportedPlatform(String),

    /// Standard I/O operation error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// General internal platform error.
    #[error("Internal platform error: {0}")]
    Internal(String),
}

impl PlatformError {
    /// Maps this platform error into the canonical `CoreError` taxonomy.
    #[must_use]
    pub fn to_core_error(&self) -> CoreError {
        match self {
            Self::ProcessNotFound { pid } => CoreError::NotFound {
                entity: "Process",
                id: pid.to_string(),
            },
            Self::CriticalProcessProtected { pid, name } => CoreError::PermissionDenied {
                action: format!("kill_process:{pid}:{name}"),
                reason: "Protected critical system process".to_string(),
            },
            Self::WindowNotFound { window_id } => CoreError::NotFound {
                entity: "Window",
                id: window_id.to_string(),
            },
            Self::LaunchFailed { executable, reason } => CoreError::Internal {
                message: format!("Launch failed for '{executable}': {reason}"),
            },
            Self::CommandInjectionRejected { command, reason } => CoreError::PermissionDenied {
                action: format!("launch_app:{command}"),
                reason: format!("Security violation: {reason}"),
            },
            Self::UnsupportedPlatform(msg) => CoreError::Internal {
                message: format!("Unsupported platform: {msg}"),
            },
            Self::Io(err) => CoreError::Internal {
                message: format!("I/O error: {err}"),
            },
            Self::Internal(msg) => CoreError::Internal {
                message: msg.clone(),
            },
        }
    }
}

/// Convenience Result type for platform operations.
pub type Result<T> = std::result::Result<T, PlatformError>;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_core::ErrorCategory;

    #[test]
    fn test_platform_error_mapping() {
        let err = PlatformError::CriticalProcessProtected {
            pid: 4,
            name: "System".to_string(),
        };
        assert_eq!(err.to_core_error().category(), ErrorCategory::Security);

        let not_found = PlatformError::ProcessNotFound { pid: 999999 };
        assert_eq!(
            not_found.to_core_error().category(),
            ErrorCategory::NotFound
        );
    }
}
