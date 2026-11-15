//! Domain errors for SQLite persistence, migration, and storage operations.

use aissistant_core::CoreError;
use thiserror::Error;

/// Storage and database subsystem errors.
#[derive(Debug, Error)]
pub enum StorageError {
    /// SQLite engine or SQL syntax error.
    #[error("SQLite database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Schema migration failed or encountered invalid versioning.
    #[error("Schema migration error at version {version}: {message}")]
    Migration {
        /// Target schema version where failure occurred.
        version: u32,
        /// Migration failure reason.
        message: String,
    },

    /// Value serialization or deserialization failed.
    #[error("Storage serialization failed: {message}")]
    Serialization {
        /// Serializer error description.
        message: String,
    },

    /// Requested key or record not found.
    #[error("Storage key '{key}' was not found")]
    NotFound {
        /// Missing key name.
        key: String,
    },

    /// The cryptographic audit chain is broken or has been tampered with.
    #[error("Cryptographic audit log corrupted: {reason}")]
    CorruptAuditChain {
        /// Tampering details.
        reason: String,
    },

    /// Filesystem I/O error during database creation or WAL handling.
    #[error("Storage I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl StorageError {
    /// Maps this storage error into the canonical `CoreError` taxonomy.
    #[must_use]
    pub fn to_core_error(&self) -> CoreError {
        match self {
            Self::NotFound { key } => CoreError::NotFound {
                entity: "StorageRecord",
                id: key.clone(),
            },
            Self::CorruptAuditChain { reason } => CoreError::PermissionDenied {
                action: "storage.audit_integrity".to_string(),
                reason: reason.clone(),
            },
            Self::Serialization { message } => CoreError::Serialization {
                message: message.clone(),
            },
            Self::Migration { version, message } => CoreError::ConfigValidation {
                field: format!("schema_version_{version}"),
                reason: message.clone(),
            },
            Self::Sqlite(err) => CoreError::Internal {
                message: format!("SQLite error: {err}"),
            },
            Self::Io(err) => CoreError::Internal {
                message: format!("Storage I/O error: {err}"),
            },
        }
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization {
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_error_mapping() {
        let not_found = StorageError::NotFound {
            key: "pref.theme".to_string(),
        };
        let core_err = not_found.to_core_error();
        assert_eq!(
            core_err.category(),
            aissistant_core::ErrorCategory::NotFound
        );

        let corrupt = StorageError::CorruptAuditChain {
            reason: "digest mismatch".to_string(),
        };
        let core_corrupt = corrupt.to_core_error();
        assert_eq!(
            core_corrupt.category(),
            aissistant_core::ErrorCategory::Security
        );
    }
}
