//! Audio recording service layer.
//!
//! This module provides a unified interface for all recording modes:
//! - Mic: Single microphone capture (shared between dictation and segmented)
//! - Segmentation: Optional add-on layer for automatic segmentation
//! - Conference: Dual-channel (mic + loopback) with diarization

use crate::domain::traits::AudioRecording;
use crate::domain::types::AudioSegment;
use crate::domain::types::ConferenceRecording;
use crate::recording::conference::ConferenceRecorder;
use crate::recording::microphone::AudioRecorder;
use crate::recording::segmentation::{SegmentationConfig, SegmentationMonitor};
use anyhow::Result;
use async_channel::{Receiver, Sender};
use parking_lot::Mutex;
use std::sync::Arc;

/// Unified audio service wrapping all recording modes.
///
/// The mic recorder is shared between dictation and segmented modes.
/// Segmentation is an optional add-on layer that monitors the mic's
/// samples buffer and produces audio segments.
pub struct AudioService {
    /// Microphone recorder (trait object for testability)
    mic: Arc<dyn AudioRecording>,
    /// Shared samples buffer from the mic recorder
    mic_samples: Arc<Mutex<Vec<f32>>>,
    /// Optional segmentation monitor
    segmentation: Option<Arc<SegmentationMonitor>>,
    /// Conference recorder (mic + loopback)
    conference: Arc<ConferenceRecorder>,
}

impl AudioService {
    /// Create a new AudioService with the given segmentation configuration.
    ///
    /// Uses the default `AudioRecorder` for microphone capture.
    pub fn new(seg_config: SegmentationConfig) -> Result<Self> {
        let mic = Arc::new(AudioRecorder::new());
        let mic_samples = mic.samples().clone();

        Ok(Self {
            mic,
            mic_samples,
            segmentation: Some(Arc::new(SegmentationMonitor::new(seg_config))),
            conference: Arc::new(ConferenceRecorder::new()),
        })
    }

    /// Create with a custom microphone recorder (for testing).
    ///
    /// This constructor enables dependency injection of mock recorders.
    #[cfg(test)]
    pub fn with_recorder(
        mic: Arc<dyn AudioRecording>,
        mic_samples: Arc<Mutex<Vec<f32>>>,
        seg_config: Option<SegmentationConfig>,
    ) -> Result<Self> {
        Ok(Self {
            mic,
            mic_samples,
            segmentation: seg_config.map(|c| Arc::new(SegmentationMonitor::new(c))),
            conference: Arc::new(ConferenceRecorder::new()),
        })
    }

    /// Create with default configuration (fallback if custom config fails).
    pub fn new_default() -> Self {
        let mic = Arc::new(AudioRecorder::new());
        let mic_samples = mic.samples().clone();

        Self {
            mic,
            mic_samples,
            segmentation: Some(Arc::new(SegmentationMonitor::new(
                SegmentationConfig::default(),
            ))),
            conference: Arc::new(ConferenceRecorder::new()),
        }
    }

    // === Mic (shared between dictation and segmented modes) ===

    /// Start microphone recording.
    pub fn start_mic(&self) -> Result<()> {
        self.mic.start()
    }

    /// Stop microphone recording and return captured samples.
    pub fn stop_mic(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.mic.stop()
    }

    /// Get current microphone amplitude.
    pub fn mic_amplitude(&self) -> f32 {
        self.mic.amplitude()
    }

    /// Check if microphone is currently recording.
    #[allow(dead_code)]
    pub fn is_mic_recording(&self) -> bool {
        self.mic.is_recording()
    }

    // === Segmentation (add-on layer) ===

    /// Start segmentation monitoring on top of the mic recording.
    ///
    /// The mic must already be recording before calling this.
    pub fn start_segmentation(&self, segment_tx: Sender<AudioSegment>) -> Result<()> {
        if let Some(ref seg) = self.segmentation {
            seg.start(self.mic_samples.clone(), segment_tx);
        }
        Ok(())
    }

    /// Stop segmentation monitoring.
    ///
    /// Must be called **before** `stop_mic()` so the monitor can drain
    /// remaining audio from the ring buffer.
    pub fn stop_segmentation(&self) {
        if let Some(ref seg) = self.segmentation {
            seg.stop(&self.mic_samples);
        }
    }

    /// Check if speech is currently detected (for segmented mode UI).
    pub fn is_speech_detected(&self) -> bool {
        self.segmentation
            .as_ref()
            .is_some_and(|seg| seg.is_speech_detected())
    }

    // === Conference Mode (unchanged) ===

    /// Start conference recording (mic + loopback).
    pub fn start_conference(&self) -> Result<()> {
        self.conference.start_conference()
    }

    /// Stop conference recording and return both channels.
    pub fn stop_conference(&self) -> ConferenceRecording {
        self.conference.stop_conference()
    }

    /// Get microphone amplitude for conference mode.
    pub fn get_mic_amplitude(&self) -> f32 {
        self.conference.get_mic_amplitude()
    }

    /// Get loopback amplitude for conference mode.
    pub fn get_loopback_amplitude(&self) -> f32 {
        self.conference.get_loopback_amplitude()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::traits::Transcription;
    use crate::test_support::mocks::{MockAudioRecorder, MockTranscription};

    #[test]
    fn test_audio_service_with_mock_recorder() {
        let mock = Arc::new(MockAudioRecorder::with_samples(vec![0.1, 0.2, 0.3]));
        let samples_buf = mock.samples_buffer();
        let service = AudioService::with_recorder(mock, samples_buf, None).unwrap();

        assert!(!service.is_mic_recording());
        service.start_mic().unwrap();

        let (samples, _) = service.stop_mic();
        assert_eq!(samples, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_audio_service_amplitude() {
        let mock = Arc::new(MockAudioRecorder::with_amplitude(0.75));
        let samples_buf = mock.samples_buffer();
        let service = AudioService::with_recorder(mock, samples_buf, None).unwrap();

        assert_eq!(service.mic_amplitude(), 0.75);
    }

    /// Integration test: full dictation start -> stop -> transcribe workflow.
    #[test]
    fn test_dictation_workflow_end_to_end() {
        // Simulate 1 second of 440Hz sine wave at 16kHz
        let num_samples = 16000;
        let samples: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16000.0).sin())
            .collect();

        let mock_recorder = Arc::new(MockAudioRecorder::with_samples(samples.clone()));
        let samples_buf = mock_recorder.samples_buffer();
        let mock_transcriber = MockTranscription::returning("Привіт, світе");

        let service = AudioService::with_recorder(mock_recorder, samples_buf, None).unwrap();

        // Start recording
        service.start_mic().unwrap();

        // Stop and get samples
        let (recorded_samples, _) = service.stop_mic();
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
        let samples_buf = mock.samples_buffer();
        let service = AudioService::with_recorder(mock, samples_buf, None).unwrap();

        // Amplitude available before recording starts (mock returns constant)
        assert_eq!(service.mic_amplitude(), 0.6);

        // Start recording - amplitude still available
        service.start_mic().unwrap();
        assert_eq!(service.mic_amplitude(), 0.6);
    }

    #[test]
    fn test_segmentation_config_default() {
        let config = SegmentationConfig::default();
        assert!(config.use_vad);
        assert_eq!(config.segment_interval_secs, 10);
        assert_eq!(config.vad_silence_threshold_ms, 1000);
        assert_eq!(config.vad_min_speech_ms, 500);
    }
}
