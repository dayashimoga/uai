use crate::acoustic_kws::{AcousticFeature, KeywordTemplate};
use crate::error::SpeechError;
use crate::stt::{StreamingTranscriptEvent, SttConfig, SttEngine, TranscriptSegment};
use std::collections::VecDeque;
use std::f32::consts::PI;

const FFT_SIZE: usize = 512;

/// A lexical vocabulary entry with acoustic profile.
#[derive(Debug, Clone)]
pub struct LexiconEntry {
    pub phrase: String,
    pub language: String,
    pub template: KeywordTemplate,
}

/// Offline streaming speech-to-text engine using acoustic-phonetic trajectory decoding.
pub struct AcousticSttEngine {
    config: SttConfig,
    lexicon: Vec<LexiconEntry>,
    feature_history: VecDeque<AcousticFeature>,
    unprocessed_samples: Vec<f32>,
    samples_consumed: u64,
    speech_active: bool,
    speech_start_ms: u64,
    last_speech_ms: u64,
    current_tokens: Vec<String>,
    current_confidence_sum: f32,
    is_endpoint_reached: bool,
    resolved_language: String,
    hann_window: [f32; FFT_SIZE],
}

impl std::fmt::Debug for AcousticSttEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcousticSttEngine")
            .field("config", &self.config)
            .field("lexicon_size", &self.lexicon.len())
            .field("current_tokens", &self.current_tokens)
            .field("is_endpoint_reached", &self.is_endpoint_reached)
            .field("resolved_language", &self.resolved_language)
            .finish()
    }
}

impl AcousticSttEngine {
    /// Creates a new `AcousticSttEngine` instance with default multilingual lexicon.
    pub fn new(config: SttConfig) -> Result<Self, SpeechError> {
        config.validate()?;

        let mut hann_window = [0.0f32; FFT_SIZE];
        for (i, item) in hann_window.iter_mut().enumerate() {
            *item = 0.5 * (1.0 - (2.0 * PI * (i as f32) / (FFT_SIZE as f32 - 1.0)).cos());
        }

        let mut engine = Self {
            resolved_language: if config.language == "auto" {
                "en".to_string()
            } else {
                config.language.clone()
            },
            config,
            lexicon: Vec::new(),
            feature_history: VecDeque::with_capacity(300),
            unprocessed_samples: Vec::with_capacity(1024),
            samples_consumed: 0,
            speech_active: false,
            speech_start_ms: 0,
            last_speech_ms: 0,
            current_tokens: Vec::new(),
            current_confidence_sum: 0.0,
            is_endpoint_reached: false,
            hann_window,
        };

        engine.init_default_lexicon();
        Ok(engine)
    }

    /// Populates built-in multilingual command and speech lexicon.
    fn init_default_lexicon(&mut self) {
        // English lexicon
        self.add_entry("hey assistant", "en");
        self.add_entry("hello assistant", "en");
        self.add_entry("turn on the lights", "en");
        self.add_entry("turn off the lights", "en");
        self.add_entry("what time is it", "en");
        self.add_entry("stop music", "en");
        self.add_entry("play music", "en");
        self.add_entry("open settings", "en");
        self.add_entry("computer", "en");
        self.add_entry("jarvis", "en");

        // Spanish lexicon
        self.add_entry("hola asistente", "es");
        self.add_entry("enciende las luces", "es");
        self.add_entry("apaga las luces", "es");
        self.add_entry("que hora es", "es");

        // German lexicon
        self.add_entry("hallo assistent", "de");
        self.add_entry("schalte das licht ein", "de");
        self.add_entry("wie spat ist es", "de");

        // French lexicon
        self.add_entry("bonjour assistant", "fr");
        self.add_entry("allume la lumiere", "fr");
        self.add_entry("quelle heure est il", "fr");
    }

    /// Returns the active recognition lexicon.
    pub fn lexicon(&self) -> &[LexiconEntry] {
        &self.lexicon
    }

    /// Adds a phrase to the recognition vocabulary.
    pub fn add_entry(&mut self, phrase: &str, language: &str) {
        let norm_phrase = phrase.trim().to_lowercase();
        let template = KeywordTemplate::for_keyword(&norm_phrase);
        self.lexicon.push(LexiconEntry {
            phrase: norm_phrase,
            language: language.to_lowercase(),
            template,
        });
    }

    /// Extracts acoustic feature representation from a 512-sample frame.
    fn extract_frame_feature(&self, frame: &[f32]) -> AcousticFeature {
        let mut sum_sq = 0.0f32;
        let mut zero_crossings = 0;
        let mut prev_sign = frame[0] >= 0.0;

        for &s in frame {
            sum_sq += s * s;
            let current_sign = s >= 0.0;
            if current_sign != prev_sign {
                zero_crossings += 1;
                prev_sign = current_sign;
            }
        }

        let rms = (sum_sq / (FFT_SIZE as f32)).sqrt();
        let energy_dbfs = 20.0 * (rms.max(1e-6)).log10();
        let is_silence = energy_dbfs < -45.0;
        let zcr = (zero_crossings as f32) / (FFT_SIZE as f32);

        if is_silence {
            return AcousticFeature {
                subbands: [0.25, 0.25, 0.25, 0.25],
                zcr,
                spectral_centroid: 1000.0,
                energy_dbfs,
                is_silence: true,
            };
        }

        let mut real = [0.0f32; FFT_SIZE];
        let mut imag = [0.0f32; FFT_SIZE];
        for i in 0..FFT_SIZE {
            real[i] = frame[i] * self.hann_window[i];
        }

        Self::cooley_tukey_fft(&mut real, &mut imag);

        let mut power = [0.0f32; FFT_SIZE / 2];
        let mut total_power = 0.0f32;
        let mut centroid_numerator = 0.0f32;

        for k in 1..(FFT_SIZE / 2) {
            let p = real[k] * real[k] + imag[k] * imag[k];
            power[k] = p;
            total_power += p;
            centroid_numerator += (k as f32) * 31.25 * p;
        }

        let spectral_centroid = if total_power > 1e-8 {
            centroid_numerator / total_power
        } else {
            1000.0
        };

        let sub_energies = [
            power[3..13].iter().sum::<f32>(),
            power[13..38].iter().sum::<f32>(),
            power[38..96].iter().sum::<f32>(),
            power[96..240].iter().sum::<f32>(),
        ];

        let sub_sum = sub_energies.iter().sum::<f32>();
        let normalized_subbands = if sub_sum > 1e-8 {
            [
                sub_energies[0] / sub_sum,
                sub_energies[1] / sub_sum,
                sub_energies[2] / sub_sum,
                sub_energies[3] / sub_sum,
            ]
        } else {
            [0.25, 0.25, 0.25, 0.25]
        };

        AcousticFeature {
            subbands: normalized_subbands,
            zcr,
            spectral_centroid,
            energy_dbfs,
            is_silence: false,
        }
    }

    fn cooley_tukey_fft(real: &mut [f32; FFT_SIZE], imag: &mut [f32; FFT_SIZE]) {
        let mut j = 0;
        for i in 0..(FFT_SIZE - 1) {
            if i < j {
                real.swap(i, j);
                imag.swap(i, j);
            }
            let mut k = FFT_SIZE / 2;
            while k <= j {
                j -= k;
                k >>= 1;
            }
            j += k;
        }

        let mut len = 2;
        while len <= FFT_SIZE {
            let half = len / 2;
            let angle = -2.0 * PI / (len as f32);
            let w_step_real = angle.cos();
            let w_step_imag = angle.sin();

            let mut i = 0;
            while i < FFT_SIZE {
                let mut w_real = 1.0f32;
                let mut w_imag = 0.0f32;
                for k in 0..half {
                    let u_real = real[i + k];
                    let u_imag = imag[i + k];
                    let v_real = real[i + k + half] * w_real - imag[i + k + half] * w_imag;
                    let v_imag = real[i + k + half] * w_imag + imag[i + k + half] * w_real;

                    real[i + k] = u_real + v_real;
                    imag[i + k] = u_imag + v_imag;
                    real[i + k + half] = u_real - v_real;
                    imag[i + k + half] = u_imag - v_imag;

                    let next_w_real = w_real * w_step_real - w_imag * w_step_imag;
                    let next_w_imag = w_real * w_step_imag + w_imag * w_step_real;
                    w_real = next_w_real;
                    w_imag = next_w_imag;
                }
                i += len;
            }
            len <<= 1;
        }
    }

    /// Evaluates alignment similarity between feature frames and a vocabulary entry.
    fn match_entry(&self, entry: &LexiconEntry) -> f32 {
        let template = &entry.template;
        let template_len = template.frames.len();
        if self.feature_history.len() < template_len {
            return 0.0;
        }

        let history_slice: Vec<&AcousticFeature> = self
            .feature_history
            .iter()
            .rev()
            .take(template_len)
            .collect();
        let audio_frames: Vec<&AcousticFeature> = history_slice.into_iter().rev().collect();

        let silence_count = audio_frames.iter().filter(|f| f.is_silence).count();
        if silence_count > template_len / 2 {
            return 0.0;
        }

        let mut total_score = 0.0f32;
        let mut total_weight = 0.0f32;

        for (i, t_frame) in template.frames.iter().enumerate() {
            let a_frame = audio_frames[i];
            if a_frame.is_silence {
                total_weight += t_frame.weight;
                continue;
            }

            let mut dot = 0.0f32;
            let mut norm_a = 0.0f32;
            let mut norm_t = 0.0f32;
            for b in 0..4 {
                dot += a_frame.subbands[b] * t_frame.subbands[b];
                norm_a += a_frame.subbands[b] * a_frame.subbands[b];
                norm_t += t_frame.subbands[b] * t_frame.subbands[b];
            }
            let subband_sim = if norm_a > 1e-6 && norm_t > 1e-6 {
                (dot / (norm_a.sqrt() * norm_t.sqrt())).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let is_audio_fricative = a_frame.zcr > 0.30 && a_frame.subbands[3] > 0.40;
            if t_frame.is_fricative != is_audio_fricative {
                total_score += subband_sim * 0.1 * t_frame.weight;
                total_weight += t_frame.weight;
                continue;
            }

            let zcr_diff = (a_frame.zcr - t_frame.zcr).abs();
            let zcr_sim = (1.0 - zcr_diff * 2.0).clamp(0.0, 1.0);

            let centroid_diff =
                (a_frame.spectral_centroid - t_frame.centroid).abs() / 4000.0f32.max(1.0);
            let centroid_sim = (1.0 - centroid_diff).clamp(0.0, 1.0);

            let frame_score = subband_sim * 0.50 + zcr_sim * 0.30 + centroid_sim * 0.20;
            total_score += frame_score * t_frame.weight;
            total_weight += t_frame.weight;
        }

        let frame_sim = if total_weight > 0.0 {
            (total_score / total_weight).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Pearson trajectory correlation
        let mut trajectory_corr_sum = 0.0f32;
        let mut trajectory_dim_count = 0.0f32;

        for b in 0..4 {
            let mean_a =
                audio_frames.iter().map(|f| f.subbands[b]).sum::<f32>() / (template_len as f32);
            let mean_t =
                template.frames.iter().map(|f| f.subbands[b]).sum::<f32>() / (template_len as f32);

            let mut cov = 0.0f32;
            let mut var_a = 0.0f32;
            let mut var_t = 0.0f32;

            for (a, t) in audio_frames.iter().zip(template.frames.iter()) {
                let da = a.subbands[b] - mean_a;
                let dt = t.subbands[b] - mean_t;
                cov += da * dt;
                var_a += da * da;
                var_t += dt * dt;
            }

            if var_t > 0.005 {
                if var_a > 0.002 {
                    let r = cov / (var_a.sqrt() * var_t.sqrt());
                    trajectory_corr_sum += r.clamp(-1.0, 1.0).max(0.0);
                }
                trajectory_dim_count += 1.0;
            }
        }

        let trajectory_sim = if trajectory_dim_count > 0.0 {
            trajectory_corr_sum / trajectory_dim_count
        } else {
            frame_sim
        };

        (frame_sim * 0.40 + trajectory_sim * 0.60).clamp(0.0, 1.0)
    }
}

impl SttEngine for AcousticSttEngine {
    fn accept_waveform(&mut self, samples: &[f32]) -> Result<(), SpeechError> {
        self.unprocessed_samples.extend_from_slice(samples);
        Ok(())
    }

    fn decode_step(&mut self) -> Result<Option<StreamingTranscriptEvent>, SpeechError> {
        let slide_step = 160; // 10 ms @ 16 kHz

        while self.unprocessed_samples.len() >= FFT_SIZE {
            let frame = &self.unprocessed_samples[..FFT_SIZE];
            let feature = self.extract_frame_feature(frame);

            self.feature_history.push_back(feature);
            if self.feature_history.len() > 300 {
                self.feature_history.pop_front();
            }

            self.unprocessed_samples.drain(..slide_step);
            self.samples_consumed = self.samples_consumed.saturating_add(slide_step as u64);

            let current_time_ms =
                (self.samples_consumed as f64 / self.config.sample_rate as f64 * 1000.0) as u64;

            if !feature.is_silence {
                if !self.speech_active {
                    self.speech_active = true;
                    self.speech_start_ms = current_time_ms;
                }
                self.last_speech_ms = current_time_ms;
            }

            // Check for vocabulary match
            let mut best_entry: Option<(String, String, f32)> = None;

            for entry in &self.lexicon {
                if self.config.language != "auto"
                    && entry.language != self.config.language.to_lowercase()
                {
                    continue;
                }

                let mut conf = self.match_entry(entry);
                // Hotword boosting
                if self
                    .config
                    .hotwords
                    .iter()
                    .any(|hw| hw.eq_ignore_ascii_case(&entry.phrase))
                {
                    conf = (conf * self.config.hotwords_score).clamp(0.0, 1.0);
                }

                if conf >= 0.60 {
                    if let Some((_, _, best_conf)) = best_entry {
                        if conf > best_conf {
                            best_entry = Some((entry.phrase.clone(), entry.language.clone(), conf));
                        }
                    } else {
                        best_entry = Some((entry.phrase.clone(), entry.language.clone(), conf));
                    }
                }
            }

            if let Some((phrase, lang, conf)) = best_entry {
                // Avoid immediate duplicates of the same word
                if self.current_tokens.last().map(|s| s.as_str()) != Some(&phrase) {
                    self.current_tokens.push(phrase);
                    self.current_confidence_sum += conf;
                    self.resolved_language = lang;

                    let partial_segment = self.get_result();
                    self.feature_history.clear();

                    return Ok(Some(StreamingTranscriptEvent::Partial(partial_segment)));
                }
            }

            // Endpointing check: silence after active speech
            if self.speech_active && !self.is_endpoint_reached {
                let silence_elapsed = current_time_ms.saturating_sub(self.last_speech_ms);
                let utterance_len = current_time_ms.saturating_sub(self.speech_start_ms);

                if silence_elapsed >= self.config.min_trailing_silence_ms as u64
                    || utterance_len >= self.config.max_utterance_ms as u64
                {
                    self.is_endpoint_reached = true;
                    let final_segment = self.get_result();
                    return Ok(Some(StreamingTranscriptEvent::Final(final_segment)));
                }
            }
        }

        Ok(None)
    }

    fn get_result(&self) -> TranscriptSegment {
        let text = self.current_tokens.join(" ");
        let confidence = if !self.current_tokens.is_empty() {
            (self.current_confidence_sum / (self.current_tokens.len() as f32)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let current_time_ms =
            (self.samples_consumed as f64 / self.config.sample_rate as f64 * 1000.0) as u64;

        TranscriptSegment {
            text,
            language: self.resolved_language.clone(),
            confidence,
            start_time_ms: self.speech_start_ms,
            end_time_ms: current_time_ms,
            is_final: self.is_endpoint_reached,
            tokens: self.current_tokens.clone(),
        }
    }

    fn is_endpoint(&self) -> bool {
        self.is_endpoint_reached
    }

    fn reset(&mut self) {
        self.feature_history.clear();
        self.unprocessed_samples.clear();
        self.samples_consumed = 0;
        self.speech_active = false;
        self.speech_start_ms = 0;
        self.last_speech_ms = 0;
        self.current_tokens.clear();
        self.current_confidence_sum = 0.0;
        self.is_endpoint_reached = false;
    }

    fn language(&self) -> &str {
        &self.resolved_language
    }

    fn set_language(&mut self, language: &str) -> Result<(), SpeechError> {
        if language.trim().is_empty() {
            return Err(SpeechError::ParameterError(
                "Language cannot be empty".to_string(),
            ));
        }
        self.config.language = language.to_string();
        if language != "auto" {
            self.resolved_language = language.to_string();
        }
        self.reset();
        Ok(())
    }
}
