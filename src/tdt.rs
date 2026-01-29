//! Parakeet TDT v3 speech-to-text backend.
//!
//! Alternative STT backend using NVIDIA's Parakeet TDT model via parakeet-rs.
//! Provides better Ukrainian accuracy (WER 5-7%) compared to Whisper (WER 13.72%).
//!
//! Key features:
//! - 25 languages with auto-detection
//! - Built-in punctuation and capitalization
//! - Word-level timestamps

use anyhow::Result;

#[cfg(feature = "tdt")]
use anyhow::Context;

#[cfg(feature = "tdt")]
use parakeet_rs::{ParakeetTDT, Transcriber};

#[cfg(feature = "tdt")]
use std::sync::Mutex;

/// Parakeet TDT speech-to-text backend.
///
/// Uses interior mutability (Mutex) because parakeet-rs requires &mut self
/// for transcription, but our Transcription trait uses &self.
#[allow(dead_code)]
pub struct ParakeetSTT {
    #[cfg(feature = "tdt")]
    model: Mutex<ParakeetTDT>,
    model_dir: String,
}

#[allow(dead_code)]
impl ParakeetSTT {
    /// Create a new ParakeetSTT instance from a model directory.
    ///
    /// The directory should contain:
    /// - encoder-model.int8.onnx (or encoder-model.onnx)
    /// - decoder_joint-model.int8.onnx (or decoder_joint-model.onnx)
    /// - vocab.txt
    #[cfg(feature = "tdt")]
    pub fn new(model_dir: &str) -> Result<Self> {
        let model = ParakeetTDT::from_pretrained(model_dir, None)
            .with_context(|| format!("Failed to load Parakeet TDT model from {}", model_dir))?;

        Ok(Self {
            model: Mutex::new(model),
            model_dir: model_dir.to_string(),
        })
    }

    /// Stub constructor when TDT feature is disabled.
    #[cfg(not(feature = "tdt"))]
    pub fn new(_model_dir: &str) -> Result<Self> {
        anyhow::bail!(
            "TDT not available. Build with feature 'tdt': cargo build --features tdt"
        );
    }

    /// Transcribe audio samples to text.
    ///
    /// # Arguments
    /// * `samples` - Audio samples at 16kHz mono
    /// * `_language` - Language hint (currently ignored; TDT uses auto-detection)
    #[cfg(feature = "tdt")]
    pub fn transcribe(&self, samples: &[f32], _language: Option<&str>) -> Result<String> {
        // ParakeetTDT expects samples at 16kHz as Vec<f32>
        let mut model = self
            .model
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock TDT model: {}", e))?;

        let result = model
            .transcribe_samples(samples.to_vec(), 16000, 1, None)
            .context("Failed to transcribe audio with Parakeet TDT")?;

        Ok(result.text)
    }

    /// Stub transcribe when TDT feature is disabled.
    #[cfg(not(feature = "tdt"))]
    pub fn transcribe(&self, _samples: &[f32], _language: Option<&str>) -> Result<String> {
        anyhow::bail!("TDT not available. Build with feature 'tdt'.");
    }

    /// Get the model directory path.
    pub fn model_dir(&self) -> &str {
        &self.model_dir
    }
}

// === Trait Implementation ===

use crate::traits::Transcription;
use std::path::Path;

impl Transcription for ParakeetSTT {
    fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        ParakeetSTT::transcribe(self, samples, Some(language))
    }

    fn is_loaded(&self) -> bool {
        true // ParakeetSTT only exists when model is loaded
    }

    fn model_name(&self) -> Option<String> {
        Some(format!("Parakeet TDT v3 ({})", self.model_dir))
    }

    fn load_model(&mut self, _path: &Path) -> Result<()> {
        anyhow::bail!(
            "ParakeetSTT does not support runtime model loading; use TranscriptionService"
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_model_dir_accessor() {
        // Can't test actual functionality without model files
        // Just verify the module compiles correctly
    }
}
