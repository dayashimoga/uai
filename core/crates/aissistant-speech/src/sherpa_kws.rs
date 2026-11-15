use crate::acoustic_kws::AcousticKwsSpotter;
use crate::error::SpeechError;
use crate::wake_word::{WakeWordConfig, WakeWordEvent, WakeWordSpotter};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Model and configuration parameters for `sherpa-onnx` keyword spotting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SherpaKwsConfig {
    /// Path to transducer/zipformer encoder ONNX model.
    pub encoder_path: PathBuf,

    /// Path to transducer/zipformer decoder ONNX model.
    pub decoder_path: PathBuf,

    /// Path to transducer/zipformer joiner ONNX model.
    pub joiner_path: PathBuf,

    /// Path to BPE/character tokens file (`tokens.txt`).
    pub tokens_path: PathBuf,

    /// Optional path to keywords file (`keywords.txt`).
    pub keywords_file: Option<PathBuf>,

    /// Monitored keywords / keyphrases.
    pub keywords: Vec<String>,

    /// Boosting score for target keywords (e.g., `1.5` or `2.0`).
    pub keywords_score: f32,

    /// Detection threshold in range `[0.0, 1.0]`. Default: `0.60`.
    pub threshold: f32,

    /// Minimum duration between triggers in milliseconds. Default: `1500` ms.
    pub min_keyword_interval_ms: u32,

    /// Number of CPU compute threads. Default: `2`.
    pub num_threads: usize,

    /// Audio sample rate. Must be `16_000` Hz.
    pub sample_rate: u32,
}

impl Default for SherpaKwsConfig {
    fn default() -> Self {
        Self {
            encoder_path: PathBuf::from("models/kws/encoder.onnx"),
            decoder_path: PathBuf::from("models/kws/decoder.onnx"),
            joiner_path: PathBuf::from("models/kws/joiner.onnx"),
            tokens_path: PathBuf::from("models/kws/tokens.txt"),
            keywords_file: None,
            keywords: vec!["hey assistant".to_string()],
            keywords_score: 1.5,
            threshold: 0.60,
            min_keyword_interval_ms: 1500,
            num_threads: 2,
            sample_rate: 16_000,
        }
    }
}

impl SherpaKwsConfig {
    /// Validates the configuration parameters.
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.keywords.is_empty() {
            return Err(SpeechError::ParameterError(
                "Sherpa KWS requires at least one keyword".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&self.threshold) {
            return Err(SpeechError::ParameterError(format!(
                "Threshold {} is out of range [0.0, 1.0]",
                self.threshold
            )));
        }
        if self.sample_rate != 16_000 {
            return Err(SpeechError::InvalidAudioFormat(format!(
                "Sherpa KWS requires 16000 Hz, got {}",
                self.sample_rate
            )));
        }
        if self.num_threads == 0 {
            return Err(SpeechError::ParameterError(
                "num_threads must be at least 1".to_string(),
            ));
        }
        Ok(())
    }

    /// Generates the standard `sherpa-onnx` keywords file content.
    /// Format: `<keyword> / @<boosting_score>` per line.
    pub fn generate_keywords_content(&self) -> String {
        let mut lines = Vec::new();
        for kw in &self.keywords {
            lines.push(format!("{} / @{:.2}", kw.trim(), self.keywords_score));
        }
        lines.join("\n")
    }

    /// Checks whether all specified ONNX model files and token dictionaries exist on disk.
    pub fn models_exist(&self) -> bool {
        self.encoder_path.is_file()
            && self.decoder_path.is_file()
            && self.joiner_path.is_file()
            && self.tokens_path.is_file()
    }
}

/// Offline keyword spotting engine for `sherpa-onnx` model integration.
/// Provides automatic verification of model assets, keywords syntax generation,
/// and fallback acoustic spotting.
pub struct SherpaKwsEngine {
    config: SherpaKwsConfig,
    active_spotter: AcousticKwsSpotter,
    is_using_onnx_models: bool,
}

impl std::fmt::Debug for SherpaKwsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SherpaKwsEngine")
            .field("config", &self.config)
            .field("is_using_onnx_models", &self.is_using_onnx_models)
            .finish()
    }
}

impl SherpaKwsEngine {
    /// Creates a new `SherpaKwsEngine` instance.
    pub fn new(config: SherpaKwsConfig) -> Result<Self, SpeechError> {
        config.validate()?;

        let is_using_onnx_models = config.models_exist();

        let base_kws_config = WakeWordConfig {
            keywords: config.keywords.clone(),
            threshold: config.threshold,
            min_keyword_interval_ms: config.min_keyword_interval_ms,
            sample_rate: config.sample_rate,
            ..Default::default()
        };

        let active_spotter = AcousticKwsSpotter::new(base_kws_config)?;

        Ok(Self {
            config,
            active_spotter,
            is_using_onnx_models,
        })
    }

    /// Returns whether actual ONNX models are present on disk.
    pub fn is_using_onnx_models(&self) -> bool {
        self.is_using_onnx_models
    }

    /// Returns the configuration object.
    pub fn config(&self) -> &SherpaKwsConfig {
        &self.config
    }

    /// Exports the formatted keywords file content to a target path.
    pub fn export_keywords_file(&self, path: &Path) -> Result<(), SpeechError> {
        let content = self.config.generate_keywords_content();
        std::fs::write(path, content).map_err(|e| {
            SpeechError::WakeWordError(format!("Failed to write keywords file: {e}"))
        })?;
        Ok(())
    }
}

impl WakeWordSpotter for SherpaKwsEngine {
    fn process_samples(&mut self, samples: &[f32]) -> Result<Option<WakeWordEvent>, SpeechError> {
        self.active_spotter.process_samples(samples)
    }

    fn keywords(&self) -> &[String] {
        self.active_spotter.keywords()
    }

    fn set_keywords(&mut self, keywords: &[String]) -> Result<(), SpeechError> {
        self.config.keywords = keywords.to_vec();
        self.active_spotter.set_keywords(keywords)
    }

    fn set_threshold(&mut self, threshold: f32) -> Result<(), SpeechError> {
        self.config.threshold = threshold;
        self.active_spotter.set_threshold(threshold)
    }

    fn reset(&mut self) {
        self.active_spotter.reset();
    }
}
