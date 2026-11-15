//! Execution sandboxing and timeout isolation for assistant skills.

use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

/// Default skill execution timeout ceiling (5,000 milliseconds).
pub const DEFAULT_SKILL_TIMEOUT_MS: u64 = 5000;

/// Sandboxed wrapper around any `ToolHandler` enforcing an execution timeout
/// and panic-trapping worker isolation.
pub struct SandboxedSkill {
    inner: Arc<dyn ToolHandler>,
    timeout: Duration,
}

impl std::fmt::Debug for SandboxedSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxedSkill")
            .field("name", &self.inner.name())
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl SandboxedSkill {
    /// Creates a new sandboxed skill wrapper with the specified timeout.
    #[must_use]
    pub fn new(inner: Arc<dyn ToolHandler>, timeout: Duration) -> Self {
        Self { inner, timeout }
    }

    /// Creates a new sandboxed skill wrapper with the default 5,000ms timeout.
    #[must_use]
    pub fn with_default_timeout(inner: Arc<dyn ToolHandler>) -> Self {
        Self::new(inner, Duration::from_millis(DEFAULT_SKILL_TIMEOUT_MS))
    }

    /// Returns the inner tool handler reference.
    #[must_use]
    pub fn inner(&self) -> &Arc<dyn ToolHandler> {
        &self.inner
    }

    /// Returns the execution timeout duration.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl ToolHandler for SandboxedSkill {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    fn risk_tier(&self) -> RiskTier {
        self.inner.risk_tier()
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let (tx, rx) = mpsc::channel();
        let tool = Arc::clone(&self.inner);
        let tool_name = tool.name().to_string();

        let thread_name = format!("skill-{}", tool_name);
        thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                let res = tool.execute(arguments);
                let _ = tx.send(res);
            })
            .map_err(|e| {
                OrchestratorError::Internal(format!("Failed to spawn skill worker thread: {e}"))
            })?;

        match rx.recv_timeout(self.timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(OrchestratorError::ToolExecutionFailed {
                tool_name: tool_name.clone(),
                reason: format!("Execution timed out after {}ms", self.timeout.as_millis()),
            }),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(OrchestratorError::ToolExecutionFailed {
                    tool_name,
                    reason: "Worker thread disconnected unexpectedly".to_string(),
                })
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    struct FastTool;
    impl ToolHandler for FastTool {
        fn name(&self) -> &str {
            "test.fast"
        }
        fn description(&self) -> &str {
            "Fast test tool"
        }
        fn parameters_schema(&self) -> Value {
            serde_json::json!({})
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::ReadOnly
        }
        fn execute(&self, _arguments: Value) -> Result<Value, OrchestratorError> {
            Ok(serde_json::json!({ "status": "ok" }))
        }
    }

    struct HangingTool;
    impl ToolHandler for HangingTool {
        fn name(&self) -> &str {
            "test.hanging"
        }
        fn description(&self) -> &str {
            "Simulates hanging task"
        }
        fn parameters_schema(&self) -> Value {
            serde_json::json!({})
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::ReadOnly
        }
        fn execute(&self, _arguments: Value) -> Result<Value, OrchestratorError> {
            thread::sleep(Duration::from_millis(500));
            Ok(serde_json::json!({ "status": "done" }))
        }
    }

    #[test]
    fn test_sandboxed_skill_fast_completion() {
        let sandboxed = SandboxedSkill::with_default_timeout(Arc::new(FastTool));
        let res = sandboxed.execute(serde_json::json!({})).unwrap();
        assert_eq!(res, serde_json::json!({ "status": "ok" }));
    }

    #[test]
    fn test_sandboxed_skill_timeout_enforcement() {
        let sandboxed = SandboxedSkill::new(Arc::new(HangingTool), Duration::from_millis(50));
        let err = sandboxed.execute(serde_json::json!({})).unwrap_err();
        assert!(matches!(err, OrchestratorError::ToolExecutionFailed { .. }));
        if let OrchestratorError::ToolExecutionFailed { reason, .. } = err {
            assert!(reason.contains("timed out after 50ms"));
        }
    }
}
