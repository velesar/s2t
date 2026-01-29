//! Shared types used across multiple modules.
//!
//! This module contains common data structures that are used by multiple
//! parts of the application to avoid duplication and circular dependencies.

use crate::history::HistoryEntry;
use crate::traits::HistoryRepository;
use async_channel::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Shared history storage as a trait object.
///
/// Used by dialogs and UI components that need access to history
/// through the `HistoryRepository` trait abstraction.
pub type SharedHistory = Arc<Mutex<dyn HistoryRepository<Entry = HistoryEntry>>>;

/// Application state for recording modes.
///
/// Tracks the current phase of the recording lifecycle.
/// Used by both UI and recording handlers.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AppState {
    Idle,
    Recording,
    Processing,
}

/// Result from stopping conference recording.
///
/// Contains audio samples from both microphone and system loopback channels,
/// along with completion receivers for synchronization.
#[derive(Default)]
pub struct ConferenceRecording {
    /// Audio samples captured from the microphone
    pub mic_samples: Vec<f32>,
    /// Audio samples captured from system audio (loopback)
    pub loopback_samples: Vec<f32>,
    /// Completion signal for microphone recording
    pub mic_completion: Option<Receiver<()>>,
    /// Completion signal for loopback recording
    pub loopback_completion: Option<Receiver<()>>,
}

impl ConferenceRecording {
    /// Create a new ConferenceRecording with the given data
    pub fn new(
        mic_samples: Vec<f32>,
        loopback_samples: Vec<f32>,
        mic_completion: Option<Receiver<()>>,
        loopback_completion: Option<Receiver<()>>,
    ) -> Self {
        Self {
            mic_samples,
            loopback_samples,
            mic_completion,
            loopback_completion,
        }
    }

    /// Check if recording has any audio data
    #[allow(dead_code)] // Utility method for future validation use
    pub fn has_audio(&self) -> bool {
        !self.mic_samples.is_empty() || !self.loopback_samples.is_empty()
    }

    /// Get total duration in seconds (based on longest channel at 16kHz)
    pub fn duration_secs(&self) -> f32 {
        self.mic_samples.len().max(self.loopback_samples.len()) as f32 / 16000.0
    }
}

/// Segment of audio ready for transcription.
///
/// Produced by `ContinuousRecorder` during automatic segmentation
/// and consumed by the UI layer for parallel transcription.
#[derive(Debug, Clone)]
pub struct AudioSegment {
    pub samples: Vec<f32>,
    pub start_time: Instant,
    pub end_time: Instant,
    pub segment_id: usize,
}
