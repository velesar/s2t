use crate::diarization::DiarizationEngine;
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
                result.push(' ');
            }
            result.push_str("[Учасник] ");
            result.push_str(&loopback_text);
        }

        Ok(result)
    }

    /// Transcribe with Sortformer-based diarization (Production approach)
    /// Uses Sortformer to identify speakers, then transcribes each segment
    pub fn transcribe_with_sortformer(
        &self,
        mic_samples: &[f32],
        loopback_samples: &[f32],
        language: Option<&str>,
        diarization_engine: &mut DiarizationEngine,
    ) -> Result<String> {
        // Merge mic and loopback into mono mix for diarization
        // This gives us a single audio stream with all speakers
        let max_len = mic_samples.len().max(loopback_samples.len());
        let mut mixed_samples = Vec::with_capacity(max_len);

        for i in 0..max_len {
            let mic_val = mic_samples.get(i).copied().unwrap_or(0.0);
            let loopback_val = loopback_samples.get(i).copied().unwrap_or(0.0);
            // Mix both channels (simple average)
            mixed_samples.push((mic_val + loopback_val) / 2.0);
        }

        // Perform diarization
        let diarization_segments = diarization_engine
            .diarize(&mixed_samples)
            .context("Помилка diarization")?;

        if diarization_segments.is_empty() {
            // Fallback to channel-based if no segments found
            return self.transcribe_with_diarization(mic_samples, loopback_samples, language);
        }

        // Transcribe each segment
        let mut result_parts = Vec::new();

        for seg in diarization_segments {
            let start_idx = (seg.start_time * 16000.0) as usize;
            let end_idx = (seg.end_time * 16000.0).min(mixed_samples.len() as f64) as usize;

            if start_idx >= end_idx || start_idx >= mixed_samples.len() {
                continue;
            }

            let segment_samples = &mixed_samples[start_idx..end_idx.min(mixed_samples.len())];

            if segment_samples.is_empty() {
                continue;
            }

            // Transcribe this segment
            let segment_text = self.transcribe(segment_samples, language)?;

            if !segment_text.is_empty() {
                result_parts.push(format!(
                    "[Спікер {}] {}",
                    seg.speaker_id + 1, // 1-indexed for user
                    segment_text
                ));
            }
        }

        if result_parts.is_empty() {
            // Fallback if no transcription succeeded
            return self.transcribe_with_diarization(mic_samples, loopback_samples, language);
        }

        Ok(result_parts.join(" "))
    }

    /// Transcribe with automatic diarization method selection
    /// Uses Sortformer if available, otherwise falls back to channel-based
    pub fn transcribe_with_auto_diarization(
        &self,
        mic_samples: &[f32],
        loopback_samples: &[f32],
        language: Option<&str>,
        diarization_method: &str,
        diarization_engine: Option<&mut DiarizationEngine>,
    ) -> Result<String> {
        // Use Sortformer if method is "sortformer" and engine is available
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

        // Fallback to channel-based
        self.transcribe_with_diarization(mic_samples, loopback_samples, language)
    }

    /// Get the model path
    pub fn model_path(&self) -> &str {
        &self.model_path
    }
}

// === Trait Implementation ===

use crate::traits::Transcription;

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
}
