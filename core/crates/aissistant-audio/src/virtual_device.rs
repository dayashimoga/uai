use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use crate::error::AudioError;
use crate::meter::{AudioLevelMeter, AudioLevelSnapshot};
use crate::resampler::AudioResampler;
use crate::ring_buffer::{AudioRingBuffer, RingBufferConsumer, RingBufferMode};

/// Deterministic in-memory virtual audio driver for CI and headless environments.
///
/// Enables complete integration testing of capture, resampling, ring buffer ingestion,
/// mute enforcement, level metering, and playback without requiring physical hardware.
#[derive(Debug, Clone)]
pub struct VirtualAudioDevice {
    capture_ring: AudioRingBuffer,
    playback_ring: AudioRingBuffer,
    resampler: Arc<Mutex<AudioResampler>>,
    meter: Arc<Mutex<AudioLevelMeter>>,
    is_capturing: Arc<AtomicBool>,
    is_playing: Arc<AtomicBool>,
    is_muted: Arc<AtomicBool>,
}

impl VirtualAudioDevice {
    /// Create a new virtual audio device with 16 kHz mono output specs.
    pub fn new(capacity_samples: usize) -> Result<Self, AudioError> {
        let capture_ring = AudioRingBuffer::new(capacity_samples, RingBufferMode::Overwrite);
        let playback_ring = AudioRingBuffer::new(capacity_samples, RingBufferMode::Strict);
        let resampler = AudioResampler::to_speech_standard(16_000, 1)?;
        let meter = AudioLevelMeter::default();

        Ok(Self {
            capture_ring,
            playback_ring,
            resampler: Arc::new(Mutex::new(resampler)),
            meter: Arc::new(Mutex::new(meter)),
            is_capturing: Arc::new(AtomicBool::new(false)),
            is_playing: Arc::new(AtomicBool::new(false)),
            is_muted: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Start virtual capture.
    pub fn start_capture(&self) {
        self.is_capturing.store(true, Ordering::Release);
    }

    /// Pause virtual capture.
    pub fn pause_capture(&self) {
        self.is_capturing.store(false, Ordering::Release);
    }

    /// True if capture is active.
    pub fn is_capturing(&self) -> bool {
        self.is_capturing.load(Ordering::Acquire)
    }

    /// Set software mute.
    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::Release);
    }

    /// True if software muted.
    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Acquire)
    }

    /// Push synthetic audio frames into the virtual microphone.
    ///
    /// If muted, zeroes are injected instead. Resamples and updates level meter.
    pub fn inject_capture_samples(
        &self,
        samples: &[f32],
        source_rate: u32,
        channels: u16,
    ) -> Result<usize, AudioError> {
        if !self.is_capturing.load(Ordering::Acquire) {
            return Ok(0);
        }

        let is_muted = self.is_muted.load(Ordering::Acquire);
        let effective_samples = if is_muted {
            vec![0.0f32; samples.len()]
        } else {
            samples.to_vec()
        };

        let mut resampler = self
            .resampler
            .lock()
            .map_err(|e| AudioError::BackendError(format!("Lock poisoned: {e}")))?;
        let mut mono_16k = Vec::new();

        if source_rate != 16_000 || channels != 1 {
            let mut temp_resampler = AudioResampler::new(source_rate, 16_000, channels)?;
            temp_resampler.process(&effective_samples, &mut mono_16k)?;
        } else {
            resampler.process(&effective_samples, &mut mono_16k)?;
        }

        // Update level meter
        if let Ok(mut meter) = self.meter.lock() {
            meter.process(&mono_16k);
        }

        // Push to capture ring buffer
        let pushed = self.capture_ring.push_slice(&mono_16k);
        Ok(pushed)
    }

    /// Obtain consumer handle to read captured 16 kHz mono audio.
    pub fn capture_consumer(&self) -> RingBufferConsumer {
        let (_, consumer) = self.capture_ring.clone().split();
        consumer
    }

    /// Get current audio level snapshot.
    pub fn get_level_snapshot(&self) -> AudioLevelSnapshot {
        if let Ok(meter) = self.meter.lock() {
            meter.current_snapshot()
        } else {
            AudioLevelMeter::default().current_snapshot()
        }
    }

    /// Start virtual playback.
    pub fn start_playback(&self) {
        self.is_playing.store(true, Ordering::Release);
    }

    /// Pause virtual playback.
    pub fn pause_playback(&self) {
        self.is_playing.store(false, Ordering::Release);
    }

    /// True if playback is active.
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Acquire)
    }

    /// Queue audio samples for playback.
    pub fn queue_playback_samples(&self, samples: &[f32]) -> usize {
        self.playback_ring.push_slice(samples)
    }

    /// Consume pending playback samples (for test verification of speaker output).
    pub fn read_playback_samples(&self, out: &mut [f32]) -> usize {
        self.playback_ring.pop_slice(out)
    }

    /// Instant flush of all pending playback audio (barge-in interruption).
    pub fn flush_playback(&self) {
        self.playback_ring.clear();
    }
}
