use crate::audio::AudioRecorder;
use crate::ring_buffer::RingBuffer;
use crate::vad::VoiceActivityDetector;
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
    vad_silence_threshold_ms: u32,
    vad_min_speech_ms: u32,
    /// Current VAD speech detection state (for UI display)
    is_speech_detected: Arc<std::sync::atomic::AtomicBool>,
}

impl ContinuousRecorder {
    /// Create a new continuous recorder
    pub fn new(
        use_vad: bool,
        segment_interval_secs: u32,
        vad_silence_threshold_ms: u32,
        vad_min_speech_ms: u32,
    ) -> Result<Self> {
        Ok(Self {
            recorder: Arc::new(AudioRecorder::new()),
            ring_buffer: Arc::new(RingBuffer::new_30s()),
            segment_tx: Arc::new(Mutex::new(None)),
            is_recording: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            segment_counter: Arc::new(Mutex::new(0)),
            last_segment_time: Arc::new(Mutex::new(None)),
            segment_interval_secs,
            use_vad,
            vad_silence_threshold_ms,
            vad_min_speech_ms,
            is_speech_detected: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
        let vad_silence_threshold_ms = self.vad_silence_threshold_ms;
        let vad_min_speech_ms = self.vad_min_speech_ms;
        let segment_interval = Duration::from_secs(self.segment_interval_secs as u64);
        let is_speech_detected = self.is_speech_detected.clone();

        // Start a thread that periodically reads from recorder and segments
        let recorder_samples_ref = self.recorder.samples.clone();
        std::thread::spawn(move || {
            let check_interval = Duration::from_millis(500); // Check every 500ms
            let mut last_samples_len = 0;

            // Create VAD INSIDE thread - solves Send trait issue
            // (webrtc_vad::Vad is not Send, so we can't pass it across thread boundaries)
            let vad = if use_vad {
                VoiceActivityDetector::with_thresholds(vad_silence_threshold_ms, vad_min_speech_ms).ok()
            } else {
                None
            };

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

                // Minimum segment length: 0.5 seconds
                let min_samples = (SAMPLE_RATE as usize) / 2;

                // Check if we should create a segment
                let should_segment = if let Some(ref vad) = vad {
                    // VAD mode: check for speech end (silence after speech)
                    // Peek last 5 seconds of audio for better VAD context
                    let samples = ring_buffer.peek_last(SAMPLE_RATE as usize * 5);

                    // Update speech detection state for UI (check last 1 second)
                    let recent_samples = ring_buffer.peek_last(SAMPLE_RATE as usize);
                    let speech_now = vad.is_speech(&recent_samples).unwrap_or(false);
                    is_speech_detected.store(speech_now, std::sync::atomic::Ordering::SeqCst);

                    // Only segment if we have at least 1 second of audio
                    samples.len() >= SAMPLE_RATE as usize &&
                        vad.detect_speech_end(&samples).unwrap_or(false)
                } else {
                    // No VAD - always show as "listening"
                    is_speech_detected.store(false, std::sync::atomic::Ordering::SeqCst);

                    // Fixed interval fallback: check if enough time has passed
                    let last_time = *last_segment_time.lock().unwrap();
                    last_time
                        .map(|t| Instant::now().duration_since(t) >= segment_interval)
                        .unwrap_or(false)
                };

                if should_segment {
                    // Extract segment from ring buffer
                    let segment_samples = ring_buffer.read_all();

                    // Only send segment if it meets minimum length requirement
                    if segment_samples.len() >= min_samples {
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
                            if let Err(e) = tx.try_send(segment) {
                                eprintln!("Помилка відправки сегменту: {:?}", e);
                            }
                        }

                        *last_segment_time.lock().unwrap() = Some(end_time);
                    }
                    // If too short, audio stays in buffer and accumulates
                }
            }
        });

        Ok(())
    }


    /// Stop continuous recording
    pub fn stop_continuous(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.is_recording.store(false, std::sync::atomic::Ordering::SeqCst);

        // Give the monitoring thread a moment to finish its current iteration
        std::thread::sleep(Duration::from_millis(100));

        // Get remaining samples from ring buffer
        let ring_remaining = self.ring_buffer.read_all();

        // Also get any samples that are still in the recorder but not yet in the ring buffer
        let recorder_samples = self.recorder.samples.lock().unwrap().clone();

        // Combine: ring buffer content + any unprocessed recorder samples
        // The ring buffer might have already processed some of these, so we take
        // only samples that weren't sent as segments yet
        let mut remaining = ring_remaining;
        if remaining.is_empty() && !recorder_samples.is_empty() {
            // If ring buffer is empty, the monitoring thread might have just processed it
            // Use the last portion of recorder samples as the final segment
            let last_segment_samples = (SAMPLE_RATE as usize) * 5; // Up to 5 seconds
            let start = recorder_samples.len().saturating_sub(last_segment_samples);
            remaining = recorder_samples[start..].to_vec();
        }

        // Minimum samples for a valid segment (0.5 seconds)
        let min_samples = (SAMPLE_RATE as usize) / 2;

        // Send remaining audio as final segment if it meets minimum length
        if remaining.len() >= min_samples {
            let segment_id = {
                let mut counter = self.segment_counter.lock().unwrap();
                *counter += 1;
                *counter
            };

            if let Some(ref tx) = *self.segment_tx.lock().unwrap() {
                let segment = AudioSegment {
                    samples: remaining,
                    start_time: self.last_segment_time.lock().unwrap().unwrap_or_else(Instant::now),
                    end_time: Instant::now(),
                    segment_id,
                };
                // Use blocking send to ensure the segment is queued
                let _ = tx.send_blocking(segment);
            }
        }

        // NOW close the channel
        *self.segment_tx.lock().unwrap() = None;

        // Get final samples from recorder
        let (final_samples, completion_rx) = self.recorder.stop_recording();

        (final_samples, completion_rx)
    }

    /// Add samples to the ring buffer (called from audio callback)
    pub fn add_samples(&self, samples: &[f32]) {
        self.ring_buffer.write(samples);
    }

    /// Get current amplitude
    pub fn get_amplitude(&self) -> f32 {
        self.recorder.get_amplitude()
    }

    /// Check if speech is currently detected (for UI display)
    pub fn is_speech_detected(&self) -> bool {
        self.is_speech_detected.load(std::sync::atomic::Ordering::SeqCst)
    }
}
