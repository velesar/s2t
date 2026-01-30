//! Segmentation monitor for continuous recording mode.
//!
//! Extracts audio segmentation logic as a standalone add-on layer.
//! Any recorder can be segmented by attaching a `SegmentationMonitor`
//! that reads from the recorder's shared samples buffer.

use crate::domain::traits::VoiceDetection;
use crate::domain::types::AudioSegment;
use crate::recording::core::WHISPER_SAMPLE_RATE;
use crate::recording::ring_buffer::RingBuffer;
use crate::vad::{create_vad, VadConfig, VadEngine};
use async_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Configuration for audio segmentation.
pub struct SegmentationConfig {
    pub use_vad: bool,
    pub segment_interval_secs: u32,
    pub vad_silence_threshold_ms: u32,
    pub vad_min_speech_ms: u32,
    pub vad_engine: VadEngine,
    pub silero_threshold: f32,
}

impl Default for SegmentationConfig {
    fn default() -> Self {
        Self {
            use_vad: true,
            segment_interval_secs: 10,
            vad_silence_threshold_ms: 1000,
            vad_min_speech_ms: 500,
            vad_engine: VadEngine::WebRTC,
            silero_threshold: 0.5,
        }
    }
}

/// Monitors a shared samples buffer and produces audio segments.
///
/// Runs a background thread that periodically reads new samples from the
/// recorder's buffer into a ring buffer, checks for segmentation points
/// (via VAD or fixed intervals), and sends completed segments through a channel.
pub struct SegmentationMonitor {
    config: SegmentationConfig,
    ring_buffer: Arc<RingBuffer>,
    is_running: Arc<AtomicBool>,
    segment_counter: Arc<Mutex<usize>>,
    last_segment_time: Arc<Mutex<Option<Instant>>>,
    segment_tx: Arc<Mutex<Option<Sender<AudioSegment>>>>,
    is_speech_detected: Arc<AtomicBool>,
}

impl SegmentationMonitor {
    pub fn new(config: SegmentationConfig) -> Self {
        Self {
            config,
            ring_buffer: Arc::new(RingBuffer::new_30s()),
            is_running: Arc::new(AtomicBool::new(false)),
            segment_counter: Arc::new(Mutex::new(0)),
            last_segment_time: Arc::new(Mutex::new(None)),
            segment_tx: Arc::new(Mutex::new(None)),
            is_speech_detected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the monitoring thread.
    ///
    /// Reads new samples from `samples_buffer` (the recorder's shared buffer),
    /// feeds them into the ring buffer, and produces segments via the channel.
    pub fn start(&self, samples_buffer: Arc<Mutex<Vec<f32>>>, segment_tx: Sender<AudioSegment>) {
        self.ring_buffer.clear();
        *self.segment_tx.lock().unwrap() = Some(segment_tx);
        self.is_running.store(true, Ordering::SeqCst);
        *self.segment_counter.lock().unwrap() = 0;
        *self.last_segment_time.lock().unwrap() = Some(Instant::now());

        let ring_buffer = self.ring_buffer.clone();
        let is_running = self.is_running.clone();
        let segment_tx = self.segment_tx.clone();
        let segment_counter = self.segment_counter.clone();
        let last_segment_time = self.last_segment_time.clone();
        let use_vad = self.config.use_vad;
        let vad_engine = self.config.vad_engine;
        let vad_silence_threshold_ms = self.config.vad_silence_threshold_ms;
        let vad_min_speech_ms = self.config.vad_min_speech_ms;
        let silero_threshold = self.config.silero_threshold;
        let segment_interval = Duration::from_secs(self.config.segment_interval_secs as u64);
        let is_speech_detected = self.is_speech_detected.clone();

        std::thread::spawn(move || {
            let check_interval = Duration::from_millis(500);
            let mut last_samples_len = 0;

            // Create VAD inside thread — VAD implementations are not Send
            let vad: Option<Box<dyn VoiceDetection>> = if use_vad {
                let config = VadConfig {
                    engine: vad_engine,
                    silence_threshold_ms: vad_silence_threshold_ms,
                    min_speech_ms: vad_min_speech_ms,
                    silero_threshold,
                };
                create_vad(&config).ok()
            } else {
                None
            };

            while is_running.load(Ordering::SeqCst) {
                std::thread::sleep(check_interval);

                // Read only new samples from the recorder's shared buffer
                {
                    let samples = samples_buffer.lock().unwrap();
                    let current_len = samples.len();
                    if current_len > last_samples_len {
                        ring_buffer.write(&samples[last_samples_len..]);
                        last_samples_len = current_len;
                    }
                }

                // Minimum segment length: 0.5 seconds
                let min_samples = (WHISPER_SAMPLE_RATE as usize) / 2;

                // Check if we should create a segment
                let should_segment = if let Some(ref vad) = vad {
                    // VAD mode: check for speech end (silence after speech)
                    let samples = ring_buffer.peek_last(WHISPER_SAMPLE_RATE as usize * 5);

                    // Update speech detection state for UI (check last 1 second)
                    let recent_samples = ring_buffer.peek_last(WHISPER_SAMPLE_RATE as usize);
                    let speech_now = vad.is_speech(&recent_samples).unwrap_or(false);
                    is_speech_detected.store(speech_now, Ordering::SeqCst);

                    // Only segment if we have at least 1 second of audio
                    samples.len() >= WHISPER_SAMPLE_RATE as usize
                        && vad.detect_speech_end(&samples).unwrap_or(false)
                } else {
                    // No VAD — always show as "listening"
                    is_speech_detected.store(false, Ordering::SeqCst);

                    // Fixed interval fallback
                    let last_time = *last_segment_time.lock().unwrap();
                    last_time
                        .map(|t| Instant::now().duration_since(t) >= segment_interval)
                        .unwrap_or(false)
                };

                if should_segment {
                    let segment_samples = ring_buffer.read_all();

                    if segment_samples.len() >= min_samples {
                        let segment_id = {
                            let mut counter = segment_counter.lock().unwrap();
                            *counter += 1;
                            *counter
                        };

                        let start_time = last_segment_time
                            .lock()
                            .unwrap()
                            .unwrap_or_else(Instant::now);
                        let end_time = Instant::now();

                        let segment = AudioSegment {
                            samples: segment_samples,
                            start_time,
                            end_time,
                            segment_id,
                        };

                        if let Some(ref tx) = *segment_tx.lock().unwrap() {
                            if let Err(e) = tx.try_send(segment) {
                                eprintln!("Помилка відправки сегменту: {:?}", e);
                            }
                        }

                        *last_segment_time.lock().unwrap() = Some(end_time);
                    }
                }
            }
        });
    }

    /// Stop the monitoring thread, drain remaining audio, and send the final segment.
    ///
    /// Must be called **before** stopping the recorder, so the ring buffer
    /// and recorder's samples buffer are still available.
    pub fn stop(&self, samples_buffer: &Arc<Mutex<Vec<f32>>>) {
        self.is_running.store(false, Ordering::SeqCst);

        // Give the monitoring thread a moment to finish its current iteration
        std::thread::sleep(Duration::from_millis(100));

        // Get remaining samples from ring buffer
        let ring_remaining = self.ring_buffer.read_all();

        // Fallback: if ring buffer is empty, use last 5s from recorder's samples
        let mut remaining = ring_remaining;
        if remaining.is_empty() {
            let recorder_samples = samples_buffer.lock().unwrap();
            if !recorder_samples.is_empty() {
                let last_segment_samples = (WHISPER_SAMPLE_RATE as usize) * 5;
                let start = recorder_samples.len().saturating_sub(last_segment_samples);
                remaining = recorder_samples[start..].to_vec();
            }
        }

        // Minimum samples for a valid segment (0.5 seconds)
        let min_samples = (WHISPER_SAMPLE_RATE as usize) / 2;

        if remaining.len() >= min_samples {
            let segment_id = {
                let mut counter = self.segment_counter.lock().unwrap();
                *counter += 1;
                *counter
            };

            if let Some(ref tx) = *self.segment_tx.lock().unwrap() {
                let segment = AudioSegment {
                    samples: remaining,
                    start_time: self
                        .last_segment_time
                        .lock()
                        .unwrap()
                        .unwrap_or_else(Instant::now),
                    end_time: Instant::now(),
                    segment_id,
                };
                let _ = tx.send_blocking(segment);
            }
        }

        // Close the channel
        *self.segment_tx.lock().unwrap() = None;
    }

    /// Check if speech is currently detected (for UI display).
    pub fn is_speech_detected(&self) -> bool {
        self.is_speech_detected.load(Ordering::SeqCst)
    }
}
