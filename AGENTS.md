# AGENTS.md — Voice Dictation (s2t)

## Project Overview

**Voice Dictation (s2t)** is a Rust GTK4 desktop application for offline speech-to-text transcription on Linux using Whisper.

## Tech Stack

- **Language:** Rust 2021 Edition (1.93.0)
- **GUI:** GTK4 0.9 + glib/gio 0.20
- **Audio:** CPAL 0.15 (capture) + Rubato 0.16 (resampling) + nnnoiseless 0.5 (denoise)
- **Speech Recognition:** whisper-rs 0.12 (whisper.cpp) + parakeet-rs 0.2 (NVIDIA TDT)
- **Diarization:** parakeet-rs 0.2 (Sortformer speaker identification)
- **VAD:** webrtc-vad 0.4 + voice_activity_detector 0.2.1 (Silero)
- **Async:** Tokio 1.x + async-channel 2.3

## Quick Commands

```bash
cargo build --release
cargo run --release
cargo test
cargo clippy -- -D warnings
cargo fmt --all
```

## Architecture

```
src/
├── main.rs              # Entry point, GUI init
├── domain/              # Core contracts (traits, types)
├── app/                 # DI container, config, channels
├── recording/           # Audio capture (mic, loopback, conference)
├── transcription/       # STT backends (Whisper, TDT, diarization)
├── infrastructure/      # System adapters (hotkeys, tray, paste, models)
├── ui/                  # GTK interface
├── dialogs/             # Settings, model management, history
├── vad/                 # Voice activity detection
├── history/             # Transcription history
├── cli/                 # CLI interface
└── test_support/        # Mock implementations
```

## Key Patterns

- Thread-safe shared state: `Arc<Mutex<T>>`
- Inter-component messaging: `async_channel`
- Error handling: `anyhow` with `.context()`
- DI via `AppContext` struct

## Development Guidelines

1. No clippy warnings — run `cargo clippy -- -D warnings`
2. Keep functions focused, max 7 parameters
3. Module files under 500 lines
4. Use `anyhow` with `.context()` for all fallible operations
5. Run `pre-commit run --all-files` before committing

## Git Conventions

- Commit messages: imperative mood, concise first line (<72 chars)
- Branch naming: `feature/`, `fix/`, `chore/`, `refactor/` prefixes
- Never force-push to main/master

## File Locations

| Data | Path |
|------|------|
| Config | `~/.config/voice-dictation/config.toml` |
| History | `~/.local/share/voice-dictation/history.json` |
| Models | `~/.local/share/whisper/` |
| Recordings | `~/.local/share/voice-dictation/recordings/` |
