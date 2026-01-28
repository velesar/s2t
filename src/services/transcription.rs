//! Transcription service layer.
//!
//! Note: Most methods are currently unused (hybrid migration phase).
//! They will be used as handlers are migrated to use TranscriptionService directly.

#![allow(dead_code)]

use crate::diarization::DiarizationEngine;
use crate::whisper::WhisperSTT;
use anyhow::{Context, Result};
use std::path::Path;

/// Unified transcription service wrapping Whisper and diarization
pub struct TranscriptionService {
    whisper: Option<WhisperSTT>,
    diarization: DiarizationEngine,
}

impl TranscriptionService {
    /// Create a new TranscriptionService without a loaded model
    pub fn new() -> Self {
        Self {
            whisper: None,
            diarization: DiarizationEngine::default(),
        }
    }

    /// Create a new TranscriptionService with a pre-loaded model
    pub fn with_model(model_path: &str) -> Result<Self> {
        let whisper = WhisperSTT::new(model_path)?;
        Ok(Self {
            whisper: Some(whisper),
            diarization: DiarizationEngine::default(),
        })
    }

    /// Create with both Whisper and diarization engine
    pub fn with_diarization(model_path: Option<&str>, diarization: DiarizationEngine) -> Result<Self> {
        let whisper = match model_path {
            Some(path) => Some(WhisperSTT::new(path)?),
            None => None,
        };

        Ok(Self {
            whisper,
            diarization,
        })
    }

    /// Load or replace the Whisper model
    pub fn load_model(&mut self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy();
        let whisper = WhisperSTT::new(&path_str)
            .with_context(|| format!("Failed to load Whisper model from {}", path_str))?;
        self.whisper = Some(whisper);
        Ok(())
    }

    /// Check if a model is loaded
    pub fn is_loaded(&self) -> bool {
        self.whisper.is_some()
    }

    /// Set the diarization engine
    pub fn set_diarization(&mut self, engine: DiarizationEngine) {
        self.diarization = engine;
    }

    /// Get mutable reference to diarization engine
    pub fn diarization_mut(&mut self) -> &mut DiarizationEngine {
        &mut self.diarization
    }

    /// Check if diarization is available
    pub fn is_diarization_available(&self) -> bool {
        self.diarization.is_available()
    }

    /// Transcribe audio samples
    pub fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        let whisper = self.whisper.as_ref()
            .context("Модель не завантажено")?;

        whisper.transcribe(samples, Some(language))
    }

    /// Transcribe conference recording with diarization
    pub fn transcribe_conference(
        &mut self,
        mic_samples: &[f32],
        loopback_samples: &[f32],
        language: &str,
        diarization_method: &str,
    ) -> Result<String> {
        let whisper = self.whisper.as_ref()
            .context("Модель не завантажено")?;

        whisper.transcribe_with_auto_diarization(
            mic_samples,
            loopback_samples,
            Some(language),
            diarization_method,
            Some(&mut self.diarization),
        )
    }

    /// Get reference to WhisperSTT (for legacy compatibility during migration)
    pub fn whisper(&self) -> Option<&WhisperSTT> {
        self.whisper.as_ref()
    }
}

impl Default for TranscriptionService {
    fn default() -> Self {
        Self::new()
    }
}
