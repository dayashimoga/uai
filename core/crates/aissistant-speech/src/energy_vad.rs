use crate::error::SpeechError;
use crate::vad::VadEngine;

/// Deterministic offline energy and zero-crossing rate Voice Activity Detection engine.
///
/// Computes adaptive ambient noise floor estimation and SNR-scaled speech probabilities
/// with zero external model dependencies or network calls.
#[derive(Debug, Clone)]
pub struct EnergyVadEngine {
    window_size: usize,
    noise_floor_energy: f32,
    alpha_noise_adapt: f32,
    snr_threshold_db: f32,
    sigmoid_steepness: f32,
    initialized_frames: usize,
}

impl Default for EnergyVadEngine {
    fn default() -> Self {
        Self::new(512, 12.0)
    }
}

impl EnergyVadEngine {
    /// Create a new EnergyVadEngine with specified window size and SNR threshold in decibels.
    pub fn new(window_size: usize, snr_threshold_db: f32) -> Self {
        Self {
            window_size,
            noise_floor_energy: 1e-4, // ~ -40 dBFS initial floor
            alpha_noise_adapt: 0.95,
            snr_threshold_db,
            sigmoid_steepness: 0.35,
            initialized_frames: 0,
        }
    }

    /// Set adaptive noise update rate (0.8 to 0.99).
    pub fn set_adapt_rate(&mut self, rate: f32) {
        self.alpha_noise_adapt = rate.clamp(0.5, 0.999);
    }
}

impl VadEngine for EnergyVadEngine {
    fn process_window(&mut self, window: &[f32]) -> Result<f32, SpeechError> {
        if window.is_empty() {
            return Ok(0.0);
        }

        let mut sum_sq = 0.0f32;
        let mut zero_crossings = 0;
        let mut prev = window[0];

        for &sample in window {
            sum_sq += sample * sample;
            if (sample >= 0.0 && prev < 0.0) || (sample < 0.0 && prev >= 0.0) {
                zero_crossings += 1;
            }
            prev = sample;
        }

        let frame_energy = (sum_sq / window.len() as f32).max(1e-9);
        let _zcr = zero_crossings as f32 / window.len() as f32;

        // Initialize noise floor with first few frames
        if self.initialized_frames < 10 {
            self.noise_floor_energy = if self.initialized_frames == 0 {
                frame_energy
            } else {
                self.noise_floor_energy.min(frame_energy)
            };
            self.initialized_frames += 1;
        }

        let snr_db = 10.0 * (frame_energy / self.noise_floor_energy.max(1e-9)).log10();

        // Update noise floor if current frame is likely background silence
        if snr_db < self.snr_threshold_db {
            self.noise_floor_energy = self.noise_floor_energy * self.alpha_noise_adapt
                + frame_energy * (1.0 - self.alpha_noise_adapt);
        }

        // Calibrated sigmoid function mapping SNR to speech probability [0.0, 1.0]
        let x = self.sigmoid_steepness * (snr_db - self.snr_threshold_db);
        let prob = 1.0 / (1.0 + (-x).exp());

        Ok(prob.clamp(0.0, 1.0))
    }

    fn reset(&mut self) {
        self.noise_floor_energy = 1e-4;
        self.initialized_frames = 0;
    }

    fn window_size(&self) -> usize {
        self.window_size
    }
}
