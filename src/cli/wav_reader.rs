//! WAV file reading and audio preparation utilities.

use anyhow::{Context, Result};
use rubato::{FftFixedIn, Resampler};
use std::path::Path;

/// Audio data read from a WAV file.
pub struct WavAudio {
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels (1=mono, 2=stereo)
    pub channels: u16,
    /// Duration in seconds
    pub duration_secs: f64,
    /// Samples per channel (interleaved for stereo)
    pub samples: Vec<f32>,
}

/// Prepared audio ready for Whisper transcription.
pub struct PreparedAudio {
    /// Mono samples at 16kHz
    pub samples: Vec<f32>,
    /// Left channel samples at 16kHz (only if stereo)
    pub left: Option<Vec<f32>>,
    /// Right channel samples at 16kHz (only if stereo)
    pub right: Option<Vec<f32>>,
    /// Whether original was stereo (for future use)
    #[allow(dead_code)]
    pub is_stereo: bool,
}

/// Read a WAV file and convert to f32 samples.
///
/// Supports 8/16/24/32-bit integer and 32-bit float formats.
pub fn read_wav(path: &Path) -> Result<WavAudio> {
    let reader =
        hound::WavReader::open(path).with_context(|| format!("Failed to open WAV file: {}", path.display()))?;

    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;
    let bits_per_sample = spec.bits_per_sample;
    let sample_format = spec.sample_format;

    let samples: Vec<f32> = match sample_format {
        hound::SampleFormat::Int => {
            let max_value = (1i64 << (bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_value))
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to read WAV samples")?
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to read WAV samples")?,
    };

    let total_samples = samples.len() / channels as usize;
    let duration_secs = total_samples as f64 / sample_rate as f64;

    Ok(WavAudio {
        sample_rate,
        channels,
        duration_secs,
        samples,
    })
}

/// Extract a single channel from interleaved stereo samples.
fn extract_channel(samples: &[f32], channel_index: usize, num_channels: usize) -> Vec<f32> {
    samples
        .iter()
        .skip(channel_index)
        .step_by(num_channels)
        .copied()
        .collect()
}

/// Convert stereo to mono by averaging channels.
fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels == 1 {
        return samples.to_vec();
    }

    let num_channels = channels as usize;
    let num_frames = samples.len() / num_channels;
    let mut mono = Vec::with_capacity(num_frames);

    for i in 0..num_frames {
        let mut sum = 0.0;
        for ch in 0..num_channels {
            sum += samples[i * num_channels + ch];
        }
        mono.push(sum / num_channels as f32);
    }

    mono
}

/// Resample audio to 16kHz using rubato.
fn resample_to_16khz(samples: &[f32], input_rate: u32) -> Result<Vec<f32>> {
    const TARGET_RATE: u32 = 16000;

    if input_rate == TARGET_RATE {
        return Ok(samples.to_vec());
    }

    let mut resampler = FftFixedIn::<f32>::new(
        input_rate as usize,
        TARGET_RATE as usize,
        1024, // chunk size
        2,    // sub chunks
        1,    // channels
    )
    .context("Failed to create resampler")?;

    let mut output = Vec::new();
    let mut input_pos = 0;

    // Process full chunks
    let frames_needed = resampler.input_frames_next();
    while input_pos + frames_needed <= samples.len() {
        let input_chunk: Vec<Vec<f32>> = vec![samples[input_pos..input_pos + frames_needed].to_vec()];
        let resampled = resampler.process(&input_chunk, None).context("Resampling failed")?;
        output.extend_from_slice(&resampled[0]);
        input_pos += frames_needed;
    }

    // Process remaining samples with padding
    if input_pos < samples.len() {
        let remaining = &samples[input_pos..];
        let mut padded = remaining.to_vec();
        padded.resize(frames_needed, 0.0);
        let input_chunk: Vec<Vec<f32>> = vec![padded];
        let resampled = resampler
            .process(&input_chunk, None)
            .context("Resampling final chunk failed")?;

        // Calculate how many output samples we actually need
        let remaining_duration = remaining.len() as f64 / input_rate as f64;
        let expected_output = (remaining_duration * TARGET_RATE as f64).ceil() as usize;
        let actual_output = expected_output.min(resampled[0].len());
        output.extend_from_slice(&resampled[0][..actual_output]);
    }

    Ok(output)
}

/// Apply noise suppression using nnnoiseless with proper resampling.
fn apply_denoise(samples: &[f32]) -> Result<Vec<f32>> {
    use crate::recording::denoise::NnnoiselessDenoiser;

    let denoiser = NnnoiselessDenoiser::new();
    denoiser.denoise_buffer(samples)
}

use crate::cli::args::ChannelMode;

/// Prepare audio for Whisper transcription.
///
/// Handles channel selection, resampling to 16kHz, and optional denoising.
pub fn prepare_for_whisper(audio: &WavAudio, channel_mode: ChannelMode, denoise: bool) -> Result<PreparedAudio> {
    let is_stereo = audio.channels == 2;

    // Extract channels based on mode
    let (main_samples, left, right) = match (channel_mode, is_stereo) {
        (ChannelMode::Mix, true) => {
            let mono = to_mono(&audio.samples, audio.channels);
            (mono, None, None)
        }
        (ChannelMode::Mix, false) => (audio.samples.clone(), None, None),
        (ChannelMode::Left, true) => {
            let left = extract_channel(&audio.samples, 0, 2);
            (left, None, None)
        }
        (ChannelMode::Left, false) => (audio.samples.clone(), None, None),
        (ChannelMode::Right, true) => {
            let right = extract_channel(&audio.samples, 1, 2);
            (right, None, None)
        }
        (ChannelMode::Right, false) => (audio.samples.clone(), None, None),
        (ChannelMode::Both, true) => {
            let left_ch = extract_channel(&audio.samples, 0, 2);
            let right_ch = extract_channel(&audio.samples, 1, 2);
            let mono = to_mono(&audio.samples, audio.channels);
            (mono, Some(left_ch), Some(right_ch))
        }
        (ChannelMode::Both, false) => {
            // Mono file with Both mode - just use the single channel
            (audio.samples.clone(), None, None)
        }
    };

    // Resample main samples to 16kHz
    let mut samples = resample_to_16khz(&main_samples, audio.sample_rate)?;

    // Resample channel-specific samples if present
    let left = if let Some(l) = left {
        Some(resample_to_16khz(&l, audio.sample_rate)?)
    } else {
        None
    };

    let right = if let Some(r) = right {
        Some(resample_to_16khz(&r, audio.sample_rate)?)
    } else {
        None
    };

    // Apply denoising if requested
    if denoise {
        samples = apply_denoise(&samples).context("Denoising failed")?;
    }

    Ok(PreparedAudio {
        samples,
        left,
        right,
        is_stereo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_channel_left() {
        let stereo = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let left = extract_channel(&stereo, 0, 2);
        assert_eq!(left, vec![1.0, 3.0, 5.0]);
    }

    #[test]
    fn test_extract_channel_right() {
        let stereo = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let right = extract_channel(&stereo, 1, 2);
        assert_eq!(right, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_to_mono_stereo() {
        let stereo = vec![1.0, 2.0, 3.0, 4.0];
        let mono = to_mono(&stereo, 2);
        assert_eq!(mono, vec![1.5, 3.5]);
    }

    #[test]
    fn test_to_mono_already_mono() {
        let mono = vec![1.0, 2.0, 3.0];
        let result = to_mono(&mono, 1);
        assert_eq!(result, mono);
    }

    #[test]
    fn test_resample_same_rate() {
        let samples = vec![1.0, 2.0, 3.0, 4.0];
        let result = resample_to_16khz(&samples, 16000).unwrap();
        assert_eq!(result, samples);
    }
}
