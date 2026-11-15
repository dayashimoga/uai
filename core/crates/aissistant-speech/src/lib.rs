pub mod acoustic_kws;
pub mod acoustic_stt;
pub mod barge_in;
pub mod energy_vad;
pub mod error;
pub mod sherpa_kws;
pub mod sherpa_stt;
pub mod silero_vad;
pub mod stt;
pub mod vad;
pub mod wake_word;

pub use acoustic_kws::{AcousticFeature, AcousticKwsSpotter, KeywordTemplate};
pub use acoustic_stt::{AcousticSttEngine, LexiconEntry};
pub use barge_in::{BargeInDetector, BargeInEvent};
pub use energy_vad::EnergyVadEngine;
pub use error::SpeechError;
pub use sherpa_kws::{SherpaKwsConfig, SherpaKwsEngine};
pub use sherpa_stt::{SherpaSttConfig, SherpaSttEngine, SherpaSttModelType};
pub use silero_vad::{SileroVadConfig, SileroVadEngine};
pub use stt::{DecodingMethod, StreamingTranscriptEvent, SttConfig, SttEngine, TranscriptSegment};
pub use vad::{UtteranceSegmenter, VadConfig, VadEngine, VadEvent, VadState};
pub use wake_word::{WakeWordConfig, WakeWordEvent, WakeWordSpotter};

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

    /// Generates synthetic audio that mimics the acoustic profile of "hey assistant":
    /// 1. "hey": vowel diphthong (low fundamental 180 Hz + formant 600 Hz + 1800 Hz)
    /// 2. transition dip
    /// 3. "uh": vowel (800 Hz)
    /// 4. "ss": high-frequency sibilant (4200 Hz - 5500 Hz high band burst with high ZCR)
    /// 5. "is": vowel (1100 Hz)
    /// 6. "ant": dental transient + vowel
    fn generate_synthetic_hey_assistant(sample_rate: u32) -> Vec<f32> {
        let mut samples = Vec::new();

        // 1. "hey" (~60 ms = 960 samples)
        let n1 = (sample_rate as f32 * 0.060) as usize;
        for t in 0..n1 {
            let f0 = (2.0 * PI * 180.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            let f1 = (2.0 * PI * 600.0 * (t as f32) / sample_rate as f32).sin() * 0.5;
            let f2 = (2.0 * PI * 1800.0 * (t as f32) / sample_rate as f32).sin() * 0.2;
            samples.push(f0 + f1 + f2);
        }

        // 2. Transition (~20 ms)
        let n2 = (sample_rate as f32 * 0.020) as usize;
        for t in 0..n2 {
            let f0 = (2.0 * PI * 200.0 * (t as f32) / sample_rate as f32).sin() * 0.3;
            let f1 = (2.0 * PI * 1000.0 * (t as f32) / sample_rate as f32).sin() * 0.3;
            samples.push(f0 + f1);
        }

        // 3. "uh" (~40 ms)
        let n3 = (sample_rate as f32 * 0.040) as usize;
        for t in 0..n3 {
            let f0 = (2.0 * PI * 180.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            let f1 = (2.0 * PI * 800.0 * (t as f32) / sample_rate as f32).sin() * 0.5;
            samples.push(f0 + f1);
        }

        // 4. "ss" sibilant (~80 ms): high frequency multi-carrier resembling fricative
        let n4 = (sample_rate as f32 * 0.080) as usize;
        for t in 0..n4 {
            let s1 = (2.0 * PI * 4200.0 * (t as f32) / sample_rate as f32).sin() * 0.35;
            let s2 = (2.0 * PI * 4800.0 * (t as f32) / sample_rate as f32).sin() * 0.35;
            let s3 = (2.0 * PI * 5500.0 * (t as f32) / sample_rate as f32).sin() * 0.30;
            samples.push(s1 + s2 + s3);
        }

        // 5. "is" (~40 ms)
        let n5 = (sample_rate as f32 * 0.040) as usize;
        for t in 0..n5 {
            let f0 = (2.0 * PI * 220.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            let f1 = (2.0 * PI * 1100.0 * (t as f32) / sample_rate as f32).sin() * 0.5;
            samples.push(f0 + f1);
        }

        // 6. "tant" (~60 ms)
        let n6 = (sample_rate as f32 * 0.060) as usize;
        for t in 0..n6 {
            let f0 = (2.0 * PI * 200.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            let f1 = (2.0 * PI * 1200.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            samples.push(f0 + f1);
        }

        // 7. Trailing samples to allow FFT window (512 samples) to complete final frames
        let n7 = 512;
        for t in 0..n7 {
            let decay = 1.0 - (t as f32 / n7 as f32);
            let s = (2.0 * PI * 200.0 * (t as f32) / sample_rate as f32).sin() * 0.2 * decay;
            samples.push(s);
        }

        samples
    }

    /// Generates synthetic audio mimicking the acoustic profile of "computer":
    /// 1. "k": plosive burst (2200 Hz)
    /// 2. "om": low-frequency vowel + nasal (200 Hz fundamental + 650 Hz formant)
    /// 3. "pyu": upper glide (1800 Hz)
    /// 4. "ter": mid vowel (1200 Hz)
    fn generate_synthetic_computer(sample_rate: u32) -> Vec<f32> {
        let mut samples = Vec::new();

        // 1. "k" burst (~40 ms)
        let n1 = (sample_rate as f32 * 0.040) as usize;
        for t in 0..n1 {
            let s1 = (2.0 * PI * 800.0 * (t as f32) / sample_rate as f32).sin() * 0.2;
            let s2 = (2.0 * PI * 2200.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            let s3 = (2.0 * PI * 3500.0 * (t as f32) / sample_rate as f32).sin() * 0.1;
            samples.push(s1 + s2 + s3);
        }

        // 2. "om" nasal vowel (~100 ms): strong low band (80-400 Hz)
        let n2 = (sample_rate as f32 * 0.100) as usize;
        for t in 0..n2 {
            let f0 = (2.0 * PI * 160.0 * (t as f32) / sample_rate as f32).sin() * 0.6;
            let f1 = (2.0 * PI * 650.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            samples.push(f0 + f1);
        }

        // 3. "pyu" glide (~80 ms)
        let n3 = (sample_rate as f32 * 0.080) as usize;
        for t in 0..n3 {
            let f0 = (2.0 * PI * 200.0 * (t as f32) / sample_rate as f32).sin() * 0.3;
            let f1 = (2.0 * PI * 1800.0 * (t as f32) / sample_rate as f32).sin() * 0.5;
            samples.push(f0 + f1);
        }

        // 4. "ter" (~80 ms)
        let n4 = (sample_rate as f32 * 0.080) as usize;
        for t in 0..n4 {
            let f0 = (2.0 * PI * 180.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            let f1 = (2.0 * PI * 1200.0 * (t as f32) / sample_rate as f32).sin() * 0.4;
            samples.push(f0 + f1);
        }

        // Trailing padding to clear FFT window
        let n_trail = 512;
        for t in 0..n_trail {
            let decay = 1.0 - (t as f32 / n_trail as f32);
            let s = (2.0 * PI * 180.0 * (t as f32) / sample_rate as f32).sin() * 0.2 * decay;
            samples.push(s);
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

    // =========================================================================
    // Wake-Word / Acoustic KWS Tests (S2.P2.2)
    // =========================================================================

    #[test]
    fn test_wake_word_config_validation() {
        // Valid default
        let valid = WakeWordConfig::default();
        assert!(valid.validate().is_ok());

        // Empty keywords rejected
        let invalid_kw = WakeWordConfig {
            keywords: vec![],
            ..Default::default()
        };
        assert!(invalid_kw.validate().is_err());

        // Whitespace-only keyword rejected
        let invalid_ws = WakeWordConfig {
            keywords: vec!["   ".to_string()],
            ..Default::default()
        };
        assert!(invalid_ws.validate().is_err());

        // Invalid threshold rejected
        let invalid_thresh = WakeWordConfig {
            threshold: 1.5,
            ..Default::default()
        };
        assert!(invalid_thresh.validate().is_err());

        // Non-16kHz sample rate rejected
        let invalid_sr = WakeWordConfig {
            sample_rate: 44_100,
            ..Default::default()
        };
        assert!(invalid_sr.validate().is_err());
    }

    #[test]
    fn test_acoustic_kws_synthetic_hey_assistant_detection() {
        let config = WakeWordConfig {
            keywords: vec!["hey assistant".to_string()],
            threshold: 0.60,
            min_keyword_interval_ms: 1000,
            ..Default::default()
        };
        let mut spotter = AcousticKwsSpotter::new(config).expect("Init spotter failed");

        // Feed some initial ambient silence / lead-in
        let silence = vec![0.0001f32; 1600];
        let ev_silence = spotter.process_samples(&silence).expect("Process silence");
        assert_eq!(ev_silence, None);

        // Feed synthetic "hey assistant"
        let keyword_audio = generate_synthetic_hey_assistant(16_000);
        let event = spotter
            .process_samples(&keyword_audio)
            .expect("Process keyword audio");

        assert!(
            event.is_some(),
            "Expected wake-word 'hey assistant' to be spotted"
        );
        let ev = event.unwrap();
        assert_eq!(ev.keyword, "hey assistant");
        assert!(
            ev.confidence >= 0.60,
            "Expected confidence >= 0.60, got {}",
            ev.confidence
        );
        assert!(ev.end_sample >= ev.start_sample);
    }

    #[test]
    fn test_acoustic_kws_false_accept_rejection_silence_and_noise() {
        let config = WakeWordConfig {
            keywords: vec!["hey assistant".to_string()],
            threshold: 0.60,
            ..Default::default()
        };
        let mut spotter = AcousticKwsSpotter::new(config).expect("Init spotter failed");

        // 1. Silence: 1.0 second of zero samples
        let silence = vec![0.0f32; 16_000];
        let ev_silence = spotter.process_samples(&silence).expect("Process silence");
        assert_eq!(ev_silence, None);

        // 2. Pure 440 Hz sine tone: static harmonic with zero formant dynamics
        let pure_tone = generate_synthetic_speech_tone(16_000, 1000, 440.0);
        let ev_tone = spotter.process_samples(&pure_tone).expect("Process tone");
        assert_eq!(ev_tone, None);

        // 3. Monotonous speech tone: steady 200 Hz with no sibilant or vowel shifts
        let mono_speech = generate_synthetic_speech_tone(16_000, 800, 200.0);
        let ev_speech = spotter
            .process_samples(&mono_speech)
            .expect("Process mono speech");
        assert_eq!(ev_speech, None);
    }

    #[test]
    fn test_acoustic_kws_debounce_cooldown() {
        let config = WakeWordConfig {
            keywords: vec!["hey assistant".to_string()],
            threshold: 0.60,
            min_keyword_interval_ms: 1500, // 1500 ms debounce
            ..Default::default()
        };
        let mut spotter = AcousticKwsSpotter::new(config).expect("Init spotter failed");

        let keyword_audio = generate_synthetic_hey_assistant(16_000);

        // First trigger succeeds
        let ev1 = spotter
            .process_samples(&keyword_audio)
            .expect("Process kw 1");
        assert!(ev1.is_some());

        // Immediate repeat within 50 ms (well within 1500 ms debounce)
        let ev2 = spotter
            .process_samples(&keyword_audio)
            .expect("Process kw 2");
        assert_eq!(ev2, None, "Second trigger should be suppressed by debounce");

        // Advance stream by feeding 1600 ms of silence (exceeds 1500 ms debounce)
        let silence_padding = vec![0.0001f32; (16_000.0 * 1.6) as usize];
        let _ = spotter.process_samples(&silence_padding);

        // Now third trigger after debounce should succeed!
        let ev3 = spotter
            .process_samples(&keyword_audio)
            .expect("Process kw 3");
        assert!(
            ev3.is_some(),
            "Trigger after debounce interval should succeed"
        );
    }

    #[test]
    fn test_acoustic_kws_dynamic_keyword_updates() {
        let config = WakeWordConfig {
            keywords: vec!["computer".to_string()],
            threshold: 0.60,
            ..Default::default()
        };
        let mut spotter = AcousticKwsSpotter::new(config).expect("Init failed");

        assert_eq!(spotter.keywords(), &["computer".to_string()]);

        // Playing "hey assistant" does not trigger when only "computer" is active
        let hey_assistant_audio = generate_synthetic_hey_assistant(16_000);
        let ev1 = spotter
            .process_samples(&hey_assistant_audio)
            .expect("Process audio");
        assert_eq!(ev1, None);

        // Dynamically add "hey assistant"
        spotter
            .set_keywords(&["computer".to_string(), "hey assistant".to_string()])
            .expect("Set keywords failed");
        assert_eq!(spotter.keywords().len(), 2);

        // Now playing "hey assistant" triggers!
        let ev2 = spotter
            .process_samples(&hey_assistant_audio)
            .expect("Process audio 2");
        assert!(ev2.is_some());
        assert_eq!(ev2.unwrap().keyword, "hey assistant");
    }

    #[test]
    fn test_acoustic_kws_threshold_sensitivity() {
        let config = WakeWordConfig {
            keywords: vec!["hey assistant".to_string()],
            threshold: 0.999, // Unattainably strict
            ..Default::default()
        };
        let mut spotter = AcousticKwsSpotter::new(config).expect("Init failed");
        let audio = generate_synthetic_hey_assistant(16_000);

        let ev_strict = spotter.process_samples(&audio).expect("Process strict");
        assert_eq!(ev_strict, None);

        // Lower threshold to reachable level
        spotter.set_threshold(0.50).expect("Set threshold failed");
        spotter.reset();
        let ev_lenient = spotter.process_samples(&audio).expect("Process lenient");
        assert!(ev_lenient.is_some());
    }

    #[test]
    fn test_multi_keyword_discrimination() {
        let config = WakeWordConfig {
            keywords: vec!["hey assistant".to_string(), "computer".to_string()],
            threshold: 0.60,
            min_keyword_interval_ms: 500,
            ..Default::default()
        };
        let mut spotter = AcousticKwsSpotter::new(config).expect("Init failed");

        // 1. Play "computer" audio -> detects "computer"
        let computer_audio = generate_synthetic_computer(16_000);
        let ev1 = spotter
            .process_samples(&computer_audio)
            .expect("Process computer");
        assert!(ev1.is_some(), "Expected 'computer' to trigger");
        assert_eq!(ev1.unwrap().keyword, "computer");

        // Wait past debounce
        let silence = vec![0.0001f32; 16_000];
        let _ = spotter.process_samples(&silence);

        // 2. Play "hey assistant" audio -> detects "hey assistant"
        let hey_assistant_audio = generate_synthetic_hey_assistant(16_000);
        let ev2 = spotter
            .process_samples(&hey_assistant_audio)
            .expect("Process hey assistant");
        assert!(ev2.is_some(), "Expected 'hey assistant' to trigger");
        assert_eq!(ev2.unwrap().keyword, "hey assistant");
    }

    #[test]
    fn test_sherpa_kws_engine_config_and_export() {
        let config = SherpaKwsConfig {
            keywords: vec!["hey assistant".to_string(), "computer".to_string()],
            keywords_score: 2.0,
            threshold: 0.65,
            ..Default::default()
        };
        assert!(config.validate().is_ok());

        // Check keywords content formatting
        let content = config.generate_keywords_content();
        assert!(content.contains("hey assistant / @2.00"));
        assert!(content.contains("computer / @2.00"));

        let mut engine = SherpaKwsEngine::new(config).expect("Init SherpaKwsEngine failed");
        assert_eq!(engine.keywords().len(), 2);

        // Export to temp file
        let temp_dir = tempfile::tempdir().expect("Create tempdir failed");
        let file_path = temp_dir.path().join("keywords.txt");
        engine
            .export_keywords_file(&file_path)
            .expect("Export keywords file failed");

        let read_back = std::fs::read_to_string(&file_path).expect("Read exported file");
        assert_eq!(read_back, content);

        // Verify spotting functions transparently
        let audio = generate_synthetic_hey_assistant(16_000);
        let event = engine.process_samples(&audio).expect("Process samples");
        assert!(event.is_some());
        assert_eq!(event.unwrap().keyword, "hey assistant");
    }

    // =========================================================================
    // Speech-to-Text (STT) Tests (S2.P3.1)
    // =========================================================================

    #[test]
    fn test_stt_config_validation() {
        let valid = SttConfig::default();
        assert!(valid.validate().is_ok());

        let invalid_lang = SttConfig {
            language: "".to_string(),
            ..Default::default()
        };
        assert!(invalid_lang.validate().is_err());

        let invalid_sr = SttConfig {
            sample_rate: 44_100,
            ..Default::default()
        };
        assert!(invalid_sr.validate().is_err());

        let invalid_beam = SttConfig {
            beam_size: 0,
            ..Default::default()
        };
        assert!(invalid_beam.validate().is_err());

        let invalid_silence = SttConfig {
            min_trailing_silence_ms: 0,
            ..Default::default()
        };
        assert!(invalid_silence.validate().is_err());

        let invalid_utterance = SttConfig {
            min_trailing_silence_ms: 1000,
            max_utterance_ms: 500,
            ..Default::default()
        };
        assert!(invalid_utterance.validate().is_err());
    }

    #[test]
    fn test_acoustic_stt_streaming_partial_and_final_transcript() {
        let config = SttConfig {
            language: "en".to_string(),
            min_trailing_silence_ms: 500,
            ..Default::default()
        };
        let mut stt = AcousticSttEngine::new(config).expect("Init AcousticSttEngine failed");

        // 1. Stream synthetic speech ("hey assistant") in 256-sample chunks
        let audio = generate_synthetic_hey_assistant(16_000);
        let mut partial_emitted = false;

        for chunk in audio.chunks(256) {
            stt.accept_waveform(chunk).expect("Accept chunk");
            if let Some(StreamingTranscriptEvent::Partial(seg)) =
                stt.decode_step().expect("Decode step")
            {
                assert_eq!(seg.language, "en");
                assert!(!seg.is_final);
                assert!(!seg.text.is_empty());
                partial_emitted = true;
            }
        }

        assert!(
            partial_emitted,
            "Expected at least one partial transcript event during speech"
        );
        let current_res = stt.get_result();
        assert!(
            current_res.text.contains("hey assistant"),
            "Expected 'hey assistant' in hypothesis, got '{}'",
            current_res.text
        );
        assert!(!stt.is_endpoint());

        // 2. Stream trailing silence (600 ms, exceeds min_trailing_silence_ms 500 ms)
        let silence_padding = vec![0.0001f32; (16_000.0 * 0.6) as usize];
        let mut final_emitted = false;

        for chunk in silence_padding.chunks(256) {
            stt.accept_waveform(chunk).expect("Accept silence chunk");
            if let Some(StreamingTranscriptEvent::Final(seg)) =
                stt.decode_step().expect("Decode step")
            {
                assert!(seg.is_final);
                assert!(seg.text.contains("hey assistant"));
                final_emitted = true;
                break;
            }
        }

        assert!(final_emitted, "Expected final transcript upon endpoint");
        assert!(stt.is_endpoint());
    }

    #[test]
    fn test_acoustic_stt_multilingual_language_switching() {
        let config = SttConfig {
            language: "es".to_string(),
            ..Default::default()
        };
        let mut stt = AcousticSttEngine::new(config).expect("Init failed");
        assert_eq!(stt.language(), "es");

        // Switch language to German
        stt.set_language("de").expect("Set language failed");
        assert_eq!(stt.language(), "de");

        // Invalid empty language rejected
        assert!(stt.set_language("   ").is_err());
    }

    #[test]
    fn test_acoustic_stt_custom_vocabulary_addition() {
        let config = SttConfig::default();
        let mut stt = AcousticSttEngine::new(config).expect("Init failed");

        // Add custom vocabulary
        stt.add_entry("launch calculator", "en");

        let custom_entry = stt
            .lexicon()
            .iter()
            .find(|e| e.phrase == "launch calculator");
        assert!(custom_entry.is_some());
        assert_eq!(custom_entry.unwrap().language, "en");
    }

    #[test]
    fn test_sherpa_stt_engine_transducer_and_whisper_configs() {
        // 1. Transducer (Zipformer) config
        let transducer_config = SherpaSttConfig {
            model_type: SherpaSttModelType::Transducer,
            language: "en".to_string(),
            min_trailing_silence_ms: 600,
            ..Default::default()
        };
        assert!(transducer_config.validate().is_ok());

        let mut engine =
            SherpaSttEngine::new(transducer_config).expect("Init SherpaSttEngine failed");
        assert_eq!(engine.language(), "en");
        assert!(!engine.is_using_onnx_models()); // Falls back to acoustic engine when ONNX absent

        // Stream synthetic audio
        let audio = generate_synthetic_hey_assistant(16_000);
        engine.accept_waveform(&audio).expect("Accept waveform");

        let mut decoded = false;
        while let Some(event) = engine.decode_step().expect("Decode") {
            if let StreamingTranscriptEvent::Partial(seg) = event {
                assert!(seg.text.contains("hey assistant"));
                decoded = true;
                break;
            }
        }
        assert!(
            decoded,
            "Expected partial transcript event from SherpaSttEngine"
        );

        // 2. Whisper config
        let whisper_config = SherpaSttConfig {
            model_type: SherpaSttModelType::Whisper,
            language: "es".to_string(),
            encoder_path: std::path::PathBuf::from("models/whisper/encoder.onnx"),
            decoder_path: std::path::PathBuf::from("models/whisper/decoder.onnx"),
            joiner_path: None,
            tokens_path: std::path::PathBuf::from("models/whisper/tokens.txt"),
            ..Default::default()
        };
        assert!(whisper_config.validate().is_ok());
    }
}
