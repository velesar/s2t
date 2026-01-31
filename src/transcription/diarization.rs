use anyhow::{Context, Result};
use std::path::PathBuf;

use parakeet_rs::sortformer::{DiarizationConfig, Sortformer};

const SAMPLE_RATE: u32 = 16000;

/// Speaker diarization segment with speaker ID and timestamps
#[derive(Debug, Clone)]
pub struct DiarizationSegment {
    pub speaker_id: usize,
    pub start_time: f64,
    pub end_time: f64,
}

/// Diarization engine using NVIDIA Sortformer
pub struct DiarizationEngine {
    sortformer: Option<Sortformer>,
    model_path: Option<PathBuf>,
}

impl DiarizationEngine {
    /// Create a new diarization engine
    /// If model_path is None, diarization will be disabled
    pub fn new(model_path: Option<PathBuf>) -> Self {
        Self {
            sortformer: None,
            model_path,
        }
    }

    /// Load the Sortformer model
    pub fn load_model(&mut self) -> Result<()> {
        if let Some(ref model_path) = self.model_path {
            if !model_path.exists() {
                anyhow::bail!(
                    "Модель Sortformer не знайдено: {}. Завантажте модель з HuggingFace.",
                    model_path.display()
                );
            }

            let config = DiarizationConfig::callhome();
            self.sortformer = Some(
                Sortformer::with_config(model_path, None, config)
                    .context("Не вдалося завантажити модель Sortformer")?,
            );
        }
        Ok(())
    }

    /// Check if diarization is available
    pub fn is_available(&self) -> bool {
        self.sortformer.is_some()
    }

    /// Perform diarization on audio samples
    /// Returns segments with speaker IDs and timestamps
    pub fn diarize(&mut self, audio_samples: &[f32]) -> Result<Vec<DiarizationSegment>> {
        if let Some(ref mut sortformer) = self.sortformer {
            // Sortformer expects audio as Vec<f32> at 16kHz
            let segments = sortformer
                .diarize(audio_samples.to_vec(), SAMPLE_RATE, 1)
                .context("Помилка diarization")?;

            Ok(segments
                .into_iter()
                .map(|seg| DiarizationSegment {
                    speaker_id: seg.speaker_id,
                    start_time: seg.start as f64,
                    end_time: seg.end as f64,
                })
                .collect())
        } else {
            anyhow::bail!("Diarization не доступна. Завантажте модель Sortformer.")
        }
    }
}

impl Default for DiarizationEngine {
    fn default() -> Self {
        Self::new(None)
    }
}
