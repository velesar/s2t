//! CLI argument definitions using clap.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// STT backend selection.
#[derive(Clone, Copy, ValueEnum, Default, Debug)]
pub enum SttBackend {
    /// Whisper backend (default)
    #[default]
    Whisper,
    /// Parakeet TDT backend
    Tdt,
}

/// Diarization method selection.
#[derive(Clone, Copy, ValueEnum, Default, Debug)]
pub enum DiarizationMethod {
    /// No diarization (default)
    #[default]
    None,
    /// Channel-based diarization (stereo: left=mic, right=loopback)
    Channel,
    /// Sortformer neural diarization
    Sortformer,
}

/// Voice Dictation - Offline speech-to-text for Linux
#[derive(Parser)]
#[command(name = "voice-dictation")]
#[command(about = "Offline speech-to-text transcription using Whisper", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Transcribe a WAV file to text
    Transcribe(TranscribeArgs),
    /// List available and downloaded models
    Models,
    /// Evaluate denoiser effectiveness on a WAV file
    DenoiseEval(DenoiseEvalArgs),
}

#[derive(Parser)]
pub struct TranscribeArgs {
    /// Path to WAV file to transcribe
    pub input: PathBuf,

    /// Output file (stdout if omitted)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Config file path (default: ~/.config/voice-dictation/config.toml)
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Override model path or filename
    #[arg(short, long)]
    pub model: Option<String>,

    /// Override language (uk, en, auto, etc.)
    #[arg(short, long)]
    pub language: Option<String>,

    /// Channel handling mode for stereo files
    #[arg(long, value_enum, default_value_t = ChannelMode::Mix)]
    pub channel: ChannelMode,

    /// [DEPRECATED] Use --diarization=channel instead
    #[arg(long, hide = true)]
    pub diarize: bool,

    /// STT backend (whisper or tdt)
    #[arg(long, value_enum, default_value_t = SttBackend::Whisper)]
    pub backend: SttBackend,

    /// Diarization method (none, channel, sortformer)
    #[arg(long, value_enum, default_value_t = DiarizationMethod::None)]
    pub diarization: DiarizationMethod,

    /// Path to Sortformer model (optional, uses default location if not specified)
    #[arg(long)]
    pub sortformer_model: Option<PathBuf>,

    /// Path to TDT model directory (optional, uses default location if not specified)
    #[arg(long)]
    pub tdt_model: Option<PathBuf>,

    /// Output format
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// Enable noise suppression
    #[arg(long)]
    pub denoise: bool,
}

#[derive(Clone, Copy, ValueEnum, Default)]
pub enum ChannelMode {
    /// Mix both channels to mono (default)
    #[default]
    Mix,
    /// Use left channel only (typically microphone)
    Left,
    /// Use right channel only (typically loopback)
    Right,
    /// Transcribe each channel separately with speaker labels
    Both,
}

#[derive(Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    /// Plain text output
    #[default]
    Text,
    /// JSON output with metadata
    Json,
}

#[derive(Parser)]
pub struct DenoiseEvalArgs {
    /// Path to WAV file to evaluate
    pub input: PathBuf,

    /// Channel to evaluate (for stereo files)
    #[arg(long, value_enum, default_value_t = ChannelMode::Mix)]
    pub channel: ChannelMode,

    /// Output directory for denoised WAV file (default: same directory as input)
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,

    /// Run VAD comparison (WebRTC + Silero speech % on original vs denoised)
    #[arg(long)]
    pub vad: bool,

    /// Run transcription A/B comparison (requires a loaded model)
    #[arg(long)]
    pub transcribe: bool,

    /// Override model path or filename (for --transcribe)
    #[arg(short, long)]
    pub model: Option<String>,

    /// Override language (for --transcribe)
    #[arg(short, long)]
    pub language: Option<String>,

    /// Config file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}
