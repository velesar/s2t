//! CLI mode for voice-dictation.
//!
//! Provides command-line transcription of WAV files without requiring GTK/display server.

pub mod args;
pub mod denoise_eval;
pub mod transcribe;
pub mod wav_reader;

pub use args::Cli;
pub use args::Commands;
