use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{LlmError, Result};

/// Declaration of a tool available for model tool-calling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Distinct identifier of the tool (e.g. `system_battery_level`, `file_read`).
    pub name: String,
    /// Human-readable purpose of the tool used in model system prompts.
    pub description: String,
    /// JSON Schema describing the structure and constraints of tool arguments.
    pub parameters: Value,
}

impl ToolDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

/// Invocation request parsed from model-generated structured output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Target tool name.
    pub name: String,
    /// Arguments payload formatted according to the tool's parameter schema.
    pub arguments: Value,
}

impl ToolCall {
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            name: name.into(),
            arguments,
        }
    }
}

/// Validator verifying that model-generated JSON tool calls strictly adhere to registered tool schemas.
#[derive(Debug, Default)]
pub struct ToolCallValidator;

impl ToolCallValidator {
    /// Parses a JSON output string and validates the invocation against available tool definitions.
    pub fn parse_and_validate(raw_output: &str, tools: &[ToolDefinition]) -> Result<ToolCall> {
        let trimmed = raw_output.trim();
        let value: Value = serde_json::from_str(trimmed).map_err(|e| {
            LlmError::GrammarError(format!("Malformed JSON output from model: {e}"))
        })?;

        let obj = value.as_object().ok_or_else(|| {
            LlmError::GrammarError(
                "Model output must be a JSON object with 'name' and 'arguments'".to_string(),
            )
        })?;

        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                LlmError::GrammarError("Missing or non-string 'name' in tool call".to_string())
            })?
            .to_string();

        let arguments = obj.get("arguments").cloned().unwrap_or(Value::Null);

        // Find matching tool
        let tool = tools
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| LlmError::GrammarError(format!("Unknown tool called: '{name}'")))?;

        // Validate arguments against tool parameter schema
        Self::validate_schema(&arguments, &tool.parameters)?;

        Ok(ToolCall::new(name, arguments))
    }

    /// Recursively validates a JSON value against a JSON Schema.
    pub fn validate_schema(value: &Value, schema: &Value) -> Result<()> {
        // Validate enum if present
        if let Some(enum_vals) = schema.get("enum").and_then(Value::as_array) {
            if !enum_vals.contains(value) {
                return Err(LlmError::GrammarError(format!(
                    "Value '{value}' does not match allowed enum values: {enum_vals:?}"
                )));
            }
        }

        // Validate type
        if let Some(expected_type) = schema.get("type").and_then(Value::as_str) {
            match expected_type {
                "string" => {
                    if !value.is_string() {
                        return Err(LlmError::GrammarError(format!(
                            "Expected string, found {value:?}"
                        )));
                    }
                }
                "integer" => {
                    if !value.is_i64() && !value.is_u64() {
                        return Err(LlmError::GrammarError(format!(
                            "Expected integer, found {value:?}"
                        )));
                    }
                }
                "number" => {
                    if !value.is_number() {
                        return Err(LlmError::GrammarError(format!(
                            "Expected number, found {value:?}"
                        )));
                    }
                }
                "boolean" => {
                    if !value.is_boolean() {
                        return Err(LlmError::GrammarError(format!(
                            "Expected boolean, found {value:?}"
                        )));
                    }
                }
                "null" => {
                    if !value.is_null() {
                        return Err(LlmError::GrammarError(format!(
                            "Expected null, found {value:?}"
                        )));
                    }
                }
                "array" => {
                    let arr = value.as_array().ok_or_else(|| {
                        LlmError::GrammarError(format!("Expected array, found {value:?}"))
                    })?;

                    if let Some(item_schema) = schema.get("items") {
                        for item in arr {
                            Self::validate_schema(item, item_schema)?;
                        }
                    }
                }
                "object" => {
                    let obj = value.as_object().ok_or_else(|| {
                        LlmError::GrammarError(format!("Expected object, found {value:?}"))
                    })?;

                    // Check required properties
                    if let Some(required) = schema.get("required").and_then(Value::as_array) {
                        for req_key in required.iter().filter_map(Value::as_str) {
                            if !obj.contains_key(req_key) {
                                return Err(LlmError::GrammarError(format!(
                                    "Missing required property '{req_key}'"
                                )));
                            }
                        }
                    }

                    // Check properties
                    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                        for (key, val) in obj {
                            if let Some(prop_schema) = properties.get(key) {
                                Self::validate_schema(val, prop_schema)?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}
