use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperSTT {
    ctx: WhisperContext,
    #[allow(dead_code)]
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

    /// Transcribe with channel-based diarization (MVP approach)
    /// Transcribes mic and loopback channels separately and merges with speaker labels
    pub fn transcribe_with_diarization(
        &self,
        mic_samples: &[f32],
        loopback_samples: &[f32],
        language: Option<&str>,
    ) -> Result<String> {
        // Transcribe microphone channel
        let mic_text = if !mic_samples.is_empty() {
            self.transcribe(mic_samples, language)?
        } else {
            String::new()
        };

        // Transcribe loopback channel
        let loopback_text = if !loopback_samples.is_empty() {
            self.transcribe(loopback_samples, language)?
        } else {
            String::new()
        };

        // Merge with speaker labels
        let mut result = String::new();

        if !mic_text.is_empty() {
            result.push_str("[Ви] ");
            result.push_str(&mic_text);
        }

        if !loopback_text.is_empty() {
            if !result.is_empty() {
                result.push_str(" ");
            }
            result.push_str("[Учасник] ");
            result.push_str(&loopback_text);
        }

        Ok(result)
    }

}
