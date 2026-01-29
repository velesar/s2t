//! CLI transcription command implementation.

use crate::cli::args::{DiarizationMethod, OutputFormat, SttBackend, TranscribeArgs};
use crate::cli::wav_reader::{prepare_for_whisper, read_wav, PreparedAudio};
use crate::config::{load_config, models_dir, sortformer_models_dir, tdt_models_dir, Config};
use crate::diarization::DiarizationEngine;
use crate::models::{get_model_path, list_downloaded_models};
use crate::services::TranscriptionService;
use crate::traits::Transcription;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// JSON output structure for transcription results.
#[derive(Serialize)]
struct TranscriptionOutput {
    version: String,
    input_file: String,
    duration_secs: f64,
    language: String,
    model: String,
    backend: String,
    diarization: String,
    denoise: bool,
    transcription: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    segments: Vec<TranscriptionSegment>,
    metrics: TranscriptionMetrics,
}

#[derive(Serialize)]
struct TranscriptionSegment {
    speaker: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_time: Option<f64>,
}

/// Metrics for transcription performance analysis.
#[derive(Serialize, Default)]
struct TranscriptionMetrics {
    /// Total transcription time in milliseconds
    execution_time_ms: u64,
    /// Input audio length in milliseconds
    audio_duration_ms: u64,
    /// Real-time factor (execution/audio, lower=faster)
    rtf: f64,
    /// Number of words in output
    word_count: usize,
    /// Number of characters in output
    char_count: usize,
    /// Number of speaker segments (from diarization)
    segment_count: usize,
}

/// Run the transcribe command.
pub fn run(args: TranscribeArgs) -> Result<()> {
    // Handle deprecated --diarize flag
    let effective_diarization = if args.diarize && matches!(args.diarization, DiarizationMethod::None) {
        eprintln!("Warning: --diarize is deprecated, use --diarization=channel instead");
        DiarizationMethod::Channel
    } else {
        args.diarization
    };

    // Validate backend + diarization combinations
    if matches!(args.backend, SttBackend::Tdt) && !matches!(effective_diarization, DiarizationMethod::None) {
        bail!("TDT backend does not support diarization. TDT is a pure STT backend without speaker identification. Use --diarization=none with --backend=tdt");
    }

    // 1. Load config (custom path or default)
    let config = load_config_cascade(&args)?;

    // 2. Read WAV file
    eprintln!("Reading: {}", args.input.display());
    let audio = read_wav(&args.input)?;
    eprintln!(
        "  {} channels, {}Hz, {:.1}s",
        audio.channels, audio.sample_rate, audio.duration_secs
    );

    // 3. Prepare audio for transcription
    let denoise = args.denoise || config.denoise_enabled;
    let prepared = prepare_for_whisper(&audio, args.channel, denoise)?;

    let language = args
        .language
        .as_deref()
        .unwrap_or(&config.language);

    // 4. Run transcription based on backend
    let start_time = Instant::now();
    let result = match args.backend {
        SttBackend::Whisper => {
            let model_path = resolve_whisper_model(&args, &config)?;
            eprintln!("Loading Whisper model: {}", model_path.display());
            let service = TranscriptionService::with_model(&model_path.to_string_lossy())?;

            eprintln!(
                "Transcribing (backend: whisper, diarization: {:?}, language: {})...",
                effective_diarization, language
            );
            transcribe_with_whisper(&service, &prepared, language, &args, effective_diarization, &config)?
        }
        SttBackend::Tdt => {
            let model_dir = resolve_tdt_model(&args, &config)?;
            eprintln!("Loading TDT model from: {}", model_dir.display());
            let service = TranscriptionService::with_tdt(&model_dir.to_string_lossy())?;

            eprintln!("Transcribing (backend: tdt, language: {})...", language);
            let text = service.transcribe(&prepared.samples, language)?;
            TranscriptionResult {
                text: text.trim().to_string(),
                segments: Vec::new(),
                model_name: model_dir.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "tdt".to_string()),
            }
        }
    };
    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    // 5. Calculate metrics
    let audio_duration_ms = (audio.duration_secs * 1000.0) as u64;
    let metrics = TranscriptionMetrics {
        execution_time_ms,
        audio_duration_ms,
        rtf: if audio_duration_ms > 0 {
            execution_time_ms as f64 / audio_duration_ms as f64
        } else {
            0.0
        },
        word_count: result.text.split_whitespace().count(),
        char_count: result.text.chars().count(),
        segment_count: result.segments.len(),
    };

    eprintln!(
        "Done in {:.1}s (RTF: {:.2})",
        execution_time_ms as f64 / 1000.0,
        metrics.rtf
    );

    // 6. Output result
    output_result(
        &result,
        &args,
        &audio.duration_secs,
        language,
        effective_diarization,
        denoise,
        &metrics,
    )?;

    Ok(())
}

/// Load config with cascade: custom path -> default path -> defaults.
fn load_config_cascade(args: &TranscribeArgs) -> Result<Config> {
    if let Some(ref custom_path) = args.config {
        // Load from custom path
        let content = fs::read_to_string(custom_path)
            .with_context(|| format!("Failed to read config: {}", custom_path.display()))?;
        toml::from_str(&content).with_context(|| "Failed to parse config")
    } else {
        // Use default config or defaults
        Ok(load_config().unwrap_or_default())
    }
}

/// Resolve Whisper model path from CLI args or config.
fn resolve_whisper_model(args: &TranscribeArgs, config: &Config) -> Result<PathBuf> {
    if let Some(ref model_arg) = args.model {
        let model_path = Path::new(model_arg);

        // If it's an absolute path or relative path that exists, use it directly
        if model_path.is_absolute() && model_path.exists() {
            return Ok(model_path.to_path_buf());
        }

        // If it's a relative path that exists in current dir
        if model_path.exists() {
            return Ok(model_path.to_path_buf());
        }

        // Try as a filename in models directory
        let in_models_dir = get_model_path(model_arg);
        if in_models_dir.exists() {
            return Ok(in_models_dir);
        }

        bail!(
            "Whisper model not found: {}. Tried: {}, {}",
            model_arg,
            model_path.display(),
            in_models_dir.display()
        );
    }

    // Use config default
    let model_path = get_model_path(&config.default_model);
    if model_path.exists() {
        return Ok(model_path);
    }

    // Try to find any downloaded model
    let downloaded = list_downloaded_models();
    if let Some(first) = downloaded.first() {
        let path = get_model_path(&first.filename);
        eprintln!(
            "Warning: configured model '{}' not found, using '{}'",
            config.default_model, first.filename
        );
        return Ok(path);
    }

    bail!(
        "No Whisper model found. Download one using the GUI or place a model in {}",
        models_dir().display()
    );
}

/// Resolve TDT model directory from CLI args or config.
fn resolve_tdt_model(args: &TranscribeArgs, config: &Config) -> Result<PathBuf> {
    // 1. CLI argument takes priority
    if let Some(ref tdt_path) = args.tdt_model {
        if tdt_path.exists() {
            return Ok(tdt_path.clone());
        }
        bail!("TDT model directory not found: {}", tdt_path.display());
    }

    // 2. Config setting
    if let Some(ref tdt_path) = config.tdt_model_path {
        let path = PathBuf::from(tdt_path);
        if path.exists() {
            return Ok(path);
        }
        eprintln!("Warning: configured TDT model '{}' not found", tdt_path);
    }

    // 3. Default location
    let default_dir = tdt_models_dir();
    let encoder_path = default_dir.join("encoder-model.int8.onnx");
    if encoder_path.exists() {
        return Ok(default_dir);
    }

    // Also check for non-quantized version
    let encoder_path_fp = default_dir.join("encoder-model.onnx");
    if encoder_path_fp.exists() {
        return Ok(default_dir);
    }

    bail!(
        "TDT model not found. Place model files (encoder-model.int8.onnx, decoder_joint-model.int8.onnx, vocab.txt) in {}",
        default_dir.display()
    );
}

/// Resolve Sortformer model path from CLI args or config.
fn resolve_sortformer_model(args: &TranscribeArgs, config: &Config) -> Result<PathBuf> {
    // 1. CLI argument takes priority
    if let Some(ref sf_path) = args.sortformer_model {
        if sf_path.exists() {
            return Ok(sf_path.clone());
        }
        bail!("Sortformer model not found: {}", sf_path.display());
    }

    // 2. Config setting
    if let Some(ref sf_path) = config.sortformer_model_path {
        let path = PathBuf::from(sf_path);
        if path.exists() {
            return Ok(path);
        }
        eprintln!("Warning: configured Sortformer model '{}' not found", sf_path);
    }

    // 3. Default location
    let default_dir = sortformer_models_dir();
    let model_path = default_dir.join("diar_streaming_sortformer_4spk-v2.1.onnx");
    if model_path.exists() {
        return Ok(model_path);
    }

    bail!(
        "Sortformer model not found. Place diar_streaming_sortformer_4spk-v2.1.onnx in {}",
        default_dir.display()
    );
}

/// Transcription result with text, segments, and model info.
struct TranscriptionResult {
    text: String,
    segments: Vec<TranscriptionSegment>,
    model_name: String,
}

/// Transcribe audio using Whisper backend with specified diarization method.
fn transcribe_with_whisper(
    service: &TranscriptionService,
    prepared: &PreparedAudio,
    language: &str,
    args: &TranscribeArgs,
    diarization: DiarizationMethod,
    config: &Config,
) -> Result<TranscriptionResult> {
    let whisper = service
        .whisper()
        .context("Whisper model not loaded")?;

    let model_name = args.model.clone().unwrap_or_else(|| config.default_model.clone());

    match diarization {
        DiarizationMethod::None => {
            // No diarization - simple transcription
            let text = whisper.transcribe(&prepared.samples, Some(language))?;
            Ok(TranscriptionResult {
                text: text.trim().to_string(),
                segments: Vec::new(),
                model_name,
            })
        }
        DiarizationMethod::Channel => {
            // Channel-based diarization (stereo: left=mic, right=loopback)
            transcribe_channel_diarization(whisper, prepared, language, model_name)
        }
        DiarizationMethod::Sortformer => {
            // Sortformer neural diarization
            transcribe_sortformer_diarization(whisper, prepared, language, args, config, model_name)
        }
    }
}

/// Channel-based diarization: transcribe left and right channels separately.
fn transcribe_channel_diarization(
    whisper: &crate::whisper::WhisperSTT,
    prepared: &PreparedAudio,
    language: &str,
    model_name: String,
) -> Result<TranscriptionResult> {
    match (&prepared.left, &prepared.right) {
        (Some(left), Some(right)) => {
            let left_text = whisper.transcribe(left, Some(language))?;
            let right_text = whisper.transcribe(right, Some(language))?;

            let mut segments = Vec::new();
            let mut full_text = String::new();

            if !left_text.trim().is_empty() {
                full_text.push_str("[Mic] ");
                full_text.push_str(left_text.trim());
                segments.push(TranscriptionSegment {
                    speaker: "Mic".to_string(),
                    text: left_text.trim().to_string(),
                    start_time: None,
                    end_time: None,
                });
            }

            if !right_text.trim().is_empty() {
                if !full_text.is_empty() {
                    full_text.push('\n');
                }
                full_text.push_str("[Loopback] ");
                full_text.push_str(right_text.trim());
                segments.push(TranscriptionSegment {
                    speaker: "Loopback".to_string(),
                    text: right_text.trim().to_string(),
                    start_time: None,
                    end_time: None,
                });
            }

            Ok(TranscriptionResult {
                text: full_text,
                segments,
                model_name,
            })
        }
        _ => {
            // No stereo channels available, fall back to mixed mono
            let text = whisper.transcribe(&prepared.samples, Some(language))?;
            Ok(TranscriptionResult {
                text: text.trim().to_string(),
                segments: Vec::new(),
                model_name,
            })
        }
    }
}

/// Sortformer neural diarization: use Sortformer to identify speakers, then transcribe segments.
fn transcribe_sortformer_diarization(
    whisper: &crate::whisper::WhisperSTT,
    prepared: &PreparedAudio,
    language: &str,
    args: &TranscribeArgs,
    config: &Config,
    model_name: String,
) -> Result<TranscriptionResult> {
    // Resolve Sortformer model path
    let sortformer_path = resolve_sortformer_model(args, config)?;

    // Initialize and load diarization engine
    let mut engine = DiarizationEngine::new(Some(sortformer_path));
    engine.load_model().context("Failed to load Sortformer model")?;

    if !engine.is_available() {
        bail!("Sortformer diarization not available. Build with --features sortformer");
    }

    // Run diarization to get speaker segments
    let diar_segments = engine.diarize(&prepared.samples)?;

    if diar_segments.is_empty() {
        // No speakers detected, fall back to simple transcription
        let text = whisper.transcribe(&prepared.samples, Some(language))?;
        return Ok(TranscriptionResult {
            text: text.trim().to_string(),
            segments: Vec::new(),
            model_name,
        });
    }

    // Transcribe each speaker segment
    let sample_rate = 16000.0; // Whisper expects 16kHz
    let mut segments = Vec::new();
    let mut full_text = String::new();

    for seg in &diar_segments {
        let start_sample = (seg.start_time * sample_rate) as usize;
        let end_sample = (seg.end_time * sample_rate).min(prepared.samples.len() as f64) as usize;

        if start_sample >= end_sample || end_sample > prepared.samples.len() {
            continue;
        }

        let segment_audio = &prepared.samples[start_sample..end_sample];

        // Skip very short segments (less than 0.5s)
        if segment_audio.len() < (sample_rate * 0.5) as usize {
            continue;
        }

        let text = whisper.transcribe(segment_audio, Some(language))?;
        let text = text.trim();

        if !text.is_empty() {
            let speaker = format!("Speaker {}", seg.speaker_id + 1);

            if !full_text.is_empty() {
                full_text.push('\n');
            }
            full_text.push_str(&format!("[{}] {}", speaker, text));

            segments.push(TranscriptionSegment {
                speaker,
                text: text.to_string(),
                start_time: Some(seg.start_time),
                end_time: Some(seg.end_time),
            });
        }
    }

    Ok(TranscriptionResult {
        text: full_text,
        segments,
        model_name,
    })
}

/// Output result in requested format.
fn output_result(
    result: &TranscriptionResult,
    args: &TranscribeArgs,
    duration_secs: &f64,
    language: &str,
    diarization: DiarizationMethod,
    denoise: bool,
    metrics: &TranscriptionMetrics,
) -> Result<()> {
    let output_text = match args.format {
        OutputFormat::Text => result.text.clone(),
        OutputFormat::Json => {
            let backend_str = match args.backend {
                SttBackend::Whisper => "whisper",
                SttBackend::Tdt => "tdt",
            };
            let diarization_str = match diarization {
                DiarizationMethod::None => "none",
                DiarizationMethod::Channel => "channel",
                DiarizationMethod::Sortformer => "sortformer",
            };

            let output = TranscriptionOutput {
                version: env!("CARGO_PKG_VERSION").to_string(),
                input_file: args.input.to_string_lossy().to_string(),
                duration_secs: *duration_secs,
                language: language.to_string(),
                model: result.model_name.clone(),
                backend: backend_str.to_string(),
                diarization: diarization_str.to_string(),
                denoise,
                transcription: result.text.clone(),
                segments: result
                    .segments
                    .iter()
                    .map(|s| TranscriptionSegment {
                        speaker: s.speaker.clone(),
                        text: s.text.clone(),
                        start_time: s.start_time,
                        end_time: s.end_time,
                    })
                    .collect(),
                metrics: TranscriptionMetrics {
                    execution_time_ms: metrics.execution_time_ms,
                    audio_duration_ms: metrics.audio_duration_ms,
                    rtf: metrics.rtf,
                    word_count: metrics.word_count,
                    char_count: metrics.char_count,
                    segment_count: metrics.segment_count,
                },
            };
            serde_json::to_string_pretty(&output).context("Failed to serialize JSON")?
        }
    };

    // Write to file or stdout
    if let Some(ref output_path) = args.output {
        fs::write(output_path, &output_text)
            .with_context(|| format!("Failed to write output: {}", output_path.display()))?;
        eprintln!("Output written to: {}", output_path.display());
    } else {
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        writeln!(handle, "{}", output_text)?;
    }

    Ok(())
}

/// List available models.
pub fn list_models() -> Result<()> {
    use crate::models::{format_size, get_available_models};

    let available = get_available_models();
    let downloaded = list_downloaded_models();
    let downloaded_names: std::collections::HashSet<_> =
        downloaded.iter().map(|m| &m.filename).collect();

    println!("Available Whisper models:");
    println!();

    for model in &available {
        let status = if downloaded_names.contains(&model.filename) {
            "[downloaded]"
        } else {
            ""
        };

        println!(
            "  {:30} {:>10}  {}",
            model.filename,
            format_size(model.size_bytes),
            status
        );
    }

    println!();
    println!("Models directory: {}", models_dir().display());
    println!();
    println!("Downloaded models: {}", downloaded.len());

    if downloaded.is_empty() {
        println!();
        println!("No models downloaded. Use the GUI to download models,");
        println!("or manually place .bin files in the models directory.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::ChannelMode;

    #[test]
    fn test_load_config_cascade_uses_defaults() {
        let args = TranscribeArgs {
            input: PathBuf::from("test.wav"),
            output: None,
            config: None,
            model: None,
            language: None,
            channel: ChannelMode::Mix,
            diarize: false,
            backend: SttBackend::Whisper,
            diarization: DiarizationMethod::None,
            sortformer_model: None,
            tdt_model: None,
            format: OutputFormat::Text,
            denoise: false,
        };

        let config = load_config_cascade(&args).unwrap();
        assert_eq!(config.language, "uk");
    }

    #[test]
    fn test_metrics_rtf_calculation() {
        let metrics = TranscriptionMetrics {
            execution_time_ms: 5000,
            audio_duration_ms: 10000,
            rtf: 0.5,
            word_count: 100,
            char_count: 500,
            segment_count: 2,
        };
        assert!((metrics.rtf - 0.5).abs() < 0.001);
    }
}
