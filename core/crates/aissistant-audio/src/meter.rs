use serde::{Deserialize, Serialize};

/// Audio telemetry snapshot representing instantaneous level metrics.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioLevelSnapshot {
    /// Root-mean-square energy level in decibels relative to full scale (-96.0 to 0.0 dBFS).
    pub rms_dbfs: f32,
    /// Peak amplitude in decibels relative to full scale (-96.0 to 0.0 dBFS).
    pub peak_dbfs: f32,
    /// Normalized RMS level mapped linearly from [floor_dbfs, 0] to [0.0, 1.0].
    pub normalized_rms: f32,
    /// Normalized peak level with smoothed ballistics [0.0, 1.0].
    pub normalized_peak: f32,
    /// True if the signal peak exceeds full scale threshold (-0.05 dBFS or >= 0.995).
    pub is_clipping: bool,
    /// True if signal RMS is below ambient silence threshold (default -60 dBFS).
    pub is_silent: bool,
}

/// Real-time audio level meter computing RMS energy, peak amplitude, and smoothed ballistics.
#[derive(Debug, Clone)]
pub struct AudioLevelMeter {
    floor_dbfs: f32,
    silence_threshold_dbfs: f32,
    decay_coeff: f32,
    smoothed_peak: f32,
}

impl Default for AudioLevelMeter {
    fn default() -> Self {
        Self::new(-96.0, -60.0, 0.85)
    }
}

impl AudioLevelMeter {
    /// Create a new level meter with custom floor, silence threshold, and release decay.
    pub fn new(floor_dbfs: f32, silence_threshold_dbfs: f32, decay_coeff: f32) -> Self {
        Self {
            floor_dbfs,
            silence_threshold_dbfs,
            decay_coeff: decay_coeff.clamp(0.0, 0.999),
            smoothed_peak: 0.0,
        }
    }

    /// Process a block of audio samples and compute the level metrics.
    ///
    /// Real-time safe: Zero allocations, zero locks.
    pub fn process(&mut self, samples: &[f32]) -> AudioLevelSnapshot {
        if samples.is_empty() {
            return AudioLevelSnapshot {
                rms_dbfs: self.floor_dbfs,
                peak_dbfs: self.floor_dbfs,
                normalized_rms: 0.0,
                normalized_peak: self.smoothed_peak,
                is_clipping: false,
                is_silent: true,
            };
        }

        let mut sum_sq = 0.0f32;
        let mut peak_val = 0.0f32;

        for &s in samples {
            let abs_s = s.abs();
            if abs_s > peak_val {
                peak_val = abs_s;
            }
            sum_sq += s * s;
        }

        let rms_linear = (sum_sq / samples.len() as f32).sqrt();

        // Convert to dBFS
        let rms_dbfs = Self::linear_to_dbfs(rms_linear, self.floor_dbfs);
        let peak_dbfs = Self::linear_to_dbfs(peak_val, self.floor_dbfs);

        // Normalize to [0.0, 1.0]
        let range = -self.floor_dbfs;
        let norm_rms = ((rms_dbfs - self.floor_dbfs) / range).clamp(0.0, 1.0);
        let raw_norm_peak = ((peak_dbfs - self.floor_dbfs) / range).clamp(0.0, 1.0);

        // Ballistics: Instant attack, smooth exponential decay release
        if raw_norm_peak >= self.smoothed_peak {
            self.smoothed_peak = raw_norm_peak;
        } else {
            self.smoothed_peak =
                self.smoothed_peak * self.decay_coeff + raw_norm_peak * (1.0 - self.decay_coeff);
        }

        let is_clipping = peak_val >= 0.995;
        let is_silent = rms_dbfs < self.silence_threshold_dbfs;

        AudioLevelSnapshot {
            rms_dbfs,
            peak_dbfs,
            normalized_rms: norm_rms,
            normalized_peak: self.smoothed_peak,
            is_clipping,
            is_silent,
        }
    }

    /// Reset meter state.
    pub fn reset(&mut self) {
        self.smoothed_peak = 0.0;
    }

    #[inline]
    fn linear_to_dbfs(linear: f32, floor_dbfs: f32) -> f32 {
        if linear <= 1e-5 {
            floor_dbfs
        } else {
            (20.0 * linear.log10()).max(floor_dbfs).min(0.0)
        }
    }
}
