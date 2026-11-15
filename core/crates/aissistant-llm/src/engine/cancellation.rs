use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Thread-safe cooperative cancellation handle for LLM token generation loops.
///
/// Can be cloned and shared with audio barge-in detectors, timeout watchdogs,
/// or user UI cancel buttons.
#[derive(Debug, Clone, Default)]
pub struct LlmCancellationHandle {
    cancelled: Arc<AtomicBool>,
}

impl LlmCancellationHandle {
    /// Creates a new uncancelled handle.
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Signals cancellation to any ongoing generation thread.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Checks whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Resets the cancellation flag for a new generation turn.
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}
