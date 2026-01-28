use std::sync::{Arc, Mutex};

/// Ring buffer for audio streaming
/// Maintains a fixed-size buffer that overwrites oldest data when full
pub struct RingBuffer {
    buffer: Arc<Mutex<Vec<f32>>>,
    capacity: usize,
    write_pos: Arc<Mutex<usize>>,
    size: Arc<Mutex<usize>>,
}

impl RingBuffer {
    /// Create a new ring buffer with specified capacity (in samples)
    /// For 30 seconds at 16kHz: 30 * 16000 = 480000 samples
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(vec![0.0; capacity_samples])),
            capacity: capacity_samples,
            write_pos: Arc::new(Mutex::new(0)),
            size: Arc::new(Mutex::new(0)),
        }
    }

    /// Create a ring buffer for ~30 seconds of audio at 16kHz
    pub fn new_30s() -> Self {
        Self::new(30 * 16000) // 30 seconds * 16000 samples/sec
    }

    /// Write samples to the buffer (overwrites oldest if full)
    pub fn write(&self, samples: &[f32]) {
        let mut buffer = self.buffer.lock().unwrap();
        let mut write_pos = self.write_pos.lock().unwrap();
        let mut size = self.size.lock().unwrap();

        for &sample in samples {
            buffer[*write_pos] = sample;
            *write_pos = (*write_pos + 1) % self.capacity;
            *size = (*size + 1).min(self.capacity);
        }
    }

    /// Read all available samples from the buffer (clears buffer)
    pub fn read_all(&self) -> Vec<f32> {
        let buffer = self.buffer.lock().unwrap();
        let mut write_pos = self.write_pos.lock().unwrap();
        let mut size = self.size.lock().unwrap();

        let current_size = *size;
        let current_write_pos = *write_pos;

        if current_size == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(current_size);

        // Read from oldest to newest
        let start_pos = if current_size < self.capacity {
            0
        } else {
            current_write_pos
        };

        for i in 0..current_size {
            let idx = (start_pos + i) % self.capacity;
            result.push(buffer[idx]);
        }

        // Clear buffer
        *size = 0;
        *write_pos = 0;

        result
    }

    /// Read last N samples without clearing buffer
    pub fn peek_last(&self, n: usize) -> Vec<f32> {
        let buffer = self.buffer.lock().unwrap();
        let write_pos = self.write_pos.lock().unwrap();
        let size = self.size.lock().unwrap();

        let read_size = n.min(*size);
        if read_size == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(read_size);
        let start_pos = if *size < self.capacity {
            0
        } else {
            *write_pos
        };

        for i in 0..read_size {
            let idx = (start_pos + i) % self.capacity;
            result.push(buffer[idx]);
        }

        result
    }

    /// Get current size (number of samples in buffer)
    pub fn len(&self) -> usize {
        *self.size.lock().unwrap()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear the buffer
    pub fn clear(&self) {
        *self.size.lock().unwrap() = 0;
        *self.write_pos.lock().unwrap() = 0;
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new_30s()
    }
}
