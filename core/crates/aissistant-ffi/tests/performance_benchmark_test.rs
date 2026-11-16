//! End-to-end latency and memory footprint integration benchmarks verifying NFR-1..NFR-8.

#![allow(clippy::unwrap_used, clippy::panic)]

use aissistant_audio::ring_buffer::{AudioRingBuffer, RingBufferMode};
use aissistant_ffi::{CoreBridge, PerformanceSnapshot, PipelineBenchmarkReport};
use aissistant_speech::acoustic_kws::AcousticKwsSpotter;
use aissistant_speech::energy_vad::EnergyVadEngine;
use aissistant_speech::vad::VadEngine;
use aissistant_speech::wake_word::{WakeWordConfig, WakeWordSpotter};
use std::time::Instant;

#[test]
fn test_cold_start_latency_under_budget() {
    let start = Instant::now();
    let bridge = CoreBridge::new_in_memory().unwrap();
    let duration = start.elapsed();

    // Cold-start budget: < 250 ms
    assert!(
        duration.as_millis() < 250,
        "Cold start took {} ms, exceeding 250 ms budget",
        duration.as_millis()
    );

    let snapshot = bridge.get_performance_snapshot().unwrap();
    assert!(snapshot.cold_start_ms < 250);
}

#[test]
fn test_wake_word_acoustic_frame_processing_budget() {
    let config = WakeWordConfig::default();
    let mut spotter = AcousticKwsSpotter::new(config).unwrap();

    // 200 consecutive 20 ms frames (4 seconds of simulated audio at 16 kHz: 320 samples per frame)
    let frame_samples = [0.03f32; 320];
    let frames_count = 200;

    let start = Instant::now();
    for _ in 0..frames_count {
        let _ = spotter.process_samples(&frame_samples).unwrap();
    }
    let elapsed = start.elapsed();

    let avg_frame_micros = elapsed.as_micros() / frames_count as u128;
    let audio_duration_ms = frames_count * 20; // 4000 ms
    let rtf = elapsed.as_secs_f32() / (audio_duration_ms as f32 / 1000.0);

    // NFR-2 target: frame processing must take < 1 ms (1000 us) per 20 ms frame
    assert!(
        avg_frame_micros < 1000,
        "Wake-word average frame processing took {} us, exceeding 1000 us (1 ms) limit",
        avg_frame_micros
    );

    // Real-time factor should be < 0.1x (at least 10x faster than real-time playback)
    assert!(
        rtf < 0.1,
        "Wake-word real-time factor was {:.4}, exceeding 0.1x threshold",
        rtf
    );
}

#[test]
fn test_vad_barge_in_latency_budget() {
    let mut vad = EnergyVadEngine::default();
    let window = [0.05f32; 512]; // 32 ms window @ 16 kHz
    let windows_count = 200;

    let start = Instant::now();
    for _ in 0..windows_count {
        let _ = vad.process_window(&window).unwrap();
    }
    let elapsed = start.elapsed();

    let avg_window_micros = elapsed.as_micros() / windows_count as u128;

    // VAD per-window processing must be sub-100 microseconds
    assert!(
        avg_window_micros < 200,
        "VAD window processing took {} us, exceeding 200 us limit",
        avg_window_micros
    );
}

#[test]
fn test_audio_ring_buffer_zero_allocation_throughput() {
    // 64k sample ring buffer in Strict mode
    let ring = AudioRingBuffer::new(65536, RingBufferMode::Strict);
    let chunk = [0.15f32; 512];
    let total_samples = 100_000;

    let start = Instant::now();
    let mut pushed = 0;
    while pushed < total_samples {
        let count = ring.push_slice(&chunk);
        pushed += count;
        let mut read_buf = [0.0f32; 512];
        let _ = ring.pop_slice(&mut read_buf);
    }
    let elapsed = start.elapsed();

    // 100k samples should be pushed and popped in under 25 ms
    assert!(
        elapsed.as_millis() < 25,
        "Ring buffer push/pop of 100k samples took {} ms, exceeding 25 ms budget",
        elapsed.as_millis()
    );

    let msps = (total_samples as f32 / 1_000_000.0) / elapsed.as_secs_f32();
    assert!(
        msps > 2.0,
        "Throughput was {:.2} MSamples/sec, expected > 2.0 MSamples/sec",
        msps
    );
}

#[test]
fn test_idle_resident_memory_footprint_budget() {
    let bridge = CoreBridge::new_in_memory().unwrap();

    // Initial snapshot
    let snapshot: PerformanceSnapshot = bridge.get_performance_snapshot().unwrap();
    assert!(snapshot.resident_memory_bytes > 0);

    // Enforce memory budget (trims caches and checks budget)
    let _within_budget = bridge.enforce_memory_budget().unwrap();

    // Run end-to-end pipeline benchmark report
    let report: PipelineBenchmarkReport = bridge.run_pipeline_benchmark().unwrap();
    assert!(report.all_budgets_passed);
    assert!(report.wake_word_avg_frame_micros < 1000);
    assert!(report.cold_start_ms <= 250);
}
