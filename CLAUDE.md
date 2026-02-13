# Voice Dictation - Development Guide

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
- **System Tray:** ksni 0.3
- **Hotkeys:** global-hotkey 0.5
- **HTTP:** reqwest 0.12 (model downloads)

## Quick Commands

```bash
# Build (includes Sortformer diarization by default)
cargo build --release

# Run
cargo run --release

# Test
cargo test

# Lint
cargo clippy

# Install locally
./install.sh
```

## Architecture

```
src/
├── main.rs                   # Entry point, GUI init, hotkey polling (327 LOC)
│
├── domain/                   # Core contracts
│   ├── traits.rs             # 7 traits: AudioRecording, Transcription, VoiceDetection,
│   │                         #   HistoryRepository, AudioDenoising, ConfigProvider, UIStateUpdater
│   └── types.rs              # AppState, AudioSegment, ConferenceRecording, SharedHistory
│
├── app/                      # Application orchestration
│   ├── context.rs            # AppContext (DI container: audio, transcription, config, history, diarization, channels)
│   ├── channels.rs           # UIChannels (5 async channels: models, history, settings, recording, hotkeys)
│   └── config.rs             # Config (18 fields) + save/load + directory paths
│
├── recording/                # Audio capture (8 files)
│   ├── microphone.rs         # AudioRecorder (CPAL mic input + Rubato resampling)
│   ├── loopback.rs           # LoopbackRecorder (parec system audio capture)
│   ├── conference.rs         # ConferenceRecorder (mic + loopback combined)
│   ├── core.rs               # RecordingCore (shared recorder boilerplate)
│   ├── segmentation.rs       # SegmentationMonitor (VAD-based audio chunking)
│   ├── ring_buffer.rs        # Circular buffer for streaming (30 sec at 16kHz)
│   ├── denoise.rs            # NnnoiselessDenoiser (RNNoise 48kHz with resampling)
│   └── service.rs            # AudioService (facade for all recorders)
│
├── transcription/            # Speech-to-text (4 files)
│   ├── whisper.rs            # WhisperSTT (whisper.cpp bindings)
│   ├── tdt.rs                # ParakeetSTT (NVIDIA TDT ONNX backend)
│   ├── service.rs            # TranscriptionService (Whisper/TDT backend abstraction)
│   └── diarization.rs        # DiarizationEngine (Sortformer speaker identification)
│
├── infrastructure/           # External system adapters (5 files)
│   ├── hotkeys.rs            # Global hotkey registration (global-hotkey crate)
│   ├── tray.rs               # System tray (ksni StatusNotifierItem)
│   ├── paste.rs              # Auto-paste (xdotool key ctrl+v)
│   ├── recordings.rs         # WAV file storage (conference recordings)
│   └── models.rs             # Model catalog, download, management (Whisper/TDT/Sortformer)
│
├── ui/                       # GTK user interface (7 files)
│   ├── mod.rs                # Window setup, build_ui(), tray event loop
│   ├── state.rs              # UIContext, RecordingContext, ModeUIs
│   ├── dispatch.rs           # Recording mode routing (dictation/conference/continuous)
│   ├── mic.rs                # Dictation mode handler (record -> transcribe -> output)
│   ├── conference.rs         # Conference mode handler (mic + loopback -> diarize)
│   ├── conference_file.rs    # Conference file mode (record-only)
│   └── widgets.rs            # Common widget builders
│
├── dialogs/                  # Dialog windows (8 files)
│   ├── settings.rs           # Settings dialog (language, backend, VAD, hotkeys)
│   ├── model/                # Model management dialog
│   │   ├── mod.rs            # Dialog entry point
│   │   ├── download.rs       # Download progress UI
│   │   └── list.rs           # Model list rows
│   └── history/              # History browser dialog
│       ├── mod.rs            # Dialog entry point
│       ├── list.rs           # History list rows
│       └── export.rs         # Export to text
│
├── vad/                      # Voice activity detection (3 files)
│   ├── mod.rs                # VAD factory and configuration
│   ├── webrtc.rs             # WebRTC VAD (energy-based, fast)
│   └── silero.rs             # Silero VAD (neural network, accurate)
│
├── history/                  # Transcription history (4 files)
│   ├── mod.rs                # History struct, search, cleanup, HistoryRepository impl
│   ├── entry.rs              # HistoryEntry struct
│   ├── persistence.rs        # JSON load/save
│   └── export.rs             # Export to text format
│
├── cli/                      # CLI interface (4 files)
│   ├── args.rs               # Clap argument definitions
│   ├── transcribe.rs         # CLI transcription pipeline (WAV -> STT -> output)
│   ├── denoise_eval.rs       # Denoiser evaluation tool
│   └── wav_reader.rs         # WAV file parsing utilities
│
└── test_support/             # Test infrastructure
    └── mocks.rs              # 6 mock implementations for domain traits (592 LOC)
```

**Codebase size:** 57 files, 10,929 LOC, 1,246 symbols, 152 unit tests

## Key Patterns

### Shared State
```rust
// Thread-safe shared state via Arc<Mutex<T>>
let config: Arc<Mutex<Config>> = Arc::new(Mutex::new(load_config()?));
```

### Async Communication
```rust
// Inter-component messaging via async channels
let (tx, rx) = async_channel::unbounded::<AudioSegment>();
```

### Error Handling
```rust
// Use anyhow with context
fs::read_to_string(&path)
    .with_context(|| format!("Failed to read: {}", path.display()))?;
```

## MCP Tools Available

### codegraph - Code Navigation
Use for exploring code structure and finding references:
- `find_symbol` - Search symbols by name
- `get_symbol_info` - Get symbol details
- `get_callers` / `get_callees` - Find references
- `get_file_symbols` - List symbols in a file
- `get_module_deps` - Module dependency analysis
- `find_hotspot_symbols` - Find highly-referenced code

### mental-model - Project Knowledge
Use for accessing audit findings and architecture info:
- `get_model_section` - Get architecture/structure info
- `get_findings` - View known issues to fix
- `get_context` - Get file's architectural context

## Development Guidelines

### Code Quality Standards
1. **No clippy warnings** - Run `cargo clippy` before committing
2. **Keep functions focused** - Max 7 parameters, extract structs if needed
3. **Module size** - Prefer files under 500 lines
4. **Error handling** - Use `anyhow` with `.context()` for all fallible operations

### When Adding Features
1. Check existing patterns in similar modules
2. Use `codegraph` to find related code
3. Keep changes minimal and focused
4. Add tests for new functionality
5. Run `cargo clippy` and `cargo test`

### Known Technical Debt
Remaining items (P0–P2 remediation is complete):
- **P3:** Dead code cleanup, async model loading, doc comments, resampler quality
- CI/CD pipeline and packaging (Flatpak, AppImage, RPM)

## File Locations

| Data | Path |
|------|------|
| Config | `~/.config/voice-dictation/config.toml` |
| History | `~/.local/share/voice-dictation/history.json` |
| Models | `~/.local/share/whisper/` |
| Recordings | `~/.local/share/voice-dictation/recordings/` |

## External Dependencies

Runtime tools (not in Cargo.toml):
- `xdotool` - Auto-paste feature (optional)
- `pactl`, `parec` - System audio capture for conference mode
