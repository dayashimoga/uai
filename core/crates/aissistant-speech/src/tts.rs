use crate::error::SpeechError;
use serde::{Deserialize, Serialize};

/// Configuration parameters for offline text-to-speech synthesis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TtsConfig {
    /// Audio sample rate in Hz (16,000 or 22,050 Hz). Default: `16_000`.
    pub sample_rate: u32,

    /// Speaking rate multiplier in range `[0.5, 2.0]`. Default: `1.0`.
    pub speed: f32,

    /// Playback volume scaling factor in range `[0.0, 1.0]`. Default: `1.0`.
    pub volume: f32,

    /// Speaker ID index for multi-speaker neural models. Default: `0`.
    pub speaker_id: u32,

    /// Trailing silence duration between sentences in milliseconds. Default: `150` ms.
    pub sentence_silence_ms: u32,
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            speed: 1.0,
            volume: 1.0,
            speaker_id: 0,
            sentence_silence_ms: 150,
        }
    }
}

impl TtsConfig {
    /// Validates the configuration parameters.
    pub fn validate(&self) -> Result<(), SpeechError> {
        if self.sample_rate != 16_000 && self.sample_rate != 22_050 {
            return Err(SpeechError::InvalidAudioFormat(format!(
                "TTS requires 16000 or 22050 Hz sample rate, got {}",
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
        Ok(())
    }
}

/// A chunk of synthesized speech audio representing a sentence or phrase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TtsAudioChunk {
    /// Mono f32 audio samples in range `[-1.0, 1.0]`.
    pub samples: Vec<f32>,

    /// Audio sample rate in Hz.
    pub sample_rate: u32,

    /// 0-based sentence index in the current turn.
    pub sentence_index: usize,

    /// Flag indicating whether this chunk concludes the synthesis turn.
    pub is_final: bool,
}

/// Streaming sentence boundary tokenizer.
/// Buffers incoming token fragments and yields complete sentences at natural
/// punctuation marks ('.', '!', '?', '\n') while respecting abbreviations and numbers.
#[derive(Debug, Clone, Default)]
pub struct SentenceChunker {
    buffer: String,
    sentence_count: usize,
}

impl SentenceChunker {
    /// Creates a new sentence chunker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends text or token stream to the buffer.
    pub fn push_text(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    /// Attempts to extract the next complete sentence from the buffer.
    pub fn next_sentence(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            return None;
        }

        let chars: Vec<char> = self.buffer.chars().collect();
        let len = chars.len();

        for i in 0..len {
            let ch = chars[i];
            let is_boundary = ch == '.' || ch == '!' || ch == '?' || ch == '\n';

            if is_boundary {
                // Check if this period is an abbreviation or decimal number
                if ch == '.' && Self::is_abbreviation_or_number(&chars, i) {
                    continue;
                }

                // Sentence boundary found
                let boundary_idx = i + 1;
                let sentence: String = chars[..boundary_idx].iter().collect();
                let remaining: String = chars[boundary_idx..].iter().collect();

                self.buffer = remaining.trim_start().to_string();
                let trimmed = sentence.trim().to_string();

                if !trimmed.is_empty() {
                    self.sentence_count += 1;
                    return Some(trimmed);
                }
            }
        }

        None
    }

    /// Flushes any remaining non-empty text in the buffer as a final sentence fragment.
    pub fn flush(&mut self) -> Option<String> {
        let trimmed = self.buffer.trim().to_string();
        self.buffer.clear();
        if !trimmed.is_empty() {
            self.sentence_count += 1;
            Some(trimmed)
        } else {
            None
        }
    }

    /// Total number of sentences yielded.
    pub fn sentence_count(&self) -> usize {
        self.sentence_count
    }

    /// Checks if a period at index `idx` is part of an abbreviation (e.g. "Dr.", "e.g.")
    /// or decimal number (e.g. "3.14").
    fn is_abbreviation_or_number(chars: &[char], idx: usize) -> bool {
        // Decimal number check: digit before and digit after (e.g. 3.14)
        if idx > 0
            && idx + 1 < chars.len()
            && chars[idx - 1].is_ascii_digit()
            && chars[idx + 1].is_ascii_digit()
        {
            return true;
        }

        // Look back at the word preceding the period
        let mut start = idx;
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }

        let word: String = chars[start..=idx].iter().collect();
        let word_lower = word.to_lowercase();

        matches!(
            word_lower.as_str(),
            "dr."
                | "mr."
                | "mrs."
                | "ms."
                | "prof."
                | "sr."
                | "jr."
                | "vs."
                | "etc."
                | "e.g."
                | "i.e."
        )
    }
}

/// Abstract contract for offline text-to-speech synthesis engines.
pub trait TtsEngine: Send {
    /// Synthesizes speech audio for a single sentence.
    fn synthesize_sentence(&mut self, text: &str) -> Result<TtsAudioChunk, SpeechError>;

    /// Signals immediate cancellation of active synthesis (e.g., during barge-in).
    fn cancel(&mut self);

    /// Checks whether cancellation was requested.
    fn is_cancelled(&self) -> bool;

    /// Updates the speaking rate multiplier `[0.5, 2.0]`.
    fn set_speed(&mut self, speed: f32) -> Result<(), SpeechError>;

    /// Updates the speaker ID index.
    fn set_speaker_id(&mut self, speaker_id: u32) -> Result<(), SpeechError>;

    /// Returns the active speaking rate.
    fn speed(&self) -> f32;

    /// Returns the active speaker ID.
    fn speaker_id(&self) -> u32;

    /// Returns the audio sample rate in Hz.
    fn sample_rate(&self) -> u32;
}
