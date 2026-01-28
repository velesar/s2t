//! Mock implementations for unit testing.
//!
//! These mocks implement the core traits from `crate::traits` to enable
//! testing without real audio devices or Whisper models.

use crate::traits::{AudioRecording, Transcription};
use anyhow::Result;
use async_channel::Receiver;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Mock audio recorder for testing.
///
/// Returns predefined samples and tracks recording state.
pub struct MockAudioRecorder {
    is_recording: AtomicBool,
    samples_to_return: Mutex<Vec<f32>>,
    amplitude: f32,
}

impl MockAudioRecorder {
    /// Create a mock recorder with default samples (1 second of silence).
    pub fn new() -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            samples_to_return: Mutex::new(vec![0.0; 16000]),
            amplitude: 0.5,
        }
    }

    /// Create a mock recorder returning specific samples.
    pub fn with_samples(samples: Vec<f32>) -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            samples_to_return: Mutex::new(samples),
            amplitude: 0.5,
        }
    }

    /// Create a mock recorder with custom amplitude.
    pub fn with_amplitude(amplitude: f32) -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            samples_to_return: Mutex::new(vec![0.0; 16000]),
            amplitude,
        }
    }

    /// Set samples to be returned on next stop().
    #[allow(dead_code)] // Utility for test scenarios
    pub fn set_samples(&self, samples: Vec<f32>) {
        *self.samples_to_return.lock().unwrap() = samples;
    }
}

impl Default for MockAudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRecording for MockAudioRecorder {
    fn start(&self) -> Result<()> {
        self.is_recording.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.is_recording.store(false, Ordering::SeqCst);
        let samples = self.samples_to_return.lock().unwrap().clone();
        (samples, None)
    }

    fn amplitude(&self) -> f32 {
        self.amplitude
    }

    fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}

/// Mock transcription service for testing.
///
/// Returns predefined text instead of actually transcribing.
pub struct MockTranscription {
    result: Mutex<String>,
    is_loaded: bool,
    model_name: Option<String>,
}

impl MockTranscription {
    /// Create a mock that returns the given text.
    pub fn returning(text: &str) -> Self {
        Self {
            result: Mutex::new(text.to_string()),
            is_loaded: true,
            model_name: Some("mock-model".to_string()),
        }
    }

    /// Create an unloaded mock (simulates no model loaded).
    pub fn unloaded() -> Self {
        Self {
            result: Mutex::new(String::new()),
            is_loaded: false,
            model_name: None,
        }
    }

    /// Set the text to return on next transcribe().
    pub fn set_result(&self, text: &str) {
        *self.result.lock().unwrap() = text.to_string();
    }
}

impl Transcription for MockTranscription {
    fn transcribe(&self, _samples: &[f32], _language: &str) -> Result<String> {
        if !self.is_loaded {
            anyhow::bail!("Model not loaded");
        }
        Ok(self.result.lock().unwrap().clone())
    }

    fn is_loaded(&self) -> bool {
        self.is_loaded
    }

    fn model_name(&self) -> Option<String> {
        self.model_name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_recorder_basic() {
        let recorder = MockAudioRecorder::new();

        assert!(!recorder.is_recording());
        recorder.start().unwrap();
        assert!(recorder.is_recording());

        let (samples, completion) = recorder.stop();
        assert!(!recorder.is_recording());
        assert_eq!(samples.len(), 16000); // 1 second at 16kHz
        assert!(completion.is_none());
    }

    #[test]
    fn test_mock_recorder_custom_samples() {
        let recorder = MockAudioRecorder::with_samples(vec![0.1, 0.2, 0.3]);
        recorder.start().unwrap();
        let (samples, _) = recorder.stop();
        assert_eq!(samples, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_mock_recorder_amplitude() {
        let recorder = MockAudioRecorder::with_amplitude(0.75);
        assert_eq!(recorder.amplitude(), 0.75);
    }

    #[test]
    fn test_mock_transcription_returns_text() {
        let transcriber = MockTranscription::returning("hello world");
        let result = transcriber.transcribe(&[], "en").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_mock_transcription_is_loaded() {
        let loaded = MockTranscription::returning("test");
        assert!(loaded.is_loaded());
        assert!(loaded.model_name().is_some());

        let unloaded = MockTranscription::unloaded();
        assert!(!unloaded.is_loaded());
        assert!(unloaded.model_name().is_none());
    }

    #[test]
    fn test_mock_transcription_unloaded_fails() {
        let transcriber = MockTranscription::unloaded();
        let result = transcriber.transcribe(&[], "en");
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_transcription_set_result() {
        let transcriber = MockTranscription::returning("initial");
        assert_eq!(transcriber.transcribe(&[], "en").unwrap(), "initial");

        transcriber.set_result("updated");
        assert_eq!(transcriber.transcribe(&[], "en").unwrap(), "updated");
    }
}
