//! Persona-aware prompt injection and customization for conversational orchestrator.

use aissistant_storage::{UserPersona, UserPreference};

/// Formatter that injects user persona directives and explicit preferences into system prompts.
#[derive(Debug, Clone, Copy, Default)]
pub struct PersonaPromptInjector;

impl PersonaPromptInjector {
    /// Injects active persona directives and explicit user preferences into a system prompt.
    #[must_use]
    pub fn inject_persona(
        base_prompt: &str,
        persona: Option<&UserPersona>,
        preferences: &[UserPreference],
    ) -> String {
        let mut prompt = String::with_capacity(base_prompt.len() + 512);

        // If persona provides an override, check if it replaces or prefixes/augments the base prompt.
        if let Some(p) = persona {
            if let Some(custom_prompt) = &p.system_prompt_override {
                if !custom_prompt.trim().is_empty() {
                    prompt.push_str(custom_prompt.trim());
                    prompt.push_str("\n\n");
                } else {
                    prompt.push_str(base_prompt.trim());
                    prompt.push_str("\n\n");
                }
            } else {
                prompt.push_str(base_prompt.trim());
                prompt.push_str("\n\n");
            }

            prompt.push_str("--- Persona & Style Directives ---\n");
            prompt.push_str(&format!("- Active Persona: {}\n", p.name));
            prompt.push_str(&format!(
                "- Tone Directive: {}\n",
                p.tone.system_instruction()
            ));
            prompt.push_str(&format!(
                "- Verbosity Directive: {}\n",
                p.verbosity.system_instruction()
            ));
        } else {
            prompt.push_str(base_prompt.trim());
            prompt.push_str("\n\n");
        }

        // Add explicit user preferences if any
        if !preferences.is_empty() {
            prompt.push_str("\n--- Explicit User Preferences ---\n");
            for pref in preferences {
                prompt.push_str(&format!(
                    "- [{}] {}: {}\n",
                    pref.category, pref.key, pref.value_json
                ));
            }
        }

        prompt.trim().to_string()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_storage::{PersonaTone, ResponseVerbosity};

    #[test]
    fn test_inject_persona_none_and_empty_prefs() {
        let base = "Base system instructions.";
        let res = PersonaPromptInjector::inject_persona(base, None, &[]);
        assert_eq!(res, base);
    }

    #[test]
    fn test_inject_persona_with_tone_and_verbosity() {
        let base = "Base assistant prompt.";
        let persona = UserPersona {
            id: "p1".to_string(),
            name: "Technical Expert".to_string(),
            tone: PersonaTone::Technical,
            verbosity: ResponseVerbosity::Concise,
            voice_id: "voice_1".to_string(),
            speaking_rate: 1.0,
            pitch: 1.0,
            system_prompt_override: None,
            is_active: true,
            created_at_ms: 0,
            updated_at_ms: 0,
        };

        let res = PersonaPromptInjector::inject_persona(base, Some(&persona), &[]);
        assert!(res.contains("Base assistant prompt."));
        assert!(res.contains("Active Persona: Technical Expert"));
        assert!(res.contains("rigorous, technically precise engineering tone"));
        assert!(res.contains("strictly to the point"));
    }

    #[test]
    fn test_inject_persona_with_override_and_preferences() {
        let base = "Base prompt to be overridden.";
        let persona = UserPersona {
            id: "p2".to_string(),
            name: "Custom Agent".to_string(),
            tone: PersonaTone::Casual,
            verbosity: ResponseVerbosity::Detailed,
            voice_id: "voice_2".to_string(),
            speaking_rate: 1.0,
            pitch: 1.0,
            system_prompt_override: Some("You are a specialized code tutor.".to_string()),
            is_active: true,
            created_at_ms: 0,
            updated_at_ms: 0,
        };

        let prefs = vec![
            UserPreference {
                key: "code.language".to_string(),
                value_json: "\"rust\"".to_string(),
                category: "coding".to_string(),
                updated_at_ms: 1000,
            },
            UserPreference {
                key: "editor.wrap".to_string(),
                value_json: "true".to_string(),
                category: "ui".to_string(),
                updated_at_ms: 1000,
            },
        ];

        let res = PersonaPromptInjector::inject_persona(base, Some(&persona), &prefs);
        assert!(res.contains("You are a specialized code tutor."));
        assert!(!res.contains("Base prompt to be overridden."));
        assert!(res.contains("warm, conversational, friendly"));
        assert!(res.contains("comprehensive, deeply explanatory"));
        assert!(res.contains("- [coding] code.language: \"rust\""));
        assert!(res.contains("- [ui] editor.wrap: true"));
    }
}
