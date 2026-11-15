use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream};

use crate::device::AudioDeviceManager;
use crate::error::AudioError;
use crate::meter::{AudioLevelMeter, AudioLevelSnapshot};
use crate::resampler::AudioResampler;
use crate::ring_buffer::{AudioRingBuffer, RingBufferConsumer, RingBufferMode, RingBufferProducer};
use crate::virtual_device::VirtualAudioDevice;

/// Configuration for real-time audio capture.
#[derive(Debug, Clone)]
pub struct AudioCaptureConfig {
    /// Target hardware input device name (None for default).
    pub device_name: Option<String>,
    /// Target sample rate for downstream speech processing (standard 16,000 Hz).
    pub target_sample_rate: u32,
    /// Ring buffer capacity in samples (e.g. 5 seconds * 16,000 = 80,000 samples).
    pub ring_buffer_capacity: usize,
    /// Ring buffer behavior when full (typically Overwrite for wake-word spotting).
    pub ring_buffer_mode: RingBufferMode,
}

impl Default for AudioCaptureConfig {
    fn default() -> Self {
        Self {
            device_name: None,
            target_sample_rate: 16_000,
            ring_buffer_capacity: 131_072, // ~8.19 seconds of 16 kHz audio
            ring_buffer_mode: RingBufferMode::Overwrite,
        }
    }
}

/// Real-time audio capture stream feeding resampled 16 kHz mono frames into a lock-free ring buffer.
pub struct AudioCaptureStream {
    _stream: Option<Stream>,
    virtual_device: Option<VirtualAudioDevice>,
    ring_buffer: AudioRingBuffer,
    meter: Arc<Mutex<AudioLevelMeter>>,
    is_running: Arc<AtomicBool>,
    is_muted: Arc<AtomicBool>,
    native_sample_rate: u32,
    native_channels: u16,
}

impl std::fmt::Debug for AudioCaptureStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioCaptureStream")
            .field("is_running", &self.is_running.load(Ordering::Relaxed))
            .field("is_muted", &self.is_muted.load(Ordering::Relaxed))
            .field("native_sample_rate", &self.native_sample_rate)
            .field("native_channels", &self.native_channels)
            .finish()
    }
}

impl AudioCaptureStream {
    /// Initialize a new audio capture stream bound to the specified or default hardware microphone.
    ///
    /// If hardware audio is unavailable, falls back cleanly to a virtual audio driver.
    pub fn new(config: AudioCaptureConfig) -> Result<Self, AudioError> {
        let ring_buffer =
            AudioRingBuffer::new(config.ring_buffer_capacity, config.ring_buffer_mode);
        let meter = Arc::new(Mutex::new(AudioLevelMeter::default()));
        let is_running = Arc::new(AtomicBool::new(false));
        let is_muted = Arc::new(AtomicBool::new(false));

        // Attempt physical hardware discovery
        match AudioDeviceManager::find_input_cpal_device(config.device_name.as_deref()) {
            Ok(device) => {
                let default_cfg = device.default_input_config().map_err(|e| {
                    AudioError::BackendError(format!("Failed to query default input config: {e}"))
                })?;

                let native_rate = default_cfg.sample_rate().0;
                let native_channels = default_cfg.channels();
                let sample_format = default_cfg.sample_format();

                let resampler = Arc::new(Mutex::new(AudioResampler::new(
                    native_rate,
                    config.target_sample_rate,
                    native_channels,
                )?));

                let (producer, _) = ring_buffer.clone().split();
                let stream = Self::build_cpal_stream(
                    &device,
                    &default_cfg.into(),
                    sample_format,
                    producer,
                    resampler,
                    Arc::clone(&meter),
                    Arc::clone(&is_muted),
                )?;

                Ok(Self {
                    _stream: Some(stream),
                    virtual_device: None,
                    ring_buffer,
                    meter,
                    is_running,
                    is_muted,
                    native_sample_rate: native_rate,
                    native_channels,
                })
            }
            Err(_) => {
                // Fallback to virtual audio driver for headless / container environments
                let virtual_dev = VirtualAudioDevice::new(config.ring_buffer_capacity)?;
                Ok(Self {
                    _stream: None,
                    virtual_device: Some(virtual_dev),
                    ring_buffer,
                    meter,
                    is_running,
                    is_muted,
                    native_sample_rate: 16_000,
                    native_channels: 1,
                })
            }
        }
    }

    /// Create a virtual capture stream explicitly for deterministic testing.
    pub fn new_virtual(capacity_samples: usize) -> Result<Self, AudioError> {
        let virtual_dev = VirtualAudioDevice::new(capacity_samples)?;
        let ring_buffer = AudioRingBuffer::new(capacity_samples, RingBufferMode::Overwrite);
        let meter = Arc::new(Mutex::new(AudioLevelMeter::default()));
        let is_running = Arc::new(AtomicBool::new(false));
        let is_muted = Arc::new(AtomicBool::new(false));

        Ok(Self {
            _stream: None,
            virtual_device: Some(virtual_dev),
            ring_buffer,
            meter,
            is_running,
            is_muted,
            native_sample_rate: 16_000,
            native_channels: 1,
        })
    }

    /// Start recording audio from the microphone.
    pub fn start(&self) -> Result<(), AudioError> {
        if let Some(stream) = &self._stream {
            stream
                .play()
                .map_err(|e| AudioError::StreamPlayError(e.to_string()))?;
        } else if let Some(v) = &self.virtual_device {
            v.start_capture();
        }
        self.is_running.store(true, Ordering::Release);
        Ok(())
    }

    /// Pause audio recording.
    pub fn pause(&self) -> Result<(), AudioError> {
        if let Some(stream) = &self._stream {
            stream
                .pause()
                .map_err(|e| AudioError::StreamPlayError(e.to_string()))?;
        } else if let Some(v) = &self.virtual_device {
            v.pause_capture();
        }
        self.is_running.store(false, Ordering::Release);
        Ok(())
    }

    /// Set software mute. When muted, stream continues running but zeroes are fed.
    pub fn set_muted(&self, muted: bool) {
        self.is_muted.store(muted, Ordering::Release);
        if let Some(v) = &self.virtual_device {
            v.set_muted(muted);
        }
    }

    /// Check if stream is currently software muted.
    pub fn is_muted(&self) -> bool {
        self.is_muted.load(Ordering::Acquire)
    }

    /// Check if stream is currently actively recording.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Acquire)
    }

    /// Obtain a consumer handle to read captured 16 kHz mono audio samples.
    pub fn consumer(&self) -> RingBufferConsumer {
        if let Some(v) = &self.virtual_device {
            v.capture_consumer()
        } else {
            let (_, consumer) = self.ring_buffer.clone().split();
            consumer
        }
    }

    /// Get current audio level snapshot (RMS, peak, clipping, silence).
    pub fn get_level_snapshot(&self) -> AudioLevelSnapshot {
        if let Some(v) = &self.virtual_device {
            v.get_level_snapshot()
        } else if let Ok(meter) = self.meter.lock() {
            meter.current_snapshot()
        } else {
            AudioLevelMeter::default().current_snapshot()
        }
    }

    /// Access the underlying virtual driver (if in virtual mode).
    pub fn virtual_device(&self) -> Option<&VirtualAudioDevice> {
        self.virtual_device.as_ref()
    }

    /// Native hardware sample rate.
    pub fn native_sample_rate(&self) -> u32 {
        self.native_sample_rate
    }

    /// Native hardware channel count.
    pub fn native_channels(&self) -> u16 {
        self.native_channels
    }

    fn build_cpal_stream(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        format: SampleFormat,
        producer: RingBufferProducer,
        resampler: Arc<Mutex<AudioResampler>>,
        meter: Arc<Mutex<AudioLevelMeter>>,
        is_muted: Arc<AtomicBool>,
    ) -> Result<Stream, AudioError> {
        let err_fn = |err| {
            eprintln!("Audio capture stream error: {err}");
        };

        match format {
            SampleFormat::F32 => device
                .build_input_stream(
                    config,
                    move |data: &[f32], _| {
                        Self::process_input_chunk(data, &producer, &resampler, &meter, &is_muted);
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamBuildError(e.to_string())),
            SampleFormat::I16 => device
                .build_input_stream(
                    config,
                    move |data: &[i16], _| {
                        let f32_samples: Vec<f32> =
                            data.iter().map(|s| s.to_float_sample()).collect();
                        Self::process_input_chunk(
                            &f32_samples,
                            &producer,
                            &resampler,
                            &meter,
                            &is_muted,
                        );
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamBuildError(e.to_string())),
            SampleFormat::U16 => device
                .build_input_stream(
                    config,
                    move |data: &[u16], _| {
                        let f32_samples: Vec<f32> =
                            data.iter().map(|s| s.to_float_sample()).collect();
                        Self::process_input_chunk(
                            &f32_samples,
                            &producer,
                            &resampler,
                            &meter,
                            &is_muted,
                        );
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| AudioError::StreamBuildError(e.to_string())),
            _ => Err(AudioError::StreamBuildError(format!(
                "Unsupported sample format: {format:?}"
            ))),
        }
    }

    #[inline]
    fn process_input_chunk(
        raw_samples: &[f32],
        producer: &RingBufferProducer,
        resampler: &Arc<Mutex<AudioResampler>>,
        meter: &Arc<Mutex<AudioLevelMeter>>,
        is_muted: &Arc<AtomicBool>,
    ) {
        let muted = is_muted.load(Ordering::Acquire);
        let mut mono_16k = Vec::with_capacity(raw_samples.len());

        let input_to_process = if muted {
            vec![0.0f32; raw_samples.len()]
        } else {
            raw_samples.to_vec()
        };

        if let Ok(mut res) = resampler.lock() {
            let _ = res.process(&input_to_process, &mut mono_16k);
        }

        if let Ok(mut m) = meter.lock() {
            m.process(&mono_16k);
        }

        producer.push_slice(&mono_16k);
    }
}
