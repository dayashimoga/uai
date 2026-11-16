//! Voice pipeline supervisor coordinating audio capture, level metering,
//! wake-word spotting, and streaming speech-to-text across FFI.

use crate::error::FfiError;
use crate::events::{BridgeEvent, EventStreamSink};
use aissistant_audio::{AudioLevelMeter, VirtualAudioDevice};
use aissistant_speech::{
    AcousticKwsSpotter, AcousticSttEngine, StreamingTranscriptEvent, SttConfig, SttEngine,
    WakeWordConfig, WakeWordSpotter,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Operational listening mode for voice interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u32)]
pub enum VoiceListeningMode {
    #[default]
    PushToTalk = 0,
    WakeWord = 1,
}

impl From<u32> for VoiceListeningMode {
    fn from(value: u32) -> Self {
        match value {
            1 => VoiceListeningMode::WakeWord,
            _ => VoiceListeningMode::PushToTalk,
        }
    }
}

/// Unified voice session supervisor connecting audio I/O, level metering,
/// wake-word spotting, and speech recognition to the FFI event stream sink.
pub struct VoicePipelineSupervisor {
    mode: VoiceListeningMode,
    is_listening: AtomicBool,
    level_meter: AudioLevelMeter,
    wake_spotter: AcousticKwsSpotter,
    stt_engine: AcousticSttEngine,
    virtual_device: VirtualAudioDevice,
    event_sink: Arc<EventStreamSink>,
    keywords: Vec<String>,
}

impl std::fmt::Debug for VoicePipelineSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoicePipelineSupervisor")
            .field("mode", &self.mode)
            .field("is_listening", &self.is_listening.load(Ordering::Relaxed))
            .field("keywords", &self.keywords)
            .finish()
    }
}

impl VoicePipelineSupervisor {
    /// Creates a new `VoicePipelineSupervisor`.
    pub fn new(event_sink: Arc<EventStreamSink>) -> Result<Self, FfiError> {
        let level_meter = AudioLevelMeter::default();

        let kws_config = WakeWordConfig {
            keywords: vec!["hey assistant".to_string()],
            threshold: 0.60,
            sample_rate: 16_000,
            ..Default::default()
        };
        let wake_spotter = AcousticKwsSpotter::new(kws_config)
            .map_err(|e| FfiError::Voice(format!("Failed to initialize wake-word spotter: {e}")))?;

        let stt_config = SttConfig {
            language: "en".to_string(),
            sample_rate: 16_000,
            min_trailing_silence_ms: 600,
            ..Default::default()
        };
        let stt_engine = AcousticSttEngine::new(stt_config)
            .map_err(|e| FfiError::Voice(format!("Failed to initialize STT engine: {e}")))?;

        let virtual_device = VirtualAudioDevice::new(32_768)
            .map_err(|e| FfiError::Voice(format!("Failed to create virtual audio device: {e}")))?;

        Ok(Self {
            mode: VoiceListeningMode::PushToTalk,
            is_listening: AtomicBool::new(false),
            level_meter,
            wake_spotter,
            stt_engine,
            virtual_device,
            event_sink,
            keywords: vec!["hey assistant".to_string()],
        })
    }

    /// Starts audio listening in the specified mode.
    pub fn start_listening(&mut self, mode: VoiceListeningMode) -> Result<(), FfiError> {
        self.mode = mode;
        self.is_listening.store(true, Ordering::Release);
        self.virtual_device.set_muted(false);
        self.virtual_device.start_capture();

        let mode_str = match mode {
            VoiceListeningMode::PushToTalk => "push_to_talk",
            VoiceListeningMode::WakeWord => "wake_word",
        };

        self.event_sink.broadcast(BridgeEvent::VoiceStateChanged {
            is_listening: true,
            mode: mode_str.to_string(),
        });

        Ok(())
    }

    /// Stops audio listening and returns to idle state.
    pub fn stop_listening(&mut self) -> Result<(), FfiError> {
        self.is_listening.store(false, Ordering::Release);
        self.virtual_device.set_muted(true);
        self.virtual_device.pause_capture();

        self.event_sink.broadcast(BridgeEvent::VoiceStateChanged {
            is_listening: false,
            mode: "idle".to_string(),
        });

        Ok(())
    }

    /// Sets the active listening mode without stopping ongoing capture.
    pub fn set_mode(&mut self, mode: VoiceListeningMode) -> Result<(), FfiError> {
        self.mode = mode;
        let mode_str = match mode {
            VoiceListeningMode::PushToTalk => "push_to_talk",
            VoiceListeningMode::WakeWord => "wake_word",
        };

        self.event_sink.broadcast(BridgeEvent::VoiceStateChanged {
            is_listening: self.is_listening.load(Ordering::Acquire),
            mode: mode_str.to_string(),
        });

        Ok(())
    }

    /// Updates the active wake-word keywords.
    pub fn set_keywords(&mut self, keywords: &[String]) -> Result<(), FfiError> {
        self.wake_spotter
            .set_keywords(keywords)
            .map_err(|e| FfiError::Voice(format!("Failed to set wake words: {e}")))?;
        self.keywords = keywords.to_vec();
        Ok(())
    }

    /// Ingests streaming audio samples, updates level metering, checks wake-word,
    /// and streams STT transcripts across the event sink.
    pub fn process_audio_chunk(&mut self, samples: &[f32]) -> Result<(), FfiError> {
        if samples.is_empty() {
            return Ok(());
        }

        // 1. Audio level metering
        let meter_snapshot = self.level_meter.process(samples);
        self.event_sink.broadcast(BridgeEvent::VoiceLevelUpdated {
            rms_dbfs: meter_snapshot.rms_dbfs,
            peak_dbfs: meter_snapshot.peak_dbfs,
        });

        // 2. Ingest into virtual capture stream
        let _ = self
            .virtual_device
            .inject_capture_samples(samples, 16_000, 1);

        // 3. Wake-word detection (evaluated in WakeWord mode, or whenever continuous listening is on)
        if self.mode == VoiceListeningMode::WakeWord {
            if let Ok(Some(ev)) = self.wake_spotter.process_samples(samples) {
                self.event_sink.broadcast(BridgeEvent::WakeWordTriggered {
                    keyword: ev.keyword.clone(),
                    confidence: ev.confidence,
                });
                // Once wake word triggers, transition to active listening
                self.is_listening.store(true, Ordering::Release);
            }
        }

        // 4. Speech-to-text recognition
        if self.is_listening.load(Ordering::Acquire) {
            let _ = self.stt_engine.accept_waveform(samples);

            while let Ok(Some(event)) = self.stt_engine.decode_step() {
                match event {
                    StreamingTranscriptEvent::Partial(seg) => {
                        self.event_sink.broadcast(BridgeEvent::TranscriptUpdated {
                            text: seg.text,
                            is_final: false,
                            language: seg.language,
                            confidence: seg.confidence,
                        });
                    }
                    StreamingTranscriptEvent::Final(seg) => {
                        self.event_sink.broadcast(BridgeEvent::TranscriptUpdated {
                            text: seg.text,
                            is_final: true,
                            language: seg.language,
                            confidence: seg.confidence,
                        });
                        // In PushToTalk mode, a finalized sentence automatically returns to idle
                        if self.mode == VoiceListeningMode::PushToTalk {
                            self.is_listening.store(false, Ordering::Release);
                            self.event_sink.broadcast(BridgeEvent::VoiceStateChanged {
                                is_listening: false,
                                mode: "idle".to_string(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Returns whether the supervisor is currently listening.
    pub fn is_listening(&self) -> bool {
        self.is_listening.load(Ordering::Acquire)
    }

    /// Returns active voice listening mode.
    pub fn mode(&self) -> VoiceListeningMode {
        self.mode
    }

    /// Returns active monitored keywords.
    pub fn keywords(&self) -> &[String] {
        &self.keywords
    }

    /// Resets all internal buffers and state machines.
    pub fn reset(&mut self) {
        self.is_listening.store(false, Ordering::Release);
        self.level_meter.reset();
        self.wake_spotter.reset();
        self.stt_engine.reset();
    }

    /// Trims and clears all audio buffers and intermediate acoustic features to reclaim memory.
    pub fn clear_buffers(&mut self) {
        self.reset();
    }
}
