//! Offline bi-directional live voice interpreter mode and dialogue state manager.

use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use aissistant_llm::{Language, TranslationEngine};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::OrchestratorError;

/// Active speaker participant in a two-party interpretation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpreterSpeaker {
    PartyA,
    PartyB,
}

impl InterpreterSpeaker {
    /// String representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PartyA => "party_a",
            Self::PartyB => "party_b",
        }
    }

    /// Returns the opposite speaker participant.
    #[must_use]
    pub const fn other(&self) -> Self {
        match self {
            Self::PartyA => Self::PartyB,
            Self::PartyB => Self::PartyA,
        }
    }
}

impl fmt::Display for InterpreterSpeaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for InterpreterSpeaker {
    type Err = OrchestratorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "party_a" | "a" | "speaker_a" => Ok(Self::PartyA),
            "party_b" | "b" | "speaker_b" => Ok(Self::PartyB),
            other => Err(OrchestratorError::Internal(format!(
                "Invalid interpreter speaker: '{other}'"
            ))),
        }
    }
}

/// A recorded translation turn in an active interpreter session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpreterTurn {
    /// Monotonically increasing sequence number within the session.
    pub turn_id: u32,
    /// Participant who spoke this turn.
    pub speaker: InterpreterSpeaker,
    /// Original input utterance.
    pub source_text: String,
    /// Translated target utterance.
    pub translated_text: String,
    /// Spoken input language.
    pub source_lang: Language,
    /// Output translated language.
    pub target_lang: Language,
    /// Translation confidence score in `[0.0, 1.0]`.
    pub confidence: f32,
    /// Processing latency in milliseconds.
    pub latency_ms: u64,
    /// Unix timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Configuration settings for an interpreter dialogue session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpreterSessionConfig {
    /// Native language spoken by Party A.
    pub party_a_lang: Language,
    /// Native language spoken by Party B.
    pub party_b_lang: Language,
    /// Whether to automatically alternate speakers on turn completion.
    pub auto_swap_speaker: bool,
}

impl Default for InterpreterSessionConfig {
    fn default() -> Self {
        Self {
            party_a_lang: Language::English,
            party_b_lang: Language::Spanish,
            auto_swap_speaker: true,
        }
    }
}

/// Stateful bi-directional interpreter conversation manager.
#[derive(Debug)]
pub struct InterpreterSession {
    session_id: String,
    config: InterpreterSessionConfig,
    active_speaker: InterpreterSpeaker,
    turns: Vec<InterpreterTurn>,
    engine: TranslationEngine,
}

impl InterpreterSession {
    /// Creates a new interpretation session with the specified configuration.
    #[must_use]
    pub fn new(config: InterpreterSessionConfig) -> Self {
        Self {
            session_id: format!("interp_{}", Uuid::new_v4().simple()),
            config,
            active_speaker: InterpreterSpeaker::PartyA,
            turns: Vec::new(),
            engine: TranslationEngine::new(),
        }
    }

    /// Unique session identifier.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Current session configuration.
    #[must_use]
    pub fn config(&self) -> &InterpreterSessionConfig {
        &self.config
    }

    /// Currently active speaker awaiting speech or text input.
    #[must_use]
    pub fn active_speaker(&self) -> InterpreterSpeaker {
        self.active_speaker
    }

    /// Manually sets the active speaker.
    pub fn set_active_speaker(&mut self, speaker: InterpreterSpeaker) {
        self.active_speaker = speaker;
    }

    /// Inverts language assignments between Party A and Party B.
    pub fn swap_languages(&mut self) {
        std::mem::swap(&mut self.config.party_a_lang, &mut self.config.party_b_lang);
    }

    /// Slice of all completed turns in this session.
    #[must_use]
    pub fn turns(&self) -> &[InterpreterTurn] {
        &self.turns
    }

    /// Submits a new utterance using the currently designated active speaker.
    pub fn submit_turn(&mut self, text: &str) -> Result<InterpreterTurn, OrchestratorError> {
        let speaker = self.active_speaker;
        self.submit_turn_with_speaker(speaker, text)
    }

    /// Submits an utterance for an explicitly specified speaker.
    pub fn submit_turn_with_speaker(
        &mut self,
        speaker: InterpreterSpeaker,
        text: &str,
    ) -> Result<InterpreterTurn, OrchestratorError> {
        let (source_lang, target_lang) = match speaker {
            InterpreterSpeaker::PartyA => (self.config.party_a_lang, self.config.party_b_lang),
            InterpreterSpeaker::PartyB => (self.config.party_b_lang, self.config.party_a_lang),
        };

        let translation = self
            .engine
            .translate(text, source_lang, target_lang)
            .map_err(OrchestratorError::Llm)?;

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let turn = InterpreterTurn {
            turn_id: (self.turns.len() + 1) as u32,
            speaker,
            source_text: translation.source_text,
            translated_text: translation.translated_text,
            source_lang,
            target_lang,
            confidence: translation.confidence,
            latency_ms: translation.latency_ms,
            timestamp_ms: now_ms,
        };

        self.turns.push(turn.clone());

        if self.config.auto_swap_speaker {
            self.active_speaker = speaker.other();
        }

        Ok(turn)
    }

    /// Clears conversation history while keeping the session and language configuration intact.
    pub fn clear_history(&mut self) {
        self.turns.clear();
        self.active_speaker = InterpreterSpeaker::PartyA;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_speaker_toggle_and_parsing() {
        assert_eq!(
            InterpreterSpeaker::PartyA.other(),
            InterpreterSpeaker::PartyB
        );
        assert_eq!(
            InterpreterSpeaker::PartyB.other(),
            InterpreterSpeaker::PartyA
        );
        assert_eq!(InterpreterSpeaker::PartyA.as_str(), "party_a");
        assert_eq!(InterpreterSpeaker::PartyB.as_str(), "party_b");

        assert_eq!(
            "party_a".parse::<InterpreterSpeaker>().unwrap(),
            InterpreterSpeaker::PartyA
        );
        assert_eq!(
            "party_b".parse::<InterpreterSpeaker>().unwrap(),
            InterpreterSpeaker::PartyB
        );
        assert!("invalid_speaker".parse::<InterpreterSpeaker>().is_err());
    }

    #[test]
    fn test_interpreter_session_bi_directional_alternation() {
        let config = InterpreterSessionConfig {
            party_a_lang: Language::English,
            party_b_lang: Language::Spanish,
            auto_swap_speaker: true,
        };

        let mut session = InterpreterSession::new(config);
        assert_eq!(session.active_speaker(), InterpreterSpeaker::PartyA);
        assert!(session.turns().is_empty());

        // Turn 1: Party A speaks English -> translated to Spanish
        let turn1 = session.submit_turn("Hello").unwrap();
        assert_eq!(turn1.turn_id, 1);
        assert_eq!(turn1.speaker, InterpreterSpeaker::PartyA);
        assert_eq!(turn1.source_lang, Language::English);
        assert_eq!(turn1.target_lang, Language::Spanish);
        assert_eq!(turn1.translated_text, "Hola");

        // Auto-swap switched active speaker to Party B
        assert_eq!(session.active_speaker(), InterpreterSpeaker::PartyB);

        // Turn 2: Party B speaks Spanish -> translated to English
        let turn2 = session.submit_turn("Gracias").unwrap();
        assert_eq!(turn2.turn_id, 2);
        assert_eq!(turn2.speaker, InterpreterSpeaker::PartyB);
        assert_eq!(turn2.source_lang, Language::Spanish);
        assert_eq!(turn2.target_lang, Language::English);
        assert_eq!(turn2.translated_text, "Thank you");

        // Switched back to Party A
        assert_eq!(session.active_speaker(), InterpreterSpeaker::PartyA);
        assert_eq!(session.turns().len(), 2);
    }

    #[test]
    fn test_interpreter_session_swap_languages_and_clear() {
        let config = InterpreterSessionConfig {
            party_a_lang: Language::English,
            party_b_lang: Language::French,
            auto_swap_speaker: false,
        };

        let mut session = InterpreterSession::new(config);
        session.swap_languages();
        assert_eq!(session.config().party_a_lang, Language::French);
        assert_eq!(session.config().party_b_lang, Language::English);

        let turn = session.submit_turn("Bonjour").unwrap();
        assert_eq!(turn.source_lang, Language::French);
        assert_eq!(turn.target_lang, Language::English);
        assert_eq!(turn.translated_text, "Hello");

        // Clear history
        session.clear_history();
        assert!(session.turns().is_empty());
        assert_eq!(session.active_speaker(), InterpreterSpeaker::PartyA);
    }
}
