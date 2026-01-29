//! Transcription service layer.
//!
//! Provides a unified interface for speech-to-text transcription
//! using Whisper, abstracting away model loading and lifecycle.

use crate::whisper::WhisperSTT;
use anyhow::{Context, Result};
use std::path::Path;

/// Unified transcription service wrapping Whisper.
pub struct TranscriptionService {
    whisper: Option<WhisperSTT>,
}

impl TranscriptionService {
    /// Create a new TranscriptionService without a loaded model.
    pub fn new() -> Self {
        Self { whisper: None }
    }

    /// Create a new TranscriptionService with a pre-loaded model.
    pub fn with_model(model_path: &str) -> Result<Self> {
        let whisper = WhisperSTT::new(model_path)?;
        Ok(Self {
            whisper: Some(whisper),
        })
    }

    /// Get reference to WhisperSTT (for conference mode diarization).
    ///
    /// Conference mode needs direct access to WhisperSTT for
    /// `transcribe_with_auto_diarization()` which requires a
    /// mutable DiarizationEngine reference from AppContext.
    pub fn whisper(&self) -> Option<&WhisperSTT> {
        self.whisper.as_ref()
    }
}

impl Default for TranscriptionService {
    fn default() -> Self {
        Self::new()
    }
}

// === Trait Implementation ===

use crate::traits::Transcription;

impl Transcription for TranscriptionService {
    fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        let whisper = self.whisper.as_ref().context("Модель не завантажено")?;
        whisper.transcribe(samples, Some(language))
    }

    fn is_loaded(&self) -> bool {
        self.whisper.is_some()
    }

    fn model_name(&self) -> Option<String> {
        self.whisper.as_ref().and_then(Transcription::model_name)
    }

    fn load_model(&mut self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy();
        let whisper = WhisperSTT::new(&path_str)
            .with_context(|| format!("Failed to load Whisper model from {}", path_str))?;
        self.whisper = Some(whisper);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_unloaded_service() {
        let service = TranscriptionService::new();
        assert!(!service.is_loaded());
    }

    #[test]
    fn test_default_creates_unloaded_service() {
        let service = TranscriptionService::default();
        assert!(!service.is_loaded());
    }

    #[test]
    fn test_whisper_returns_none_when_unloaded() {
        let service = TranscriptionService::new();
        assert!(service.whisper().is_none());
    }

    #[test]
    fn test_transcribe_fails_when_no_model() {
        let service = TranscriptionService::new();
        let result = service.transcribe(&[0.0; 100], "uk");
        assert!(result.is_err());
    }

    #[test]
    fn test_trait_is_loaded() {
        let service = TranscriptionService::new();
        assert!(!Transcription::is_loaded(&service));
    }

    #[test]
    fn test_trait_model_name_none_when_unloaded() {
        let service = TranscriptionService::new();
        assert!(Transcription::model_name(&service).is_none());
    }
}
