use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::error::SpeechError;

/// Configuration parameters for Voice Activity Detection and speech segmentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    /// Expected audio sample rate in Hz (standard 16,000 Hz).
    pub sample_rate: u32,
    /// Analysis window size in samples (standard 512 samples = 32 ms at 16 kHz).
    pub window_size: usize,
    /// Probability threshold for classifying a window as speech (0.0 to 1.0, default 0.5).
    pub speech_threshold: f32,
    /// Minimum consecutive speech duration required to trigger speech onset (in milliseconds).
    pub min_speech_duration_ms: u32,
    /// Minimum trailing silence duration required to trigger end-of-speech utterance completion (in milliseconds).
    pub min_silence_duration_ms: u32,
    /// Pre-speech padding duration preserved before speech onset (in milliseconds, default 200 ms).
    pub speech_pad_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            window_size: 512,
            speech_threshold: 0.5,
            min_speech_duration_ms: 64,
            min_silence_duration_ms: 600,
            speech_pad_ms: 200,
        }
    }
}

/// Dynamic state machine status of the utterance segmenter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VadState {
    /// Ambient background silence.
    Silence,
    /// Speech detected but waiting for minimum onset duration confirmation.
    OnsetPending { consecutive_frames: usize },
    /// Confirmed active user speech in progress.
    Speaking { active_ms: usize },
    /// Silence detected after speech; waiting for trailing silence timeout confirmation.
    OffsetPending {
        consecutive_silence_frames: usize,
        active_ms: usize,
    },
}

/// Discrete event produced by the VAD utterance segmenter.
#[derive(Debug, Clone, PartialEq)]
pub enum VadEvent {
    /// Voice onset detected and confirmed.
    SpeechStarted { timestamp_ms: u64 },
    /// Active speech continuing with given confidence probability.
    SpeechContinuing { probability: f32 },
    /// End-of-speech utterance boundary completed with accumulated speech audio.
    SpeechFinished {
        utterance: Vec<f32>,
        duration_ms: u64,
    },
    /// Ambient silence.
    Silence,
}

/// Abstract interface for offline Voice Activity Detection algorithms.
pub trait VadEngine: Send + Sync {
    /// Process a single analysis window (typically 512 samples) and return speech probability in [0.0, 1.0].
    fn process_window(&mut self, window: &[f32]) -> Result<f32, SpeechError>;
    /// Reset internal state / recurrent hidden states.
    fn reset(&mut self);
    /// Expected window size in samples.
    fn window_size(&self) -> usize;
}

/// Streaming utterance segmenter converting continuous audio frames into discrete speech segments.
pub struct UtteranceSegmenter {
    config: VadConfig,
    engine: Box<dyn VadEngine>,
    state: VadState,
    window_buffer: Vec<f32>,
    pre_speech_ring: VecDeque<f32>,
    current_utterance: Vec<f32>,
    max_pre_speech_samples: usize,
    frames_for_onset: usize,
    frames_for_offset: usize,
    total_samples_processed: usize,
}

impl std::fmt::Debug for UtteranceSegmenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UtteranceSegmenter")
            .field("config", &self.config)
            .field("state", &self.state)
            .field("current_utterance_samples", &self.current_utterance.len())
            .field("total_samples_processed", &self.total_samples_processed)
            .finish()
    }
}

impl UtteranceSegmenter {
    /// Create a new utterance segmenter with the given VAD engine and configuration.
    pub fn new(engine: Box<dyn VadEngine>, config: VadConfig) -> Self {
        let ms_per_sample = config.sample_rate as f64 / 1000.0;
        let ms_per_window = (config.window_size as f64 / ms_per_sample).max(1.0);

        let frames_for_onset =
            ((config.min_speech_duration_ms as f64 / ms_per_window).ceil() as usize).max(1);
        let frames_for_offset =
            ((config.min_silence_duration_ms as f64 / ms_per_window).ceil() as usize).max(1);
        let max_pre_speech_samples = (config.speech_pad_ms as f64 * ms_per_sample).round() as usize;

        Self {
            config,
            engine,
            state: VadState::Silence,
            window_buffer: Vec::with_capacity(512),
            pre_speech_ring: VecDeque::with_capacity(max_pre_speech_samples),
            current_utterance: Vec::new(),
            max_pre_speech_samples,
            frames_for_onset,
            frames_for_offset,
            total_samples_processed: 0,
        }
    }

    /// Process a stream of incoming 16 kHz audio samples and yield any VAD events.
    pub fn process_samples(&mut self, samples: &[f32]) -> Result<Vec<VadEvent>, SpeechError> {
        let mut events = Vec::new();

        for &sample in samples {
            self.window_buffer.push(sample);
            self.total_samples_processed += 1;

            if self.window_buffer.len() >= self.config.window_size {
                let window = std::mem::replace(
                    &mut self.window_buffer,
                    Vec::with_capacity(self.config.window_size),
                );
                let event = self.process_window(&window)?;
                if event != VadEvent::Silence {
                    events.push(event);
                }
            }
        }

        Ok(events)
    }

    fn process_window(&mut self, window: &[f32]) -> Result<VadEvent, SpeechError> {
        let prob = self.engine.process_window(window)?;
        let is_speech = prob >= self.config.speech_threshold;
        let window_duration_ms =
            (window.len() as f64 / (self.config.sample_rate as f64 / 1000.0)).round() as usize;

        match self.state {
            VadState::Silence => {
                if is_speech {
                    self.state = VadState::OnsetPending {
                        consecutive_frames: 1,
                    };
                    if self.frames_for_onset <= 1 {
                        return self.trigger_speech_onset(window);
                    }
                } else {
                    // Maintain pre-speech ring buffer
                    for &s in window {
                        if self.pre_speech_ring.len() >= self.max_pre_speech_samples {
                            self.pre_speech_ring.pop_front();
                        }
                        self.pre_speech_ring.push_back(s);
                    }
                }
                Ok(VadEvent::Silence)
            }
            VadState::OnsetPending { consecutive_frames } => {
                if is_speech {
                    let new_count = consecutive_frames + 1;
                    if new_count >= self.frames_for_onset {
                        self.trigger_speech_onset(window)
                    } else {
                        self.state = VadState::OnsetPending {
                            consecutive_frames: new_count,
                        };
                        Ok(VadEvent::Silence)
                    }
                } else {
                    // False alarm: revert to silence
                    self.state = VadState::Silence;
                    Ok(VadEvent::Silence)
                }
            }
            VadState::Speaking { active_ms } => {
                self.current_utterance.extend_from_slice(window);
                if is_speech {
                    self.state = VadState::Speaking {
                        active_ms: active_ms + window_duration_ms,
                    };
                    Ok(VadEvent::SpeechContinuing { probability: prob })
                } else {
                    self.state = VadState::OffsetPending {
                        consecutive_silence_frames: 1,
                        active_ms: active_ms + window_duration_ms,
                    };
                    Ok(VadEvent::SpeechContinuing { probability: prob })
                }
            }
            VadState::OffsetPending {
                consecutive_silence_frames,
                active_ms,
            } => {
                self.current_utterance.extend_from_slice(window);
                if is_speech {
                    // Resumed speaking during trailing silence
                    self.state = VadState::Speaking {
                        active_ms: active_ms + window_duration_ms,
                    };
                    Ok(VadEvent::SpeechContinuing { probability: prob })
                } else {
                    let new_silence_count = consecutive_silence_frames + 1;
                    if new_silence_count >= self.frames_for_offset {
                        // Utterance complete!
                        let utterance = std::mem::take(&mut self.current_utterance);
                        let duration_ms = (utterance.len() as f64
                            / (self.config.sample_rate as f64 / 1000.0))
                            .round() as u64;
                        self.state = VadState::Silence;
                        self.pre_speech_ring.clear();

                        Ok(VadEvent::SpeechFinished {
                            utterance,
                            duration_ms,
                        })
                    } else {
                        self.state = VadState::OffsetPending {
                            consecutive_silence_frames: new_silence_count,
                            active_ms: active_ms + window_duration_ms,
                        };
                        Ok(VadEvent::SpeechContinuing { probability: prob })
                    }
                }
            }
        }
    }

    fn trigger_speech_onset(&mut self, current_window: &[f32]) -> Result<VadEvent, SpeechError> {
        let window_duration_ms = (current_window.len() as f64
            / (self.config.sample_rate as f64 / 1000.0))
            .round() as usize;
        self.state = VadState::Speaking {
            active_ms: window_duration_ms,
        };

        // Prepend pre-speech padding
        self.current_utterance.clear();
        self.current_utterance
            .extend(self.pre_speech_ring.drain(..));
        self.current_utterance.extend_from_slice(current_window);

        let timestamp_ms = (self.total_samples_processed as f64
            / (self.config.sample_rate as f64 / 1000.0))
            .round() as u64;
        Ok(VadEvent::SpeechStarted { timestamp_ms })
    }

    /// True if currently in active speech or offset pending state.
    pub fn is_speaking(&self) -> bool {
        matches!(
            self.state,
            VadState::Speaking { .. } | VadState::OffsetPending { .. }
        )
    }

    /// Current VAD state.
    pub fn state(&self) -> VadState {
        self.state
    }

    /// Reset segmenter state and buffer.
    pub fn reset(&mut self) {
        self.engine.reset();
        self.state = VadState::Silence;
        self.window_buffer.clear();
        self.pre_speech_ring.clear();
        self.current_utterance.clear();
    }
}
