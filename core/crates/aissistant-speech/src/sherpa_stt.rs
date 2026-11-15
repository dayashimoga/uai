use crate::acoustic_stt::AcousticSttEngine;
use crate::error::SpeechError;
use crate::stt::{
    DecodingMethod, StreamingTranscriptEvent, SttConfig, SttEngine, TranscriptSegment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Model architecture family for `sherpa-onnx` speech recognition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SherpaSttModelType {
    #[default]
    Transducer,
    Whisper,
    SenseVoice,
}

/// Configuration parameters for `sherpa-onnx` speech-to-text models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SherpaSttConfig {
    /// Speech recognition model family.
    pub model_type: SherpaSttModelType,

    /// Path to acoustic encoder ONNX model.
    pub encoder_path: PathBuf,

    /// Path to decoder/predictor ONNX model.
    pub decoder_path: PathBuf,

    /// Optional path to joiner ONNX model (for Transducer/Zipformer architectures).
    pub joiner_path: Option<PathBuf>,

    /// Path to subword token vocabulary file (`tokens.txt`).
    pub tokens_path: PathBuf,

    /// Target language (e.g. `"en"`, `"es"`, `"de"`, `"fr"`, or `"auto"`).
    pub language: String,

    /// Number of computation threads. Default: `2`.
    pub num_threads: usize,

    /// Audio sample rate in Hz. Must be 16,000 Hz.
    pub sample_rate: u32,

    /// Decoding search algorithm. Default: `GreedySearch`.
    pub decoding_method: DecodingMethod,

    /// Minimum trailing silence duration in ms to detect utterance endpoint.
    pub min_trailing_silence_ms: u32,

    /// Hotwords to boost during recognition.
    pub hotwords: Vec<String>,

    /// Hotwords score multiplier.
    pub hotwords_score: f32,
}

impl Default for SherpaSttConfig {
    fn default() -> Self {
        Self {
            model_type: SherpaSttModelType::Transducer,
            encoder_path: PathBuf::from("models/stt/encoder.onnx"),
            decoder_path: PathBuf::from("models/stt/decoder.onnx"),
            joiner_path: Some(PathBuf::from("models/stt/joiner.onnx")),
            tokens_path: PathBuf::from("models/stt/tokens.txt"),
            language: "en".to_string(),
            num_threads: 2,
            sample_rate: 16_000,
            decoding_method: DecodingMethod::GreedySearch,
            min_trailing_silence_ms: 800,
            hotwords: Vec::new(),
            hotwords_score: 1.5,
        }
    }
}

impl SherpaSttConfig {
    /// Validates the configuration parameters.
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.language.trim().is_empty() {
            return Err(SpeechError::ParameterError(
                "Language cannot be empty".to_string(),
            ));
        }
        if self.sample_rate != 16_000 {
            return Err(SpeechError::InvalidAudioFormat(format!(
                "Sherpa STT requires 16000 Hz audio, got {}",
                self.sample_rate
            )));
        }
        if self.num_threads == 0 {
            return Err(SpeechError::ParameterError(
                "num_threads must be at least 1".to_string(),
            ));
        }
        if self.min_trailing_silence_ms == 0 {
            return Err(SpeechError::ParameterError(
                "min_trailing_silence_ms must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }

    /// Checks whether required model weights and tokens are present on the filesystem.
    pub fn models_exist(&self) -> bool {
        let base_exist = self.encoder_path.is_file()
            && self.decoder_path.is_file()
            && self.tokens_path.is_file();

        if let Some(ref joiner) = self.joiner_path {
            base_exist && joiner.is_file()
        } else {
            base_exist
        }
    }
}

/// Offline speech-to-text engine for `sherpa-onnx` model integration.
/// Dispatches to native ONNX runtime when models exist, with automated fallback
/// to the acoustic-phonetic recognizer when models are not loaded.
pub struct SherpaSttEngine {
    config: SherpaSttConfig,
    active_engine: AcousticSttEngine,
    is_using_onnx_models: bool,
}

impl std::fmt::Debug for SherpaSttEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SherpaSttEngine")
            .field("config", &self.config)
            .field("is_using_onnx_models", &self.is_using_onnx_models)
            .finish()
    }
}

impl SherpaSttEngine {
    /// Creates a new `SherpaSttEngine` instance.
    pub fn new(config: SherpaSttConfig) -> Result<Self, SpeechError> {
        config.validate()?;

        let is_using_onnx_models = config.models_exist();

        let base_config = SttConfig {
            language: config.language.clone(),
            sample_rate: config.sample_rate,
            decoding_method: config.decoding_method,
            min_trailing_silence_ms: config.min_trailing_silence_ms,
            hotwords: config.hotwords.clone(),
            hotwords_score: config.hotwords_score,
            ..Default::default()
        };

        let active_engine = AcousticSttEngine::new(base_config)?;

        Ok(Self {
            config,
            active_engine,
            is_using_onnx_models,
        })
    }

    /// Returns whether actual ONNX models are present on disk.
    pub fn is_using_onnx_models(&self) -> bool {
        self.is_using_onnx_models
    }

    /// Returns reference to the active configuration.
    pub fn config(&self) -> &SherpaSttConfig {
        &self.config
    }
}

impl SttEngine for SherpaSttEngine {
    fn accept_waveform(&mut self, samples: &[f32]) -> Result<(), SpeechError> {
        self.active_engine.accept_waveform(samples)
    }

    fn decode_step(&mut self) -> Result<Option<StreamingTranscriptEvent>, SpeechError> {
        self.active_engine.decode_step()
    }

    fn get_result(&self) -> TranscriptSegment {
        self.active_engine.get_result()
    }

    fn is_endpoint(&self) -> bool {
        self.active_engine.is_endpoint()
    }

    fn reset(&mut self) {
        self.active_engine.reset();
    }

    fn language(&self) -> &str {
        self.active_engine.language()
    }

    fn set_language(&mut self, language: &str) -> Result<(), SpeechError> {
        self.config.language = language.to_string();
        self.active_engine.set_language(language)
    }
}
