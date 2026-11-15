use serde::{Deserialize, Serialize};

/// Conversation session tracking multi-turn context, token consumption, and prefix hashing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmSession {
    pub session_id: String,
    pub context_window_limit: usize,
    pub history_turns: Vec<(String, String)>,
    pub prefix_hash: u64,
}

impl LlmSession {
    /// Creates a new conversation session with a defined context ceiling.
    pub fn new(session_id: impl Into<String>, context_window_limit: usize) -> Self {
        Self {
            session_id: session_id.into(),
            context_window_limit,
            history_turns: Vec::new(),
            prefix_hash: 0,
        }
    }

    /// Appends a user or assistant turn to the conversation history.
    pub fn add_turn(&mut self, role: impl Into<String>, content: impl Into<String>) {
        let role_str = role.into();
        let content_str = content.into();

        // Update incremental prefix hash (simple 64-bit hash combining turn content)
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.prefix_hash.hash(&mut hasher);
        role_str.hash(&mut hasher);
        content_str.hash(&mut hasher);
        self.prefix_hash = hasher.finish();

        self.history_turns.push((role_str, content_str));
    }

    /// Clears conversation history and resets prefix cache tracking.
    pub fn clear(&mut self) {
        self.history_turns.clear();
        self.prefix_hash = 0;
    }

    /// Formats the conversation history into standard ChatML template format.
    ///
    /// ChatML syntax: `<|im_start|>role\ncontent<|im_end|>\n`
    pub fn format_chatml_prompt(&self, system_prompt: Option<&str>) -> String {
        let mut prompt = String::new();

        if let Some(sys) = system_prompt {
            prompt.push_str("<|im_start|>system\n");
            prompt.push_str(sys);
            prompt.push_str("<|im_end|>\n");
        }

        for (role, content) in &self.history_turns {
            prompt.push_str(&format!("<|im_start|>{role}\n{content}<|im_end|>\n"));
        }

        prompt.push_str("<|im_start|>assistant\n");
        prompt
    }

    /// Formats the conversation history into Llama 3 template format.
    ///
    /// Syntax: `<|start_header_id|>role<|end_header_id|>\n\ncontent<|eot_id|>`
    pub fn format_llama3_prompt(&self, system_prompt: Option<&str>) -> String {
        let mut prompt = String::from("<|begin_of_text|>");

        if let Some(sys) = system_prompt {
            prompt.push_str(&format!(
                "<|start_header_id|>system<|end_header_id|>\n\n{sys}<|eot_id|>"
            ));
        }

        for (role, content) in &self.history_turns {
            prompt.push_str(&format!(
                "<|start_header_id|>{role}<|end_header_id|>\n\n{content}<|eot_id|>"
            ));
        }

        prompt.push_str("<|start_header_id|>assistant<|end_header_id|>\n\n");
        prompt
    }

    /// Estimates token consumption across conversation turns (~4 bytes per token).
    pub fn estimate_token_count(&self) -> usize {
        let total_chars: usize = self
            .history_turns
            .iter()
            .map(|(r, c)| r.len() + c.len() + 16)
            .sum();
        total_chars / 4
    }

    /// Checks if conversation history exceeds the configured context window limit.
    pub fn is_context_exceeded(&self) -> bool {
        self.estimate_token_count() >= self.context_window_limit
    }
}
