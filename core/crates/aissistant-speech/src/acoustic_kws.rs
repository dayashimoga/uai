use crate::error::SpeechError;
use crate::wake_word::{WakeWordConfig, WakeWordEvent, WakeWordSpotter};
use std::collections::VecDeque;
use std::f32::consts::PI;

/// Number of samples in one analysis frame (32 ms @ 16 kHz).
const FFT_SIZE: usize = 512;

/// Acoustic feature representation of a single analysis frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcousticFeature {
    /// Normalized sub-band energies:
    /// - [0]: Low / Fundamental (80 - 400 Hz)
    /// - [1]: Lower-Mid / Formant 1 (400 - 1200 Hz)
    /// - [2]: Upper-Mid / Formant 2 (1200 - 3000 Hz)
    /// - [3]: High / Sibilants & Fricatives (3000 - 7500 Hz)
    pub subbands: [f32; 4],

    /// Zero-crossing rate normalized to `[0.0, 1.0]`.
    pub zcr: f32,

    /// Spectral centroid in Hz.
    pub spectral_centroid: f32,

    /// Frame energy in dBFS.
    pub energy_dbfs: f32,

    /// Indicates whether frame energy is below minimum ambient noise floor.
    pub is_silence: bool,
}

impl Default for AcousticFeature {
    fn default() -> Self {
        Self {
            subbands: [0.25, 0.25, 0.25, 0.25],
            zcr: 0.0,
            spectral_centroid: 1000.0,
            energy_dbfs: -100.0,
            is_silence: true,
        }
    }
}

/// Target acoustic frame for template matching.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetFrame {
    /// Expected normalized subband distribution.
    pub subbands: [f32; 4],
    /// Expected normalized zero-crossing rate.
    pub zcr: f32,
    /// Expected spectral centroid in Hz.
    pub centroid: f32,
    /// Weight/importance of this frame in alignment.
    pub weight: f32,
    /// Is this frame expected to be a sibilant/fricative sound?
    pub is_fricative: bool,
}

/// Acoustic reference template for a target keyword or phrase.
#[derive(Debug, Clone)]
pub struct KeywordTemplate {
    pub keyword: String,
    pub frames: Vec<TargetFrame>,
    pub min_duration_ms: u32,
    pub max_duration_ms: u32,
}

impl KeywordTemplate {
    /// Builds an acoustic template for a given keyword or keyphrase.
    /// Uses fine-tuned profiles for known keywords, and algorithmic phonetic
    /// profile generation for custom phrases.
    pub fn for_keyword(keyword: &str) -> Self {
        let norm = keyword.trim().to_lowercase();
        match norm.as_str() {
            "hey assistant" | "assistant" => Self::hey_assistant_template(&norm),
            "computer" => Self::computer_template(&norm),
            "jarvis" => Self::jarvis_template(&norm),
            _ => Self::synthesize_template_from_phrase(&norm),
        }
    }

    /// Calibrated acoustic template for "hey assistant".
    /// Profile stages:
    /// 1. "hey": voiced diphthong (/eɪ/) -> high lower-mid band [1], low ZCR.
    /// 2. transition / short dip.
    /// 3. "uh": schwa vowel -> mid band.
    /// 4. "ss": unvoiced sibilant (/s/) -> dominant high band [3], high ZCR, high centroid.
    /// 5. "ist": mid formant vowel followed by dental stop.
    /// 6. "ant": vowel + nasal + stop.
    fn hey_assistant_template(keyword: &str) -> Self {
        let mut frames = Vec::new();

        // 1. "hey" (~6 frames / 60 ms): strong low & lower-mid formant
        for _ in 0..6 {
            frames.push(TargetFrame {
                subbands: [0.35, 0.45, 0.15, 0.05],
                zcr: 0.08,
                centroid: 800.0,
                weight: 1.0,
                is_fricative: false,
            });
        }

        // 2. Short transition (~2 frames)
        for _ in 0..2 {
            frames.push(TargetFrame {
                subbands: [0.30, 0.35, 0.25, 0.10],
                zcr: 0.15,
                centroid: 1200.0,
                weight: 0.7,
                is_fricative: false,
            });
        }

        // 3. "uh" / vowel onset (~4 frames)
        for _ in 0..4 {
            frames.push(TargetFrame {
                subbands: [0.30, 0.45, 0.20, 0.05],
                zcr: 0.10,
                centroid: 900.0,
                weight: 1.0,
                is_fricative: false,
            });
        }

        // 4. "ss" sibilant (~8 frames): dominant high frequencies > 3500 Hz, high ZCR
        for _ in 0..8 {
            frames.push(TargetFrame {
                subbands: [0.05, 0.10, 0.25, 0.60],
                zcr: 0.45,
                centroid: 4500.0,
                weight: 1.5,
                is_fricative: true,
            });
        }

        // 5. "is" vowel (~4 frames)
        for _ in 0..4 {
            frames.push(TargetFrame {
                subbands: [0.25, 0.45, 0.25, 0.05],
                zcr: 0.12,
                centroid: 1100.0,
                weight: 1.0,
                is_fricative: false,
            });
        }

        // 6. "tant" suffix (~6 frames): dental stop transient + vowel/nasal
        for _ in 0..6 {
            frames.push(TargetFrame {
                subbands: [0.35, 0.35, 0.20, 0.10],
                zcr: 0.18,
                centroid: 1400.0,
                weight: 1.1,
                is_fricative: false,
            });
        }

        Self {
            keyword: keyword.to_string(),
            frames,
            min_duration_ms: 300,
            max_duration_ms: 1800,
        }
    }

    /// Calibrated acoustic template for "computer".
    /// Profile: Plosive 'k' -> vowel+nasal 'om' -> plosive 'p' -> glide 'yu' -> 'ter'.
    fn computer_template(keyword: &str) -> Self {
        let mut frames = Vec::new();

        // 1. "k" burst (~4 frames)
        for _ in 0..4 {
            frames.push(TargetFrame {
                subbands: [0.15, 0.30, 0.40, 0.15],
                zcr: 0.28,
                centroid: 2200.0,
                weight: 1.1,
                is_fricative: false,
            });
        }

        // 2. "om" vowel+nasal (~10 frames): dominant low band
        for _ in 0..10 {
            frames.push(TargetFrame {
                subbands: [0.50, 0.35, 0.12, 0.03],
                zcr: 0.07,
                centroid: 650.0,
                weight: 1.2,
                is_fricative: false,
            });
        }

        // 3. "pyu" plosive + glide (~8 frames)
        for _ in 0..8 {
            frames.push(TargetFrame {
                subbands: [0.20, 0.35, 0.35, 0.10],
                zcr: 0.15,
                centroid: 1800.0,
                weight: 1.0,
                is_fricative: false,
            });
        }

        // 4. "ter" vowel/liquid (~8 frames)
        for _ in 0..8 {
            frames.push(TargetFrame {
                subbands: [0.35, 0.40, 0.20, 0.05],
                zcr: 0.14,
                centroid: 1200.0,
                weight: 1.0,
                is_fricative: false,
            });
        }

        Self {
            keyword: keyword.to_string(),
            frames,
            min_duration_ms: 240,
            max_duration_ms: 1500,
        }
    }

    /// Calibrated acoustic template for "jarvis".
    /// Profile: Affricate 'j' -> vowel 'ar' -> fricative 'v' -> sibilant 'is'.
    fn jarvis_template(keyword: &str) -> Self {
        let mut frames = Vec::new();

        // 1. "j" (~4 frames)
        for _ in 0..4 {
            frames.push(TargetFrame {
                subbands: [0.25, 0.35, 0.30, 0.10],
                zcr: 0.22,
                centroid: 1700.0,
                weight: 1.0,
                is_fricative: false,
            });
        }

        // 2. "ar" vowel (~8 frames)
        for _ in 0..8 {
            frames.push(TargetFrame {
                subbands: [0.45, 0.40, 0.12, 0.03],
                zcr: 0.08,
                centroid: 750.0,
                weight: 1.2,
                is_fricative: false,
            });
        }

        // 3. "v" voiced fricative (~4 frames)
        for _ in 0..4 {
            frames.push(TargetFrame {
                subbands: [0.35, 0.25, 0.25, 0.15],
                zcr: 0.20,
                centroid: 1400.0,
                weight: 0.9,
                is_fricative: false,
            });
        }

        // 4. "is" vowel + sibilant 's' (~8 frames)
        for _ in 0..8 {
            frames.push(TargetFrame {
                subbands: [0.08, 0.15, 0.30, 0.47],
                zcr: 0.38,
                centroid: 3800.0,
                weight: 1.4,
                is_fricative: true,
            });
        }

        Self {
            keyword: keyword.to_string(),
            frames,
            min_duration_ms: 240,
            max_duration_ms: 1500,
        }
    }

    /// Synthesizes acoustic phonetic targets for an arbitrary keyword phrase.
    fn synthesize_template_from_phrase(keyword: &str) -> Self {
        let mut frames = Vec::new();

        for ch in keyword.chars() {
            match ch.to_ascii_lowercase() {
                'a' | 'o' | 'u' => {
                    // Open back vowels: strong low band
                    for _ in 0..4 {
                        frames.push(TargetFrame {
                            subbands: [0.45, 0.40, 0.12, 0.03],
                            zcr: 0.07,
                            centroid: 700.0,
                            weight: 1.0,
                            is_fricative: false,
                        });
                    }
                }
                'e' | 'i' | 'y' => {
                    // Front vowels: mid formant
                    for _ in 0..4 {
                        frames.push(TargetFrame {
                            subbands: [0.30, 0.45, 0.20, 0.05],
                            zcr: 0.10,
                            centroid: 1100.0,
                            weight: 1.0,
                            is_fricative: false,
                        });
                    }
                }
                's' | 'z' | 'x' => {
                    // Sibilants: sharp high frequency
                    for _ in 0..5 {
                        frames.push(TargetFrame {
                            subbands: [0.05, 0.10, 0.25, 0.60],
                            zcr: 0.45,
                            centroid: 4500.0,
                            weight: 1.4,
                            is_fricative: true,
                        });
                    }
                }
                'f' | 'v' | 'h' => {
                    // Fricatives: broad high-mid
                    for _ in 0..3 {
                        frames.push(TargetFrame {
                            subbands: [0.15, 0.25, 0.35, 0.25],
                            zcr: 0.25,
                            centroid: 2500.0,
                            weight: 1.0,
                            is_fricative: true,
                        });
                    }
                }
                'm' | 'n' | 'l' | 'r' | 'w' => {
                    // Nasals & liquids: low formant
                    for _ in 0..4 {
                        frames.push(TargetFrame {
                            subbands: [0.50, 0.30, 0.15, 0.05],
                            zcr: 0.08,
                            centroid: 650.0,
                            weight: 1.0,
                            is_fricative: false,
                        });
                    }
                }
                ' ' => {
                    // Word boundary pause
                    frames.push(TargetFrame {
                        subbands: [0.25, 0.25, 0.25, 0.25],
                        zcr: 0.12,
                        centroid: 1200.0,
                        weight: 0.5,
                        is_fricative: false,
                    });
                }
                _ => {
                    // Consonant stops / default
                    for _ in 0..3 {
                        frames.push(TargetFrame {
                            subbands: [0.25, 0.35, 0.25, 0.15],
                            zcr: 0.20,
                            centroid: 1600.0,
                            weight: 1.0,
                            is_fricative: false,
                        });
                    }
                }
            }
        }

        let num_frames = frames.len().max(10);
        let duration_ms = (num_frames * 10) as u32;

        Self {
            keyword: keyword.to_string(),
            frames,
            min_duration_ms: duration_ms.saturating_sub(150).max(200),
            max_duration_ms: duration_ms.saturating_add(500),
        }
    }
}

/// Offline acoustic keyword spotter running in real time without external model files.
pub struct AcousticKwsSpotter {
    config: WakeWordConfig,
    templates: Vec<KeywordTemplate>,
    feature_history: VecDeque<AcousticFeature>,
    unprocessed_samples: Vec<f32>,
    samples_consumed: u64,
    last_trigger_ms: Option<u64>,
    hann_window: [f32; FFT_SIZE],
}

impl std::fmt::Debug for AcousticKwsSpotter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcousticKwsSpotter")
            .field("config", &self.config)
            .field("templates_count", &self.templates.len())
            .field("feature_history_len", &self.feature_history.len())
            .field("samples_consumed", &self.samples_consumed)
            .field("last_trigger_ms", &self.last_trigger_ms)
            .finish()
    }
}

impl AcousticKwsSpotter {
    /// Creates a new acoustic keyword spotter from the given configuration.
    pub fn new(config: WakeWordConfig) -> Result<Self, SpeechError> {
        config.validate()?;

        let templates = config
            .keywords
            .iter()
            .map(|kw| KeywordTemplate::for_keyword(kw))
            .collect();

        // Precompute Hann window
        let mut hann_window = [0.0f32; FFT_SIZE];
        for (i, item) in hann_window.iter_mut().enumerate() {
            *item = 0.5 * (1.0 - (2.0 * PI * (i as f32) / (FFT_SIZE as f32 - 1.0)).cos());
        }

        Ok(Self {
            config,
            templates,
            feature_history: VecDeque::with_capacity(200),
            unprocessed_samples: Vec::with_capacity(1024),
            samples_consumed: 0,
            last_trigger_ms: None,
            hann_window,
        })
    }

    /// Extracts acoustic feature representation from a 512-sample frame.
    fn extract_frame_feature(&self, frame: &[f32]) -> AcousticFeature {
        debug_assert_eq!(frame.len(), FFT_SIZE);

        // 1. RMS Energy & dBFS
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
        let is_silence = energy_dbfs < self.config.min_energy_dbfs;
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

        // 2. Windowed Real FFT
        let mut real = [0.0f32; FFT_SIZE];
        let mut imag = [0.0f32; FFT_SIZE];
        for i in 0..FFT_SIZE {
            real[i] = frame[i] * self.hann_window[i];
        }

        Self::cooley_tukey_fft(&mut real, &mut imag);

        // 3. Compute Power Spectrum & Subbands
        // Frequency resolution: 16000 / 512 = 31.25 Hz per bin.
        // Band 0: bins 3..13   (93 - 406 Hz)
        // Band 1: bins 13..38  (406 - 1187 Hz)
        // Band 2: bins 38..96  (1187 - 3000 Hz)
        // Band 3: bins 96..240 (3000 - 7500 Hz)
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

    /// In-place radix-2 Cooley-Tukey FFT implementation.
    fn cooley_tukey_fft(real: &mut [f32; FFT_SIZE], imag: &mut [f32; FFT_SIZE]) {
        // Bit-reversal permutation
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

        // Cooley-Tukey butterflies
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

    /// Evaluates alignment similarity between feature frames and a keyword template.
    /// Returns confidence score in range `[0.0, 1.0]`.
    ///
    /// Performance optimized: Zero allocations during continuous template correlation.
    fn match_template(&self, template: &KeywordTemplate) -> f32 {
        let template_len = template.frames.len();
        if self.feature_history.len() < template_len {
            return 0.0;
        }

        let start_idx = self.feature_history.len() - template_len;

        // 1. Guard against all-silence or pure tone / static noise
        let silence_count = (0..template_len)
            .filter(|&i| self.feature_history[start_idx + i].is_silence)
            .count();
        if silence_count > template_len / 2 {
            return 0.0;
        }

        // Spectral dynamics check: real speech features must vary over time
        let mut subband_variance = 0.0f32;
        for band in 0..4 {
            let mean = (0..template_len)
                .map(|i| self.feature_history[start_idx + i].subbands[band])
                .sum::<f32>()
                / (template_len as f32);
            let var = (0..template_len)
                .map(|i| (self.feature_history[start_idx + i].subbands[band] - mean).powi(2))
                .sum::<f32>()
                / (template_len as f32);
            subband_variance += var;
        }

        // Pure tones (e.g., sine wave 440 Hz) and white noise have low subband variance across frames
        if subband_variance < 0.002 {
            return 0.0;
        }

        // 2. Compute weighted frame-by-frame similarity
        let mut total_score = 0.0f32;
        let mut total_weight = 0.0f32;

        for (i, t_frame) in template.frames.iter().enumerate() {
            let a_frame = &self.feature_history[start_idx + i];
            if a_frame.is_silence {
                total_weight += t_frame.weight;
                continue;
            }

            // Subband cosine similarity
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

            // Phonological category discrimination: sibilant vs non-sibilant
            let is_audio_fricative = a_frame.zcr > 0.30 && a_frame.subbands[3] > 0.40;
            if t_frame.is_fricative != is_audio_fricative {
                // Penalize mismatch between sibilant and non-sibilant phonemes
                total_score += subband_sim * 0.1 * t_frame.weight;
                total_weight += t_frame.weight;
                continue;
            }

            // ZCR proximity
            let zcr_diff = (a_frame.zcr - t_frame.zcr).abs();
            let zcr_sim = (1.0 - zcr_diff * 2.0).clamp(0.0, 1.0);

            // Centroid proximity
            let centroid_diff =
                (a_frame.spectral_centroid - t_frame.centroid).abs() / 4000.0f32.max(1.0);
            let centroid_sim = (1.0 - centroid_diff).clamp(0.0, 1.0);

            // Combined frame score
            let frame_score = subband_sim * 0.50 + zcr_sim * 0.30 + centroid_sim * 0.20;

            total_score += frame_score * t_frame.weight;
            total_weight += t_frame.weight;
        }

        let frame_sim = if total_weight > 0.0 {
            (total_score / total_weight).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // 3. Pearson trajectory correlation across time
        let mut trajectory_corr_sum = 0.0f32;
        let mut trajectory_dim_count = 0.0f32;

        for b in 0..4 {
            let mean_a = (0..template_len)
                .map(|i| self.feature_history[start_idx + i].subbands[b])
                .sum::<f32>()
                / (template_len as f32);
            let mean_t =
                template.frames.iter().map(|f| f.subbands[b]).sum::<f32>() / (template_len as f32);

            let mut cov = 0.0f32;
            let mut var_a = 0.0f32;
            let mut var_t = 0.0f32;

            for (i, t) in template.frames.iter().enumerate() {
                let a = &self.feature_history[start_idx + i];
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
                } else {
                    trajectory_corr_sum += 0.0;
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

impl WakeWordSpotter for AcousticKwsSpotter {
    fn process_samples(&mut self, samples: &[f32]) -> Result<Option<WakeWordEvent>, SpeechError> {
        self.unprocessed_samples.extend_from_slice(samples);

        let mut detected_event: Option<WakeWordEvent> = None;

        // Slide window by slide_step
        while self.unprocessed_samples.len() >= FFT_SIZE {
            let frame = &self.unprocessed_samples[..FFT_SIZE];
            let feature = self.extract_frame_feature(frame);

            self.feature_history.push_back(feature);
            if self.feature_history.len() > 150 {
                self.feature_history.pop_front();
            }

            self.unprocessed_samples.drain(..self.config.slide_step);
            self.samples_consumed = self
                .samples_consumed
                .saturating_add(self.config.slide_step as u64);

            let current_time_ms =
                (self.samples_consumed as f64 / self.config.sample_rate as f64 * 1000.0) as u64;

            // Debounce check
            if let Some(last_ms) = self.last_trigger_ms {
                let debounce_elapsed = current_time_ms.saturating_sub(last_ms);
                if debounce_elapsed < self.config.min_keyword_interval_ms as u64 {
                    continue;
                }
            }

            // Evaluate each template
            for template in &self.templates {
                let confidence = self.match_template(template);
                if confidence >= self.config.threshold {
                    let template_duration_samples =
                        (template.frames.len() * self.config.slide_step) as u64;
                    let end_sample =
                        self.samples_consumed + (FFT_SIZE - self.config.slide_step) as u64;
                    let start_sample = end_sample.saturating_sub(template_duration_samples);

                    self.last_trigger_ms = Some(current_time_ms);
                    detected_event = Some(WakeWordEvent {
                        keyword: template.keyword.clone(),
                        confidence,
                        timestamp_ms: current_time_ms,
                        start_sample,
                        end_sample,
                    });
                    // Advance history to avoid immediate duplicate triggers from same utterance
                    self.feature_history.clear();
                    break;
                }
            }

            if detected_event.is_some() {
                break;
            }
        }

        Ok(detected_event)
    }

    fn keywords(&self) -> &[String] {
        &self.config.keywords
    }

    fn set_keywords(&mut self, keywords: &[String]) -> Result<(), SpeechError> {
        let mut new_config = self.config.clone();
        new_config.keywords = keywords.to_vec();
        new_config.validate()?;

        self.templates = new_config
            .keywords
            .iter()
            .map(|kw| KeywordTemplate::for_keyword(kw))
            .collect();
        self.config = new_config;
        self.reset();
        Ok(())
    }

    fn set_threshold(&mut self, threshold: f32) -> Result<(), SpeechError> {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(SpeechError::ParameterError(format!(
                "Threshold {} is out of range [0.0, 1.0]",
                threshold
            )));
        }
        self.config.threshold = threshold;
        Ok(())
    }

    fn reset(&mut self) {
        self.feature_history.clear();
        self.unprocessed_samples.clear();
        self.samples_consumed = 0;
        self.last_trigger_ms = None;
    }
}
