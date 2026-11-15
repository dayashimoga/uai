pub mod capture;
pub mod device;
pub mod error;
pub mod meter;
pub mod playback;
pub mod resampler;
pub mod ring_buffer;
pub mod virtual_device;

pub use capture::{AudioCaptureConfig, AudioCaptureStream};
pub use device::{AudioDeviceInfo, AudioDeviceManager};
pub use error::AudioError;
pub use meter::{AudioLevelMeter, AudioLevelSnapshot};
pub use playback::{AudioPlaybackConfig, AudioPlaybackStream};
pub use resampler::{resample_batch, AudioResampler};
pub use ring_buffer::{AudioRingBuffer, RingBufferConsumer, RingBufferMode, RingBufferProducer};
pub use virtual_device::VirtualAudioDevice;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::f32::consts::PI;
    use std::thread;

    #[test]
    fn test_ring_buffer_strict_mode_basic_push_pop() {
        let rb = AudioRingBuffer::new(128, RingBufferMode::Strict);
        assert_eq!(rb.capacity(), 128);
        assert_eq!(rb.available_read(), 0);
        assert!(rb.is_empty());
        assert!(!rb.is_full());

        // Push 4 samples
        assert!(rb.push(0.1).is_ok());
        assert!(rb.push(0.2).is_ok());
        assert!(rb.push(0.3).is_ok());
        assert!(rb.push(0.4).is_ok());

        assert_eq!(rb.available_read(), 4);
        assert_eq!(rb.available_write(), 124);

        // Pop individual samples
        assert_eq!(rb.pop(), Some(0.1));
        assert_eq!(rb.pop(), Some(0.2));
        assert_eq!(rb.available_read(), 2);

        // Pop slice
        let mut out = [0.0f32; 2];
        let read = rb.pop_slice(&mut out);
        assert_eq!(read, 2);
        assert_eq!(out, [0.3, 0.4]);

        assert!(rb.is_empty());
        assert_eq!(rb.pop(), None);
        assert_eq!(rb.underruns(), 1);
    }

    #[test]
    fn test_ring_buffer_strict_overflow_rejection() {
        let rb = AudioRingBuffer::new(64, RingBufferMode::Strict);
        let cap = rb.capacity();

        let input = vec![1.0f32; cap];
        let written = rb.push_slice(&input);
        assert_eq!(written, cap);
        assert!(rb.is_full());

        // Next push must fail with BufferOverflow
        let res = rb.push(2.0);
        assert!(matches!(res, Err(AudioError::BufferOverflow { .. })));
        assert_eq!(rb.overruns(), 1);
    }

    #[test]
    fn test_ring_buffer_overwrite_mode_wake_word_buffer() {
        // Small power of two buffer
        let rb = AudioRingBuffer::new(64, RingBufferMode::Overwrite);
        let cap = rb.capacity();

        // Push 1..=64
        for i in 0..cap {
            assert!(rb.push(i as f32).is_ok());
        }
        assert!(rb.is_full());
        assert_eq!(rb.overruns(), 0);

        // Push 10 more elements, overwriting the oldest 10
        for i in 0..10 {
            assert!(rb.push((cap + i) as f32).is_ok());
        }
        assert_eq!(rb.overruns(), 10);
        assert_eq!(rb.available_read(), cap);

        // The oldest remaining element should be 10.0, newest should be 73.0
        let mut out = vec![0.0f32; cap];
        let read = rb.pop_slice(&mut out);
        assert_eq!(read, cap);
        assert_eq!(out[0], 10.0);
        assert_eq!(out[cap - 1], (cap + 9) as f32);
    }

    #[test]
    fn test_ring_buffer_peek_slice() {
        let rb = AudioRingBuffer::new(64, RingBufferMode::Strict);
        rb.push_slice(&[10.0, 20.0, 30.0, 40.0]);

        let mut peeked = [0.0f32; 3];
        let n = rb.peek_slice(&mut peeked);
        assert_eq!(n, 3);
        assert_eq!(peeked, [10.0, 20.0, 30.0]);

        // Peek does not advance read pos
        assert_eq!(rb.available_read(), 4);
        assert_eq!(rb.pop(), Some(10.0));
    }

    #[test]
    fn test_ring_buffer_clear() {
        let rb = AudioRingBuffer::new(64, RingBufferMode::Strict);
        rb.push_slice(&[1.0, 2.0, 3.0]);
        assert_eq!(rb.available_read(), 3);

        rb.clear();
        assert_eq!(rb.available_read(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn test_ring_buffer_spsc_concurrent_threads() {
        let rb = AudioRingBuffer::new(1024, RingBufferMode::Strict);
        let (producer, consumer) = rb.split();

        let total_samples = 20_000;

        let producer_handle = thread::spawn(move || {
            let mut val = 1.0f32;
            let mut pushed = 0;
            while pushed < total_samples {
                if producer.push(val).is_ok() {
                    val += 1.0;
                    pushed += 1;
                } else {
                    thread::yield_now();
                }
            }
        });

        let consumer_handle = thread::spawn(move || {
            let mut expected = 1.0f32;
            let mut received = 0;
            while received < total_samples {
                if let Some(sample) = consumer.pop() {
                    assert_eq!(sample, expected, "Mismatch at index {received}");
                    expected += 1.0;
                    received += 1;
                } else {
                    thread::yield_now();
                }
            }
        });

        producer_handle.join().expect("Producer panicked");
        consumer_handle.join().expect("Consumer panicked");
    }

    #[test]
    fn test_resampler_identity_pass_through() {
        let mut resampler = AudioResampler::new(16_000, 16_000, 1).expect("Init failed");
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let mut output = Vec::new();

        let n = resampler
            .process(&input, &mut output)
            .expect("Process failed");
        assert_eq!(n, 4);
        assert_eq!(output, input);
    }

    #[test]
    fn test_resampler_stereo_downmix_to_mono() {
        let mut resampler = AudioResampler::new(16_000, 16_000, 2).expect("Init failed");
        // Interleaved stereo: L, R, L, R
        let input = vec![1.0, 0.0, 0.5, 0.5, -0.5, 0.5, 0.8, -0.8];
        let mut output = Vec::new();

        let n = resampler
            .process(&input, &mut output)
            .expect("Process failed");
        assert_eq!(n, 4);
        assert_eq!(output, vec![0.5, 0.5, 0.0, 0.0]);
    }

    #[test]
    fn test_resampler_48k_to_16k_frequency_and_ratio() {
        // 48 kHz -> 16 kHz is an exact 3:1 decimation ratio
        let mut resampler = AudioResampler::to_speech_standard(48_000, 1).expect("Init failed");
        assert_eq!(resampler.in_rate(), 48_000);
        assert_eq!(resampler.out_rate(), 16_000);

        // Generate 48,000 samples (1 second) of a 1,000 Hz sine wave
        let freq = 1_000.0f32;
        let mut input = Vec::with_capacity(48_000);
        for t in 0..48_000 {
            let sample = (2.0 * PI * freq * (t as f32) / 48_000.0).sin();
            input.push(sample);
        }

        let mut output = Vec::new();
        let n = resampler
            .process(&input, &mut output)
            .expect("Process failed");

        // Exactly 16,000 output samples for 1 second of audio
        assert_eq!(n, 16_000);
        assert_eq!(output.len(), 16_000);

        // Verify output is also a 1,000 Hz sine wave with high fidelity
        for (i, &s) in output.iter().take(100).enumerate() {
            let expected = (2.0 * PI * freq * (i as f32) / 16_000.0).sin();
            let diff = (s - expected).abs();
            assert!(diff < 0.08, "Diff {diff} at sample {i}");
        }
    }

    #[test]
    fn test_resampler_44k1_to_16k_non_integer_ratio() {
        let mut resampler = AudioResampler::to_speech_standard(44_100, 1).expect("Init failed");
        let input = vec![0.5f32; 44_100];
        let mut output = Vec::new();

        let n = resampler
            .process(&input, &mut output)
            .expect("Process failed");
        // Approximately 16,000 samples (+/- 1 due to fractional boundary)
        assert!((n as i32 - 16_000).abs() <= 1);

        for &val in &output {
            assert!((val - 0.5).abs() < 1e-4);
        }
    }

    #[test]
    fn test_resampler_chunked_streaming_continuity() {
        let mut resampler = AudioResampler::to_speech_standard(48_000, 1).expect("Init failed");

        // Feed two halves of 480 samples each (total 960 samples -> expected 320 output)
        let chunk1 = vec![0.7f32; 480];
        let chunk2 = vec![0.7f32; 480];

        let mut out1 = Vec::new();
        let mut out2 = Vec::new();

        resampler
            .process(&chunk1, &mut out1)
            .expect("Chunk 1 failed");
        resampler
            .process(&chunk2, &mut out2)
            .expect("Chunk 2 failed");

        let total = out1.len() + out2.len();
        assert_eq!(total, 320);
    }

    #[test]
    fn test_resampler_invalid_parameters() {
        assert!(AudioResampler::new(100, 16_000, 1).is_err());
        assert!(AudioResampler::new(48_000, 300_000, 1).is_err());
        assert!(AudioResampler::new(48_000, 16_000, 0).is_err());
        assert!(AudioResampler::new(48_000, 16_000, 16).is_err());

        let mut resampler = AudioResampler::new(48_000, 16_000, 2).expect("Init ok");
        let mut out = Vec::new();
        // Odd number of samples for stereo should fail
        assert!(resampler.process(&[1.0, 2.0, 3.0], &mut out).is_err());
    }

    #[test]
    fn test_meter_silence_detection() {
        let mut meter = AudioLevelMeter::default();
        let silence = [0.0f32; 512];

        let snap = meter.process(&silence);
        assert_eq!(snap.rms_dbfs, -96.0);
        assert_eq!(snap.peak_dbfs, -96.0);
        assert_eq!(snap.normalized_rms, 0.0);
        assert!(snap.is_silent);
        assert!(!snap.is_clipping);
    }

    #[test]
    fn test_meter_full_scale_sine_and_clipping() {
        let mut meter = AudioLevelMeter::default();

        // 1.0 peak sine wave
        let mut sine = Vec::with_capacity(1000);
        for t in 0..1000 {
            sine.push((2.0 * PI * 10.0 * (t as f32) / 1000.0).sin());
        }

        let snap = meter.process(&sine);
        // Peak of 1.0 is 0.0 dBFS
        assert!((snap.peak_dbfs - 0.0).abs() < 0.1);
        // Theoretical RMS of sine is 1 / sqrt(2) = 0.7071 -> -3.01 dBFS
        assert!((snap.rms_dbfs - (-3.01)).abs() < 0.2);
        assert!(snap.is_clipping);
        assert!(!snap.is_silent);
        assert!(snap.normalized_rms > 0.9);
    }

    #[test]
    fn test_meter_full_scale_square_wave() {
        let mut meter = AudioLevelMeter::default();
        let square = [1.0f32; 256];

        let snap = meter.process(&square);
        assert!((snap.rms_dbfs - 0.0).abs() < 0.05);
        assert!((snap.peak_dbfs - 0.0).abs() < 0.05);
        assert_eq!(snap.normalized_rms, 1.0);
        assert!(snap.is_clipping);
    }

    #[test]
    fn test_meter_ballistics_attack_and_decay() {
        let mut meter = AudioLevelMeter::new(-96.0, -60.0, 0.5);

        // Step 1: Loud burst
        let loud = [1.0f32; 128];
        let snap1 = meter.process(&loud);
        assert_eq!(snap1.normalized_peak, 1.0);

        // Step 2: Silence block -> peak decays exponentially
        let quiet = [0.0f32; 128];
        let snap2 = meter.process(&quiet);
        // Decay = 1.0 * 0.5 + 0.0 * 0.5 = 0.5
        assert!((snap2.normalized_peak - 0.5).abs() < 0.01);

        // Step 3: Another silence block -> decays further to 0.25
        let snap3 = meter.process(&quiet);
        assert!((snap3.normalized_peak - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_batch_resample_convenience() {
        let input = vec![0.25f32; 320];
        let out = resample_batch(&input, 16_000, 1, 8_000).expect("Batch failed");
        assert_eq!(out.len(), 160);
        for &s in &out {
            assert!((s - 0.25).abs() < 1e-4);
        }
    }

    #[test]
    fn test_device_manager_query_and_discovery() {
        // Query devices on current system; must return Ok result even if headless/empty
        let inputs = AudioDeviceManager::list_input_devices();
        assert!(inputs.is_ok());

        let outputs = AudioDeviceManager::list_output_devices();
        assert!(outputs.is_ok());

        let def_in = AudioDeviceManager::default_input_device();
        assert!(def_in.is_ok());

        let def_out = AudioDeviceManager::default_output_device();
        assert!(def_out.is_ok());
    }

    #[test]
    fn test_virtual_device_capture_and_metering() {
        let vdev = VirtualAudioDevice::new(1024).expect("Init vdev failed");
        assert!(!vdev.is_capturing());
        assert!(!vdev.is_muted());

        vdev.start_capture();
        assert!(vdev.is_capturing());

        // Ingest 160 samples of a full-scale tone
        let tone = vec![0.8f32; 160];
        let pushed = vdev
            .inject_capture_samples(&tone, 16_000, 1)
            .expect("Inject failed");
        assert_eq!(pushed, 160);

        // Consumer reads the samples
        let consumer = vdev.capture_consumer();
        let mut out = [0.0f32; 160];
        let read = consumer.pop_slice(&mut out);
        assert_eq!(read, 160);
        assert_eq!(out[0], 0.8);

        // Verify level meter reflects the active signal
        let snapshot = vdev.get_level_snapshot();
        assert!(!snapshot.is_silent);
        assert!(snapshot.normalized_rms > 0.5);

        vdev.pause_capture();
        assert!(!vdev.is_capturing());
    }

    #[test]
    fn test_virtual_device_software_mute() {
        let vdev = VirtualAudioDevice::new(1024).expect("Init vdev failed");
        vdev.start_capture();

        // Enable software mute
        vdev.set_muted(true);
        assert!(vdev.is_muted());

        // Inject active tone while muted
        let tone = vec![0.9f32; 160];
        vdev.inject_capture_samples(&tone, 16_000, 1)
            .expect("Inject failed");

        // Consumer receives silence (0.0)
        let consumer = vdev.capture_consumer();
        let mut out = [1.0f32; 160];
        let read = consumer.pop_slice(&mut out);
        assert_eq!(read, 160);
        for &s in &out {
            assert_eq!(s, 0.0);
        }

        // Unmute and verify non-zero passes through
        vdev.set_muted(false);
        assert!(!vdev.is_muted());

        vdev.inject_capture_samples(&tone, 16_000, 1)
            .expect("Inject failed");
        let read2 = consumer.pop_slice(&mut out);
        assert_eq!(read2, 160);
        assert_eq!(out[0], 0.9);
    }

    #[test]
    fn test_virtual_device_playback_and_instant_flush() {
        let vdev = VirtualAudioDevice::new(1024).expect("Init vdev failed");
        vdev.start_playback();
        assert!(vdev.is_playing());

        // Queue 200 samples
        let samples = vec![0.5f32; 200];
        let queued = vdev.queue_playback_samples(&samples);
        assert_eq!(queued, 200);

        // Read 50 samples
        let mut out = [0.0f32; 50];
        let read = vdev.read_playback_samples(&mut out);
        assert_eq!(read, 50);

        // Instant flush (barge-in interrupt)
        vdev.flush_playback();

        // After flush, buffer is immediately empty
        let read_after = vdev.read_playback_samples(&mut out);
        assert_eq!(read_after, 0);
    }

    #[test]
    fn test_audio_capture_stream_virtual_lifecycle() {
        let stream = AudioCaptureStream::new_virtual(2048).expect("Init virtual capture failed");
        assert!(!stream.is_running());
        assert!(!stream.is_muted());

        assert!(stream.start().is_ok());
        assert!(stream.is_running());

        stream.set_muted(true);
        assert!(stream.is_muted());

        let snapshot = stream.get_level_snapshot();
        assert!(snapshot.is_silent);

        assert!(stream.pause().is_ok());
        assert!(!stream.is_running());
    }

    #[test]
    fn test_audio_playback_stream_virtual_lifecycle_and_volume() {
        let stream = AudioPlaybackStream::new_virtual(2048).expect("Init virtual playback failed");
        assert!(!stream.is_running());
        assert_eq!(stream.get_volume(), 1.0);

        assert!(stream.start().is_ok());
        assert!(stream.is_running());

        // Set volume to 0.75
        stream.set_volume(0.75);
        assert!((stream.get_volume() - 0.75).abs() < 1e-4);

        // Play samples and instant flush
        let samples = vec![0.4f32; 100];
        let queued = stream.play_samples(&samples);
        assert_eq!(queued, 100);

        stream.flush();

        assert!(stream.pause().is_ok());
        assert!(!stream.is_running());
    }
}
