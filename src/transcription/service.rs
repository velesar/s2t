//! Transcription service layer.
//!
//! Provides a unified interface for speech-to-text transcription
//! supporting multiple backends (Whisper, Parakeet TDT).

use crate::transcription::ParakeetSTT;
use crate::transcription::WhisperSTT;
use anyhow::{Context, Result};
use std::path::Path;

/// Backend type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BackendType {
    Whisper,
    Tdt,
}

/// Transcription backend variants.
enum TranscriptionBackend {
    Whisper(WhisperSTT),
    Tdt(Box<ParakeetSTT>),
    None,
}

/// Unified transcription service supporting multiple STT backends.
pub struct TranscriptionService {
    backend: TranscriptionBackend,
}

impl TranscriptionService {
    /// Create a new TranscriptionService without a loaded model.
    pub fn new() -> Self {
        Self {
            backend: TranscriptionBackend::None,
        }
    }

    /// Create a new TranscriptionService with a Whisper model.
    pub fn with_model(model_path: &str) -> Result<Self> {
        let whisper = WhisperSTT::new(model_path)?;
        Ok(Self {
            backend: TranscriptionBackend::Whisper(whisper),
        })
    }

    /// Create a new TranscriptionService with a Parakeet TDT model.
    ///
    /// The model_dir should contain:
    /// - encoder-model.int8.onnx (or encoder-model.onnx)
    /// - decoder_joint-model.int8.onnx (or decoder_joint-model.onnx)
    /// - vocab.txt
    pub fn with_tdt(model_dir: &str) -> Result<Self> {
        let tdt = ParakeetSTT::new(model_dir)?;
        Ok(Self {
            backend: TranscriptionBackend::Tdt(Box::new(tdt)),
        })
    }

    /// Get reference to WhisperSTT (for conference mode diarization).
    ///
    /// Conference mode needs direct access to WhisperSTT for
    /// `transcribe_with_auto_diarization()` which requires a
    /// mutable DiarizationEngine reference from AppContext.
    pub fn whisper(&self) -> Option<&WhisperSTT> {
        match &self.backend {
            TranscriptionBackend::Whisper(w) => Some(w),
            _ => None,
        }
    }

    /// Get the current backend type.
    #[allow(dead_code)]
    pub fn backend_type(&self) -> Option<BackendType> {
        match &self.backend {
            TranscriptionBackend::Whisper(_) => Some(BackendType::Whisper),
            TranscriptionBackend::Tdt(_) => Some(BackendType::Tdt),
            TranscriptionBackend::None => None,
        }
    }

    /// Check if the backend has built-in punctuation.
    ///
    /// Parakeet TDT includes punctuation and capitalization;
    /// Whisper does not.
    #[allow(dead_code)]
    pub fn has_builtin_punctuation(&self) -> bool {
        matches!(&self.backend, TranscriptionBackend::Tdt(_))
    }
}

impl Default for TranscriptionService {
    fn default() -> Self {
        Self::new()
    }
}

// === Trait Implementation ===

use crate::domain::traits::Transcription;

impl Transcription for TranscriptionService {
    fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        match &self.backend {
            TranscriptionBackend::Whisper(w) => w.transcribe(samples, Some(language)),
            TranscriptionBackend::Tdt(t) => t.transcribe(samples, Some(language)),
            TranscriptionBackend::None => {
                anyhow::bail!("Модель не завантажено")
            }
        }
    }

    fn is_loaded(&self) -> bool {
        !matches!(self.backend, TranscriptionBackend::None)
    }

    fn model_name(&self) -> Option<String> {
        match &self.backend {
            TranscriptionBackend::Whisper(w) => Transcription::model_name(w),
            TranscriptionBackend::Tdt(t) => Transcription::model_name(t.as_ref()),
            TranscriptionBackend::None => None,
        }
    }

    fn load_model(&mut self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy();
        let whisper = WhisperSTT::new(&path_str)
            .with_context(|| format!("Failed to load Whisper model from {}", path_str))?;
        self.backend = TranscriptionBackend::Whisper(whisper);
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

    #[test]
    fn test_backend_type_none_when_unloaded() {
        let service = TranscriptionService::new();
        assert!(service.backend_type().is_none());
    }

    #[test]
    fn test_has_builtin_punctuation_false_when_unloaded() {
        let service = TranscriptionService::new();
        assert!(!service.has_builtin_punctuation());
    }
}
