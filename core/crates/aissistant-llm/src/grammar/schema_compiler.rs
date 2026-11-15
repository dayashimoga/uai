use serde_json::Value;

use super::gbnf::GbnfGrammar;
use super::tool_call::ToolDefinition;
use crate::error::{LlmError, Result};

/// Compiler translating JSON Schema specifications into GBNF production rules.
#[derive(Debug, Default)]
pub struct SchemaCompiler {
    rule_counter: usize,
}

impl SchemaCompiler {
    pub fn new() -> Self {
        Self { rule_counter: 0 }
    }

    /// Compiles a standard JSON schema into a GBNF grammar.
    pub fn compile_schema(&mut self, schema: &Value) -> Result<GbnfGrammar> {
        let mut grammar = GbnfGrammar::with_json_primitives();
        self.rule_counter = 0;

        let root_rule_name = self.compile_subschema(schema, &mut grammar, "schema_root")?;
        grammar.set_root(format!("ws {root_rule_name} ws"));
        grammar.validate()?;
        Ok(grammar)
    }

    /// Compiles a single tool definition into a constrained tool call grammar.
    ///
    /// Generates strict JSON: `{"name": "<tool_name>", "arguments": <schema>}`
    pub fn compile_tool(&mut self, tool: &ToolDefinition) -> Result<GbnfGrammar> {
        let mut grammar = GbnfGrammar::with_json_primitives();
        self.rule_counter = 0;

        let args_rule_name = format!("{}_args", tool.name.replace('-', "_"));
        let compiled_args =
            self.compile_subschema(&tool.parameters, &mut grammar, &args_rule_name)?;

        let tool_name_literal = GbnfGrammar::escape_literal(&tool.name);
        let tool_call_expr = format!(
            "\"{{\" ws \"\\\"name\\\": \" {} \",\" ws \"\\\"arguments\\\": \" ws {} \"}}\" ws",
            tool_name_literal, compiled_args
        );

        grammar.set_root(tool_call_expr);
        grammar.validate()?;
        Ok(grammar)
    }

    /// Compiles a list of tool definitions into a branching multi-tool grammar.
    ///
    /// If `allow_conversational_fallback` is true, the grammar allows standard text replies.
    pub fn compile_tools(
        &mut self,
        tools: &[ToolDefinition],
        allow_conversational_fallback: bool,
    ) -> Result<GbnfGrammar> {
        if tools.is_empty() && !allow_conversational_fallback {
            return Err(LlmError::GrammarError(
                "Cannot compile empty tools list without conversational fallback".to_string(),
            ));
        }

        let mut grammar = GbnfGrammar::with_json_primitives();
        self.rule_counter = 0;

        let mut tool_rule_names = Vec::new();

        for tool in tools {
            let sanitized_name = tool.name.replace('-', "_");
            let args_rule_name = format!("{sanitized_name}_args");
            let tool_rule_name = format!("tool_{sanitized_name}");

            let compiled_args =
                self.compile_subschema(&tool.parameters, &mut grammar, &args_rule_name)?;

            let tool_name_escaped = GbnfGrammar::escape_literal(&tool.name);
            let tool_expr = format!(
                "\"{{\" ws \"\\\"name\\\": \" {} \",\" ws \"\\\"arguments\\\": \" ws {} \"}}\" ws",
                tool_name_escaped, compiled_args
            );

            grammar.add_rule(&tool_rule_name, tool_expr);
            tool_rule_names.push(tool_rule_name);
        }

        if allow_conversational_fallback {
            // Conversational fallback matches plain text that does not start with JSON opening brace
            grammar.add_rule("conversational_text", "[^{\\t\\n\\r] [^\\t\\n\\r]*");
            tool_rule_names.push("conversational_text".to_string());
        }

        let root_expr = format!("ws ({}) ws", tool_rule_names.join(" | "));
        grammar.set_root(root_expr);
        grammar.validate()?;
        Ok(grammar)
    }

    fn compile_subschema(
        &mut self,
        schema: &Value,
        grammar: &mut GbnfGrammar,
        suggested_name: &str,
    ) -> Result<String> {
        let rule_name = if grammar.has_rule(suggested_name) {
            self.rule_counter += 1;
            format!("{}_{}", suggested_name, self.rule_counter)
        } else {
            suggested_name.to_string()
        };

        // If schema has "enum"
        if let Some(enum_vals) = schema.get("enum").and_then(Value::as_array) {
            if enum_vals.is_empty() {
                return Err(LlmError::GrammarError("Empty enum in schema".to_string()));
            }

            let mut literals = Vec::new();
            for val in enum_vals {
                match val {
                    Value::String(s) => literals.push(GbnfGrammar::escape_literal(s)),
                    Value::Number(n) => literals.push(format!("\"{n}\"")),
                    Value::Bool(b) => literals.push(format!("\"{b}\"")),
                    _ => {
                        return Err(LlmError::GrammarError(
                            "Only primitive enum values are supported in GBNF schema".to_string(),
                        ))
                    }
                }
            }

            let expr = format!("({}) ws", literals.join(" | "));
            grammar.add_rule(&rule_name, expr);
            return Ok(rule_name);
        }

        // Check type
        let schema_type = schema
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("object");

        match schema_type {
            "string" => Ok("string".to_string()),
            "integer" => Ok("integer".to_string()),
            "number" => Ok("number".to_string()),
            "boolean" => Ok("boolean".to_string()),
            "null" => Ok("null".to_string()),
            "array" => {
                let items_rule = if let Some(items) = schema.get("items") {
                    let item_suggested = format!("{rule_name}_item");
                    self.compile_subschema(items, grammar, &item_suggested)?
                } else {
                    "json_value".to_string()
                };

                let expr = format!("\"[\" ws ({items_rule} (\",\" ws {items_rule})*)? \"]\" ws");
                grammar.add_rule(&rule_name, expr);
                Ok(rule_name)
            }
            "object" => {
                let properties = schema.get("properties").and_then(Value::as_object);
                let required_keys: Vec<&str> = schema
                    .get("required")
                    .and_then(Value::as_array)
                    .map(|arr| arr.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();

                if let Some(props) = properties {
                    if props.is_empty() {
                        let expr = "\"{\" ws \"}\" ws";
                        grammar.add_rule(&rule_name, expr);
                        return Ok(rule_name);
                    }

                    // Compile each property's subschema
                    let mut prop_rules = Vec::new();
                    for (prop_name, prop_schema) in props {
                        let prop_val_name = format!("{rule_name}_{prop_name}");
                        let compiled_val =
                            self.compile_subschema(prop_schema, grammar, &prop_val_name)?;
                        let prop_key_literal = GbnfGrammar::escape_literal(prop_name);

                        let kv_expr = format!("{prop_key_literal} \":\" ws {compiled_val}");
                        let kv_rule_name = format!("{rule_name}_kv_{prop_name}");
                        grammar.add_rule(&kv_rule_name, kv_expr);

                        let is_required = required_keys.contains(&prop_name.as_str());
                        prop_rules.push((kv_rule_name, is_required));
                    }

                    // For deterministic tool arguments, required properties must be present
                    // If all are required:
                    let all_required = prop_rules.iter().all(|(_, req)| *req);
                    let expr = if all_required {
                        let joined = prop_rules
                            .iter()
                            .map(|(r, _)| r.as_str())
                            .collect::<Vec<_>>()
                            .join(" \",\" ws ");
                        format!("\"{{\" ws {joined} \"}}\" ws")
                    } else {
                        // Support optional properties: each property can optionally appear
                        let options = prop_rules
                            .iter()
                            .map(|(r, req)| if *req { r.clone() } else { format!("({r})?") })
                            .collect::<Vec<_>>()
                            .join(" (\",\" ws ");
                        let closing = ")?".repeat(prop_rules.len().saturating_sub(1));
                        format!("\"{{\" ws {options}{closing} \"}}\" ws")
                    };

                    grammar.add_rule(&rule_name, expr);
                    Ok(rule_name)
                } else {
                    Ok("json_object".to_string())
                }
            }
            _ => Ok("json_value".to_string()),
        }
    }
}
