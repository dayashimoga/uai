use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::error::AudioError;

/// Operating mode for the audio ring buffer when full.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingBufferMode {
    /// Reject new samples when buffer is full (returns overrun error or partial count).
    Strict,
    /// Overwrite oldest unread samples when buffer is full (essential for wake-word buffer).
    Overwrite,
}

struct RingBufferStorage {
    buffer: Box<[UnsafeCell<f32>]>,
    mask: usize,
    capacity: usize,
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,
    mode: RingBufferMode,
    overruns: AtomicU64,
    underruns: AtomicU64,
    total_pushed: AtomicU64,
    total_popped: AtomicU64,
}

// Safety: SPSC invariants guarantee that reader and writer never access the same element
// concurrently. Atomic write_pos and read_pos with Acquire/Release orderings synchronize
// ownership of individual slot cells.
unsafe impl Send for RingBufferStorage {}
unsafe impl Sync for RingBufferStorage {}

impl std::fmt::Debug for RingBufferStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RingBufferStorage")
            .field("capacity", &self.capacity)
            .field("mode", &self.mode)
            .field("write_pos", &self.write_pos.load(Ordering::Relaxed))
            .field("read_pos", &self.read_pos.load(Ordering::Relaxed))
            .field("overruns", &self.overruns.load(Ordering::Relaxed))
            .field("underruns", &self.underruns.load(Ordering::Relaxed))
            .finish()
    }
}

/// Real-time safe lock-free Single-Producer Single-Consumer (SPSC) audio ring buffer.
#[derive(Debug, Clone)]
pub struct AudioRingBuffer {
    storage: Arc<RingBufferStorage>,
}

impl AudioRingBuffer {
    /// Create a new ring buffer with the given capacity.
    ///
    /// Capacity will be rounded up to the nearest power of two (minimum 64).
    pub fn new(capacity: usize, mode: RingBufferMode) -> Self {
        let rounded = capacity.max(64).next_power_of_two();
        let mask = rounded - 1;

        let mut data = Vec::with_capacity(rounded);
        for _ in 0..rounded {
            data.push(UnsafeCell::new(0.0f32));
        }

        Self {
            storage: Arc::new(RingBufferStorage {
                buffer: data.into_boxed_slice(),
                mask,
                capacity: rounded,
                write_pos: AtomicUsize::new(0),
                read_pos: AtomicUsize::new(0),
                mode,
                overruns: AtomicU64::new(0),
                underruns: AtomicU64::new(0),
                total_pushed: AtomicU64::new(0),
                total_popped: AtomicU64::new(0),
            }),
        }
    }

    /// Split this ring buffer into dedicated `RingBufferProducer` and `RingBufferConsumer` handles.
    pub fn split(self) -> (RingBufferProducer, RingBufferConsumer) {
        (
            RingBufferProducer {
                storage: Arc::clone(&self.storage),
            },
            RingBufferConsumer {
                storage: self.storage,
            },
        )
    }

    /// Total capacity of the buffer in samples.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.storage.capacity
    }

    /// Number of samples currently available to read.
    #[inline]
    pub fn available_read(&self) -> usize {
        let w = self.storage.write_pos.load(Ordering::Acquire);
        let r = self.storage.read_pos.load(Ordering::Relaxed);
        w.wrapping_sub(r)
    }

    /// Number of free sample slots currently available to write.
    #[inline]
    pub fn available_write(&self) -> usize {
        self.storage.capacity - self.available_read()
    }

    /// Returns true if buffer has no samples available to read.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.available_read() == 0
    }

    /// Returns true if buffer is completely full.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.available_read() >= self.storage.capacity
    }

    /// Push a single sample into the buffer.
    ///
    /// Real-time safe: Zero allocations, lock-free.
    pub fn push(&self, sample: f32) -> Result<(), AudioError> {
        let w = self.storage.write_pos.load(Ordering::Relaxed);
        let r = self.storage.read_pos.load(Ordering::Acquire);
        let count = w.wrapping_sub(r);

        if count >= self.storage.capacity {
            self.storage.overruns.fetch_add(1, Ordering::Relaxed);
            match self.storage.mode {
                RingBufferMode::Strict => {
                    return Err(AudioError::BufferOverflow {
                        capacity: self.storage.capacity,
                        requested: 1,
                    });
                }
                RingBufferMode::Overwrite => {
                    // Advance read pointer to discard oldest sample
                    self.storage.read_pos.fetch_add(1, Ordering::Release);
                }
            }
        }

        let slot = w & self.storage.mask;
        unsafe {
            *self.storage.buffer[slot].get() = sample;
        }

        self.storage
            .write_pos
            .store(w.wrapping_add(1), Ordering::Release);
        self.storage.total_pushed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Push a slice of samples into the buffer.
    ///
    /// Real-time safe: Zero allocations, lock-free. Returns count of samples written.
    pub fn push_slice(&self, samples: &[f32]) -> usize {
        if samples.is_empty() {
            return 0;
        }

        let mut written = 0;
        for &sample in samples {
            if self.push(sample).is_ok() {
                written += 1;
            } else {
                // Strict mode overflow
                break;
            }
        }
        written
    }

    /// Pop a single sample from the buffer.
    ///
    /// Real-time safe: Zero allocations, lock-free.
    pub fn pop(&self) -> Option<f32> {
        let r = self.storage.read_pos.load(Ordering::Relaxed);
        let w = self.storage.write_pos.load(Ordering::Acquire);

        if r == w {
            self.storage.underruns.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        let slot = r & self.storage.mask;
        let sample = unsafe { *self.storage.buffer[slot].get() };

        self.storage
            .read_pos
            .store(r.wrapping_add(1), Ordering::Release);
        self.storage.total_popped.fetch_add(1, Ordering::Relaxed);
        Some(sample)
    }

    /// Pop multiple samples from the buffer into the provided slice.
    ///
    /// Real-time safe: Zero allocations, lock-free. Returns count of samples read.
    pub fn pop_slice(&self, out: &mut [f32]) -> usize {
        if out.is_empty() {
            return 0;
        }

        let mut read = 0;
        while read < out.len() {
            if let Some(sample) = self.pop() {
                out[read] = sample;
                read += 1;
            } else {
                break;
            }
        }
        read
    }

    /// Peek samples without advancing the read pointer.
    pub fn peek_slice(&self, out: &mut [f32]) -> usize {
        let r = self.storage.read_pos.load(Ordering::Relaxed);
        let w = self.storage.write_pos.load(Ordering::Acquire);
        let available = w.wrapping_sub(r);
        let to_read = out.len().min(available);

        for (i, target) in out.iter_mut().take(to_read).enumerate() {
            let slot = (r.wrapping_add(i)) & self.storage.mask;
            *target = unsafe { *self.storage.buffer[slot].get() };
        }
        to_read
    }

    /// Clear all pending samples from the buffer.
    pub fn clear(&self) {
        let w = self.storage.write_pos.load(Ordering::Acquire);
        self.storage.read_pos.store(w, Ordering::Release);
    }

    /// Total count of overruns encountered.
    #[inline]
    pub fn overruns(&self) -> u64 {
        self.storage.overruns.load(Ordering::Relaxed)
    }

    /// Total count of underruns encountered.
    #[inline]
    pub fn underruns(&self) -> u64 {
        self.storage.underruns.load(Ordering::Relaxed)
    }

    /// Total count of samples successfully pushed.
    #[inline]
    pub fn total_pushed(&self) -> u64 {
        self.storage.total_pushed.load(Ordering::Relaxed)
    }

    /// Total count of samples successfully popped.
    #[inline]
    pub fn total_popped(&self) -> u64 {
        self.storage.total_popped.load(Ordering::Relaxed)
    }
}

/// Real-time safe audio producer handle (can be sent to audio capture thread).
#[derive(Debug)]
pub struct RingBufferProducer {
    storage: Arc<RingBufferStorage>,
}

unsafe impl Send for RingBufferProducer {}

impl RingBufferProducer {
    /// Push a single sample.
    #[inline]
    pub fn push(&self, sample: f32) -> Result<(), AudioError> {
        let w = self.storage.write_pos.load(Ordering::Relaxed);
        let r = self.storage.read_pos.load(Ordering::Acquire);
        let count = w.wrapping_sub(r);

        if count >= self.storage.capacity {
            self.storage.overruns.fetch_add(1, Ordering::Relaxed);
            match self.storage.mode {
                RingBufferMode::Strict => {
                    return Err(AudioError::BufferOverflow {
                        capacity: self.storage.capacity,
                        requested: 1,
                    });
                }
                RingBufferMode::Overwrite => {
                    self.storage.read_pos.fetch_add(1, Ordering::Release);
                }
            }
        }

        let slot = w & self.storage.mask;
        unsafe {
            *self.storage.buffer[slot].get() = sample;
        }

        self.storage
            .write_pos
            .store(w.wrapping_add(1), Ordering::Release);
        self.storage.total_pushed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Push a slice of samples.
    pub fn push_slice(&self, samples: &[f32]) -> usize {
        let mut written = 0;
        for &s in samples {
            if self.push(s).is_ok() {
                written += 1;
            } else {
                break;
            }
        }
        written
    }

    /// Capacity in samples.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.storage.capacity
    }

    /// Available space in samples.
    #[inline]
    pub fn available_write(&self) -> usize {
        let w = self.storage.write_pos.load(Ordering::Relaxed);
        let r = self.storage.read_pos.load(Ordering::Acquire);
        self.storage.capacity - w.wrapping_sub(r)
    }

    /// Overruns count.
    #[inline]
    pub fn overruns(&self) -> u64 {
        self.storage.overruns.load(Ordering::Relaxed)
    }
}

/// Real-time safe audio consumer handle (can be sent to worker or STT thread).
#[derive(Debug)]
pub struct RingBufferConsumer {
    storage: Arc<RingBufferStorage>,
}

unsafe impl Send for RingBufferConsumer {}

impl RingBufferConsumer {
    /// Pop a single sample.
    #[inline]
    pub fn pop(&self) -> Option<f32> {
        let r = self.storage.read_pos.load(Ordering::Relaxed);
        let w = self.storage.write_pos.load(Ordering::Acquire);

        if r == w {
            self.storage.underruns.fetch_add(1, Ordering::Relaxed);
            return None;
        }

        let slot = r & self.storage.mask;
        let sample = unsafe { *self.storage.buffer[slot].get() };

        self.storage
            .read_pos
            .store(r.wrapping_add(1), Ordering::Release);
        self.storage.total_popped.fetch_add(1, Ordering::Relaxed);
        Some(sample)
    }

    /// Pop multiple samples into slice.
    pub fn pop_slice(&self, out: &mut [f32]) -> usize {
        let mut read = 0;
        while read < out.len() {
            if let Some(s) = self.pop() {
                out[read] = s;
                read += 1;
            } else {
                break;
            }
        }
        read
    }

    /// Peek samples without consuming.
    pub fn peek_slice(&self, out: &mut [f32]) -> usize {
        let r = self.storage.read_pos.load(Ordering::Relaxed);
        let w = self.storage.write_pos.load(Ordering::Acquire);
        let available = w.wrapping_sub(r);
        let to_read = out.len().min(available);

        for (i, target) in out.iter_mut().take(to_read).enumerate() {
            let slot = (r.wrapping_add(i)) & self.storage.mask;
            *target = unsafe { *self.storage.buffer[slot].get() };
        }
        to_read
    }

    /// Available samples to read.
    #[inline]
    pub fn available_read(&self) -> usize {
        let w = self.storage.write_pos.load(Ordering::Acquire);
        let r = self.storage.read_pos.load(Ordering::Relaxed);
        w.wrapping_sub(r)
    }

    /// Underruns count.
    #[inline]
    pub fn underruns(&self) -> u64 {
        self.storage.underruns.load(Ordering::Relaxed)
    }
}
