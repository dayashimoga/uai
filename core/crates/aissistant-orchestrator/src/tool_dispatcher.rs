//! Policy-enforced tool dispatcher, tool registration, and parameter sanitization.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use aissistant_core::SkillId;
use aissistant_llm::{ToolCall, ToolCallValidator, ToolDefinition};
use aissistant_security::RiskTier;
use serde_json::Value;

use crate::error::{OrchestratorError, Result};

/// Trait defining executable assistant tool or skill handlers.
pub trait ToolHandler: Send + Sync {
    /// Distinct identifier of the tool (must be namespaced e.g. `file.read`).
    fn name(&self) -> &str;

    /// Human-readable description of tool capabilities for prompt assembly.
    fn description(&self) -> &str;

    /// JSON Schema describing tool parameters.
    fn parameters_schema(&self) -> Value;

    /// Assessed risk tier determining security policy enforcement.
    fn risk_tier(&self) -> RiskTier;

    /// Executes the tool with the validated argument payload.
    fn execute(&self, arguments: Value) -> Result<Value>;
}

/// Redacts sensitive keys (passwords, tokens, secrets, credentials, keys) from JSON payloads.
#[must_use]
pub fn redact_sensitive_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (k, v) in map {
                let lower = k.to_ascii_lowercase();
                if lower.contains("password")
                    || lower.contains("secret")
                    || lower.contains("token")
                    || lower.contains("key")
                    || lower.contains("credential")
                    || lower.contains("auth")
                {
                    redacted.insert(k.clone(), Value::String("[REDACTED]".to_string()));
                } else {
                    redacted.insert(k.clone(), redact_sensitive_json(v));
                }
            }
            Value::Object(redacted)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_sensitive_json).collect()),
        other => other.clone(),
    }
}

/// Central registry and dispatcher for assistant tools and skills.
#[derive(Default)]
pub struct ToolDispatcher {
    tools: RwLock<HashMap<String, Arc<dyn ToolHandler>>>,
}

impl std::fmt::Debug for ToolDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tools_guard = self.tools.read().ok();
        let count = tools_guard.as_ref().map_or(0, |g| g.len());
        f.debug_struct("ToolDispatcher")
            .field("registered_tools_count", &count)
            .finish()
    }
}

impl ToolDispatcher {
    /// Creates a new empty tool dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a tool handler with the dispatcher.
    pub fn register(&self, tool: Arc<dyn ToolHandler>) -> Result<()> {
        let name = tool.name();
        // Validate namespaced skill identifier conformance
        let _ = SkillId::new(name)?;

        let mut map = self
            .tools
            .write()
            .map_err(|_| OrchestratorError::Internal("ToolDispatcher lock poisoned".to_string()))?;
        map.insert(name.to_string(), tool);
        Ok(())
    }

    /// Retrieves a tool handler by name if registered.
    pub fn get(&self, name: &str) -> Result<Option<Arc<dyn ToolHandler>>> {
        let map = self
            .tools
            .read()
            .map_err(|_| OrchestratorError::Internal("ToolDispatcher lock poisoned".to_string()))?;
        Ok(map.get(name).cloned())
    }

    /// Returns definitions of all registered tools for prompt and GBNF grammar generation.
    pub fn tool_definitions(&self) -> Result<Vec<ToolDefinition>> {
        let map = self
            .tools
            .read()
            .map_err(|_| OrchestratorError::Internal("ToolDispatcher lock poisoned".to_string()))?;
        let mut defs: Vec<ToolDefinition> = map
            .values()
            .map(|t| ToolDefinition::new(t.name(), t.description(), t.parameters_schema()))
            .collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(defs)
    }

    /// Validates a parsed tool call against registered schema and returns the target handler.
    pub fn validate_call(&self, call: &ToolCall) -> Result<Arc<dyn ToolHandler>> {
        let tool = self
            .get(&call.name)?
            .ok_or_else(|| OrchestratorError::ToolNotFound(call.name.clone()))?;

        ToolCallValidator::validate_schema(&call.arguments, &tool.parameters_schema()).map_err(
            |err| OrchestratorError::InvalidToolArguments {
                tool_name: call.name.clone(),
                reason: err.to_string(),
            },
        )?;

        Ok(tool)
    }

    /// Executes a tool call directly by name and argument payload.
    pub fn execute(&self, name: &str, arguments: Value) -> Result<Value> {
        let tool = self
            .get(name)?
            .ok_or_else(|| OrchestratorError::ToolNotFound(name.to_string()))?;

        ToolCallValidator::validate_schema(&arguments, &tool.parameters_schema()).map_err(
            |err| OrchestratorError::InvalidToolArguments {
                tool_name: name.to_string(),
                reason: err.to_string(),
            },
        )?;

        tool.execute(arguments)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    struct EchoTool;
    impl ToolHandler for EchoTool {
        fn name(&self) -> &str {
            "test.echo"
        }
        fn description(&self) -> &str {
            "Echoes back input payload"
        }
        fn parameters_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
                "required": ["message"]
            })
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::ReadOnly
        }
        fn execute(&self, arguments: Value) -> Result<Value> {
            Ok(arguments)
        }
    }

    #[test]
    fn test_tool_dispatcher_registration_and_validation() {
        let dispatcher = ToolDispatcher::new();
        dispatcher.register(Arc::new(EchoTool)).unwrap();

        let defs = dispatcher.tool_definitions().unwrap();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, "test.echo");

        // Valid call
        let valid_call = ToolCall::new("test.echo", serde_json::json!({"message": "hello"}));
        let handler = dispatcher.validate_call(&valid_call).unwrap();
        assert_eq!(handler.name(), "test.echo");

        let result = dispatcher
            .execute("test.echo", serde_json::json!({"message": "world"}))
            .unwrap();
        assert_eq!(result, serde_json::json!({"message": "world"}));

        // Invalid args
        let invalid_call = ToolCall::new("test.echo", serde_json::json!({"msg": 123}));
        assert!(dispatcher.validate_call(&invalid_call).is_err());

        // Unknown tool
        let unknown_call = ToolCall::new("unknown.tool", serde_json::json!({}));
        assert!(dispatcher.validate_call(&unknown_call).is_err());
    }

    #[test]
    fn test_redact_sensitive_json() {
        let sensitive = serde_json::json!({
            "username": "alice",
            "password": "secret_password_123",
            "api_key": "sk-abc-xyz",
            "token": "bearer 123",
            "nested": {
                "auth_header": "Basic xyz",
                "safe_field": 42
            }
        });

        let redacted = redact_sensitive_json(&sensitive);
        assert_eq!(redacted["username"], "alice");
        assert_eq!(redacted["password"], "[REDACTED]");
        assert_eq!(redacted["api_key"], "[REDACTED]");
        assert_eq!(redacted["token"], "[REDACTED]");
        assert_eq!(redacted["nested"]["auth_header"], "[REDACTED]");
        assert_eq!(redacted["nested"]["safe_field"], 42);
    }
}
