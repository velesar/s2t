//! Denoise evaluation CLI command.
//!
//! Compares original vs denoised audio using signal metrics, VAD analysis,
//! and optional transcription A/B testing.

use crate::cli::args::{ChannelMode, DenoiseEvalArgs};
use crate::cli::wav_reader::{prepare_for_whisper, read_wav};
use crate::recording::denoise::NnnoiselessDenoiser;
use anyhow::{Context, Result};
use serde::Serialize;
use std::io::Write;
use std::path::Path;

/// Signal-level metrics comparing original and denoised audio.
#[derive(Serialize)]
struct SignalMetrics {
    original_rms: f32,
    denoised_rms: f32,
    rms_reduction_pct: f32,
    original_peak: f32,
    denoised_peak: f32,
    length_diff_samples: i64,
}

/// VAD results for a single engine.
#[derive(Serialize)]
struct VadResult {
    original_speech_pct: f32,
    denoised_speech_pct: f32,
}

/// VAD comparison across engines.
#[derive(Serialize)]
struct VadMetrics {
    webrtc: VadResult,
    silero: VadResult,
}

/// Transcription A/B comparison.
#[derive(Serialize)]
struct TranscriptionMetrics {
    original_text: String,
    denoised_text: String,
    original_words: usize,
    denoised_words: usize,
}

/// Full evaluation report.
#[derive(Serialize)]
struct DenoiseReport {
    input_file: String,
    channel: String,
    duration_secs: f64,
    signal: SignalMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    vad: Option<VadMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transcription: Option<TranscriptionMetrics>,
}

/// Run the denoise-eval command.
pub fn run(args: DenoiseEvalArgs) -> Result<()> {
    // 1. Read and prepare audio
    eprintln!("Reading: {}", args.input.display());
    let audio = read_wav(&args.input)?;
    eprintln!(
        "  {} channels, {}Hz, {:.1}s",
        audio.channels, audio.sample_rate, audio.duration_secs
    );

    let channel_name = match args.channel {
        ChannelMode::Mix => "mix",
        ChannelMode::Left => "left",
        ChannelMode::Right => "right",
        ChannelMode::Both => "mix", // For eval, "both" doesn't make sense; treat as mix
    };

    // Prepare without denoising to get original 16kHz samples
    let prepared = prepare_for_whisper(&audio, args.channel, false)?;
    let original = &prepared.samples;

    // 2. Denoise
    eprintln!("Denoising...");
    let denoiser = NnnoiselessDenoiser::new();
    let denoised = denoiser
        .denoise_buffer(original)
        .context("Denoising failed")?;

    // 3. Signal metrics
    let signal = compute_signal_metrics(original, &denoised);
    eprintln!(
        "  RMS: {:.4} -> {:.4} ({:.1}% reduction)",
        signal.original_rms, signal.denoised_rms, signal.rms_reduction_pct
    );
    eprintln!(
        "  Peak: {:.4} -> {:.4}",
        signal.original_peak, signal.denoised_peak
    );
    eprintln!(
        "  Length diff: {} samples",
        signal.length_diff_samples
    );

    // 4. Write denoised WAV
    let output_dir = args
        .output_dir
        .as_deref()
        .unwrap_or_else(|| args.input.parent().unwrap_or(Path::new(".")));
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;

    let stem = args
        .input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "audio".to_string());
    let output_path = output_dir.join(format!("{}_{}_denoised.wav", stem, channel_name));
    write_wav_mono_f32(&output_path, &denoised, 16000)?;
    eprintln!("Wrote: {}", output_path.display());

    // 5. Optional VAD comparison
    let vad_metrics = if args.vad {
        eprintln!("Running VAD comparison...");
        Some(compute_vad_metrics(original, &denoised)?)
    } else {
        None
    };

    // 6. Optional transcription A/B
    let transcription_metrics = if args.transcribe {
        eprintln!("Running transcription A/B...");
        Some(compute_transcription_metrics(original, &denoised, &args)?)
    } else {
        None
    };

    // 7. Build and output JSON report
    let report = DenoiseReport {
        input_file: args.input.to_string_lossy().to_string(),
        channel: channel_name.to_string(),
        duration_secs: audio.duration_secs,
        signal,
        vad: vad_metrics,
        transcription: transcription_metrics,
    };

    let json = serde_json::to_string_pretty(&report).context("Failed to serialize report")?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{}", json)?;

    Ok(())
}

/// Compute RMS and peak for a buffer of f32 samples.
fn compute_signal_metrics(original: &[f32], denoised: &[f32]) -> SignalMetrics {
    let original_rms = rms(original);
    let denoised_rms = rms(denoised);
    let rms_reduction_pct = if original_rms > 0.0 {
        (1.0 - denoised_rms / original_rms) * 100.0
    } else {
        0.0
    };

    SignalMetrics {
        original_rms,
        denoised_rms,
        rms_reduction_pct,
        original_peak: peak(original),
        denoised_peak: peak(denoised),
        length_diff_samples: denoised.len() as i64 - original.len() as i64,
    }
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum_sq / samples.len() as f64).sqrt() as f32
}

fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max)
}

/// Compute VAD speech percentage for both WebRTC and Silero on original and denoised audio.
fn compute_vad_metrics(original: &[f32], denoised: &[f32]) -> Result<VadMetrics> {
    use crate::vad::{SileroVoiceDetector, WebRtcVoiceDetector};

    // WebRTC VAD
    let webrtc_orig = speech_percentage::<WebRtcVoiceDetector>(original, 480)?;
    let webrtc_den = speech_percentage::<WebRtcVoiceDetector>(denoised, 480)?;

    // Silero VAD
    let silero_orig = speech_percentage::<SileroVoiceDetector>(original, 512)?;
    let silero_den = speech_percentage::<SileroVoiceDetector>(denoised, 512)?;

    eprintln!(
        "  WebRTC: {:.1}% -> {:.1}%",
        webrtc_orig, webrtc_den
    );
    eprintln!(
        "  Silero: {:.1}% -> {:.1}%",
        silero_orig, silero_den
    );

    Ok(VadMetrics {
        webrtc: VadResult {
            original_speech_pct: webrtc_orig,
            denoised_speech_pct: webrtc_den,
        },
        silero: VadResult {
            original_speech_pct: silero_orig,
            denoised_speech_pct: silero_den,
        },
    })
}

/// Trait for creating VAD instances generically.
trait DefaultVad: crate::domain::traits::VoiceDetection + Sized {
    fn create() -> Result<Self>;
}

impl DefaultVad for crate::vad::WebRtcVoiceDetector {
    fn create() -> Result<Self> {
        Self::new()
    }
}

impl DefaultVad for crate::vad::SileroVoiceDetector {
    fn create() -> Result<Self> {
        Self::new()
    }
}

/// Count speech frames as percentage of total frames.
fn speech_percentage<V: DefaultVad>(samples: &[f32], frame_size: usize) -> Result<f32> {
    let vad = V::create()?;
    let mut speech_frames = 0u32;
    let mut total_frames = 0u32;

    for chunk in samples.chunks(frame_size) {
        if chunk.len() < frame_size {
            break;
        }
        total_frames += 1;
        if vad.is_speech(chunk)? {
            speech_frames += 1;
        }
    }

    if total_frames == 0 {
        return Ok(0.0);
    }
    Ok(speech_frames as f32 / total_frames as f32 * 100.0)
}

/// Run transcription on both original and denoised audio, return comparison.
fn compute_transcription_metrics(
    original: &[f32],
    denoised: &[f32],
    args: &DenoiseEvalArgs,
) -> Result<TranscriptionMetrics> {
    use crate::app::config::{load_config, Config};
    use crate::domain::traits::Transcription;
    use crate::infrastructure::models::{get_model_path, list_downloaded_models};
    use crate::transcription::TranscriptionService;

    // Load config
    let config = if let Some(ref custom_path) = args.config {
        let content = std::fs::read_to_string(custom_path)
            .with_context(|| format!("Failed to read config: {}", custom_path.display()))?;
        toml::from_str(&content).context("Failed to parse config")?
    } else {
        load_config().unwrap_or_else(|_| Config::default())
    };

    // Resolve model
    let model_path = if let Some(ref model_arg) = args.model {
        let p = std::path::PathBuf::from(model_arg);
        if p.exists() {
            p
        } else {
            let in_models = get_model_path(model_arg);
            if in_models.exists() {
                in_models
            } else {
                anyhow::bail!("Model not found: {}", model_arg);
            }
        }
    } else {
        let p = get_model_path(&config.default_model);
        if p.exists() {
            p
        } else {
            let downloaded = list_downloaded_models();
            if let Some(first) = downloaded.first() {
                get_model_path(&first.filename)
            } else {
                anyhow::bail!("No model found. Use --model to specify one.");
            }
        }
    };

    eprintln!("  Loading model: {}", model_path.display());
    let service = TranscriptionService::with_model(&model_path.to_string_lossy())?;

    let language = args
        .language
        .as_deref()
        .unwrap_or(&config.language);

    eprintln!("  Transcribing original...");
    let original_text = service.transcribe(original, language)?;
    let original_text = original_text.trim().to_string();

    eprintln!("  Transcribing denoised...");
    let denoised_text = service.transcribe(denoised, language)?;
    let denoised_text = denoised_text.trim().to_string();

    let original_words = original_text.split_whitespace().count();
    let denoised_words = denoised_text.split_whitespace().count();

    eprintln!(
        "  Words: {} (original) vs {} (denoised)",
        original_words, denoised_words
    );

    Ok(TranscriptionMetrics {
        original_text,
        denoised_text,
        original_words,
        denoised_words,
    })
}

/// Write mono 16kHz f32 samples to a WAV file.
fn write_wav_mono_f32(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("Failed to create WAV file: {}", path.display()))?;

    for &sample in samples {
        writer
            .write_sample(sample)
            .context("Failed to write WAV sample")?;
    }

    writer.finalize().context("Failed to finalize WAV file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_silence() {
        let silence = vec![0.0f32; 1000];
        assert_eq!(rms(&silence), 0.0);
    }

    #[test]
    fn test_rms_constant() {
        let constant = vec![0.5f32; 1000];
        let result = rms(&constant);
        assert!((result - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_rms_empty() {
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn test_peak_silence() {
        let silence = vec![0.0f32; 1000];
        assert_eq!(peak(&silence), 0.0);
    }

    #[test]
    fn test_peak_known() {
        let samples = vec![0.1, -0.5, 0.3, -0.2];
        assert!((peak(&samples) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_signal_metrics_identical() {
        let samples = vec![0.1f32; 100];
        let metrics = compute_signal_metrics(&samples, &samples);
        assert!((metrics.rms_reduction_pct).abs() < 0.01);
        assert_eq!(metrics.length_diff_samples, 0);
    }

    #[test]
    fn test_signal_metrics_quieter() {
        let original = vec![0.5f32; 100];
        let denoised = vec![0.25f32; 100];
        let metrics = compute_signal_metrics(&original, &denoised);
        assert!((metrics.rms_reduction_pct - 50.0).abs() < 0.1);
    }
}
