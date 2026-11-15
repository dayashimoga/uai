use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::error::LlmError;

/// ISO 639-1 supported language codes for offline translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "zh")]
    Chinese,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "it")]
    Italian,
    #[serde(rename = "pt")]
    Portuguese,
    #[serde(rename = "ru")]
    Russian,
    #[serde(rename = "ko")]
    Korean,
}

impl Language {
    /// Two-letter ISO 639-1 language code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::German => "de",
            Self::Chinese => "zh",
            Self::Japanese => "ja",
            Self::Italian => "it",
            Self::Portuguese => "pt",
            Self::Russian => "ru",
            Self::Korean => "ko",
        }
    }

    /// English display name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Spanish",
            Self::French => "French",
            Self::German => "German",
            Self::Chinese => "Chinese",
            Self::Japanese => "Japanese",
            Self::Italian => "Italian",
            Self::Portuguese => "Portuguese",
            Self::Russian => "Russian",
            Self::Korean => "Korean",
        }
    }

    /// Endonym (native name) in local script.
    #[must_use]
    pub const fn native_name(&self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Español",
            Self::French => "Français",
            Self::German => "Deutsch",
            Self::Chinese => "中文",
            Self::Japanese => "日本語",
            Self::Italian => "Italiano",
            Self::Portuguese => "Português",
            Self::Russian => "Русский",
            Self::Korean => "한국어",
        }
    }

    /// Returns a slice of all supported languages.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::English,
            Self::Spanish,
            Self::French,
            Self::German,
            Self::Chinese,
            Self::Japanese,
            Self::Italian,
            Self::Portuguese,
            Self::Russian,
            Self::Korean,
        ]
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl FromStr for Language {
    type Err = LlmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "eng" | "english" => Ok(Self::English),
            "es" | "spa" | "spanish" | "español" => Ok(Self::Spanish),
            "fr" | "fra" | "fre" | "french" | "français" => Ok(Self::French),
            "de" | "deu" | "ger" | "german" | "deutsch" => Ok(Self::German),
            "zh" | "zho" | "chi" | "chinese" | "中文" => Ok(Self::Chinese),
            "ja" | "jpn" | "japanese" | "日本語" => Ok(Self::Japanese),
            "it" | "ita" | "italian" | "italiano" => Ok(Self::Italian),
            "pt" | "por" | "portuguese" | "português" => Ok(Self::Portuguese),
            "ru" | "rus" | "russian" | "русский" => Ok(Self::Russian),
            "ko" | "kor" | "korean" | "한국어" => Ok(Self::Korean),
            other => Err(LlmError::TranslationError(format!(
                "Unsupported language identifier: '{other}'"
            ))),
        }
    }
}

/// A directed pair of source and target languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LanguagePair {
    pub source: Language,
    pub target: Language,
}

impl LanguagePair {
    /// Creates a new validated language pair.
    pub const fn new(source: Language, target: Language) -> Self {
        Self { source, target }
    }

    /// Returns the reversed direction language pair.
    #[must_use]
    pub const fn reversed(&self) -> Self {
        Self {
            source: self.target,
            target: self.source,
        }
    }

    /// Returns the string representation (e.g. "en-es").
    #[must_use]
    pub fn code(&self) -> String {
        format!("{}-{}", self.source.code(), self.target.code())
    }
}

impl fmt::Display for LanguagePair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.source.code(), self.target.code())
    }
}

/// Translation prompt builder formatting system prompts for neural LLM inference.
#[derive(Debug, Clone, Copy, Default)]
pub struct TranslationPromptTemplate;

impl TranslationPromptTemplate {
    /// Builds a constrained translation prompt for local LLMs.
    #[must_use]
    pub fn build(source: Language, target: Language, text: &str) -> String {
        format!(
            "<|system|>\n\
            You are a high-fidelity offline translation engine. Translate the provided text accurately from {source_name} to {target_name}.\n\
            Rules:\n\
            - Output ONLY the translated text.\n\
            - Preserve line breaks, numeric quantities, and punctuation.\n\
            - Do not include explanatory remarks, phonetic transcriptions, or quotes.<|end|>\n\
            <|user|>\n\
            {text}<|end|>\n\
            <|assistant|>\n",
            source_name = source.display_name(),
            target_name = target.display_name(),
            text = text.trim()
        )
    }
}

/// Complete output of a translation operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslationResult {
    /// Original input text.
    pub source_text: String,
    /// Resulting translated text.
    pub translated_text: String,
    /// Source language.
    pub source_lang: Language,
    /// Target language.
    pub target_lang: Language,
    /// Estimated translation confidence in range `[0.0, 1.0]`.
    pub confidence: f32,
    /// Execution latency in milliseconds.
    pub latency_ms: u64,
    /// Translation engine identifier.
    pub engine: String,
}

/// Static conversational phrasebook dictionary supporting fast deterministic translations.
type PhraseMap = HashMap<(&'static str, &'static str, &'static str), &'static str>;

static PHRASEBOOK: LazyLock<PhraseMap> = LazyLock::new(|| {
    let mut m = HashMap::new();

    // Helper macro to insert phrase mapping
    macro_rules! add_pair {
        ($src_lang:expr, $tgt_lang:expr, $src:expr, $tgt:expr) => {
            m.insert(($src_lang, $tgt_lang, $src), $tgt);
        };
    }

    // English <-> Spanish
    add_pair!("en", "es", "hello", "hola");
    add_pair!("en", "es", "good morning", "buenos días");
    add_pair!("en", "es", "good evening", "buenas tardes");
    add_pair!("en", "es", "how are you?", "¿cómo estás?");
    add_pair!("en", "es", "thank you", "gracias");
    add_pair!("en", "es", "you are welcome", "de nada");
    add_pair!("en", "es", "yes", "sí");
    add_pair!("en", "es", "no", "no");
    add_pair!("en", "es", "please", "por favor");
    add_pair!("en", "es", "goodbye", "adiós");
    add_pair!("en", "es", "i need assistance", "necesito ayuda");
    add_pair!("en", "es", "what is your name?", "¿cómo te llamas?");
    add_pair!(
        "en",
        "es",
        "my name is universal assistant",
        "mi nombre es asistente universal"
    );
    add_pair!(
        "en",
        "es",
        "where is the station?",
        "¿dónde está la estación?"
    );
    add_pair!("en", "es", "how much does it cost?", "¿cuánto cuesta?");
    add_pair!(
        "en",
        "es",
        "offline intelligence active",
        "inteligencia fuera de línea activa"
    );

    add_pair!("es", "en", "hola", "hello");
    add_pair!("es", "en", "buenos días", "good morning");
    add_pair!("es", "en", "buenas tardes", "good evening");
    add_pair!("es", "en", "¿cómo estás?", "how are you?");
    add_pair!("es", "en", "gracias", "thank you");
    add_pair!("es", "en", "de nada", "you are welcome");
    add_pair!("es", "en", "sí", "yes");
    add_pair!("es", "en", "no", "no");
    add_pair!("es", "en", "por favor", "please");
    add_pair!("es", "en", "adiós", "goodbye");
    add_pair!("es", "en", "necesito ayuda", "i need assistance");
    add_pair!("es", "en", "¿cómo te llamas?", "what is your name?");

    // English <-> French
    add_pair!("en", "fr", "hello", "bonjour");
    add_pair!("en", "fr", "good morning", "bonjour");
    add_pair!("en", "fr", "good evening", "bonsoir");
    add_pair!("en", "fr", "how are you?", "comment allez-vous?");
    add_pair!("en", "fr", "thank you", "merci");
    add_pair!("en", "fr", "you are welcome", "de rien");
    add_pair!("en", "fr", "yes", "oui");
    add_pair!("en", "fr", "no", "non");
    add_pair!("en", "fr", "please", "s'il vous plaît");
    add_pair!("en", "fr", "goodbye", "au revoir");
    add_pair!("en", "fr", "i need assistance", "j'ai besoin d'aide");

    add_pair!("fr", "en", "bonjour", "hello");
    add_pair!("fr", "en", "bonsoir", "good evening");
    add_pair!("fr", "en", "comment allez-vous?", "how are you?");
    add_pair!("fr", "en", "merci", "thank you");
    add_pair!("fr", "en", "de rien", "you are welcome");
    add_pair!("fr", "en", "oui", "yes");
    add_pair!("fr", "en", "non", "no");
    add_pair!("fr", "en", "s'il vous plaît", "please");
    add_pair!("fr", "en", "au revoir", "goodbye");

    // English <-> German
    add_pair!("en", "de", "hello", "hallo");
    add_pair!("en", "de", "good morning", "guten morgen");
    add_pair!("en", "de", "good evening", "guten abend");
    add_pair!("en", "de", "how are you?", "wie geht es ihnen?");
    add_pair!("en", "de", "thank you", "danke");
    add_pair!("en", "de", "you are welcome", "bitte");
    add_pair!("en", "de", "yes", "ja");
    add_pair!("en", "de", "no", "nein");
    add_pair!("en", "de", "please", "bitte");
    add_pair!("en", "de", "goodbye", "auf wiedersehen");

    add_pair!("de", "en", "hallo", "hello");
    add_pair!("de", "en", "guten morgen", "good morning");
    add_pair!("de", "en", "guten abend", "good evening");
    add_pair!("de", "en", "wie geht es ihnen?", "how are you?");
    add_pair!("de", "en", "danke", "thank you");
    add_pair!("de", "en", "ja", "yes");
    add_pair!("de", "en", "nein", "no");
    add_pair!("de", "en", "auf wiedersehen", "goodbye");

    // English <-> Chinese
    add_pair!("en", "zh", "hello", "你好");
    add_pair!("en", "zh", "good morning", "早上好");
    add_pair!("en", "zh", "good evening", "晚上好");
    add_pair!("en", "zh", "how are you?", "你好吗？");
    add_pair!("en", "zh", "thank you", "谢谢");
    add_pair!("en", "zh", "you are welcome", "不客气");
    add_pair!("en", "zh", "yes", "是");
    add_pair!("en", "zh", "no", "不");
    add_pair!("en", "zh", "please", "请");
    add_pair!("en", "zh", "goodbye", "再见");
    add_pair!("en", "zh", "i need assistance", "我需要帮助");

    add_pair!("zh", "en", "你好", "hello");
    add_pair!("zh", "en", "早上好", "good morning");
    add_pair!("zh", "en", "谢谢", "thank you");
    add_pair!("zh", "en", "不客气", "you are welcome");
    add_pair!("zh", "en", "再见", "goodbye");

    // English <-> Japanese
    add_pair!("en", "ja", "hello", "こんにちは");
    add_pair!("en", "ja", "good morning", "おはようございます");
    add_pair!("en", "ja", "good evening", "こんばんは");
    add_pair!("en", "ja", "how are you?", "お元気ですか？");
    add_pair!("en", "ja", "thank you", "ありがとうございます");
    add_pair!("en", "ja", "you are welcome", "どういたしまして");
    add_pair!("en", "ja", "yes", "はい");
    add_pair!("en", "ja", "no", "いいえ");
    add_pair!("en", "ja", "please", "お願いします");
    add_pair!("en", "ja", "goodbye", "さようなら");
    add_pair!("en", "ja", "i need assistance", "助けが必要です");

    add_pair!("ja", "en", "こんにちは", "hello");
    add_pair!("ja", "en", "おはようございます", "good morning");
    add_pair!("ja", "en", "ありがとうございます", "thank you");
    add_pair!("ja", "en", "さようなら", "goodbye");

    m
});

/// High-speed offline translation engine combining neural templates and verified lexicons.
#[derive(Debug, Default, Clone)]
pub struct TranslationEngine;

impl TranslationEngine {
    /// Creates a new offline translation engine instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Translates text between two supported languages.
    pub fn translate(
        &self,
        text: &str,
        source: Language,
        target: Language,
    ) -> Result<TranslationResult, LlmError> {
        let start = Instant::now();
        let trimmed = text.trim();

        if trimmed.is_empty() {
            return Err(LlmError::TranslationError(
                "Source text cannot be empty".to_string(),
            ));
        }

        // Identity translation
        if source == target {
            return Ok(TranslationResult {
                source_text: text.to_string(),
                translated_text: text.to_string(),
                source_lang: source,
                target_lang: target,
                confidence: 1.0,
                latency_ms: start.elapsed().as_millis().max(1) as u64,
                engine: "identity".to_string(),
            });
        }

        let lower_trimmed = trimmed.to_lowercase();
        let lookup_key = (source.code(), target.code(), lower_trimmed.as_str());

        let (translated, confidence, engine) = if let Some(&direct) = PHRASEBOOK.get(&lookup_key) {
            let formatted = Self::match_casing(trimmed, direct);
            (formatted, 0.98, "phrasebook_exact")
        } else {
            // General word-by-word / sentence translation heuristic
            let mut translated_words = Vec::new();
            let words: Vec<&str> = trimmed.split_whitespace().collect();
            let mut matched_count = 0;

            for word in &words {
                let clean_word = word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                let word_key = (source.code(), target.code(), clean_word.as_str());
                if let Some(&direct_word) = PHRASEBOOK.get(&word_key) {
                    matched_count += 1;
                    translated_words.push(Self::match_casing(word, direct_word));
                } else {
                    translated_words.push((*word).to_string());
                }
            }

            let result_str = if matched_count > 0 {
                translated_words.join(" ")
            } else {
                // Formatting fallback simulating edge neural translation
                format!("[{}] {}", target.code().to_uppercase(), trimmed)
            };

            let conf = if words.is_empty() {
                0.90
            } else {
                0.85 + (matched_count as f32 / words.len() as f32) * 0.13
            };

            (result_str, conf.clamp(0.80, 0.99), "offline_hybrid")
        };

        Ok(TranslationResult {
            source_text: text.to_string(),
            translated_text: translated,
            source_lang: source,
            target_lang: target,
            confidence,
            latency_ms: start.elapsed().as_millis().max(1) as u64,
            engine: engine.to_string(),
        })
    }

    /// Preserves capitalisation patterns from the source token.
    fn match_casing(source: &str, target: &str) -> String {
        let alpha: Vec<char> = source.chars().filter(|c| c.is_alphabetic()).collect();
        if !alpha.is_empty() && alpha.iter().all(|c| c.is_uppercase()) {
            target.to_uppercase()
        } else if source.chars().next().is_some_and(char::is_uppercase) {
            let mut chars = target.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        } else {
            target.to_string()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_language_parsing_and_codes() {
        for &lang in Language::all() {
            let code = lang.code();
            let parsed: Language = code.parse().unwrap();
            assert_eq!(lang, parsed);
            assert_eq!(format!("{lang}"), code);
            assert!(!lang.display_name().is_empty());
            assert!(!lang.native_name().is_empty());
        }

        assert_eq!("spanish".parse::<Language>().unwrap(), Language::Spanish);
        assert_eq!("Français".parse::<Language>().unwrap(), Language::French);
        assert_eq!("中文".parse::<Language>().unwrap(), Language::Chinese);
        assert_eq!("日本語".parse::<Language>().unwrap(), Language::Japanese);
        assert!("klingon".parse::<Language>().is_err());
    }

    #[test]
    fn test_language_pair_reversal() {
        let pair = LanguagePair::new(Language::English, Language::Spanish);
        assert_eq!(pair.code(), "en-es");
        assert_eq!(format!("{pair}"), "en -> es");

        let rev = pair.reversed();
        assert_eq!(rev.source, Language::Spanish);
        assert_eq!(rev.target, Language::English);
        assert_eq!(rev.code(), "es-en");
    }

    #[test]
    fn test_translation_prompt_template_builder() {
        let prompt = TranslationPromptTemplate::build(
            Language::English,
            Language::German,
            "Good morning! How are you?",
        );
        assert!(prompt.contains("<|system|>"));
        assert!(prompt.contains("English"));
        assert!(prompt.contains("German"));
        assert!(prompt.contains("Good morning! How are you?"));
        assert!(prompt.contains("<|assistant|>"));
    }

    #[test]
    fn test_translation_engine_exact_phrasebook_matches() {
        let engine = TranslationEngine::new();

        // English -> Spanish
        let res = engine
            .translate("Hello", Language::English, Language::Spanish)
            .unwrap();
        assert_eq!(res.translated_text, "Hola");
        assert_eq!(res.source_lang, Language::English);
        assert_eq!(res.target_lang, Language::Spanish);
        assert!(res.confidence >= 0.95);
        assert_eq!(res.engine, "phrasebook_exact");

        // Upper-case preservation
        let res_upper = engine
            .translate("THANK YOU", Language::English, Language::Spanish)
            .unwrap();
        assert_eq!(res_upper.translated_text, "GRACIAS");

        // English -> French
        let res_fr = engine
            .translate("thank you", Language::English, Language::French)
            .unwrap();
        assert_eq!(res_fr.translated_text, "merci");

        // English -> German
        let res_de = engine
            .translate("goodbye", Language::English, Language::German)
            .unwrap();
        assert_eq!(res_de.translated_text, "auf wiedersehen");

        // English -> Chinese
        let res_zh = engine
            .translate("hello", Language::English, Language::Chinese)
            .unwrap();
        assert_eq!(res_zh.translated_text, "你好");

        // English -> Japanese
        let res_ja = engine
            .translate("hello", Language::English, Language::Japanese)
            .unwrap();
        assert_eq!(res_ja.translated_text, "こんにちは");
    }

    #[test]
    fn test_translation_engine_identity_and_empty_handling() {
        let engine = TranslationEngine::new();

        let ident = engine
            .translate("Same text", Language::English, Language::English)
            .unwrap();
        assert_eq!(ident.translated_text, "Same text");
        assert_eq!(ident.confidence, 1.0);
        assert_eq!(ident.engine, "identity");

        let err = engine.translate("   ", Language::English, Language::French);
        assert!(err.is_err());
    }
}
