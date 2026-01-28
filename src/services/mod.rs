//! Services layer for audio recording and transcription.
//!
//! This module provides high-level services that abstract low-level components.
//! AudioService is used internally by AppContext, TranscriptionService is re-exported.

pub mod audio;
pub mod transcription;

pub use transcription::TranscriptionService;
