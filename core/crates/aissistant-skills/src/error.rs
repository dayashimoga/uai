//! Domain error taxonomy for assistant skills.

use aissistant_core::CoreError;
use aissistant_orchestrator::OrchestratorError;
use thiserror::Error;

/// Domain errors occurring within skill execution, validation, and parsing.
#[derive(Debug, Error)]
pub enum SkillError {
    /// Skill execution exceeded its permitted timeout.
    #[error("Skill '{skill_name}' timed out after {timeout_ms}ms")]
    ExecutionTimeout {
        /// Namespaced skill name.
        skill_name: String,
        /// Maximum allowed execution duration in milliseconds.
        timeout_ms: u64,
    },

    /// Missing or invalid argument payload supplied to skill.
    #[error("Invalid arguments for skill '{skill_name}': {reason}")]
    InvalidArguments {
        /// Namespaced skill name.
        skill_name: String,
        /// Reason description.
        reason: String,
    },

    /// Mathematical expression evaluation error.
    #[error("Math evaluation error in expression '{expression}': {reason}")]
    EvaluationError {
        /// Expression that failed to evaluate.
        expression: String,
        /// Specific evaluation failure reason.
        reason: String,
    },

    /// Attempted conversion between incompatible unit dimensions.
    #[error("Incompatible unit conversion from '{from}' to '{to}'")]
    IncompatibleUnits {
        /// Source unit identifier.
        from: String,
        /// Target unit identifier.
        to: String,
    },

    /// Unrecognized unit of measurement.
    #[error("Unknown unit: '{0}'")]
    UnknownUnit(String),

    /// Core domain validation error.
    #[error("Core error: {0}")]
    Core(#[from] aissistant_core::CoreError),

    /// Orchestrator error.
    #[error("Orchestrator error: {0}")]
    Orchestrator(#[from] aissistant_orchestrator::OrchestratorError),

    /// JSON serialization or deserialization failure.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// General internal skill failure.
    #[error("Internal skill error: {0}")]
    Internal(String),
}

impl SkillError {
    /// Maps this skill error into the canonical `CoreError` taxonomy.
    #[must_use]
    pub fn to_core_error(&self) -> CoreError {
        match self {
            Self::Core(err) => err.clone(),
            Self::ExecutionTimeout {
                skill_name,
                timeout_ms,
            } => CoreError::Internal {
                message: format!("Skill '{skill_name}' timed out after {timeout_ms}ms"),
            },
            Self::InvalidArguments { skill_name, reason } => CoreError::ConfigValidation {
                field: format!("skill_args:{skill_name}"),
                reason: reason.clone(),
            },
            Self::EvaluationError { expression, reason } => CoreError::ConfigValidation {
                field: format!("calc_expr:{expression}"),
                reason: reason.clone(),
            },
            Self::IncompatibleUnits { from, to } => CoreError::ConfigValidation {
                field: "unit_conversion".to_string(),
                reason: format!("Incompatible units: {from} -> {to}"),
            },
            Self::UnknownUnit(unit) => CoreError::NotFound {
                entity: "Unit",
                id: unit.clone(),
            },
            Self::Orchestrator(err) => err.to_core_error(),
            Self::Serialization(err) => CoreError::Serialization {
                message: err.to_string(),
            },
            Self::Internal(msg) => CoreError::Internal {
                message: msg.clone(),
            },
        }
    }

    /// Maps this skill error into an `OrchestratorError` for the tool loop.
    #[must_use]
    pub fn to_orchestrator_error(&self) -> OrchestratorError {
        match self {
            Self::ExecutionTimeout {
                skill_name,
                timeout_ms,
            } => OrchestratorError::ToolExecutionFailed {
                tool_name: skill_name.clone(),
                reason: format!("Execution timed out after {timeout_ms}ms"),
            },
            Self::InvalidArguments { skill_name, reason } => {
                OrchestratorError::InvalidToolArguments {
                    tool_name: skill_name.clone(),
                    reason: reason.clone(),
                }
            }
            Self::EvaluationError {
                expression: _,
                reason,
            } => OrchestratorError::ToolExecutionFailed {
                tool_name: "system.calc".to_string(),
                reason: reason.clone(),
            },
            Self::IncompatibleUnits { from, to } => OrchestratorError::ToolExecutionFailed {
                tool_name: "system.convert".to_string(),
                reason: format!("Incompatible units: {from} -> {to}"),
            },
            Self::UnknownUnit(unit) => OrchestratorError::ToolExecutionFailed {
                tool_name: "system.convert".to_string(),
                reason: format!("Unknown unit: {unit}"),
            },
            Self::Orchestrator(err) => match err {
                OrchestratorError::InvalidToolArguments { tool_name, reason } => {
                    OrchestratorError::InvalidToolArguments {
                        tool_name: tool_name.clone(),
                        reason: reason.clone(),
                    }
                }
                other => OrchestratorError::Internal(other.to_string()),
            },
            other => OrchestratorError::Internal(other.to_string()),
        }
    }
}

/// Convenience type alias for results returning `SkillError`.
pub type Result<T> = std::result::Result<T, SkillError>;

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_core::ErrorCategory;

    #[test]
    fn test_skill_error_conversions() {
        let err = SkillError::ExecutionTimeout {
            skill_name: "system.test".to_string(),
            timeout_ms: 5000,
        };
        assert_eq!(err.to_core_error().category(), ErrorCategory::Internal);

        let orch_err = err.to_orchestrator_error();
        assert!(matches!(
            orch_err,
            OrchestratorError::ToolExecutionFailed { .. }
        ));

        let arg_err = SkillError::InvalidArguments {
            skill_name: "system.calc".to_string(),
            reason: "Missing expr".to_string(),
        };
        assert_eq!(
            arg_err.to_core_error().category(),
            ErrorCategory::Configuration
        );
    }
}
