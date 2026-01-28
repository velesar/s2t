//! Audio recording service layer.
//!
//! This module provides a unified interface for all recording modes.
//! The methods are currently unused as handlers haven't been migrated yet
//! (hybrid migration phase).

use crate::audio::AudioRecorder;
use crate::conference_recorder::ConferenceRecorder;
use crate::continuous::{AudioSegment, ContinuousRecorder};
use anyhow::Result;
use async_channel::{Receiver, Sender};
use std::sync::Arc;

/// Configuration for continuous recording mode
#[allow(dead_code)]
pub struct ContinuousConfig {
    pub use_vad: bool,
    pub segment_interval_secs: u32,
    pub vad_silence_threshold_ms: u32,
    pub vad_min_speech_ms: u32,
}

impl Default for ContinuousConfig {
    fn default() -> Self {
        Self {
            use_vad: true,
            segment_interval_secs: 10,
            vad_silence_threshold_ms: 1000,
            vad_min_speech_ms: 500,
        }
    }
}

/// Result from stopping conference recording
#[allow(dead_code)]
pub struct ConferenceRecording {
    pub mic_samples: Vec<f32>,
    pub loopback_samples: Vec<f32>,
    pub mic_completion: Option<Receiver<()>>,
    pub loopback_completion: Option<Receiver<()>>,
}

/// Unified audio service wrapping all recording modes
#[allow(dead_code)]
pub struct AudioService {
    mic: Arc<AudioRecorder>,
    conference: Arc<ConferenceRecorder>,
    continuous: Arc<ContinuousRecorder>,
}

#[allow(dead_code)]

impl AudioService {
    /// Create a new AudioService with the given continuous recording configuration
    pub fn new(continuous_config: ContinuousConfig) -> Result<Self> {
        let continuous = ContinuousRecorder::new(
            continuous_config.use_vad,
            continuous_config.segment_interval_secs,
            continuous_config.vad_silence_threshold_ms,
            continuous_config.vad_min_speech_ms,
        )?;

        Ok(Self {
            mic: Arc::new(AudioRecorder::new()),
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
        self.mic.start_recording()
    }

    /// Stop dictation and return recorded samples
    pub fn stop_dictation(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.mic.stop_recording()
    }

    /// Get current microphone amplitude for dictation mode
    pub fn get_dictation_amplitude(&self) -> f32 {
        self.mic.get_amplitude()
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
        let (mic_samples, loopback_samples, mic_completion, loopback_completion) =
            self.conference.stop_conference();

        ConferenceRecording {
            mic_samples,
            loopback_samples,
            mic_completion,
            loopback_completion,
        }
    }

    /// Get microphone amplitude for conference mode
    pub fn get_mic_amplitude(&self) -> f32 {
        self.conference.get_mic_amplitude()
    }

    /// Get loopback amplitude for conference mode
    pub fn get_loopback_amplitude(&self) -> f32 {
        self.conference.get_loopback_amplitude()
    }

    // === Access to underlying recorders (for legacy code during migration) ===

    /// Get reference to mic recorder (for legacy compatibility)
    pub fn mic_recorder(&self) -> &Arc<AudioRecorder> {
        &self.mic
    }

    /// Get reference to conference recorder (for legacy compatibility)
    pub fn conference_recorder(&self) -> &Arc<ConferenceRecorder> {
        &self.conference
    }

    /// Get reference to continuous recorder (for legacy compatibility)
    pub fn continuous_recorder(&self) -> &Arc<ContinuousRecorder> {
        &self.continuous
    }
}
