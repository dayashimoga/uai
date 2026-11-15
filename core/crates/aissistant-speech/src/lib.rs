pub mod barge_in;
pub mod energy_vad;
pub mod error;
pub mod silero_vad;
pub mod vad;

pub use barge_in::{BargeInDetector, BargeInEvent};
pub use energy_vad::EnergyVadEngine;
pub use error::SpeechError;
pub use silero_vad::{SileroVadConfig, SileroVadEngine};
pub use vad::{UtteranceSegmenter, VadConfig, VadEngine, VadEvent, VadState};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::f32::consts::PI;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn generate_synthetic_speech_tone(
        sample_rate: u32,
        duration_ms: u32,
        freq_hz: f32,
    ) -> Vec<f32> {
        let num_samples = (sample_rate as f64 * (duration_ms as f64 / 1000.0)).round() as usize;
        let mut samples = Vec::with_capacity(num_samples);
        for t in 0..num_samples {
            // Harmonic mixture resembling human voice fundamental + overtone
            let f0 = (2.0 * PI * freq_hz * (t as f32) / sample_rate as f32).sin();
            let f1 = (2.0 * PI * (freq_hz * 2.0) * (t as f32) / sample_rate as f32).sin() * 0.5;
            samples.push((f0 + f1) * 0.4);
        }
        samples
    }

    #[test]
    fn test_energy_vad_silence_detection() {
        let mut vad = EnergyVadEngine::default();
        let silence = vec![0.0f32; 512];

        // Process a few silence frames
        for _ in 0..5 {
            let prob = vad
                .process_window(&silence)
                .expect("Process silence failed");
            assert!(
                prob < 0.1,
                "Expected low probability for silence, got {prob}"
            );
        }
    }

    #[test]
    fn test_energy_vad_active_speech_detection() {
        let mut vad = EnergyVadEngine::default();
        let speech_frame = generate_synthetic_speech_tone(16_000, 32, 220.0);
        assert_eq!(speech_frame.len(), 512);

        // Process initial silence to set noise floor
        let silence = vec![0.001f32; 512];
        for _ in 0..5 {
            let _ = vad.process_window(&silence);
        }

        // Process speech frame
        let prob = vad
            .process_window(&speech_frame)
            .expect("Process speech failed");
        assert!(
            prob > 0.5,
            "Expected high probability for speech, got {prob}"
        );
    }

    #[test]
    fn test_silero_vad_window_size_validation() {
        let mut engine = SileroVadEngine::new(SileroVadConfig::default()).expect("Init failed");
        assert_eq!(engine.window_size(), 512);

        // Valid window size passes
        let valid_window = vec![0.1f32; 512];
        assert!(engine.process_window(&valid_window).is_ok());

        // Invalid window size rejected
        let invalid_window = vec![0.1f32; 250];
        assert!(matches!(
            engine.process_window(&invalid_window),
            Err(SpeechError::InvalidAudioFormat(_))
        ));
    }

    #[test]
    fn test_utterance_segmenter_silence_maintains_silence_state() {
        let engine = Box::new(EnergyVadEngine::default());
        let mut segmenter = UtteranceSegmenter::new(engine, VadConfig::default());

        let silence = vec![0.0001f32; 1600]; // 100 ms of silence
        let events = segmenter.process_samples(&silence).expect("Process failed");

        assert!(events.is_empty() || events.iter().all(|e| *e == VadEvent::Silence));
        assert_eq!(segmenter.state(), VadState::Silence);
        assert!(!segmenter.is_speaking());
    }

    #[test]
    fn test_utterance_segmenter_speech_onset_and_trailing_silence() {
        let engine = Box::new(EnergyVadEngine::default());
        let config = VadConfig {
            min_speech_duration_ms: 64,   // 2 frames
            min_silence_duration_ms: 300, // ~10 frames
            speech_pad_ms: 100,
            ..Default::default()
        };
        let mut segmenter = UtteranceSegmenter::new(engine, config);

        // 1. Initial background noise (sets noise floor & fills pre-speech padding)
        let noise = vec![0.001f32; 16_000 / 2]; // 500 ms
        segmenter
            .process_samples(&noise)
            .expect("Process noise failed");

        // 2. Active speech block (400 ms of 200 Hz tone)
        let speech = generate_synthetic_speech_tone(16_000, 400, 200.0);
        let events1 = segmenter
            .process_samples(&speech)
            .expect("Process speech failed");

        // Verify onset detected
        let has_onset = events1
            .iter()
            .any(|e| matches!(e, VadEvent::SpeechStarted { .. }));
        assert!(has_onset, "Speech onset was not triggered");
        assert!(segmenter.is_speaking());

        // 3. Trailing silence block (500 ms, exceeds 300 ms min_silence_duration)
        let trailing_silence = vec![0.001f32; 8000]; // 500 ms
        let events2 = segmenter
            .process_samples(&trailing_silence)
            .expect("Process trailing silence failed");

        // Verify utterance completion
        let finished_event = events2
            .iter()
            .find(|e| matches!(e, VadEvent::SpeechFinished { .. }));
        assert!(
            finished_event.is_some(),
            "SpeechFinished event was not emitted"
        );

        if let Some(VadEvent::SpeechFinished {
            utterance,
            duration_ms,
        }) = finished_event
        {
            assert!(*duration_ms >= 400, "Duration {duration_ms} ms too short");
            assert!(!utterance.is_empty(), "Utterance buffer is empty");
        }

        assert_eq!(segmenter.state(), VadState::Silence);
        assert!(!segmenter.is_speaking());
    }

    #[test]
    fn test_barge_in_detection_when_playback_active() {
        let playback_active = Arc::new(AtomicBool::new(true));
        let flushed = Arc::new(AtomicBool::new(false));
        let flushed_clone = Arc::clone(&flushed);

        let mut detector =
            BargeInDetector::new(Arc::clone(&playback_active), 0.5, 2).on_flush(move || {
                flushed_clone.store(true, Ordering::Release);
            });

        // Frame 1: High speech probability (consecutive frame 1/2) -> No trigger yet
        let event1 = detector.process_vad_frame(0.85, 100);
        assert_eq!(event1, None);
        assert!(!flushed.load(Ordering::Acquire));
        assert_eq!(detector.total_barge_ins(), 0);

        // Frame 2: High speech probability (consecutive frame 2/2) -> Triggers barge-in!
        let event2 = detector.process_vad_frame(0.90, 132);
        assert!(event2.is_some());
        let ev = event2.unwrap();
        assert_eq!(ev.timestamp_ms, 132);
        assert_eq!(ev.consecutive_frames, 2);
        assert!(flushed.load(Ordering::Acquire));
        assert_eq!(detector.total_barge_ins(), 1);
    }

    #[test]
    fn test_barge_in_ignored_when_playback_inactive() {
        let playback_active = Arc::new(AtomicBool::new(false));
        let flushed = Arc::new(AtomicBool::new(false));
        let flushed_clone = Arc::clone(&flushed);

        let mut detector =
            BargeInDetector::new(Arc::clone(&playback_active), 0.5, 2).on_flush(move || {
                flushed_clone.store(true, Ordering::Release);
            });

        // Even with high speech probability, no barge-in if assistant isn't speaking
        let event1 = detector.process_vad_frame(0.9, 100);
        let event2 = detector.process_vad_frame(0.95, 132);

        assert_eq!(event1, None);
        assert_eq!(event2, None);
        assert!(!flushed.load(Ordering::Acquire));
        assert_eq!(detector.total_barge_ins(), 0);
    }

    #[test]
    fn test_barge_in_speech_drop_resets_counter() {
        let playback_active = Arc::new(AtomicBool::new(true));
        let mut detector = BargeInDetector::new(playback_active, 0.6, 2);

        // High speech frame 1
        assert_eq!(detector.process_vad_frame(0.8, 100), None);

        // Dip to silence -> resets consecutive speech counter
        assert_eq!(detector.process_vad_frame(0.1, 132), None);

        // Another high speech frame 1 -> still not 2 consecutive
        assert_eq!(detector.process_vad_frame(0.8, 164), None);
        assert_eq!(detector.total_barge_ins(), 0);

        // High speech frame 2 -> triggers!
        assert!(detector.process_vad_frame(0.8, 196).is_some());
        assert_eq!(detector.total_barge_ins(), 1);
    }
}
