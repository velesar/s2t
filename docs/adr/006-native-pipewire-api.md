# ADR-006: Native PipeWire API for System Audio Capture

## Status
Proposed (Deferred Implementation)

## Context
The application captures system audio (loopback) for conference recording mode.
Currently uses PulseAudio CLI tools (`pactl`, `parec`) which work via PipeWire's
compatibility layer on modern Linux systems.

The `pipewire 0.9` Rust crate is already in dependencies but unused.
Investigation was done to evaluate native PipeWire API vs current approach.

### Current Implementation (loopback.rs)
- Uses `pactl list sources short` to find `.monitor` sources
- Uses `parec --format=s16le --rate=16000 --channels=1` to capture
- ~140 lines, works on both PipeWire and PulseAudio systems
- Runtime dependency: `pulseaudio-utils` package

### Native PipeWire Findings

**Challenges discovered:**

1. **No direct monitor sources** - PipeWire architecture differs from PulseAudio:
   - Requires loopback module setup (`libpipewire-module-loopback`)
   - Or user configuration via `pw-loopback` / routing tools
   - Source discovery still may need pactl fallback

2. **Callback-driven architecture** - Different from polling model:
   - Requires dedicated MainLoop thread
   - PipeWire calls your callbacks (inverted control)
   - Realtime constraints on process callbacks

3. **Thread safety** - PipeWire objects are NOT `Send`/`Sync`:
   - All operations must happen on MainLoop thread
   - Need lock-free channels for cross-thread communication

4. **Industry reality** - Many applications still use pactl/parec via
   compatibility layer even on PipeWire systems

## Decision
Defer native PipeWire implementation. Keep current pactl/parec approach.

### Rationale
- Current implementation works reliably on both PipeWire and PulseAudio
- Native implementation would add ~400 lines with significant complexity
- Loopback/monitor capture in native PipeWire is non-trivial
- Benefit (removing pulseaudio-utils dependency) doesn't justify effort

## Future Implementation Plan

When/if implementing native PipeWire:

### Prerequisites
- PipeWire loopback APIs stabilize
- Clear pattern emerges for monitor source discovery

### Architecture

```
Main Thread (GTK)
    ↓
    ← async_channel →
    ↓
PipeWire Thread:
├── MainLoop + Context + Core
├── Registry (source enumeration)
├── Stream (audio capture)
│   └── process callback → channel.send(samples)
└── MainLoop::run()
```

### Implementation Steps

1. **Thread setup**: Dedicated thread with PipeWire MainLoop
2. **Registry enumeration**: Find audio sinks/sources
3. **Loopback discovery**: Either:
   - Use pw-loopback module programmatically
   - Require user to configure loopback externally
   - Fall back to pactl for source name
4. **Stream creation**: Configure format (F32 preferred)
5. **Buffer processing**: Dequeue in callback, send via channel
6. **Graceful shutdown**: Stop stream, exit MainLoop

### Estimated Effort
- ~400-500 lines of code
- Medium complexity threading
- Testing on multiple systems required

## Consequences

### Positive
- Simple, working implementation retained
- No additional complexity in codebase
- Clear documentation for future work

### Negative
- Runtime dependency on `pulseaudio-utils` continues
- Indirect PipeWire access via compatibility layer
