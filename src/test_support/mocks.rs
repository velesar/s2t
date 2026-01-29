//! Mock implementations for unit testing.
//!
//! These mocks implement the core traits from `crate::traits` to enable
//! testing without real audio devices or Whisper models.

use crate::history::HistoryEntry;
use crate::domain::traits::{
    AudioDenoising, AudioRecording, ConfigProvider, HistoryRepository, Transcription,
    VoiceDetection,
};
use anyhow::Result;
use async_channel::Receiver;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Mock audio recorder for testing.
///
/// Returns predefined samples and tracks recording state.
/// Provides a shared samples buffer for service-level testing.
pub struct MockAudioRecorder {
    is_recording: AtomicBool,
    samples_to_return: Mutex<Vec<f32>>,
    amplitude: f32,
    samples_buffer: Arc<Mutex<Vec<f32>>>,
}

impl MockAudioRecorder {
    /// Create a mock recorder with default samples (1 second of silence).
    pub fn new() -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            samples_to_return: Mutex::new(vec![0.0; 16000]),
            amplitude: 0.5,
            samples_buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a mock recorder returning specific samples.
    pub fn with_samples(samples: Vec<f32>) -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            samples_to_return: Mutex::new(samples),
            amplitude: 0.5,
            samples_buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a mock recorder with custom amplitude.
    pub fn with_amplitude(amplitude: f32) -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            samples_to_return: Mutex::new(vec![0.0; 16000]),
            amplitude,
            samples_buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get the shared samples buffer (for AudioService test constructor).
    pub fn samples_buffer(&self) -> Arc<Mutex<Vec<f32>>> {
        self.samples_buffer.clone()
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

    fn load_model(&mut self, path: &std::path::Path) -> Result<()> {
        self.is_loaded = true;
        self.model_name = Some(path.to_string_lossy().to_string());
        Ok(())
    }
}

/// Mock voice activity detector for testing.
///
/// Returns configurable speech detection results.
pub struct MockVoiceDetector {
    speech_result: bool,
    speech_end_result: bool,
    reset_count: std::sync::atomic::AtomicUsize,
}

impl MockVoiceDetector {
    /// Create a mock that always detects speech.
    pub fn detecting_speech() -> Self {
        Self {
            speech_result: true,
            speech_end_result: false,
            reset_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Create a mock that never detects speech.
    pub fn silent() -> Self {
        Self {
            speech_result: false,
            speech_end_result: false,
            reset_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Create a mock that reports speech end.
    pub fn speech_ended() -> Self {
        Self {
            speech_result: false,
            speech_end_result: true,
            reset_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get how many times reset() was called.
    pub fn reset_count(&self) -> usize {
        self.reset_count
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl VoiceDetection for MockVoiceDetector {
    fn is_speech(&self, _samples: &[f32]) -> anyhow::Result<bool> {
        Ok(self.speech_result)
    }

    fn detect_speech_end(&self, _samples: &[f32]) -> anyhow::Result<bool> {
        Ok(self.speech_end_result)
    }

    fn reset(&self) {
        self.reset_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Mock configuration provider for testing.
///
/// Returns configurable values for all config fields.
pub struct MockConfigProvider {
    pub language: String,
    pub default_model: String,
    pub auto_copy: bool,
    pub auto_paste: bool,
    pub continuous_mode: bool,
    pub recording_mode: String,
}

impl MockConfigProvider {
    /// Create a mock with default Ukrainian config.
    pub fn default_uk() -> Self {
        Self {
            language: "uk".to_string(),
            default_model: "ggml-base.bin".to_string(),
            auto_copy: true,
            auto_paste: false,
            continuous_mode: false,
            recording_mode: "dictation".to_string(),
        }
    }
}

impl ConfigProvider for MockConfigProvider {
    fn language(&self) -> String {
        self.language.clone()
    }

    fn default_model(&self) -> String {
        self.default_model.clone()
    }

    fn auto_copy(&self) -> bool {
        self.auto_copy
    }

    fn auto_paste(&self) -> bool {
        self.auto_paste
    }

    fn continuous_mode(&self) -> bool {
        self.continuous_mode
    }

    fn recording_mode(&self) -> String {
        self.recording_mode.clone()
    }
}

/// Mock audio denoiser for testing.
///
/// Can be configured to pass through audio unchanged or return
/// specific output for testing denoising pipeline integration.
pub struct MockDenoiser {
    required_rate: u32,
    passthrough: bool,
}

impl MockDenoiser {
    /// Create a mock denoiser that passes audio through unchanged.
    pub fn passthrough() -> Self {
        Self {
            required_rate: 48000,
            passthrough: true,
        }
    }

    /// Create a mock denoiser with specific sample rate.
    pub fn with_sample_rate(rate: u32) -> Self {
        Self {
            required_rate: rate,
            passthrough: true,
        }
    }
}

impl Default for MockDenoiser {
    fn default() -> Self {
        Self::passthrough()
    }
}

impl AudioDenoising for MockDenoiser {
    fn denoise(&self, samples: &[f32]) -> anyhow::Result<Vec<f32>> {
        if self.passthrough {
            Ok(samples.to_vec())
        } else {
            // Return attenuated samples to simulate denoising
            Ok(samples.iter().map(|s| s * 0.8).collect())
        }
    }

    fn required_sample_rate(&self) -> u32 {
        self.required_rate
    }

    fn reset(&self) {
        // Nothing to reset
    }
}

/// Mock history repository for testing.
///
/// Stores entries in-memory without persistence.
pub struct MockHistoryRepository {
    entries: Vec<HistoryEntry>,
}

impl MockHistoryRepository {
    /// Create an empty mock history.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl Default for MockHistoryRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoryRepository for MockHistoryRepository {
    type Entry = HistoryEntry;

    fn add(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
    }

    fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.text.to_lowercase().contains(&query_lower))
            .collect()
    }

    fn cleanup_old(&mut self, _max_age_days: u32) -> usize {
        0
    }

    fn trim_to_limit(&mut self, max_entries: usize) -> usize {
        if self.entries.len() <= max_entries {
            return 0;
        }
        let removed = self.entries.len() - max_entries;
        self.entries.truncate(max_entries);
        removed
    }

    fn save(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn remove(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
    }

    fn filter_by_date_range(
        &self,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(start) = from {
                    if e.timestamp < start {
                        return false;
                    }
                }
                if let Some(end) = to {
                    if e.timestamp > end {
                        return false;
                    }
                }
                true
            })
            .collect()
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

    // === MockVoiceDetector Tests ===

    #[test]
    fn test_mock_voice_detector_detecting_speech() {
        let vad = MockVoiceDetector::detecting_speech();
        assert!(vad.is_speech(&[0.0; 480]).unwrap());
        assert!(!vad.detect_speech_end(&[0.0; 480]).unwrap());
    }

    #[test]
    fn test_mock_voice_detector_silent() {
        let vad = MockVoiceDetector::silent();
        assert!(!vad.is_speech(&[0.0; 480]).unwrap());
    }

    #[test]
    fn test_mock_voice_detector_speech_ended() {
        let vad = MockVoiceDetector::speech_ended();
        assert!(vad.detect_speech_end(&[0.0; 480]).unwrap());
    }

    #[test]
    fn test_mock_voice_detector_reset_count() {
        let vad = MockVoiceDetector::silent();
        assert_eq!(vad.reset_count(), 0);
        vad.reset();
        vad.reset();
        assert_eq!(vad.reset_count(), 2);
    }

    // === MockConfigProvider Tests ===

    #[test]
    fn test_mock_config_provider_defaults() {
        let config = MockConfigProvider::default_uk();
        assert_eq!(ConfigProvider::language(&config), "uk");
        assert_eq!(ConfigProvider::default_model(&config), "ggml-base.bin");
        assert!(ConfigProvider::auto_copy(&config));
        assert!(!ConfigProvider::auto_paste(&config));
        assert!(!ConfigProvider::continuous_mode(&config));
        assert_eq!(ConfigProvider::recording_mode(&config), "dictation");
    }

    #[test]
    fn test_mock_config_provider_custom() {
        let config = MockConfigProvider {
            language: "en".to_string(),
            default_model: "ggml-large.bin".to_string(),
            auto_copy: false,
            auto_paste: true,
            continuous_mode: true,
            recording_mode: "continuous".to_string(),
        };
        assert_eq!(config.language(), "en");
        assert!(config.auto_paste());
        assert!(config.continuous_mode());
    }

    // === Trait Object (Box<dyn>) Tests ===

    #[test]
    fn test_audio_recording_as_trait_object() {
        let recorder: Box<dyn AudioRecording> = Box::new(MockAudioRecorder::with_samples(vec![0.5, 0.6]));
        assert!(!recorder.is_recording());
        recorder.start().unwrap();
        assert!(recorder.is_recording());
        let (samples, _) = recorder.stop();
        assert_eq!(samples, vec![0.5, 0.6]);
    }

    #[test]
    fn test_transcription_as_trait_object() {
        let transcriber: Box<dyn Transcription> = Box::new(MockTranscription::returning("test output"));
        assert!(transcriber.is_loaded());
        assert_eq!(transcriber.model_name(), Some("mock-model".to_string()));
        let text = transcriber.transcribe(&[0.0; 16000], "uk").unwrap();
        assert_eq!(text, "test output");
    }

    #[test]
    fn test_transcription_unloaded_as_trait_object() {
        let transcriber: Box<dyn Transcription> = Box::new(MockTranscription::unloaded());
        assert!(!transcriber.is_loaded());
        assert!(transcriber.transcribe(&[], "en").is_err());
    }

    #[test]
    fn test_config_provider_as_trait_object() {
        let config: Box<dyn ConfigProvider> = Box::new(MockConfigProvider::default_uk());
        assert_eq!(config.language(), "uk");
        assert_eq!(config.default_model(), "ggml-base.bin");
        assert!(config.auto_copy());
    }

    // === MockDenoiser Tests ===

    #[test]
    fn test_mock_denoiser_passthrough() {
        let denoiser = MockDenoiser::passthrough();
        let input = vec![0.1, 0.2, 0.3];
        let output = denoiser.denoise(&input).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn test_mock_denoiser_sample_rate() {
        let denoiser = MockDenoiser::with_sample_rate(16000);
        assert_eq!(denoiser.required_sample_rate(), 16000);
    }

    #[test]
    fn test_mock_denoiser_default_rate() {
        let denoiser = MockDenoiser::passthrough();
        assert_eq!(denoiser.required_sample_rate(), 48000);
    }

    #[test]
    fn test_mock_denoiser_as_trait_object() {
        let denoiser: Box<dyn AudioDenoising> = Box::new(MockDenoiser::passthrough());
        let input = vec![0.5, 0.6, 0.7];
        let output = denoiser.denoise(&input).unwrap();
        assert_eq!(output, input);
    }
}
