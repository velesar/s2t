//! Transcription service layer.
//!
//! Provides a unified interface for speech-to-text transcription
//! supporting multiple backends (Whisper, Parakeet TDT).

use crate::domain::traits::Transcription;
use crate::transcription::diarization::DiarizationEngine;
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

    /// Transcribe conference recording with diarization.
    ///
    /// Works with any loaded backend (Whisper or TDT).
    /// Selects diarization strategy based on method parameter:
    /// - "sortformer": neural speaker diarization (if engine is available)
    /// - anything else: channel-based (mic = "Ви", loopback = "Учасник")
    pub fn transcribe_conference(
        &self,
        mic_samples: &[f32],
        loopback_samples: &[f32],
        language: &str,
        diarization_method: &str,
        diarization_engine: Option<&mut DiarizationEngine>,
    ) -> Result<String> {
        // Try Sortformer diarization if requested and available
        if diarization_method == "sortformer" {
            if let Some(engine) = diarization_engine {
                if engine.is_available() {
                    return self.transcribe_with_sortformer(
                        mic_samples,
                        loopback_samples,
                        language,
                        engine,
                    );
                }
            }
        }

        // Fallback to channel-based diarization
        self.transcribe_channel_diarization(mic_samples, loopback_samples, language)
    }

    /// Channel-based diarization: transcribe mic and loopback separately.
    fn transcribe_channel_diarization(
        &self,
        mic_samples: &[f32],
        loopback_samples: &[f32],
        language: &str,
    ) -> Result<String> {
        let mic_text = if !mic_samples.is_empty() {
            Transcription::transcribe(self, mic_samples, language)?
        } else {
            String::new()
        };

        let loopback_text = if !loopback_samples.is_empty() {
            Transcription::transcribe(self, loopback_samples, language)?
        } else {
            String::new()
        };

        let mut result = String::new();
        if !mic_text.is_empty() {
            result.push_str("[Ви] ");
            result.push_str(&mic_text);
        }
        if !loopback_text.is_empty() {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str("[Учасник] ");
            result.push_str(&loopback_text);
        }

        Ok(result)
    }

    /// Sortformer-based diarization: mix channels, diarize, transcribe segments.
    fn transcribe_with_sortformer(
        &self,
        mic_samples: &[f32],
        loopback_samples: &[f32],
        language: &str,
        engine: &mut DiarizationEngine,
    ) -> Result<String> {
        let max_len = mic_samples.len().max(loopback_samples.len());
        let mut mixed = Vec::with_capacity(max_len);
        for i in 0..max_len {
            let mic_val = mic_samples.get(i).copied().unwrap_or(0.0);
            let loopback_val = loopback_samples.get(i).copied().unwrap_or(0.0);
            mixed.push((mic_val + loopback_val) / 2.0);
        }

        let segments = engine.diarize(&mixed).context("Помилка diarization")?;

        if segments.is_empty() {
            return self.transcribe_channel_diarization(mic_samples, loopback_samples, language);
        }

        let mut parts = Vec::new();
        for seg in segments {
            let start = (seg.start_time * 16000.0) as usize;
            let end = (seg.end_time * 16000.0).min(mixed.len() as f64) as usize;

            if start >= end || start >= mixed.len() {
                continue;
            }

            let segment_samples = &mixed[start..end.min(mixed.len())];
            if segment_samples.is_empty() {
                continue;
            }

            let text = Transcription::transcribe(self, segment_samples, language)?;
            if !text.is_empty() {
                parts.push(format!("[Спікер {}] {}", seg.speaker_id + 1, text));
            }
        }

        if parts.is_empty() {
            return self.transcribe_channel_diarization(mic_samples, loopback_samples, language);
        }

        Ok(parts.join(" "))
    }
}

impl Default for TranscriptionService {
    fn default() -> Self {
        Self::new()
    }
}

// === Trait Implementation ===

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
