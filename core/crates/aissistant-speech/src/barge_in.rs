use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// High-priority barge-in interruption event triggered when user speaks during assistant audio playback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BargeInEvent {
    /// Timestamp in milliseconds when interruption was triggered.
    pub timestamp_ms: u64,
    /// Confidence probability of detected voice onset.
    pub confidence: f32,
    /// Consecutive speech analysis frames observed prior to trigger.
    pub consecutive_frames: usize,
}

/// Real-time barge-in detector coordinating VAD voice onset and audio playback state.
pub struct BargeInDetector {
    playback_active: Arc<AtomicBool>,
    onset_threshold: f32,
    consecutive_frames_required: usize,
    consecutive_speech_frames: usize,
    barge_in_count: AtomicU64,
    flush_callback: Option<Box<dyn Fn() + Send + Sync>>,
}

impl std::fmt::Debug for BargeInDetector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BargeInDetector")
            .field(
                "playback_active",
                &self.playback_active.load(Ordering::Relaxed),
            )
            .field("onset_threshold", &self.onset_threshold)
            .field("consecutive_speech_frames", &self.consecutive_speech_frames)
            .field(
                "total_barge_ins",
                &self.barge_in_count.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl BargeInDetector {
    /// Create a new barge-in detector bound to an atomic playback state flag.
    pub fn new(
        playback_active: Arc<AtomicBool>,
        onset_threshold: f32,
        consecutive_frames_required: usize,
    ) -> Self {
        Self {
            playback_active,
            onset_threshold,
            consecutive_frames_required: consecutive_frames_required.max(1),
            consecutive_speech_frames: 0,
            barge_in_count: AtomicU64::new(0),
            flush_callback: None,
        }
    }

    /// Register a closure invoked immediately when barge-in is triggered (e.g. `playback_stream.flush()`).
    pub fn on_flush<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.flush_callback = Some(Box::new(callback));
        self
    }

    /// Process a VAD frame speech probability.
    ///
    /// If playback is active and voice onset reaches threshold, triggers instant barge-in.
    pub fn process_vad_frame(
        &mut self,
        speech_probability: f32,
        timestamp_ms: u64,
    ) -> Option<BargeInEvent> {
        let is_playing = self.playback_active.load(Ordering::Acquire);

        if speech_probability >= self.onset_threshold {
            self.consecutive_speech_frames += 1;

            if is_playing && self.consecutive_speech_frames >= self.consecutive_frames_required {
                // Trigger barge-in interruption!
                self.barge_in_count.fetch_add(1, Ordering::Relaxed);
                self.consecutive_speech_frames = 0;

                if let Some(flush) = &self.flush_callback {
                    flush();
                }

                return Some(BargeInEvent {
                    timestamp_ms,
                    confidence: speech_probability,
                    consecutive_frames: self.consecutive_frames_required,
                });
            }
        } else {
            self.consecutive_speech_frames = 0;
        }

        None
    }

    /// Total count of successful barge-in interruptions triggered.
    #[inline]
    pub fn total_barge_ins(&self) -> u64 {
        self.barge_in_count.load(Ordering::Relaxed)
    }

    /// Reset internal speech frame counter.
    pub fn reset(&mut self) {
        self.consecutive_speech_frames = 0;
    }
}
