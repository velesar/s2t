//! CLI argument definitions using clap.

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

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

    /// Enable speaker labels [Mic]/[Loopback] (requires --channel=both)
    #[arg(long)]
    pub diarize: bool,

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
