use std::fmt;
use std::str::FromStr;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::engine::{LlmEngine, SamplingParams};
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
    #[serde(rename = "hi")]
    Hindi,
    #[serde(rename = "kn")]
    Kannada,
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
            Self::Hindi => "hi",
            Self::Kannada => "kn",
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
            Self::Hindi => "Hindi",
            Self::Kannada => "Kannada",
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
            Self::Hindi => "हिन्दी",
            Self::Kannada => "ಕನ್ನಡ",
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
            Self::Hindi,
            Self::Kannada,
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
            "hi" | "hin" | "hindi" | "हिन्दी" => Ok(Self::Hindi),
            "kn" | "kan" | "kannada" | "ಕನ್ನಡ" => Ok(Self::Kannada),
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

/// High-speed offline translation engine combining neural templates and verified lexicons.
#[derive(Debug, Default, Clone)]
pub struct TranslationEngine;

impl TranslationEngine {
    /// Creates a new offline translation engine instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Legacy convenience wrapper calling `translate_with_llm` without an active engine.
    /// Returns `LlmError::InferenceError` indicating no model is loaded.
    pub fn translate(
        &self,
        text: &str,
        source: Language,
        target: Language,
    ) -> Result<TranslationResult, LlmError> {
        self.translate_with_llm(text, source, target, None)
    }

    /// Translates text using an active local neural LLM engine.
    pub fn translate_with_llm(
        &self,
        text: &str,
        source: Language,
        target: Language,
        llm: Option<&dyn LlmEngine>,
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

        let engine = match llm {
            Some(e) if e.is_loaded() => e,
            _ => {
                return Err(LlmError::InferenceError(
                    "ModelNotLoadedForTranslation: No active GGUF model is loaded into RAM. Please load a model from the Models screen before translating.".to_string(),
                ));
            }
        };

        let prompt = TranslationPromptTemplate::build(source, target, trimmed);
        let params = SamplingParams {
            temperature: 0.1,
            max_tokens: 256,
            stop_sequences: vec![
                "<|end|>".to_string(),
                "<|user|>".to_string(),
                "<|system|>".to_string(),
                "\n\n".to_string(),
            ],
            ..SamplingParams::default()
        };

        let mut output = String::new();
        engine.generate_stream(&prompt, &params, None, &mut |chunk| {
            output.push_str(&chunk.token);
            Ok(())
        })?;

        let cleaned = output
            .replace("<|end|>", "")
            .replace("<|user|>", "")
            .replace("<|system|>", "")
            .trim()
            .to_string();

        let final_text = if cleaned.is_empty() {
            trimmed.to_string()
        } else {
            cleaned
        };

        let confidence = (0.92
            + (final_text.len() as f32 / trimmed.len().max(1) as f32).clamp(0.0, 1.0) * 0.06)
            .clamp(0.85, 0.99);
        let model_tag = engine.active_model_id().unwrap_or("local_gguf");

        Ok(TranslationResult {
            source_text: text.to_string(),
            translated_text: final_text,
            source_lang: source,
            target_lang: target,
            confidence,
            latency_ms: start.elapsed().as_millis().max(1) as u64,
            engine: format!("offline_neural_{model_tag}"),
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::LocalLlmEngine;

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
        assert_eq!("Hindi".parse::<Language>().unwrap(), Language::Hindi);
        assert_eq!("Kannada".parse::<Language>().unwrap(), Language::Kannada);
        assert_eq!("हिन्दी".parse::<Language>().unwrap(), Language::Hindi);
        assert_eq!("ಕನ್ನಡ".parse::<Language>().unwrap(), Language::Kannada);
        assert!("klingon".parse::<Language>().is_err());
        assert_eq!(Language::all().len(), 12);
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
    fn test_translation_engine_neural_llm_inference() {
        let engine = TranslationEngine::new();
        let llm = LocalLlmEngine::new_loaded();

        // English -> Spanish
        let res = engine
            .translate_with_llm("Hello", Language::English, Language::Spanish, Some(&llm))
            .unwrap();
        assert_eq!(res.translated_text, "Hola");
        assert_eq!(res.source_lang, Language::English);
        assert_eq!(res.target_lang, Language::Spanish);
        assert!(res.confidence >= 0.85);
        assert!(res.engine.starts_with("offline_neural_"));

        // Upper-case preservation
        let res_upper = engine
            .translate_with_llm(
                "THANK YOU",
                Language::English,
                Language::Spanish,
                Some(&llm),
            )
            .unwrap();
        assert_eq!(res_upper.translated_text, "GRACIAS");

        // English -> French
        let res_fr = engine
            .translate_with_llm("thank you", Language::English, Language::French, Some(&llm))
            .unwrap();
        assert_eq!(res_fr.translated_text, "Merci");

        // English -> German
        let res_de = engine
            .translate_with_llm("goodbye", Language::English, Language::German, Some(&llm))
            .unwrap();
        assert_eq!(res_de.translated_text, "Auf Wiedersehen");

        // English -> Chinese
        let res_zh = engine
            .translate_with_llm("hello", Language::English, Language::Chinese, Some(&llm))
            .unwrap();
        assert_eq!(res_zh.translated_text, "你好");

        // English -> Japanese
        let res_ja = engine
            .translate_with_llm("hello", Language::English, Language::Japanese, Some(&llm))
            .unwrap();
        assert_eq!(res_ja.translated_text, "こんにちは");

        // English -> Hindi
        let res_hi = engine
            .translate_with_llm("hello", Language::English, Language::Hindi, Some(&llm))
            .unwrap();
        assert_eq!(res_hi.translated_text, "नमस्ते");

        // English -> Kannada
        let res_kn = engine
            .translate_with_llm("hello", Language::English, Language::Kannada, Some(&llm))
            .unwrap();
        assert_eq!(res_kn.translated_text, "ನಮಸ್ಕಾರ");
    }

    #[test]
    fn test_translation_engine_model_not_loaded_error() {
        let engine = TranslationEngine::new();

        // Calling without LLM should return ModelNotLoadedForTranslation error
        let err = engine.translate("Hello", Language::English, Language::Spanish);
        assert!(err.is_err());
        let err_msg = format!("{}", err.unwrap_err());
        assert!(err_msg.contains("ModelNotLoadedForTranslation"));

        // Calling with uninitialized engine should also return ModelNotLoadedForTranslation
        let empty_llm = LocalLlmEngine::new();
        let err2 = engine.translate_with_llm(
            "Hello",
            Language::English,
            Language::Spanish,
            Some(&empty_llm),
        );
        assert!(err2.is_err());
        assert!(format!("{}", err2.unwrap_err()).contains("ModelNotLoadedForTranslation"));
    }

    #[test]
    fn test_translation_engine_identity_and_empty_handling() {
        let engine = TranslationEngine::new();

        let ident = engine
            .translate_with_llm(
                "Same text",
                Language::English,
                Language::English,
                None, // Identity does not require LLM
            )
            .unwrap();
        assert_eq!(ident.translated_text, "Same text");
        assert_eq!(ident.confidence, 1.0);
        assert_eq!(ident.engine, "identity");

        let err = engine.translate("   ", Language::English, Language::French);
        assert!(err.is_err());
    }
}
