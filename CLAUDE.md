# Voice Dictation - Development Guide

## Project Overview

**Voice Dictation (s2t)** is a Rust GTK4 desktop application for offline speech-to-text transcription on Linux using Whisper.

## Tech Stack

- **Language:** Rust 2021 Edition
- **GUI:** GTK4 0.9 + glib/gio 0.20
- **Audio:** CPAL 0.15 (capture) + Rubato 0.16 (resampling)
- **Speech Recognition:** whisper-rs 0.12 (whisper.cpp bindings)
- **Async:** Tokio 1.x + async-channel
- **System Tray:** ksni 0.2

## Quick Commands

```bash
# Build
cargo build --release

# Run
cargo run --release

# Run with Sortformer diarization (requires rustc 1.88+)
cargo build --release --features sortformer

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
├── main.rs           # Entry point, app orchestration
├── ui.rs             # Main UI (needs refactoring - see docs/audit/)
├── config.rs         # TOML configuration
├── audio.rs          # Microphone recording
├── whisper.rs        # Whisper STT integration
├── history.rs        # Transcription history
├── continuous.rs     # Continuous recording mode
├── vad.rs            # Voice activity detection
└── ...               # See full structure in docs/audit/
```

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
See `docs/audit/RECOMMENDATIONS.md` for prioritized improvements:
- **P1:** Context structs for UI functions, Arc/Rc fix in vad.rs
- **P2:** Split ui.rs into smaller modules
- **P3:** CI/CD pipeline, integration tests

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
