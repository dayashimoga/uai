use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::device::AudioDeviceManager;
use crate::error::AudioError;
use crate::ring_buffer::{AudioRingBuffer, RingBufferMode};
use crate::virtual_device::VirtualAudioDevice;

/// Configuration for real-time audio playback.
#[derive(Debug, Clone)]
pub struct AudioPlaybackConfig {
    /// Target hardware output device name (None for default).
    pub device_name: Option<String>,
    /// Buffer capacity in samples for synthesized audio.
    pub buffer_capacity: usize,
}

impl Default for AudioPlaybackConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            buffer_capacity: 65_536,
        }
    }
}

/// Real-time audio playback stream with volume scaling and instant barge-in flushing.
pub struct AudioPlaybackStream {
    _stream: Option<Stream>,
    virtual_device: Option<VirtualAudioDevice>,
    playback_ring: AudioRingBuffer,
    volume: Arc<AtomicU32>,
    is_running: Arc<AtomicBool>,
}

impl std::fmt::Debug for AudioPlaybackStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioPlaybackStream")
            .field("is_running", &self.is_running.load(Ordering::Relaxed))
            .field("volume", &self.get_volume())
            .field("pending_samples", &self.playback_ring.available_read())
            .finish()
    }
}

impl AudioPlaybackStream {
    /// Initialize a new audio playback stream bound to the specified or default hardware output.
    pub fn new(config: AudioPlaybackConfig) -> Result<Self, AudioError> {
        let playback_ring = AudioRingBuffer::new(config.buffer_capacity, RingBufferMode::Strict);
        let volume = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let is_running = Arc::new(AtomicBool::new(false));

        // Attempt physical hardware discovery
        match AudioDeviceManager::find_output_cpal_device(config.device_name.as_deref()) {
            Ok(device) => {
                let default_cfg = device.default_output_config().map_err(|e| {
                    AudioError::BackendError(format!("Failed to query default output config: {e}"))
                })?;

                let channels = default_cfg.channels();
                let sample_format = default_cfg.sample_format();

                let (ring_producer, ring_consumer) = playback_ring.clone().split();
                let _ = ring_producer; // kept via playback_ring

                let stream = Self::build_cpal_output_stream(
                    &device,
                    &default_cfg.into(),
                    sample_format,
                    ring_consumer,
                    channels,
                    Arc::clone(&volume),
                )?;

                Ok(Self {
                    _stream: Some(stream),
                    virtual_device: None,
                    playback_ring,
                    volume,
                    is_running,
                })
            }
            Err(_) => {
                // Fallback to virtual audio driver for headless environments
                let virtual_dev = VirtualAudioDevice::new(config.buffer_capacity)?;
                Ok(Self {
                    _stream: None,
                    virtual_device: Some(virtual_dev),
                    playback_ring,
                    volume,
                    is_running,
                })
            }
        }
    }

    /// Create a virtual playback stream explicitly for deterministic testing.
    pub fn new_virtual(capacity_samples: usize) -> Result<Self, AudioError> {
        let virtual_dev = VirtualAudioDevice::new(capacity_samples)?;
        let playback_ring = AudioRingBuffer::new(capacity_samples, RingBufferMode::Strict);
        let volume = Arc::new(AtomicU32::new(1.0f32.to_bits()));
        let is_running = Arc::new(AtomicBool::new(false));

        Ok(Self {
            _stream: None,
            virtual_device: Some(virtual_dev),
            playback_ring,
            volume,
            is_running,
        })
    }

    /// Start audio playback.
    pub fn start(&self) -> Result<(), AudioError> {
        if let Some(stream) = &self._stream {
            stream
                .play()
                .map_err(|e| AudioError::StreamPlayError(e.to_string()))?;
        } else if let Some(v) = &self.virtual_device {
            v.start_playback();
        }
        self.is_running.store(true, Ordering::Release);
        Ok(())
    }

    /// Pause audio playback.
    pub fn pause(&self) -> Result<(), AudioError> {
        if let Some(stream) = &self._stream {
            stream
                .pause()
                .map_err(|e| AudioError::StreamPlayError(e.to_string()))?;
        } else if let Some(v) = &self.virtual_device {
            v.pause_playback();
        }
        self.is_running.store(false, Ordering::Release);
        Ok(())
    }

    /// Queue synthesized speech samples for playback. Returns count of samples accepted.
    pub fn play_samples(&self, samples: &[f32]) -> usize {
        if let Some(v) = &self.virtual_device {
            v.queue_playback_samples(samples)
        } else {
            self.playback_ring.push_slice(samples)
        }
    }

    /// Instant flush of all pending playback audio (vital for barge-in interruption).
    pub fn flush(&self) {
        if let Some(v) = &self.virtual_device {
            v.flush_playback();
        } else {
            self.playback_ring.clear();
        }
    }

    /// Set playback volume (0.0 to 1.0).
    pub fn set_volume(&self, vol: f32) {
        let clamped = vol.clamp(0.0, 1.0);
        self.volume.store(clamped.to_bits(), Ordering::Release);
    }

    /// Get current playback volume.
    pub fn get_volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Acquire))
    }

    /// True if playback stream is active.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Acquire)
    }

    /// Access the underlying virtual driver (if in virtual mode).
    pub fn virtual_device(&self) -> Option<&VirtualAudioDevice> {
        self.virtual_device.as_ref()
    }

    fn build_cpal_output_stream(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        format: SampleFormat,
        consumer: crate::ring_buffer::RingBufferConsumer,
        channels: u16,
        volume: Arc<AtomicU32>,
    ) -> Result<Stream, AudioError> {
        let err_fn = |err| {
            eprintln!("Audio playback stream error: {err}");
        };

        match format {
            SampleFormat::F32 => device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], _| {
                        Self::fill_output_chunk(data, &consumer, channels, &volume);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamBuildError(e.to_string())),
            _ => Err(AudioError::StreamBuildError(format!(
                "Unsupported output sample format: {format:?}"
            ))),
        }
    }

    #[inline]
    fn fill_output_chunk(
        output: &mut [f32],
        consumer: &crate::ring_buffer::RingBufferConsumer,
        channels: u16,
        volume: &Arc<AtomicU32>,
    ) {
        let vol = f32::from_bits(volume.load(Ordering::Relaxed));
        let ch = channels as usize;

        if ch == 1 {
            let read = consumer.pop_slice(output);
            for s in &mut output[..read] {
                *s *= vol;
            }
            output[read..].fill(0.0);
        } else {
            // Downmixed / mono into multi-channel output
            let frames = output.len() / ch;
            for frame in 0..frames {
                let sample = consumer.pop().unwrap_or(0.0) * vol;
                let offset = frame * ch;
                for c in 0..ch {
                    output[offset + c] = sample;
                }
            }
        }
    }
}
