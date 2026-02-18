//! Core domain traits for dependency inversion.
//!
//! These traits define contracts between layers without depending on
//! concrete implementations. They enable:
//! - Testability via mock implementations
//! - Flexibility to swap implementations
//! - Clear API boundaries

use anyhow::Result;
use async_channel::Receiver;
use chrono::{DateTime, Utc};
use std::path::Path;

/// Audio recording abstraction.
///
/// Implementors capture audio from various sources (microphone, loopback)
/// and provide samples at 16kHz mono format for Whisper transcription.
pub trait AudioRecording: Send + Sync {
    /// Start recording audio.
    ///
    /// Returns `Err` if the audio device is unavailable or already recording.
    fn start(&self) -> Result<()>;

    /// Stop recording and return captured samples.
    ///
    /// Returns:
    /// - Audio samples at 16kHz mono
    /// - Optional completion receiver for async notification when recording thread finishes
    fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>);

    /// Get current audio amplitude (0.0 - 1.0 range).
    ///
    /// Used for real-time UI visualization of audio levels.
    fn amplitude(&self) -> f32;

    /// Check if currently recording.
    fn is_recording(&self) -> bool;
}

/// Speech-to-text transcription abstraction.
///
/// Implementors convert audio samples to text using various STT backends.
#[allow(dead_code)] // Methods used through concrete types and trait objects
pub trait Transcription: Send + Sync {
    /// Transcribe audio samples to text.
    ///
    /// # Arguments
    /// * `samples` - Audio samples at 16kHz mono
    /// * `language` - Language code (e.g., "uk", "en", "auto")
    fn transcribe(&self, samples: &[f32], language: &str) -> Result<String>;

    /// Check if a model is loaded and ready for transcription.
    fn is_loaded(&self) -> bool;

    /// Get the name/path of the loaded model.
    fn model_name(&self) -> Option<String>;

    /// Load or replace the model from the given path.
    fn load_model(&mut self, path: &Path) -> Result<()>;
}

/// Voice activity detection abstraction.
///
/// Implementors detect speech presence in audio frames for:
/// - Automatic recording start/stop
/// - Continuous mode segmentation
///
/// Note: Uses `&self` with interior mutability pattern to allow
/// implementations to use `RefCell` or similar for thread-local state.
#[allow(dead_code)] // Methods used through concrete types
pub trait VoiceDetection {
    /// Check if audio frame contains speech.
    ///
    /// # Arguments
    /// * `samples` - Audio frame at 16kHz mono
    fn is_speech(&self, samples: &[f32]) -> Result<bool>;

    /// Detect end of speech (silence after speech).
    ///
    /// Returns true when a configurable silence duration is detected
    /// after speech activity.
    fn detect_speech_end(&self, samples: &[f32]) -> Result<bool>;

    /// Reset internal state for new recording session.
    fn reset(&self);
}

/// History storage abstraction.
///
/// Implementors persist transcription history entries.
#[allow(dead_code)] // Methods used through concrete types
pub trait HistoryRepository: Send + Sync {
    /// History entry type
    type Entry;

    /// Add a new entry to history.
    fn add(&mut self, entry: Self::Entry);

    /// Get all entries (most recent first).
    fn entries(&self) -> &[Self::Entry];

    /// Search entries by text content.
    fn search(&self, query: &str) -> Vec<&Self::Entry>;

    /// Remove entries older than max_age_days.
    fn cleanup_old(&mut self, max_age_days: u32) -> usize;

    /// Trim to maximum number of entries.
    fn trim_to_limit(&mut self, max_entries: usize) -> usize;

    /// Persist to storage.
    fn save(&self) -> Result<()>;

    /// Remove an entry by its ID.
    fn remove(&mut self, id: &str);

    /// Filter entries by date range (inclusive).
    fn filter_by_date_range(&self, from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Vec<&Self::Entry>;
}

/// Audio denoising abstraction.
///
/// Implementors apply noise suppression to audio samples.
/// Typical use case: RNNoise-based denoising for cleaner STT input.
///
/// Only used in tests — production code calls `NnnoiselessDenoiser` directly.
#[cfg(test)]
pub trait AudioDenoising: Send + Sync {
    /// Apply noise suppression to audio samples.
    ///
    /// # Arguments
    /// * `samples` - Audio samples at the required sample rate
    ///
    /// # Returns
    /// Denoised audio samples
    fn denoise(&self, samples: &[f32]) -> anyhow::Result<Vec<f32>>;

    /// Get the sample rate required by this denoiser.
    ///
    /// Most denoisers (like RNNoise) work at a specific rate (e.g., 48kHz).
    fn required_sample_rate(&self) -> u32;

    /// Reset internal state for new recording session.
    fn reset(&self);
}

/// Configuration provider abstraction.
///
/// Implementors provide application configuration values.
pub trait ConfigProvider: Send + Sync {
    fn language(&self) -> String;
    fn auto_copy(&self) -> bool;
    fn auto_paste(&self) -> bool;
    fn continuous_mode(&self) -> bool;
}

/// UI state update abstraction.
///
/// Stable interface for recording handlers to update UI state
/// without directly accessing widget fields. This decouples handlers
/// from concrete GTK widgets and enables testing with mock implementations.
pub trait UIStateUpdater {
    /// Set the status label text.
    fn set_status(&self, text: &str);

    /// Transition UI to recording state.
    fn set_recording(&self, status_text: &str);

    /// Transition UI to processing state.
    fn set_processing(&self, status_text: &str);

    /// Transition UI to idle state.
    fn set_idle(&self);

    /// Update the timer display with elapsed seconds.
    fn update_timer(&self, secs: u64);

    /// Get the current result text.
    fn get_result_text(&self) -> String;

    /// Set the result text content.
    fn set_result_text(&self, text: &str);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Test implementation of AudioRecording
    struct TestRecorder {
        recording: AtomicBool,
        samples: Vec<f32>,
    }

    impl TestRecorder {
        fn new(samples: Vec<f32>) -> Self {
            Self {
                recording: AtomicBool::new(false),
                samples,
            }
        }
    }

    impl AudioRecording for TestRecorder {
        fn start(&self) -> Result<()> {
            self.recording.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>) {
            self.recording.store(false, Ordering::SeqCst);
            (self.samples.clone(), None)
        }

        fn amplitude(&self) -> f32 {
            0.5
        }

        fn is_recording(&self) -> bool {
            self.recording.load(Ordering::SeqCst)
        }
    }

    // Safety: TestRecorder uses AtomicBool for thread safety
    unsafe impl Send for TestRecorder {}
    unsafe impl Sync for TestRecorder {}

    #[test]
    fn test_audio_recording_trait() {
        let recorder = TestRecorder::new(vec![0.1, 0.2, 0.3]);

        assert!(!recorder.is_recording());
        recorder.start().unwrap();
        assert!(recorder.is_recording());

        let (samples, _) = recorder.stop();
        assert!(!recorder.is_recording());
        assert_eq!(samples, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_amplitude_range() {
        let recorder = TestRecorder::new(vec![]);
        let amp = recorder.amplitude();
        assert!((0.0..=1.0).contains(&amp));
    }
}
