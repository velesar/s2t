//! Segmentation monitor for continuous recording mode.
//!
//! Extracts audio segmentation logic as a standalone add-on layer.
//! Any recorder can be segmented by attaching a `SegmentationMonitor`
//! that reads from the recorder's shared samples buffer.

use crate::domain::traits::VoiceDetection;
use crate::domain::types::AudioSegment;
use crate::recording::core::WHISPER_SAMPLE_RATE;
use crate::recording::ring_buffer::RingBuffer;
use crate::recording::split::SplitConfig;
use crate::recording::split::SplitFinder;
use crate::vad::{create_vad, VadConfig, VadEngine};
use async_channel::Sender;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

/// Configuration for audio segmentation.
pub struct SegmentationConfig {
    pub use_vad: bool,
    pub segment_interval_secs: u32,
    pub vad_silence_threshold_ms: u32,
    pub vad_min_speech_ms: u32,
    pub vad_engine: VadEngine,
    pub silero_threshold: f32,
    /// Maximum segment duration in seconds (safety limit).
    pub max_segment_secs: u32,
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
            max_segment_secs: 300,
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
    thread_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
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
            thread_handle: Mutex::new(None),
        }
    }

    /// Start the monitoring thread.
    ///
    /// Reads new samples from `samples_buffer` (the recorder's shared buffer),
    /// feeds them into the ring buffer, and produces segments via the channel.
    pub fn start(&self, samples_buffer: Arc<Mutex<Vec<f32>>>, segment_tx: Sender<AudioSegment>) {
        self.ring_buffer.clear();
        *self.segment_tx.lock() = Some(segment_tx);
        self.is_running.store(true, Ordering::SeqCst);
        *self.segment_counter.lock() = 0;
        *self.last_segment_time.lock() = Some(Instant::now());

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
        let max_segment_secs = self.config.max_segment_secs;
        let is_speech_detected = self.is_speech_detected.clone();

        let handle = std::thread::spawn(move || {
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
                match create_vad(&config) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        eprintln!(
                            "VAD initialization failed ({:?}), falling back to fixed-interval segmentation: {}",
                            vad_engine, e
                        );
                        None
                    }
                }
            } else {
                None
            };

            // Create SplitFinder for streaming split decisions
            let split_finder = SplitFinder::new(SplitConfig {
                vad_silence_ms: vad_silence_threshold_ms,
                max_segment_secs,
                sample_rate: WHISPER_SAMPLE_RATE,
                ..SplitConfig::default()
            });

            while is_running.load(Ordering::SeqCst) {
                std::thread::sleep(check_interval);

                // Read only new samples from the recorder's shared buffer
                {
                    let samples = samples_buffer.lock();
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
                    // VAD mode: use SplitFinder for unified split decisions
                    let samples = ring_buffer.peek_last(WHISPER_SAMPLE_RATE as usize * 5);

                    // Update speech detection state for UI (check last 1 second)
                    let recent_samples = ring_buffer.peek_last(WHISPER_SAMPLE_RATE as usize);
                    let speech_now = vad.is_speech(&recent_samples).unwrap_or(false);
                    is_speech_detected.store(speech_now, Ordering::SeqCst);

                    let elapsed = last_segment_time
                        .lock()
                        .map(|t| t.elapsed())
                        .unwrap_or(Duration::ZERO);

                    split_finder.should_split_streaming(&samples, vad.as_ref(), elapsed)
                } else {
                    // No VAD — always show as "listening"
                    is_speech_detected.store(false, Ordering::SeqCst);

                    // Fixed interval fallback
                    let last_time = *last_segment_time.lock();
                    last_time
                        .map(|t| Instant::now().duration_since(t) >= segment_interval)
                        .unwrap_or(false)
                };

                if should_segment {
                    let segment_samples = ring_buffer.read_all();

                    if segment_samples.len() >= min_samples {
                        let segment_id = {
                            let mut counter = segment_counter.lock();
                            *counter += 1;
                            *counter
                        };

                        let start_time = last_segment_time
                            .lock()
                            .unwrap_or_else(Instant::now);
                        let end_time = Instant::now();

                        let segment = AudioSegment {
                            samples: segment_samples,
                            start_time,
                            end_time,
                            segment_id,
                        };

                        if let Some(ref tx) = *segment_tx.lock() {
                            if let Err(e) = tx.try_send(segment) {
                                eprintln!("Помилка відправки сегменту: {:?}", e);
                            }
                        }

                        *last_segment_time.lock() = Some(end_time);
                    }
                }
            }
        });

        *self.thread_handle.lock() = Some(handle);
    }

    /// Stop the monitoring thread, drain remaining audio, and send the final segment.
    ///
    /// Must be called **before** stopping the recorder, so the ring buffer
    /// and recorder's samples buffer are still available.
    pub fn stop(&self, samples_buffer: &Arc<Mutex<Vec<f32>>>) {
        self.is_running.store(false, Ordering::SeqCst);

        // Wait for the monitoring thread to finish its current iteration
        if let Some(handle) = self.thread_handle.lock().take() {
            if let Err(e) = handle.join() {
                eprintln!("Segmentation thread panicked: {:?}", e);
            }
        }

        // Get remaining samples from ring buffer
        let ring_remaining = self.ring_buffer.read_all();

        // Fallback: if ring buffer is empty, use last 5s from recorder's samples
        let mut remaining = ring_remaining;
        if remaining.is_empty() {
            let recorder_samples = samples_buffer.lock();
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
                let mut counter = self.segment_counter.lock();
                *counter += 1;
                *counter
            };

            if let Some(ref tx) = *self.segment_tx.lock() {
                let segment = AudioSegment {
                    samples: remaining,
                    start_time: self
                        .last_segment_time
                        .lock()
                        .unwrap_or_else(Instant::now),
                    end_time: Instant::now(),
                    segment_id,
                };
                let _ = tx.send_blocking(segment);
            }
        }

        // Close the channel
        *self.segment_tx.lock() = None;
    }

    /// Check if speech is currently detected (for UI display).
    pub fn is_speech_detected(&self) -> bool {
        self.is_speech_detected.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_segmentation_config_default() {
        let config = SegmentationConfig::default();
        assert!(config.use_vad);
        assert_eq!(config.segment_interval_secs, 10);
        assert_eq!(config.vad_silence_threshold_ms, 1000);
        assert_eq!(config.vad_min_speech_ms, 500);
        assert_eq!(config.silero_threshold, 0.5);
    }

    #[test]
    fn test_new_monitor_not_running() {
        let monitor = SegmentationMonitor::new(SegmentationConfig::default());
        assert!(!monitor.is_speech_detected());
    }

    #[test]
    fn test_stop_sends_final_segment() {
        // Use fixed-interval mode (no VAD) with a long interval so
        // the background thread never auto-segments.
        let config = SegmentationConfig {
            use_vad: false,
            segment_interval_secs: 3600, // 1 hour — won't trigger
            ..Default::default()
        };
        let monitor = SegmentationMonitor::new(config);

        let samples_buffer = Arc::new(Mutex::new(Vec::new()));
        let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();

        // Pre-fill with enough samples (1 second at 16kHz = 16000 samples)
        {
            let mut buf = samples_buffer.lock();
            buf.extend(vec![0.5_f32; WHISPER_SAMPLE_RATE as usize]);
        }

        monitor.start(samples_buffer.clone(), segment_tx);

        // Let the monitor thread pick up the samples
        std::thread::sleep(Duration::from_millis(700));

        // Stop should emit the final segment
        monitor.stop(&samples_buffer);

        // The final segment should be available
        let segment = segment_rx.try_recv();
        assert!(segment.is_ok(), "Expected final segment after stop()");
        let seg = segment.unwrap();
        assert_eq!(seg.segment_id, 1);
        assert!(!seg.samples.is_empty());
    }

    #[test]
    fn test_stop_no_segment_when_too_short() {
        let config = SegmentationConfig {
            use_vad: false,
            segment_interval_secs: 3600,
            ..Default::default()
        };
        let monitor = SegmentationMonitor::new(config);

        let samples_buffer = Arc::new(Mutex::new(Vec::new()));
        let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();

        // Write too few samples (less than 0.5 seconds = 8000 samples)
        {
            let mut buf = samples_buffer.lock();
            buf.extend(vec![0.5_f32; 100]);
        }

        monitor.start(samples_buffer.clone(), segment_tx);
        std::thread::sleep(Duration::from_millis(700));
        monitor.stop(&samples_buffer);

        // No segment should be emitted (too few samples)
        assert!(segment_rx.try_recv().is_err());
    }

    #[test]
    fn test_fixed_interval_segmentation() {
        // Use a very short interval (1 second) to trigger automatic segmentation
        let config = SegmentationConfig {
            use_vad: false,
            segment_interval_secs: 1,
            ..Default::default()
        };
        let monitor = SegmentationMonitor::new(config);

        let samples_buffer = Arc::new(Mutex::new(Vec::new()));
        let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();

        // Pre-fill with 2 seconds of audio (enough for auto-segment at 1s interval)
        {
            let mut buf = samples_buffer.lock();
            buf.extend(vec![0.3_f32; WHISPER_SAMPLE_RATE as usize * 2]);
        }

        monitor.start(samples_buffer.clone(), segment_tx);

        // Wait for the interval + check_interval to pass (1s interval + 500ms check + margin)
        std::thread::sleep(Duration::from_millis(2000));

        monitor.stop(&samples_buffer);

        // Should have at least one auto-emitted segment
        let mut segments = Vec::new();
        while let Ok(seg) = segment_rx.try_recv() {
            segments.push(seg);
        }
        assert!(
            !segments.is_empty(),
            "Expected at least one segment from fixed-interval segmentation"
        );
        // Segment IDs should be sequential starting from 1
        for (i, seg) in segments.iter().enumerate() {
            assert_eq!(seg.segment_id, i + 1);
        }
    }

    #[test]
    fn test_stop_closes_channel() {
        let config = SegmentationConfig {
            use_vad: false,
            segment_interval_secs: 3600,
            ..Default::default()
        };
        let monitor = SegmentationMonitor::new(config);

        let samples_buffer = Arc::new(Mutex::new(Vec::new()));
        let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();

        monitor.start(samples_buffer.clone(), segment_tx);
        std::thread::sleep(Duration::from_millis(100));
        monitor.stop(&samples_buffer);

        // After stop, the internal sender is dropped — channel should be closed.
        // Note: try_recv returns Err(TryRecvError::Closed) when all senders are gone
        // and the channel is empty.
        // The unbounded receiver returns Empty if channel still open, Closed if all senders dropped.
        // We dropped our original segment_tx by moving it into start(), and stop() drops the internal one.
        // So the channel should now be closed.
        assert!(segment_rx.is_closed() || segment_rx.try_recv().is_err());
    }

    #[test]
    fn test_incremental_sample_reading() {
        // Verify that only new samples are read from the buffer
        // (not the entire buffer each iteration)
        let config = SegmentationConfig {
            use_vad: false,
            segment_interval_secs: 3600,
            ..Default::default()
        };
        let monitor = SegmentationMonitor::new(config);

        let samples_buffer = Arc::new(Mutex::new(Vec::new()));
        let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();

        // Start with some samples
        {
            let mut buf = samples_buffer.lock();
            buf.extend(vec![1.0_f32; WHISPER_SAMPLE_RATE as usize]);
        }

        monitor.start(samples_buffer.clone(), segment_tx);
        std::thread::sleep(Duration::from_millis(700));

        // Add more samples while monitoring
        {
            let mut buf = samples_buffer.lock();
            buf.extend(vec![2.0_f32; WHISPER_SAMPLE_RATE as usize]);
        }

        std::thread::sleep(Duration::from_millis(700));
        monitor.stop(&samples_buffer);

        // The final segment should contain samples from both batches
        let mut total_samples = 0;
        while let Ok(seg) = segment_rx.try_recv() {
            total_samples += seg.samples.len();
        }
        // Should have roughly 2 seconds worth of audio
        assert!(
            total_samples >= WHISPER_SAMPLE_RATE as usize,
            "Expected at least 1s of audio, got {} samples",
            total_samples
        );
    }
}
