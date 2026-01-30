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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_empty_buffer() {
        let rb = RingBuffer::new(100);
        assert_eq!(rb.capacity, 100);
        assert_eq!(rb.read_all(), Vec::<f32>::new());
    }

    #[test]
    fn test_new_30s_capacity() {
        let rb = RingBuffer::new_30s();
        assert_eq!(rb.capacity, 30 * 16000);
    }

    #[test]
    fn test_default_is_30s() {
        let rb = RingBuffer::default();
        assert_eq!(rb.capacity, 30 * 16000);
    }

    #[test]
    fn test_write_and_read_all() {
        let rb = RingBuffer::new(100);
        rb.write(&[1.0, 2.0, 3.0]);
        let result = rb.read_all();
        assert_eq!(result, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_read_all_clears_buffer() {
        let rb = RingBuffer::new(100);
        rb.write(&[1.0, 2.0, 3.0]);
        let _ = rb.read_all();
        let result = rb.read_all();
        assert!(result.is_empty());
    }

    #[test]
    fn test_multiple_writes_accumulate() {
        let rb = RingBuffer::new(100);
        rb.write(&[1.0, 2.0]);
        rb.write(&[3.0, 4.0]);
        let result = rb.read_all();
        assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_wrap_around_overwrites_oldest() {
        let rb = RingBuffer::new(4);
        // Write exactly capacity
        rb.write(&[1.0, 2.0, 3.0, 4.0]);
        // Write 2 more — should overwrite 1.0, 2.0
        rb.write(&[5.0, 6.0]);
        let result = rb.read_all();
        // Oldest data (1.0, 2.0) overwritten; should read [3.0, 4.0, 5.0, 6.0]
        assert_eq!(result, vec![3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_wrap_around_full_overwrite() {
        let rb = RingBuffer::new(3);
        // Write 6 samples into capacity-3 buffer: only last 3 survive
        rb.write(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let result = rb.read_all();
        assert_eq!(result, vec![4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_peek_last_without_clearing() {
        let rb = RingBuffer::new(100);
        rb.write(&[1.0, 2.0, 3.0, 4.0, 5.0]);

        // Peek last 3
        let peeked = rb.peek_last(3);
        assert_eq!(peeked, vec![1.0, 2.0, 3.0]);

        // Buffer should still be full
        let all = rb.read_all();
        assert_eq!(all, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn test_peek_last_more_than_available() {
        let rb = RingBuffer::new(100);
        rb.write(&[1.0, 2.0]);
        // Request more than available — should return what's there
        let peeked = rb.peek_last(10);
        assert_eq!(peeked, vec![1.0, 2.0]);
    }

    #[test]
    fn test_peek_last_empty() {
        let rb = RingBuffer::new(100);
        let peeked = rb.peek_last(5);
        assert!(peeked.is_empty());
    }

    #[test]
    fn test_peek_last_after_wrap() {
        let rb = RingBuffer::new(4);
        rb.write(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // Buffer contains [3.0, 4.0, 5.0, 6.0] after wrap
        let peeked = rb.peek_last(2);
        // peek_last reads from oldest, so first 2 of [3.0, 4.0, 5.0, 6.0]
        assert_eq!(peeked, vec![3.0, 4.0]);
    }

    #[test]
    fn test_clear() {
        let rb = RingBuffer::new(100);
        rb.write(&[1.0, 2.0, 3.0]);
        rb.clear();
        let result = rb.read_all();
        assert!(result.is_empty());
    }

    #[test]
    fn test_write_after_clear() {
        let rb = RingBuffer::new(100);
        rb.write(&[1.0, 2.0]);
        rb.clear();
        rb.write(&[3.0, 4.0]);
        let result = rb.read_all();
        assert_eq!(result, vec![3.0, 4.0]);
    }

    #[test]
    fn test_write_after_read_all() {
        let rb = RingBuffer::new(100);
        rb.write(&[1.0, 2.0]);
        let _ = rb.read_all();
        rb.write(&[5.0, 6.0]);
        let result = rb.read_all();
        assert_eq!(result, vec![5.0, 6.0]);
    }

    #[test]
    fn test_capacity_one() {
        let rb = RingBuffer::new(1);
        rb.write(&[1.0]);
        assert_eq!(rb.read_all(), vec![1.0]);
        rb.write(&[2.0, 3.0]);
        assert_eq!(rb.read_all(), vec![3.0]);
    }

    #[test]
    fn test_concurrent_write_read() {
        use std::sync::Arc;
        use std::thread;

        let rb = Arc::new(RingBuffer::new(10000));
        let rb_writer = rb.clone();

        let writer = thread::spawn(move || {
            for i in 0..1000 {
                rb_writer.write(&[i as f32]);
            }
        });

        writer.join().unwrap();
        let result = rb.read_all();
        assert_eq!(result.len(), 1000);
    }
}
