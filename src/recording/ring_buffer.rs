use std::sync::Arc;
use parking_lot::Mutex;

/// Internal state for ring buffer, consolidated into single mutex
struct RingBufferState {
    buffer: Vec<f32>,
    write_pos: usize,
    size: usize,
}

/// Ring buffer for audio streaming
/// Maintains a fixed-size buffer that overwrites oldest data when full
pub(crate) struct RingBuffer {
    state: Arc<Mutex<RingBufferState>>,
    capacity: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with specified capacity (in samples)
    /// For 30 seconds at 16kHz: 30 * 16000 = 480000 samples
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(RingBufferState {
                buffer: vec![0.0; capacity_samples],
                write_pos: 0,
                size: 0,
            })),
            capacity: capacity_samples,
        }
    }

    /// Create a ring buffer for ~30 seconds of audio at 16kHz
    pub fn new_30s() -> Self {
        Self::new(30 * 16000) // 30 seconds * 16000 samples/sec
    }

    /// Write samples to the buffer (overwrites oldest if full)
    pub fn write(&self, samples: &[f32]) {
        let mut state = self.state.lock();
        let capacity = self.capacity;

        for &sample in samples {
            let pos = state.write_pos;
            state.buffer[pos] = sample;
            state.write_pos = (pos + 1) % capacity;
            state.size = (state.size + 1).min(capacity);
        }
    }

    /// Read all available samples from the buffer (clears buffer)
    pub fn read_all(&self) -> Vec<f32> {
        let mut state = self.state.lock();

        if state.size == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(state.size);

        // Read from oldest to newest
        let start_pos = if state.size < self.capacity {
            0
        } else {
            state.write_pos
        };

        for i in 0..state.size {
            let idx = (start_pos + i) % self.capacity;
            result.push(state.buffer[idx]);
        }

        // Clear buffer
        state.size = 0;
        state.write_pos = 0;

        result
    }

    /// Read last N samples without clearing buffer
    pub fn peek_last(&self, n: usize) -> Vec<f32> {
        let state = self.state.lock();

        let read_size = n.min(state.size);
        if read_size == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(read_size);
        let start_pos = if state.size < self.capacity {
            0
        } else {
            state.write_pos
        };

        for i in 0..read_size {
            let idx = (start_pos + i) % self.capacity;
            result.push(state.buffer[idx]);
        }

        result
    }

    /// Clear the buffer
    pub fn clear(&self) {
        let mut state = self.state.lock();
        state.size = 0;
        state.write_pos = 0;
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new_30s()
    }
}
