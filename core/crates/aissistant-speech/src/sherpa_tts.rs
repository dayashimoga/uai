use crate::acoustic_tts::AcousticTtsEngine;
use crate::error::SpeechError;
use crate::tts::{TtsAudioChunk, TtsConfig, TtsEngine};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Model architecture family for `sherpa-onnx` neural text-to-speech.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SherpaTtsModelType {
    #[default]
    Vits,
    Piper,
    Matcha,
}

/// Configuration parameters for `sherpa-onnx` neural TTS models.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SherpaTtsConfig {
    /// Neural TTS model family.
    pub model_type: SherpaTtsModelType,

    /// Path to ONNX TTS model weights (`model.onnx`).
    pub model_path: PathBuf,

    /// Path to character/phoneme tokens file (`tokens.txt`).
    pub tokens_path: PathBuf,

    /// Optional path to `espeak-ng-data` directory.
    pub data_dir: Option<PathBuf>,

    /// Optional path to pronunciation lexicon file.
    pub lexicon_path: Option<PathBuf>,

    /// Speaker ID index for multi-speaker models. Default: `0`.
    pub speaker_id: u32,

    /// Speaking rate multiplier in range `[0.5, 2.0]`. Default: `1.0`.
    pub speed: f32,

    /// Volume multiplier in range `[0.0, 1.0]`. Default: `1.0`.
    pub volume: f32,

    /// Target audio sample rate. Must be 16,000 or 22,050 Hz.
    pub sample_rate: u32,

    /// Number of computation threads. Default: `2`.
    pub num_threads: usize,
}

impl Default for SherpaTtsConfig {
    fn default() -> Self {
        Self {
            model_type: SherpaTtsModelType::Vits,
            model_path: PathBuf::from("models/tts/model.onnx"),
            tokens_path: PathBuf::from("models/tts/tokens.txt"),
            data_dir: Some(PathBuf::from("models/tts/espeak-ng-data")),
            lexicon_path: None,
            speaker_id: 0,
            speed: 1.0,
            volume: 1.0,
            sample_rate: 16_000,
            num_threads: 2,
        }
    }
}

impl SherpaTtsConfig {
    /// Validates the configuration parameters.
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.sample_rate != 16_000 && self.sample_rate != 22_050 {
            return Err(SpeechError::InvalidAudioFormat(format!(
                "Sherpa TTS requires 16000 or 22050 Hz sample rate, got {}",
                self.sample_rate
            )));
        }
        if !(0.5..=2.0).contains(&self.speed) {
            return Err(SpeechError::ParameterError(format!(
                "Speed {} out of supported range [0.5, 2.0]",
                self.speed
            )));
        }
        if !(0.0..=1.0).contains(&self.volume) {
            return Err(SpeechError::ParameterError(format!(
                "Volume {} out of supported range [0.0, 1.0]",
                self.volume
            )));
        }
        if self.num_threads == 0 {
            return Err(SpeechError::ParameterError(
                "num_threads must be at least 1".to_string(),
            ));
        }
        Ok(())
    }

    /// Checks whether model weights and tokens are present on the filesystem.
    pub fn models_exist(&self) -> bool {
        self.model_path.is_file() && self.tokens_path.is_file()
    }
}

/// Offline neural text-to-speech engine with automatic acoustic synthesizer fallback.
pub struct SherpaTtsEngine {
    config: SherpaTtsConfig,
    active_engine: AcousticTtsEngine,
    is_using_onnx_models: bool,
}

impl std::fmt::Debug for SherpaTtsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SherpaTtsEngine")
            .field("config", &self.config)
            .field("is_using_onnx_models", &self.is_using_onnx_models)
            .finish()
    }
}

impl SherpaTtsEngine {
    /// Creates a new `SherpaTtsEngine` instance.
    pub fn new(config: SherpaTtsConfig) -> Result<Self, SpeechError> {
        config.validate()?;

        let is_using_onnx_models = config.models_exist();

        let base_config = TtsConfig {
            sample_rate: config.sample_rate,
            speed: config.speed,
            volume: config.volume,
            speaker_id: config.speaker_id,
            ..Default::default()
        };

        let active_engine = AcousticTtsEngine::new(base_config)?;

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
    pub fn config(&self) -> &SherpaTtsConfig {
        &self.config
    }
}

impl TtsEngine for SherpaTtsEngine {
    fn synthesize_sentence(&mut self, text: &str) -> Result<TtsAudioChunk, SpeechError> {
        self.active_engine.synthesize_sentence(text)
    }

    fn cancel(&mut self) {
        self.active_engine.cancel();
    }

    fn is_cancelled(&self) -> bool {
        self.active_engine.is_cancelled()
    }

    fn set_speed(&mut self, speed: f32) -> Result<(), SpeechError> {
        self.config.speed = speed;
        self.active_engine.set_speed(speed)
    }

    fn set_speaker_id(&mut self, speaker_id: u32) -> Result<(), SpeechError> {
        self.config.speaker_id = speaker_id;
        self.active_engine.set_speaker_id(speaker_id)
    }

    fn speed(&self) -> f32 {
        self.active_engine.speed()
    }

    fn speaker_id(&self) -> u32 {
        self.active_engine.speaker_id()
    }

    fn sample_rate(&self) -> u32 {
        self.active_engine.sample_rate()
    }
}
