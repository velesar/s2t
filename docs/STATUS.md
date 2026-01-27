# Project Status

## Overview
Voice dictation application for Linux with Whisper-based speech-to-text.

## Implementation Status

### Completed Features

| Feature | Status | Files | Notes |
|---------|--------|-------|-------|
| Core dictation | ✅ Done | `src/audio.rs`, `src/whisper.rs`, `src/ui.rs` | Basic recording and transcription |
| Model management | ✅ Done | `src/models.rs`, `src/model_dialog.rs` | Download and select Whisper models |
| History | ✅ Done | `src/history.rs`, `src/history_dialog.rs` | Save and browse transcription history |
| System tray | ✅ Done | `src/tray.rs` | Tray icon with menu |
| Settings dialog | ✅ Done | `src/settings_dialog.rs` | GUI for all settings |
| Auto-copy | ✅ Done | `src/ui.rs`, `src/config.rs` | Copy result to clipboard automatically |
| Global hotkeys | ✅ Done | `src/hotkeys.rs` | Toggle recording from anywhere |
| Config persistence | ✅ Done | `src/config.rs` | TOML config file |

### In Progress

| Feature | Status | ADR | Backlog |
|---------|--------|-----|---------|
| - | - | - | - |

### Planned Features

| Feature | Priority | ADR | Backlog | Notes |
|---------|----------|-----|---------|-------|
| STT optimization | P2 | [ADR-005](adr/005-stt-alternatives-optimization.md) | - | Quantized models for speed |
| Conference recording | P3 | [ADR-003](adr/003-loopback-recording-approach.md), [ADR-004](adr/004-speaker-diarization-approach.md) | [backlog](backlog/conference-recording.md) | Requires loopback + diarization research |

## Architecture Decision Records (ADRs)

| ADR | Title | Status |
|-----|-------|--------|
| [001](adr/001-global-hotkey-approach.md) | Global Hotkey Approach | Accepted |
| [002](adr/002-config-format.md) | Config Format (TOML) | Accepted |
| [003](adr/003-loopback-recording-approach.md) | Loopback Recording | Proposed |
| [004](adr/004-speaker-diarization-approach.md) | Speaker Diarization | Proposed |
| [005](adr/005-stt-alternatives-optimization.md) | STT Alternatives | Proposed |

## Research Tasks

| Research | Status | Doc |
|----------|--------|-----|
| Loopback recording on Linux | Not started | [research](research/loopback-recording-test.md) |
| Speaker diarization | Not started | [research](research/speaker-diarization-test.md) |
| STT optimization benchmarks | Not started | [research](research/stt-optimization-test.md) |

## Known Issues

1. **GTK4 window positioning** - Cannot set window position on Wayland (by design)

## Build Status

```bash
cargo build --release
```

Last verified: 2026-01-27
