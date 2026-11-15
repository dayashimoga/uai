use std::collections::HashSet;

use crate::error::{LlmError, Result};

/// A single GBNF grammar production rule: `name ::= expression`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbnfRule {
    pub name: String,
    pub expression: String,
}

impl GbnfRule {
    pub fn new(name: impl Into<String>, expression: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            expression: expression.into(),
        }
    }

    /// Formats the rule into GBNF syntax.
    pub fn format(&self) -> String {
        format!("{} ::= {}", self.name, self.expression)
    }
}

/// A complete GBNF grammar containing rules and utility methods for grammar-constrained decoding.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GbnfGrammar {
    rules: Vec<GbnfRule>,
}

impl GbnfGrammar {
    /// Creates a new empty grammar.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Creates a grammar pre-loaded with standard JSON terminal primitives.
    pub fn with_json_primitives() -> Self {
        let mut grammar = Self::new();
        grammar.add_primitive_rules();
        grammar
    }

    /// Adds standard JSON primitive rules to this grammar.
    pub fn add_primitive_rules(&mut self) {
        if !self.has_rule("ws") {
            self.add_rule("ws", "[ \\t\\n\\r]*");
        }
        if !self.has_rule("string") {
            self.add_rule(
                "string",
                "\"\\\"\" ([^\"\\\\] | \"\\\\\" ([\"\\\\/bfnrt] | \"u\" [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F]))* \"\\\"\" ws",
            );
        }
        if !self.has_rule("number") {
            self.add_rule(
                "number",
                "(\"-\"? ([0-9] | [1-9] [0-9]*)) (\".\" [0-9]+)? ([eE] [-+]? [0-9]+)? ws",
            );
        }
        if !self.has_rule("integer") {
            self.add_rule("integer", "(\"-\"? ([0-9] | [1-9] [0-9]*)) ws");
        }
        if !self.has_rule("boolean") {
            self.add_rule("boolean", "(\"true\" | \"false\") ws");
        }
        if !self.has_rule("null") {
            self.add_rule("null", "\"null\" ws");
        }
        if !self.has_rule("json_value") {
            self.add_rule(
                "json_value",
                "ws (string | number | boolean | null | json_array | json_object) ws",
            );
        }
        if !self.has_rule("json_array") {
            self.add_rule(
                "json_array",
                "\"[\" ws (json_value (\",\" ws json_value)*)? \"]\" ws",
            );
        }
        if !self.has_rule("json_member") {
            self.add_rule("json_member", "string \":\" ws json_value");
        }
        if !self.has_rule("json_object") {
            self.add_rule(
                "json_object",
                "\"{\" ws (json_member (\",\" ws json_member)*)? \"}\" ws",
            );
        }
    }

    /// Adds or replaces a rule in the grammar.
    pub fn add_rule(&mut self, name: impl Into<String>, expression: impl Into<String>) {
        let name_str = name.into();
        let expr_str = expression.into();

        if let Some(existing) = self.rules.iter_mut().find(|r| r.name == name_str) {
            existing.expression = expr_str;
        } else {
            self.rules.push(GbnfRule::new(name_str, expr_str));
        }
    }

    /// Sets or updates the entrypoint `root` rule.
    pub fn set_root(&mut self, expression: impl Into<String>) {
        self.add_rule("root", expression);
    }

    /// Checks if a rule with the given name exists.
    pub fn has_rule(&self, name: &str) -> bool {
        self.rules.iter().any(|r| r.name == name)
    }

    /// Retrieves a rule expression by name.
    pub fn get_rule(&self, name: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|r| r.name == name)
            .map(|r| r.expression.as_str())
    }

    /// Returns a slice of all rules.
    pub fn rules(&self) -> &[GbnfRule] {
        &self.rules
    }

    /// Formats the complete grammar into GBNF string format with `root` placed first.
    pub fn to_gbnf_string(&self) -> String {
        let mut output = String::new();

        // Ensure root is output first if present
        if let Some(root_rule) = self.rules.iter().find(|r| r.name == "root") {
            output.push_str(&root_rule.format());
            output.push('\n');
        }

        for rule in &self.rules {
            if rule.name != "root" {
                output.push_str(&rule.format());
                output.push('\n');
            }
        }

        output
    }

    /// Validates grammar integrity:
    /// 1. Ensures `root` rule exists.
    /// 2. Verifies that all rule names are non-empty and identifier-safe.
    pub fn validate(&self) -> Result<()> {
        if !self.has_rule("root") {
            return Err(LlmError::GrammarError(
                "Grammar must contain a 'root' production rule".to_string(),
            ));
        }

        let mut defined_names = HashSet::new();
        for rule in &self.rules {
            if rule.name.trim().is_empty() {
                return Err(LlmError::GrammarError(
                    "Grammar rule name cannot be empty".to_string(),
                ));
            }
            if !rule
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(LlmError::GrammarError(format!(
                    "Invalid rule name '{}': must be alphanumeric, hyphen, or underscore",
                    rule.name
                )));
            }
            defined_names.insert(rule.name.clone());
        }

        Ok(())
    }

    /// Pre-compiled grammar that enforces arbitrary valid JSON objects.
    pub fn arbitrary_json_object() -> Self {
        let mut grammar = Self::with_json_primitives();
        grammar.set_root("json_object");
        grammar
    }

    /// Pre-compiled grammar that enforces arbitrary valid JSON arrays.
    pub fn arbitrary_json_array() -> Self {
        let mut grammar = Self::with_json_primitives();
        grammar.set_root("json_array");
        grammar
    }

    /// Pre-compiled grammar that enforces strict boolean responses (`true` or `false`).
    pub fn boolean_response() -> Self {
        let mut grammar = Self::new();
        grammar.add_rule("ws", "[ \\t\\n\\r]*");
        grammar.set_root("(\"true\" | \"false\") ws");
        grammar
    }

    /// Escapes a literal string for inclusion in GBNF terminals.
    pub fn escape_literal(s: &str) -> String {
        let mut escaped = String::with_capacity(s.len() + 2);
        escaped.push('"');
        for c in s.chars() {
            match c {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                other => escaped.push(other),
            }
        }
        escaped.push('"');
        escaped
    }
}
