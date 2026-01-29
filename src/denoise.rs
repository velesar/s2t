//! Audio denoising using nnnoiseless (RNNoise).
//!
//! This module provides noise suppression for cleaner speech recognition.
//! nnnoiseless operates at 48kHz with 10ms frames (480 samples).

use crate::traits::AudioDenoising;
use anyhow::Result;
use nnnoiseless::DenoiseState;
use std::sync::Mutex;

/// Sample rate required by nnnoiseless (RNNoise)
const NNNOISELESS_SAMPLE_RATE: u32 = 48000;

/// Frame size in samples (10ms at 48kHz)
const FRAME_SIZE: usize = 480;

/// RNNoise-based denoiser using nnnoiseless.
///
/// Processes audio in 480-sample frames (10ms at 48kHz).
/// Buffers partial frames between calls to denoise().
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
}

impl Default for NnnoiselessDenoiser {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioDenoising for NnnoiselessDenoiser {
    fn denoise(&self, samples: &[f32]) -> Result<Vec<f32>> {
        let mut buffer = self.buffer.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        let mut output = Vec::with_capacity(samples.len());

        // Add incoming samples to buffer
        buffer.extend_from_slice(samples);

        // Process complete frames
        while buffer.len() >= FRAME_SIZE {
            let frame: Vec<f32> = buffer.drain(..FRAME_SIZE).collect();
            let mut frame_out = vec![0.0f32; FRAME_SIZE];
            state.process_frame(&mut frame_out, &frame);
            output.extend_from_slice(&frame_out);
        }

        Ok(output)
    }

    fn required_sample_rate(&self) -> u32 {
        NNNOISELESS_SAMPLE_RATE
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
        assert_eq!(denoiser.required_sample_rate(), 48000);
    }

    #[test]
    fn test_nnnoiseless_denoiser_small_input() {
        let denoiser = NnnoiselessDenoiser::new();
        // Input smaller than frame size should buffer and return empty
        let input = vec![0.0f32; 100];
        let output = denoiser.denoise(&input).unwrap();
        assert!(output.is_empty());
    }

    #[test]
    fn test_nnnoiseless_denoiser_exact_frame() {
        let denoiser = NnnoiselessDenoiser::new();
        // Exact frame size should process one frame
        let input = vec![0.1f32; FRAME_SIZE];
        let output = denoiser.denoise(&input).unwrap();
        assert_eq!(output.len(), FRAME_SIZE);
    }

    #[test]
    fn test_nnnoiseless_denoiser_multiple_frames() {
        let denoiser = NnnoiselessDenoiser::new();
        // Two frames worth of input
        let input = vec![0.1f32; FRAME_SIZE * 2];
        let output = denoiser.denoise(&input).unwrap();
        assert_eq!(output.len(), FRAME_SIZE * 2);
    }

    #[test]
    fn test_nnnoiseless_denoiser_buffering() {
        let denoiser = NnnoiselessDenoiser::new();

        // First call: partial frame, should buffer
        let input1 = vec![0.1f32; 200];
        let output1 = denoiser.denoise(&input1).unwrap();
        assert!(output1.is_empty());

        // Second call: completes the frame
        let input2 = vec![0.1f32; 300];
        let output2 = denoiser.denoise(&input2).unwrap();
        assert_eq!(output2.len(), FRAME_SIZE);
    }

    #[test]
    fn test_nnnoiseless_denoiser_reset() {
        let denoiser = NnnoiselessDenoiser::new();

        // Add partial frame
        let _ = denoiser.denoise(&vec![0.1f32; 200]).unwrap();

        // Reset should clear buffer
        denoiser.reset();

        // Buffer should be empty now, so small input returns nothing
        let output = denoiser.denoise(&vec![0.1f32; 100]).unwrap();
        assert!(output.is_empty());
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
        assert_eq!(denoiser.required_sample_rate(), 48000);
    }

    #[test]
    fn test_create_denoiser_disabled() {
        let denoiser = create_denoiser(false);
        assert_eq!(denoiser.required_sample_rate(), 16000);
    }
}
