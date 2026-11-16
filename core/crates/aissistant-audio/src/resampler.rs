use crate::error::AudioError;

/// Continuous streaming audio resampler and multi-channel downmixer.
///
/// Converts multi-channel interleaved audio of any standard sample rate
/// (e.g. 44.1 kHz, 48 kHz, 96 kHz) into standardized 16,000 Hz mono PCM
/// required by speech-to-text, wake-word spotting, and VAD engines.
#[derive(Debug, Clone)]
pub struct AudioResampler {
    in_rate: u32,
    out_rate: u32,
    in_channels: u16,
    step: f64,
    phase: f64,
    last_sample: f32,
    has_last_sample: bool,
    mono_scratch: Vec<f32>,
}

impl AudioResampler {
    /// Create a new stream resampler.
    pub fn new(in_rate: u32, out_rate: u32, in_channels: u16) -> Result<Self, AudioError> {
        if !(4_000..=192_000).contains(&in_rate) {
            return Err(AudioError::InvalidSampleRate {
                rate: in_rate,
                reason: "Sample rate must be between 4 kHz and 192 kHz".to_string(),
            });
        }

        if !(4_000..=192_000).contains(&out_rate) {
            return Err(AudioError::InvalidSampleRate {
                rate: out_rate,
                reason: "Target sample rate must be between 4 kHz and 192 kHz".to_string(),
            });
        }

        if !(1..=8).contains(&in_channels) {
            return Err(AudioError::InvalidChannelCount {
                channels: in_channels,
                reason: "Supported channel count is between 1 and 8".to_string(),
            });
        }

        let step = in_rate as f64 / out_rate as f64;

        Ok(Self {
            in_rate,
            out_rate,
            in_channels,
            step,
            phase: 0.0,
            last_sample: 0.0,
            has_last_sample: false,
            mono_scratch: Vec::with_capacity(512),
        })
    }

    /// Helper to create standard 16 kHz mono resampler from hardware input specs.
    pub fn to_speech_standard(in_rate: u32, in_channels: u16) -> Result<Self, AudioError> {
        Self::new(in_rate, 16_000, in_channels)
    }

    /// Input sample rate in Hz.
    #[inline]
    pub fn in_rate(&self) -> u32 {
        self.in_rate
    }

    /// Output sample rate in Hz.
    #[inline]
    pub fn out_rate(&self) -> u32 {
        self.out_rate
    }

    /// Input channels count.
    #[inline]
    pub fn in_channels(&self) -> u16 {
        self.in_channels
    }

    /// Resample a chunk of interleaved multi-channel input samples into mono output.
    ///
    /// Preserves internal phase state across calls so consecutive chunks blend without clicks.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) -> Result<usize, AudioError> {
        let ch = self.in_channels as usize;
        if input.is_empty() {
            return Ok(0);
        }

        if input.len() % ch != 0 {
            return Err(AudioError::ParameterError(format!(
                "Input length ({}) must be a multiple of channel count ({})",
                input.len(),
                ch
            )));
        }

        let num_frames = input.len() / ch;
        let start_len = output.len();

        // 1. Downmix to temporary mono frames if multi-channel (reusing scratch buffer)
        let mono_input: &[f32] = if ch == 1 {
            input
        } else {
            let inv_ch = 1.0f32 / (ch as f32);
            self.mono_scratch.clear();
            self.mono_scratch.reserve(num_frames);
            for frame in 0..num_frames {
                let offset = frame * ch;
                let sum: f32 = input[offset..offset + ch].iter().sum();
                self.mono_scratch.push(sum * inv_ch);
            }
            &self.mono_scratch
        };

        // 2. Exact 1:1 pass-through if rates match
        if self.in_rate == self.out_rate {
            output.extend_from_slice(mono_input);
            if let Some(&last) = mono_input.last() {
                self.last_sample = last;
                self.has_last_sample = true;
            }
            return Ok(mono_input.len());
        }

        // 3. Fractional linear interpolation across stream boundaries
        let mut pos = self.phase;
        let expected_out = ((num_frames as f64 - pos).max(0.0) / self.step).ceil() as usize;
        output.reserve(expected_out);

        while pos < num_frames as f64 {
            let idx = pos.floor() as usize;
            let frac = (pos - idx as f64) as f32;

            let current = mono_input[idx];
            let next = if idx + 1 < num_frames {
                mono_input[idx + 1]
            } else {
                current
            };

            let interpolated = current + frac * (next - current);

            output.push(interpolated);
            pos += self.step;
        }

        // Carry forward remaining phase fraction into next chunk
        self.phase = pos - num_frames as f64;
        if let Some(&last) = mono_input.last() {
            self.last_sample = last;
            self.has_last_sample = true;
        }

        Ok(output.len() - start_len)
    }

    /// Reset internal phase and history buffers.
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.last_sample = 0.0;
        self.has_last_sample = false;
    }
}

/// Standalone batch resampling utility.
pub fn resample_batch(
    input: &[f32],
    in_rate: u32,
    in_channels: u16,
    out_rate: u32,
) -> Result<Vec<f32>, AudioError> {
    let mut resampler = AudioResampler::new(in_rate, out_rate, in_channels)?;
    let mut output = Vec::with_capacity(
        (input.len() as f64 * out_rate as f64 / (in_rate as f64 * in_channels as f64)) as usize
            + 16,
    );
    resampler.process(input, &mut output)?;
    Ok(output)
}
