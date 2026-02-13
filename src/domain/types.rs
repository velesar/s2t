//! Shared types used across multiple modules.
//!
//! This module contains common data structures that are used by multiple
//! parts of the application to avoid duplication and circular dependencies.

use crate::domain::traits::HistoryRepository;
use async_channel::Receiver;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Shared history storage as a trait object.
///
/// Used by dialogs and UI components that need access to history
/// through the `HistoryRepository` trait abstraction.
pub type SharedHistory = Arc<Mutex<dyn HistoryRepository<Entry = HistoryEntry>>>;

/// A single transcription history entry.
///
/// Represents a completed transcription with metadata such as duration,
/// language, and optional conference recording details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub duration_secs: f32,
    pub language: String,
    #[serde(default)]
    pub recording_path: Option<String>,
    #[serde(default)]
    pub speakers: Vec<String>,
}

impl HistoryEntry {
    pub fn new(text: String, duration_secs: f32, language: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text,
            timestamp: Utc::now(),
            duration_secs,
            language,
            recording_path: None,
            speakers: Vec::new(),
        }
    }

    pub fn new_with_recording(
        text: String,
        duration_secs: f32,
        language: String,
        recording_path: Option<String>,
        speakers: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text,
            timestamp: Utc::now(),
            duration_secs,
            language,
            recording_path,
            speakers,
        }
    }

    /// Returns a preview of the text (first 80 chars, single line)
    pub fn preview(&self) -> String {
        let text = self.text.replace('\n', " ");
        let chars: Vec<char> = text.chars().collect();
        if chars.len() > 80 {
            format!("{}...", chars[..80].iter().collect::<String>())
        } else {
            text
        }
    }

    /// Returns formatted timestamp in local time (YYYY-MM-DD HH:MM)
    pub fn formatted_timestamp(&self) -> String {
        let local = self.timestamp.with_timezone(&chrono::Local);
        local.format("%Y-%m-%d %H:%M").to_string()
    }

    /// Returns formatted duration (MM:SS)
    pub fn formatted_duration(&self) -> String {
        let mins = (self.duration_secs / 60.0).floor() as u32;
        let secs = (self.duration_secs % 60.0).floor() as u32;
        format!("{:02}:{:02}", mins, secs)
    }
}

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
