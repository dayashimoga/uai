use crate::error::SpeechError;
use crate::tts::{TtsAudioChunk, TtsConfig, TtsEngine};
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Deterministic offline acoustic-phonetic speech synthesizer.
pub struct AcousticTtsEngine {
    config: TtsConfig,
    cancelled: Arc<AtomicBool>,
    sentence_counter: usize,
}

impl std::fmt::Debug for AcousticTtsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcousticTtsEngine")
            .field("config", &self.config)
            .field("is_cancelled", &self.cancelled.load(Ordering::Relaxed))
            .field("sentence_counter", &self.sentence_counter)
            .finish()
    }
}

impl AcousticTtsEngine {
    /// Creates a new `AcousticTtsEngine` from the provided configuration.
    pub fn new(config: TtsConfig) -> Result<Self, SpeechError> {
        config.validate()?;
        Ok(Self {
            config,
            cancelled: Arc::new(AtomicBool::new(false)),
            sentence_counter: 0,
        })
    }

    /// Returns shared cancellation handle.
    pub fn cancellation_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }

    /// Synthesizes a phonetic sound unit into PCM audio samples.
    fn synthesize_phoneme(
        &self,
        ch: char,
        sample_rate: u32,
        base_pitch: f32,
        duration_sec: f32,
    ) -> Vec<f32> {
        let num_samples = (sample_rate as f32 * duration_sec) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        for t in 0..num_samples {
            let t_sec = t as f32 / sample_rate as f32;

            // Envelope: Hann half-sine rise and fall
            let env = if num_samples > 0 {
                (PI * (t as f32) / (num_samples as f32))
                    .sin()
                    .clamp(0.0, 1.0)
            } else {
                1.0
            };

            let wave = match ch.to_ascii_lowercase() {
                // Open vowels (/a/, /o/, /u/): strong fundamental + low/mid formants
                'a' | 'o' | 'u' => {
                    let f0 = (2.0 * PI * base_pitch * t_sec).sin() * 0.5;
                    let f1 = (2.0 * PI * 700.0 * t_sec).sin() * 0.35;
                    let f2 = (2.0 * PI * 1200.0 * t_sec).sin() * 0.15;
                    (f0 + f1 + f2) * env
                }
                // Front vowels (/e/, /i/, /y/): higher second formant
                'e' | 'i' | 'y' => {
                    let f0 = (2.0 * PI * base_pitch * t_sec).sin() * 0.45;
                    let f1 = (2.0 * PI * 400.0 * t_sec).sin() * 0.30;
                    let f2 = (2.0 * PI * 2000.0 * t_sec).sin() * 0.25;
                    (f0 + f1 + f2) * env
                }
                // Sibilant fricatives (/s/, /z/, /x/): high-frequency shaped noise
                's' | 'z' | 'x' => {
                    let s1 = (2.0 * PI * 4200.0 * t_sec).sin() * 0.35;
                    let s2 = (2.0 * PI * 5100.0 * t_sec).sin() * 0.35;
                    let s3 = (2.0 * PI * 6000.0 * t_sec).sin() * 0.25;
                    (s1 + s2 + s3) * env * 0.6
                }
                // Nasals & liquids (/m/, /n/, /l/, /r/, /w/): low resonance
                'm' | 'n' | 'l' | 'r' | 'w' => {
                    let f0 = (2.0 * PI * base_pitch * t_sec).sin() * 0.6;
                    let f1 = (2.0 * PI * 350.0 * t_sec).sin() * 0.3;
                    (f0 + f1) * env * 0.8
                }
                // Word boundary space or punctuation: brief silence
                ' ' | ',' | '.' | '!' | '?' => 0.0,
                // Default consonant: transient burst + formant
                _ => {
                    let f0 = (2.0 * PI * base_pitch * t_sec).sin() * 0.3;
                    let f1 = (2.0 * PI * 1500.0 * t_sec).sin() * 0.4;
                    (f0 + f1) * env * 0.5
                }
            };

            samples.push(wave * self.config.volume);
        }

        samples
    }
}

impl TtsEngine for AcousticTtsEngine {
    fn synthesize_sentence(&mut self, text: &str) -> Result<TtsAudioChunk, SpeechError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(TtsAudioChunk {
                samples: Vec::new(),
                sample_rate: self.config.sample_rate,
                sentence_index: self.sentence_counter,
                is_final: true,
            });
        }

        // Speaker fundamental frequency modulation
        let base_pitch = 160.0 + (self.config.speaker_id as f32 * 25.0);
        let sample_rate = self.config.sample_rate;
        // Phoneme duration inversely proportional to speed
        let phoneme_duration_sec = 0.045 / self.config.speed.clamp(0.5, 2.0);

        let mut output_samples = Vec::new();

        for ch in trimmed.chars() {
            if self.cancelled.load(Ordering::Acquire) {
                // Immediate cancellation requested (barge-in)
                return Ok(TtsAudioChunk {
                    samples: output_samples,
                    sample_rate,
                    sentence_index: self.sentence_counter,
                    is_final: true,
                });
            }

            let p_samples =
                self.synthesize_phoneme(ch, sample_rate, base_pitch, phoneme_duration_sec);
            output_samples.extend_from_slice(&p_samples);
        }

        // Add trailing inter-sentence silence
        let silence_samples =
            (sample_rate as f32 * (self.config.sentence_silence_ms as f32 / 1000.0)) as usize;
        output_samples.resize(output_samples.len() + silence_samples, 0.0f32);

        let chunk = TtsAudioChunk {
            samples: output_samples,
            sample_rate,
            sentence_index: self.sentence_counter,
            is_final: false,
        };

        self.sentence_counter += 1;
        Ok(chunk)
    }

    fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn set_speed(&mut self, speed: f32) -> Result<(), SpeechError> {
        if !(0.5..=2.0).contains(&speed) {
            return Err(SpeechError::ParameterError(format!(
                "Speed {} out of supported range [0.5, 2.0]",
                speed
            )));
        }
        self.config.speed = speed;
        Ok(())
    }

    fn set_speaker_id(&mut self, speaker_id: u32) -> Result<(), SpeechError> {
        self.config.speaker_id = speaker_id;
        Ok(())
    }

    fn speed(&self) -> f32 {
        self.config.speed
    }

    fn speaker_id(&self) -> u32 {
        self.config.speaker_id
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }
}
