use crate::audio::AudioRecorder;
use crate::ring_buffer::RingBuffer;
use anyhow::Result;
use async_channel::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SAMPLE_RATE: u32 = 16000;

/// Segment of audio ready for transcription
#[derive(Debug, Clone)]
pub struct AudioSegment {
    pub samples: Vec<f32>,
    pub start_time: Instant,
    pub end_time: Instant,
    pub segment_id: usize,
}

/// Continuous recorder with automatic segmentation
pub struct ContinuousRecorder {
    recorder: Arc<AudioRecorder>,
    ring_buffer: Arc<RingBuffer>,
    segment_tx: Arc<Mutex<Option<Sender<AudioSegment>>>>,
    is_recording: Arc<std::sync::atomic::AtomicBool>,
    segment_counter: Arc<Mutex<usize>>,
    last_segment_time: Arc<Mutex<Option<Instant>>>,
    segment_interval_secs: u32,
    use_vad: bool,
}

impl ContinuousRecorder {
    /// Create a new continuous recorder
    pub fn new(use_vad: bool, segment_interval_secs: u32) -> Result<Self> {
        // Note: VAD support will be added later due to Send trait constraints
        // For now, we use fixed intervals only

        Ok(Self {
            recorder: Arc::new(AudioRecorder::new()),
            ring_buffer: Arc::new(RingBuffer::new_30s()),
            segment_tx: Arc::new(Mutex::new(None)),
            is_recording: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            segment_counter: Arc::new(Mutex::new(0)),
            last_segment_time: Arc::new(Mutex::new(None)),
            segment_interval_secs,
            use_vad,
        })
    }

    /// Start continuous recording with segment callback
    pub fn start_continuous(
        &self,
        segment_tx: Sender<AudioSegment>,
    ) -> Result<()> {
        self.recorder.start_recording()?;
        self.ring_buffer.clear();
        *self.segment_tx.lock().unwrap() = Some(segment_tx);
        self.is_recording.store(true, std::sync::atomic::Ordering::SeqCst);
        *self.segment_counter.lock().unwrap() = 0;
        *self.last_segment_time.lock().unwrap() = Some(Instant::now());

        // Start monitoring thread for segmentation
        let ring_buffer = self.ring_buffer.clone();
        let is_recording = self.is_recording.clone();
        let segment_tx = self.segment_tx.clone();
        let segment_counter = self.segment_counter.clone();
        let last_segment_time = self.last_segment_time.clone();
        let use_vad = self.use_vad;
        let segment_interval = Duration::from_secs(self.segment_interval_secs as u64);

        // Start a thread that periodically reads from recorder and segments
        // For MVP: we'll read samples periodically and segment them
        let recorder_samples_ref = self.recorder.samples.clone();
        std::thread::spawn(move || {
            let check_interval = Duration::from_millis(500); // Check every 500ms
            let mut last_samples_len = 0;

            while is_recording.load(std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(check_interval);

                // Read new samples from recorder
                let current_samples = {
                    let samples = recorder_samples_ref.lock().unwrap();
                    samples.clone()
                };

                let new_samples_count = current_samples.len().saturating_sub(last_samples_len);
                if new_samples_count > 0 {
                    let new_samples = &current_samples[last_samples_len..];
                    ring_buffer.write(new_samples);
                    last_samples_len = current_samples.len();
                }

                // Check if we should create a segment
                // Note: VAD support temporarily disabled due to Send trait constraints
                // TODO: Implement VAD in a separate thread-safe way
                let should_segment = {
                    // Fixed interval: check time
                    let last_time = *last_segment_time.lock().unwrap();
                    last_time
                        .map(|t| Instant::now().duration_since(t) >= segment_interval)
                        .unwrap_or(false)
                };

                if should_segment {
                    // Extract segment from ring buffer
                    let segment_samples = ring_buffer.read_all();
                    if !segment_samples.is_empty() {
                        let segment_id = {
                            let mut counter = segment_counter.lock().unwrap();
                            *counter += 1;
                            *counter
                        };

                        let start_time = last_segment_time.lock().unwrap().unwrap_or_else(|| Instant::now());
                        let end_time = Instant::now();

                        let segment = AudioSegment {
                            samples: segment_samples,
                            start_time,
                            end_time,
                            segment_id,
                        };

                        // Send segment for transcription
                        if let Some(ref tx) = *segment_tx.lock().unwrap() {
                            let _ = tx.try_send(segment);
                        }

                        *last_segment_time.lock().unwrap() = Some(end_time);
                    }
                }
            }
        });

        Ok(())
    }


    /// Stop continuous recording
    pub fn stop_continuous(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.is_recording.store(false, std::sync::atomic::Ordering::SeqCst);
        *self.segment_tx.lock().unwrap() = None;

        // Get remaining samples from ring buffer
        let remaining = self.ring_buffer.read_all();

        // Also get final samples from recorder
        let (final_samples, completion_rx) = self.recorder.stop_recording();

        // Combine remaining + final
        let mut all_samples = remaining;
        all_samples.extend(final_samples);

        (all_samples, completion_rx)
    }

    /// Add samples to the ring buffer (called from audio callback)
    pub fn add_samples(&self, samples: &[f32]) {
        self.ring_buffer.write(samples);
    }

    /// Get current amplitude
    pub fn get_amplitude(&self) -> f32 {
        self.recorder.get_amplitude()
    }
}
