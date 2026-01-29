//! CLI transcription command implementation.

use crate::cli::args::{ChannelMode, OutputFormat, TranscribeArgs};
use crate::cli::wav_reader::{prepare_for_whisper, read_wav, PreparedAudio};
use crate::config::{load_config, models_dir, Config};
use crate::models::{get_model_path, list_downloaded_models};
use crate::services::TranscriptionService;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// JSON output structure for transcription results.
#[derive(Serialize)]
struct TranscriptionOutput {
    version: String,
    input_file: String,
    duration_secs: f64,
    language: String,
    model: String,
    transcription: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    segments: Vec<TranscriptionSegment>,
}

#[derive(Serialize)]
struct TranscriptionSegment {
    speaker: String,
    text: String,
}

/// Run the transcribe command.
pub fn run(args: TranscribeArgs) -> Result<()> {
    // 1. Load config (custom path or default)
    let config = load_config_cascade(&args)?;

    // 2. Resolve model path
    let model_path = resolve_model(&args, &config)?;

    // 3. Read WAV file
    eprintln!("Reading: {}", args.input.display());
    let audio = read_wav(&args.input)?;
    eprintln!(
        "  {} channels, {}Hz, {:.1}s",
        audio.channels, audio.sample_rate, audio.duration_secs
    );

    // 4. Prepare audio for Whisper
    let denoise = args.denoise || config.denoise_enabled;
    let prepared = prepare_for_whisper(&audio, args.channel, denoise)?;

    // 5. Load model and transcribe
    eprintln!("Loading model: {}", model_path.display());
    let service = TranscriptionService::with_model(&model_path.to_string_lossy())?;

    let language = args
        .language
        .as_deref()
        .unwrap_or(&config.language);

    eprintln!("Transcribing (language: {})...", language);
    let result = transcribe_audio(&service, &prepared, language, &args)?;

    // 6. Output result
    output_result(
        &result,
        &args,
        &audio.duration_secs,
        language,
        &model_path,
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

/// Resolve model path from CLI args or config.
fn resolve_model(args: &TranscribeArgs, config: &Config) -> Result<PathBuf> {
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
            "Model not found: {}. Tried: {}, {}",
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

/// Transcribe audio based on channel mode and diarization settings.
fn transcribe_audio(
    service: &TranscriptionService,
    prepared: &PreparedAudio,
    language: &str,
    args: &TranscribeArgs,
) -> Result<TranscriptionResult> {
    let whisper = service
        .whisper()
        .context("Model not loaded")?;

    match (args.channel, args.diarize, &prepared.left, &prepared.right) {
        // Both channels with diarization
        (ChannelMode::Both, true, Some(left), Some(right)) => {
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
                });
            }

            Ok(TranscriptionResult {
                text: full_text,
                segments,
            })
        }

        // Both channels without diarization - transcribe mixed
        (ChannelMode::Both, false, _, _) => {
            let text = whisper.transcribe(&prepared.samples, Some(language))?;
            Ok(TranscriptionResult {
                text: text.trim().to_string(),
                segments: Vec::new(),
            })
        }

        // Single channel modes
        _ => {
            let text = whisper.transcribe(&prepared.samples, Some(language))?;
            Ok(TranscriptionResult {
                text: text.trim().to_string(),
                segments: Vec::new(),
            })
        }
    }
}

struct TranscriptionResult {
    text: String,
    segments: Vec<TranscriptionSegment>,
}

/// Output result in requested format.
fn output_result(
    result: &TranscriptionResult,
    args: &TranscribeArgs,
    duration_secs: &f64,
    language: &str,
    model_path: &Path,
) -> Result<()> {
    let output_text = match args.format {
        OutputFormat::Text => result.text.clone(),
        OutputFormat::Json => {
            let output = TranscriptionOutput {
                version: env!("CARGO_PKG_VERSION").to_string(),
                input_file: args.input.to_string_lossy().to_string(),
                duration_secs: *duration_secs,
                language: language.to_string(),
                model: model_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                transcription: result.text.clone(),
                segments: result
                    .segments
                    .iter()
                    .map(|s| TranscriptionSegment {
                        speaker: s.speaker.clone(),
                        text: s.text.clone(),
                    })
                    .collect(),
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
            format: OutputFormat::Text,
            denoise: false,
        };

        let config = load_config_cascade(&args).unwrap();
        assert_eq!(config.language, "uk");
    }
}
