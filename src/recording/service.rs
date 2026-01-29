//! Audio recording service layer.
//!
//! This module provides a unified interface for all recording modes:
//! - Dictation: Single recording session with transcription
//! - Continuous: Automatic segmentation with parallel transcription
//! - Conference: Dual-channel (mic + loopback) with diarization

use crate::recording::microphone::AudioRecorder;
use crate::recording::conference::ConferenceRecorder;
use crate::recording::continuous::ContinuousRecorder;
use crate::domain::traits::AudioRecording;
use crate::domain::types::AudioSegment;
use crate::domain::types::ConferenceRecording;
use crate::vad::VadEngine;
use anyhow::Result;
use async_channel::{Receiver, Sender};
use std::sync::Arc;

/// Configuration for continuous recording mode
pub struct ContinuousConfig {
    pub use_vad: bool,
    pub segment_interval_secs: u32,
    pub vad_silence_threshold_ms: u32,
    pub vad_min_speech_ms: u32,
    pub vad_engine: VadEngine,
    pub silero_threshold: f32,
}

impl Default for ContinuousConfig {
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

/// Unified audio service wrapping all recording modes.
///
/// `AudioService` provides a clean API for recording operations,
/// abstracting away the underlying recorder implementations.
///
/// The `mic` field uses the `AudioRecording` trait to enable
/// dependency injection for testing.
pub struct AudioService {
    /// Microphone recorder (trait object for testability)
    mic: Arc<dyn AudioRecording>,
    /// Conference recorder (mic + loopback)
    conference: Arc<ConferenceRecorder>,
    /// Continuous recorder with VAD
    continuous: Arc<ContinuousRecorder>,
}

impl AudioService {
    /// Create a new AudioService with the given continuous recording configuration.
    ///
    /// Uses the default `AudioRecorder` for microphone capture.
    pub fn new(continuous_config: ContinuousConfig) -> Result<Self> {
        let continuous = ContinuousRecorder::with_vad_engine(
            continuous_config.use_vad,
            continuous_config.segment_interval_secs,
            continuous_config.vad_silence_threshold_ms,
            continuous_config.vad_min_speech_ms,
            continuous_config.vad_engine,
            continuous_config.silero_threshold,
        )?;

        Ok(Self {
            mic: Arc::new(AudioRecorder::new()),
            conference: Arc::new(ConferenceRecorder::new()),
            continuous: Arc::new(continuous),
        })
    }

    /// Create with a custom microphone recorder (for testing).
    ///
    /// This constructor enables dependency injection of mock recorders.
    #[cfg(test)]
    pub fn with_recorder(
        mic: Arc<dyn AudioRecording>,
        continuous_config: ContinuousConfig,
    ) -> Result<Self> {
        let continuous = ContinuousRecorder::with_vad_engine(
            continuous_config.use_vad,
            continuous_config.segment_interval_secs,
            continuous_config.vad_silence_threshold_ms,
            continuous_config.vad_min_speech_ms,
            continuous_config.vad_engine,
            continuous_config.silero_threshold,
        )?;

        Ok(Self {
            mic,
            conference: Arc::new(ConferenceRecorder::new()),
            continuous: Arc::new(continuous),
        })
    }

    /// Create with default configuration (fallback if custom config fails)
    pub fn new_default() -> Self {
        let continuous = ContinuousRecorder::new(false, 10, 1000, 500)
            .expect("Failed to create continuous recorder with safe defaults");

        Self {
            mic: Arc::new(AudioRecorder::new()),
            conference: Arc::new(ConferenceRecorder::new()),
            continuous: Arc::new(continuous),
        }
    }

    // === Dictation Mode ===

    /// Start dictation (single recording session)
    pub fn start_dictation(&self) -> Result<()> {
        self.mic.start()
    }

    /// Stop dictation and return recorded samples
    pub fn stop_dictation(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.mic.stop()
    }

    /// Get current microphone amplitude for dictation mode
    pub fn get_dictation_amplitude(&self) -> f32 {
        self.mic.amplitude()
    }

    /// Check if dictation is currently recording
    #[allow(dead_code)] // Useful for future UI state queries
    pub fn is_dictation_recording(&self) -> bool {
        self.mic.is_recording()
    }

    // === Continuous Mode ===

    /// Start continuous recording with automatic segmentation
    pub fn start_continuous(&self, segment_tx: Sender<AudioSegment>) -> Result<()> {
        self.continuous.start_continuous(segment_tx)
    }

    /// Stop continuous recording
    pub fn stop_continuous(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.continuous.stop_continuous()
    }

    /// Get current amplitude for continuous mode
    pub fn get_continuous_amplitude(&self) -> f32 {
        self.continuous.get_amplitude()
    }

    /// Check if speech is currently detected (for continuous mode UI)
    pub fn is_speech_detected(&self) -> bool {
        self.continuous.is_speech_detected()
    }

    // === Conference Mode ===

    /// Start conference recording (mic + loopback)
    pub fn start_conference(&self) -> Result<()> {
        self.conference.start_conference()
    }

    /// Stop conference recording and return both channels
    pub fn stop_conference(&self) -> ConferenceRecording {
        self.conference.stop_conference()
    }

    /// Get microphone amplitude for conference mode
    pub fn get_mic_amplitude(&self) -> f32 {
        self.conference.get_mic_amplitude()
    }

    /// Get loopback amplitude for conference mode
    pub fn get_loopback_amplitude(&self) -> f32 {
        self.conference.get_loopback_amplitude()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::mocks::{MockAudioRecorder, MockTranscription};
    use crate::domain::traits::Transcription;

    #[test]
    fn test_audio_service_with_mock_recorder() {
        let mock = Arc::new(MockAudioRecorder::with_samples(vec![0.1, 0.2, 0.3]));
        let service = AudioService::with_recorder(mock, ContinuousConfig::default()).unwrap();

        assert!(!service.is_dictation_recording());
        service.start_dictation().unwrap();

        let (samples, _) = service.stop_dictation();
        assert_eq!(samples, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_audio_service_amplitude() {
        let mock = Arc::new(MockAudioRecorder::with_amplitude(0.75));
        let service = AudioService::with_recorder(mock, ContinuousConfig::default()).unwrap();

        assert_eq!(service.get_dictation_amplitude(), 0.75);
    }

    /// Integration test: full dictation start → stop → transcribe workflow.
    #[test]
    fn test_dictation_workflow_end_to_end() {
        // Simulate 1 second of 440Hz sine wave at 16kHz
        let num_samples = 16000;
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();

        let mock_recorder = Arc::new(MockAudioRecorder::with_samples(samples.clone()));
        let mock_transcriber = MockTranscription::returning("Привіт, світе");

        let service =
            AudioService::with_recorder(mock_recorder, ContinuousConfig::default()).unwrap();

        // Start recording
        service.start_dictation().unwrap();

        // Stop and get samples
        let (recorded_samples, _) = service.stop_dictation();
        assert_eq!(recorded_samples.len(), num_samples);

        // Transcribe the recorded samples
        let text = mock_transcriber
            .transcribe(&recorded_samples, "uk")
            .unwrap();
        assert_eq!(text, "Привіт, світе");
    }

    /// Integration test: verifies amplitude reflects recording state.
    #[test]
    fn test_dictation_amplitude_during_recording() {
        let mock = Arc::new(MockAudioRecorder::with_amplitude(0.6));
        let service = AudioService::with_recorder(mock, ContinuousConfig::default()).unwrap();

        // Amplitude available before recording starts (mock returns constant)
        assert_eq!(service.get_dictation_amplitude(), 0.6);

        // Start recording - amplitude still available
        service.start_dictation().unwrap();
        assert_eq!(service.get_dictation_amplitude(), 0.6);
    }

    #[test]
    fn test_continuous_config_default() {
        let config = ContinuousConfig::default();
        assert!(config.use_vad);
        assert_eq!(config.segment_interval_secs, 10);
        assert_eq!(config.vad_silence_threshold_ms, 1000);
        assert_eq!(config.vad_min_speech_ms, 500);
    }
}
