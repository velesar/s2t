//! Audio denoising using nnnoiseless (RNNoise).
//!
//! This module provides noise suppression for cleaner speech recognition.
//! nnnoiseless/RNNoise operates at 48kHz with 10ms frames (480 samples).
//! The denoiser accepts 16kHz input and handles resampling internally.

use crate::domain::traits::AudioDenoising;
use anyhow::{Context, Result};
use nnnoiseless::DenoiseState;
use rubato::{FftFixedIn, Resampler};
use std::sync::Mutex;

/// Input sample rate (Whisper pipeline rate)
const INPUT_SAMPLE_RATE: usize = 16000;

/// Sample rate required by nnnoiseless (RNNoise)
const NNNOISELESS_SAMPLE_RATE: usize = 48000;

/// Frame size in samples (10ms at 48kHz)
const FRAME_SIZE: usize = 480;

/// Chunk size for rubato resampler
const RESAMPLE_CHUNK: usize = 1024;

/// RNNoise-based denoiser using nnnoiseless.
///
/// Accepts 16kHz audio, resamples to 48kHz for RNNoise processing,
/// then resamples back to 16kHz. This ensures RNNoise operates at
/// its trained sample rate for correct noise suppression.
pub struct NnnoiselessDenoiser {
    state: Mutex<Box<DenoiseState<'static>>>,
    buffer: Mutex<Vec<f32>>,
}

impl NnnoiselessDenoiser {
    /// Create a new nnnoiseless denoiser.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(DenoiseState::new()),
            buffer: Mutex::new(Vec::with_capacity(FRAME_SIZE)),
        }
    }

    /// Upsample 16kHz audio to 48kHz using rubato.
    fn upsample(samples: &[f32]) -> Result<Vec<f32>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let mut resampler = FftFixedIn::<f32>::new(
            INPUT_SAMPLE_RATE,
            NNNOISELESS_SAMPLE_RATE,
            RESAMPLE_CHUNK,
            2, // sub chunks
            1, // channels
        )
        .context("Failed to create upsampler")?;

        let mut output = Vec::with_capacity(samples.len() * 3);
        let mut pos = 0;
        let frames_needed = resampler.input_frames_next();

        while pos + frames_needed <= samples.len() {
            let chunk = vec![samples[pos..pos + frames_needed].to_vec()];
            let resampled = resampler.process(&chunk, None)
                .context("Upsample failed")?;
            output.extend_from_slice(&resampled[0]);
            pos += frames_needed;
        }

        // Process remaining samples with zero-padding
        if pos < samples.len() {
            let remaining = &samples[pos..];
            let mut padded = remaining.to_vec();
            padded.resize(frames_needed, 0.0);
            let chunk = vec![padded];
            let resampled = resampler.process(&chunk, None)
                .context("Upsample final chunk failed")?;
            let remaining_duration = remaining.len() as f64 / INPUT_SAMPLE_RATE as f64;
            let expected = (remaining_duration * NNNOISELESS_SAMPLE_RATE as f64).ceil() as usize;
            let actual = expected.min(resampled[0].len());
            output.extend_from_slice(&resampled[0][..actual]);
        }

        Ok(output)
    }

    /// Downsample 48kHz audio back to 16kHz using rubato.
    fn downsample(samples: &[f32]) -> Result<Vec<f32>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let mut resampler = FftFixedIn::<f32>::new(
            NNNOISELESS_SAMPLE_RATE,
            INPUT_SAMPLE_RATE,
            RESAMPLE_CHUNK,
            2, // sub chunks
            1, // channels
        )
        .context("Failed to create downsampler")?;

        let mut output = Vec::with_capacity(samples.len() / 3);
        let mut pos = 0;
        let frames_needed = resampler.input_frames_next();

        while pos + frames_needed <= samples.len() {
            let chunk = vec![samples[pos..pos + frames_needed].to_vec()];
            let resampled = resampler.process(&chunk, None)
                .context("Downsample failed")?;
            output.extend_from_slice(&resampled[0]);
            pos += frames_needed;
        }

        // Process remaining samples with zero-padding
        if pos < samples.len() {
            let remaining = &samples[pos..];
            let mut padded = remaining.to_vec();
            padded.resize(frames_needed, 0.0);
            let chunk = vec![padded];
            let resampled = resampler.process(&chunk, None)
                .context("Downsample final chunk failed")?;
            let remaining_duration = remaining.len() as f64 / NNNOISELESS_SAMPLE_RATE as f64;
            let expected = (remaining_duration * INPUT_SAMPLE_RATE as f64).ceil() as usize;
            let actual = expected.min(resampled[0].len());
            output.extend_from_slice(&resampled[0][..actual]);
        }

        Ok(output)
    }

    /// Denoise a complete buffer of 16kHz audio.
    ///
    /// Resamples to 48kHz, runs RNNoise, resamples back to 16kHz.
    pub fn denoise_buffer(&self, samples: &[f32]) -> Result<Vec<f32>> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }

        // 16kHz → 48kHz
        let upsampled = Self::upsample(samples)?;

        // Run RNNoise at 48kHz
        let mut state = self.state.lock().unwrap();
        let mut buffer = self.buffer.lock().unwrap();

        let mut denoised_48k = Vec::with_capacity(upsampled.len());
        buffer.extend_from_slice(&upsampled);

        while buffer.len() >= FRAME_SIZE {
            let frame: Vec<f32> = buffer.drain(..FRAME_SIZE).collect();
            let mut frame_out = vec![0.0f32; FRAME_SIZE];
            state.process_frame(&mut frame_out, &frame);
            denoised_48k.extend_from_slice(&frame_out);
        }

        // Flush remaining buffered samples (pass through unprocessed)
        if !buffer.is_empty() {
            denoised_48k.extend_from_slice(&buffer);
            buffer.clear();
        }

        drop(buffer);
        drop(state);

        // 48kHz → 16kHz
        Self::downsample(&denoised_48k)
    }
}

impl Default for NnnoiselessDenoiser {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDenoising for NnnoiselessDenoiser {
    fn denoise(&self, samples: &[f32]) -> Result<Vec<f32>> {
        self.denoise_buffer(samples)
    }

    fn required_sample_rate(&self) -> u32 {
        INPUT_SAMPLE_RATE as u32
    }

    fn reset(&self) {
        let mut buffer = self.buffer.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        buffer.clear();
        *state = DenoiseState::new();
    }
}

/// No-op denoiser that passes audio through unchanged.
///
/// Used when denoising is disabled in configuration.
pub struct NoOpDenoiser;

impl NoOpDenoiser {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoOpDenoiser {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDenoising for NoOpDenoiser {
    fn denoise(&self, samples: &[f32]) -> Result<Vec<f32>> {
        Ok(samples.to_vec())
    }

    fn required_sample_rate(&self) -> u32 {
        // No-op denoiser works at any rate, but report 16kHz
        // since that's the pipeline's target rate
        16000
    }

    fn reset(&self) {
        // Nothing to reset
    }
}

/// Create a denoiser based on configuration.
///
/// # Arguments
/// * `enabled` - If true, creates NnnoiselessDenoiser; otherwise NoOpDenoiser
pub fn create_denoiser(enabled: bool) -> Box<dyn AudioDenoising> {
    if enabled {
        Box::new(NnnoiselessDenoiser::new())
    } else {
        Box::new(NoOpDenoiser::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nnnoiseless_denoiser_creation() {
        let denoiser = NnnoiselessDenoiser::new();
        assert_eq!(denoiser.required_sample_rate(), 16000);
    }

    #[test]
    fn test_nnnoiseless_denoiser_empty_input() {
        let denoiser = NnnoiselessDenoiser::new();
        let output = denoiser.denoise(&[]).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_nnnoiseless_denoiser_processes_audio() {
        let denoiser = NnnoiselessDenoiser::new();
        // 1 second of silence at 16kHz
        let input = vec![0.0f32; 16000];
        let output = denoiser.denoise(&input).unwrap();
        // Output should be approximately the same length
        // (resampling may introduce slight length differences)
        let ratio = output.len() as f64 / input.len() as f64;
        assert!(ratio > 0.95 && ratio < 1.05,
            "Output length {} too different from input length {}", output.len(), input.len());
    }

    #[test]
    fn test_nnnoiseless_denoiser_reset() {
        let denoiser = NnnoiselessDenoiser::new();
        let _ = denoiser.denoise(&vec![0.1f32; 16000]).unwrap();
        denoiser.reset();
        // After reset, should work cleanly again
        let output = denoiser.denoise(&vec![0.0f32; 16000]).unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn test_noop_denoiser_passthrough() {
        let denoiser = NoOpDenoiser::new();
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let output = denoiser.denoise(&input).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn test_noop_denoiser_sample_rate() {
        let denoiser = NoOpDenoiser::new();
        assert_eq!(denoiser.required_sample_rate(), 16000);
    }

    #[test]
    fn test_create_denoiser_enabled() {
        let denoiser = create_denoiser(true);
        assert_eq!(denoiser.required_sample_rate(), 16000);
    }

    #[test]
    fn test_create_denoiser_disabled() {
        let denoiser = create_denoiser(false);
        assert_eq!(denoiser.required_sample_rate(), 16000);
    }
}
