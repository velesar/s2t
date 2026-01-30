use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub(crate) struct WhisperSTT {
    ctx: WhisperContext,
    model_path: String,
}

impl WhisperSTT {
    pub fn new(model_path: &str) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .context("Не вдалося завантажити модель Whisper")?;

        Ok(Self {
            ctx,
            model_path: model_path.to_string(),
        })
    }

    pub fn transcribe(&self, samples: &[f32], language: Option<&str>) -> Result<String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        if let Some(lang) = language {
            params.set_language(Some(lang));
        }

        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_translate(false);

        let mut state = self.ctx.create_state()?;
        state.full(params, samples)?;

        let num_segments = state.full_n_segments()?;
        let mut text = String::new();

        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(&segment);
                text.push(' ');
            }
        }

        Ok(text.trim().to_string())
    }

}

// === Trait Implementation ===

use crate::domain::traits::Transcription;

impl Transcription for WhisperSTT {
    fn transcribe(&self, samples: &[f32], language: &str) -> anyhow::Result<String> {
        WhisperSTT::transcribe(self, samples, Some(language))
    }

    fn is_loaded(&self) -> bool {
        true // WhisperSTT only exists when model is loaded
    }

    fn model_name(&self) -> Option<String> {
        Some(self.model_path.clone())
    }

    fn load_model(&mut self, _path: &std::path::Path) -> anyhow::Result<()> {
        // WhisperSTT is created with a model; use TranscriptionService for model management
        anyhow::bail!("WhisperSTT does not support runtime model loading; use TranscriptionService")
    }
}
