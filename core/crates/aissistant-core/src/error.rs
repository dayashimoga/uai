//! Canonical error taxonomy and structured error descriptors.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// High-level categories for domain errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Validation error on user or model input.
    Validation,
    /// Configuration parameter or boundary error.
    Configuration,
    /// Permission, authentication, or security violation.
    Security,
    /// Entity or resource not found.
    NotFound,
    /// Resource ceiling or capacity reached.
    ResourceExhausted,
    /// Persistent storage or serialization error.
    Storage,
    /// Network connection or peer communication error.
    Network,
    /// Internal unexpected runtime error.
    Internal,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation => write!(f, "validation"),
            Self::Configuration => write!(f, "configuration"),
            Self::Security => write!(f, "security"),
            Self::NotFound => write!(f, "not_found"),
            Self::ResourceExhausted => write!(f, "resource_exhausted"),
            Self::Storage => write!(f, "storage"),
            Self::Network => write!(f, "network"),
            Self::Internal => write!(f, "internal"),
        }
    }
}

/// Stable string error codes suitable for telemetry, API responses, and FFI serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    /// Malformed or invalid entity identifier.
    CoreInvalidId,
    /// Configuration parameter failed validation checks.
    CoreConfigInvalid,
    /// Serialization or deserialization error.
    CoreSerializationFailed,
    /// Requested entity could not be found.
    CoreNotFound,
    /// Action rejected by security policy.
    CorePermissionDenied,
    /// Resource limit reached.
    CoreResourceExhausted,
    /// Unhandled internal error.
    CoreInternalError,
}

impl ErrorCode {
    /// Returns the static string representation of the error code.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::CoreInvalidId => "CORE_INVALID_ID",
            Self::CoreConfigInvalid => "CORE_CONFIG_INVALID",
            Self::CoreSerializationFailed => "CORE_SERIALIZATION_FAILED",
            Self::CoreNotFound => "CORE_NOT_FOUND",
            Self::CorePermissionDenied => "CORE_PERMISSION_DENIED",
            Self::CoreResourceExhausted => "CORE_RESOURCE_EXHAUSTED",
            Self::CoreInternalError => "CORE_INTERNAL_ERROR",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Structured, FFI-safe and UI-consumable error descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorDescriptor {
    /// Stable error code.
    pub code: ErrorCode,
    /// High-level category.
    pub category: ErrorCategory,
    /// Human-readable explanation.
    pub message: String,
    /// Whether repeating the action may succeed without modification.
    pub retryable: bool,
    /// Optional structured metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorDescriptor {
    /// Creates a new error descriptor.
    #[must_use]
    pub fn new(
        code: ErrorCode,
        category: ErrorCategory,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            code,
            category,
            message: message.into(),
            retryable,
            details: None,
        }
    }

    /// Attaches structured context details to the error descriptor.
    #[must_use]
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Core domain error taxonomy for the Universal AI Assistant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    /// An identifier was malformed or violated structural rules.
    #[error("Invalid {entity} ID '{value}': {reason}")]
    InvalidId {
        /// Type of entity (e.g. Device, Skill, Model).
        entity: &'static str,
        /// The invalid raw value provided.
        value: String,
        /// Explanation of validation failure.
        reason: String,
    },

    /// A configuration option was invalid or out of allowed bounds.
    #[error("Invalid configuration for field '{field}': {reason}")]
    ConfigValidation {
        /// The config field key.
        field: String,
        /// Reason for rejection.
        reason: String,
    },

    /// JSON serialization or deserialization failure.
    #[error("Serialization failed: {message}")]
    Serialization {
        /// Serializer error message.
        message: String,
    },

    /// Requested resource or entity does not exist.
    #[error("{entity} with ID '{id}' was not found")]
    NotFound {
        /// Entity category.
        entity: &'static str,
        /// Requested entity ID.
        id: String,
    },

    /// Action was rejected by the policy engine or missing OS permission.
    #[error("Permission denied for action '{action}': {reason}")]
    PermissionDenied {
        /// Attempted action or skill ID.
        action: String,
        /// Reason for denial.
        reason: String,
    },

    /// Memory, thread, or storage limit exceeded.
    #[error("Resource '{resource}' exhausted: {details}")]
    ResourceExhausted {
        /// Resource name.
        resource: &'static str,
        /// Exhaustion details.
        details: String,
    },

    /// Internal unexpected core error.
    #[error("Internal core error: {message}")]
    Internal {
        /// Internal error description.
        message: String,
    },
}

impl CoreError {
    /// Map this error to a stable `ErrorCode`.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidId { .. } => ErrorCode::CoreInvalidId,
            Self::ConfigValidation { .. } => ErrorCode::CoreConfigInvalid,
            Self::Serialization { .. } => ErrorCode::CoreSerializationFailed,
            Self::NotFound { .. } => ErrorCode::CoreNotFound,
            Self::PermissionDenied { .. } => ErrorCode::CorePermissionDenied,
            Self::ResourceExhausted { .. } => ErrorCode::CoreResourceExhausted,
            Self::Internal { .. } => ErrorCode::CoreInternalError,
        }
    }

    /// Map this error to a high-level `ErrorCategory`.
    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        match self {
            Self::InvalidId { .. } | Self::Serialization { .. } => ErrorCategory::Validation,
            Self::ConfigValidation { .. } => ErrorCategory::Configuration,
            Self::NotFound { .. } => ErrorCategory::NotFound,
            Self::PermissionDenied { .. } => ErrorCategory::Security,
            Self::ResourceExhausted { .. } => ErrorCategory::ResourceExhausted,
            Self::Internal { .. } => ErrorCategory::Internal,
        }
    }

    /// Determine whether the operation that produced this error is retryable without modification.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::ResourceExhausted { .. } => true,
            Self::InvalidId { .. }
            | Self::ConfigValidation { .. }
            | Self::Serialization { .. }
            | Self::NotFound { .. }
            | Self::PermissionDenied { .. }
            | Self::Internal { .. } => false,
        }
    }

    /// Converts this error into a serializable `ErrorDescriptor`.
    #[must_use]
    pub fn to_descriptor(&self) -> ErrorDescriptor {
        let details = match self {
            Self::InvalidId {
                entity,
                value,
                reason,
            } => Some(serde_json::json!({
                "entity": entity,
                "value": value,
                "reason": reason,
            })),
            Self::ConfigValidation { field, reason } => Some(serde_json::json!({
                "field": field,
                "reason": reason,
            })),
            Self::NotFound { entity, id } => Some(serde_json::json!({
                "entity": entity,
                "id": id,
            })),
            Self::PermissionDenied { action, reason } => Some(serde_json::json!({
                "action": action,
                "reason": reason,
            })),
            Self::ResourceExhausted { resource, details } => Some(serde_json::json!({
                "resource": resource,
                "details": details,
            })),
            Self::Serialization { .. } | Self::Internal { .. } => None,
        };

        let mut descriptor = ErrorDescriptor::new(
            self.code(),
            self.category(),
            self.to_string(),
            self.is_retryable(),
        );
        descriptor.details = details;
        descriptor
    }
}

impl From<serde_json::Error> for CoreError {
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
    fn test_error_code_strings() {
        assert_eq!(ErrorCode::CoreInvalidId.as_str(), "CORE_INVALID_ID");
        assert_eq!(
            format!("{}", ErrorCode::CoreConfigInvalid),
            "CORE_CONFIG_INVALID"
        );
        assert_eq!(ErrorCode::CoreNotFound.as_str(), "CORE_NOT_FOUND");
        assert_eq!(
            ErrorCode::CorePermissionDenied.as_str(),
            "CORE_PERMISSION_DENIED"
        );
        assert_eq!(ErrorCode::CoreInternalError.as_str(), "CORE_INTERNAL_ERROR");
    }

    #[test]
    fn test_error_category_display() {
        assert_eq!(format!("{}", ErrorCategory::Validation), "validation");
        assert_eq!(format!("{}", ErrorCategory::Security), "security");
        assert_eq!(format!("{}", ErrorCategory::Storage), "storage");
        assert_eq!(format!("{}", ErrorCategory::Network), "network");
        assert_eq!(format!("{}", ErrorCategory::Internal), "internal");
        assert_eq!(
            format!("{}", ErrorCategory::ResourceExhausted),
            "resource_exhausted"
        );
        assert_eq!(format!("{}", ErrorCategory::NotFound), "not_found");
        assert_eq!(format!("{}", ErrorCategory::Configuration), "configuration");
    }

    #[test]
    fn test_core_error_properties() {
        let err = CoreError::InvalidId {
            entity: "Device",
            value: "bad!".to_string(),
            reason: "illegal character".to_string(),
        };
        assert_eq!(err.code(), ErrorCode::CoreInvalidId);
        assert_eq!(err.category(), ErrorCategory::Validation);
        assert!(!err.is_retryable());

        let descriptor = err.to_descriptor();
        assert_eq!(descriptor.code, ErrorCode::CoreInvalidId);
        assert!(descriptor.details.is_some());
    }

    #[test]
    fn test_not_found_error_properties() {
        let err = CoreError::NotFound {
            entity: "Model",
            id: "missing-model".to_string(),
        };
        assert_eq!(err.code(), ErrorCode::CoreNotFound);
        assert_eq!(err.category(), ErrorCategory::NotFound);
        assert!(!err.is_retryable());
        let desc = err.to_descriptor();
        assert!(desc.details.is_some());
    }

    #[test]
    fn test_permission_denied_properties() {
        let err = CoreError::PermissionDenied {
            action: "file.delete".to_string(),
            reason: "user confirmation rejected".to_string(),
        };
        assert_eq!(err.code(), ErrorCode::CorePermissionDenied);
        assert_eq!(err.category(), ErrorCategory::Security);
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_internal_error_properties() {
        let err = CoreError::Internal {
            message: "unexpected state".to_string(),
        };
        assert_eq!(err.code(), ErrorCode::CoreInternalError);
        assert_eq!(err.category(), ErrorCategory::Internal);
        assert!(!err.is_retryable());
        let desc = err.to_descriptor();
        assert!(desc.details.is_none());
    }

    #[test]
    fn test_config_validation_error_properties() {
        let err = CoreError::ConfigValidation {
            field: "voice.sample_rate".to_string(),
            reason: "unsupported rate".to_string(),
        };
        assert_eq!(err.code(), ErrorCode::CoreConfigInvalid);
        assert_eq!(err.category(), ErrorCategory::Configuration);
        assert!(!err.is_retryable());
        let desc = err.to_descriptor();
        assert!(desc.details.is_some());
    }

    #[test]
    fn test_retryable_error() {
        let err = CoreError::ResourceExhausted {
            resource: "memory",
            details: "model allocation failed".to_string(),
        };
        assert!(err.is_retryable());
        assert_eq!(err.category(), ErrorCategory::ResourceExhausted);
    }

    #[test]
    fn test_serde_json_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("{bad_json").unwrap_err();
        let core_err: CoreError = json_err.into();
        assert_eq!(core_err.code(), ErrorCode::CoreSerializationFailed);
    }
}
