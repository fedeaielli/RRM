use std::collections::VecDeque;

/// Fixed-size circular buffer used as a history of metric values.
pub struct RingBuffer {
    buf: VecDeque<f32>,
    capacity: usize,
}

impl RingBuffer {
    /// Create a buffer holding at most `capacity` values.
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Maximum number of values the buffer can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Push a value, evicting the oldest value when the buffer is full.
    pub fn push(&mut self, value: f32) {
        if self.capacity == 0 {
            return;
        }
        if self.buf.len() >= self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(value);
    }

    /// Return the stored values ordered oldest to newest.
    pub fn as_slice(&self) -> Vec<f32> {
        self.buf.iter().copied().collect()
    }
}
