# ADR-006: Native PipeWire API for System Audio Capture

## Status
**Accepted** (Implemented 2026-02-13)

## Context
The application captures system audio (loopback) for conference recording mode.
Previously used PulseAudio CLI tools (`pactl`, `parec`) which worked via PipeWire's
compatibility layer on modern Linux systems.

The `pipewire 0.9` Rust crate was already in dependencies but unused.
Investigation was done to evaluate native PipeWire API vs the CLI approach.

### Previous Implementation (loopback.rs)
- Used `pactl list sources short` to find `.monitor` sources
- Used `parec --format=s16le --rate=16000 --channels=1` to capture
- ~140 lines, worked on both PipeWire and PulseAudio systems
- Runtime dependency: `pulseaudio-utils` package

### Native PipeWire Findings

**Challenges addressed:**

1. **Monitor source targeting** — solved via `stream.capture.sink = "true"` +
   `target.object` properties. PipeWire routes capture to sink monitor ports.
   Source discovery still uses `pactl` to find the node name.

2. **Callback-driven architecture** — dedicated thread runs PipeWire `MainLoopRc`.
   Process callback runs on MainLoop thread (no `RT_PROCESS` flag), so `Mutex::lock()`
   is safe for writing samples.

3. **Thread safety** — `pw::channel::channel()` provides a `Send` sender and a
   non-`Send` receiver attached to the MainLoop. The sender signals quit from
   the stop method on any thread.

4. **Format negotiation** — PipeWire handles resampling to our requested F32LE
   mono 16 kHz via its built-in audio adapter. No manual conversion needed.

## Decision
Implement native PipeWire capture, removing the `parec` subprocess dependency.

### Rationale
- Eliminates `pulseaudio-utils` runtime dependency
- Direct F32LE format — no i16→f32 conversion needed
- PipeWire handles resampling natively (higher quality)
- Clean start/stop lifecycle via `pw::channel` (no process polling/killing)
- ~300 lines — simpler than the estimated 400-500

## Implementation

### Architecture

```
Main Thread (GTK)
    ↓ stop_loopback()
    → pw::channel::Sender<()>
    ↓
PipeWire Thread:
├── MainLoopRc + ContextRc + CoreRc
├── pw::channel::Receiver → mainloop.quit()
├── StreamBox (audio capture)
│   └── process callback → samples.lock().extend()
│                         → amplitude.store()
└── MainLoop::run() (blocks until quit)
    └── completion_tx.send() on exit
```

### Key Design Decisions

1. **No `RT_PROCESS`** — process callback runs on MainLoop thread, not a
   realtime thread. Allows safe `Mutex::lock()` without lock-free buffers.
   Acceptable for recording (not playback).

2. **`pactl` for source discovery** — still used to find the monitor source
   name. This is a one-time call at start, not a runtime dependency for
   audio capture itself.

3. **`stream.capture.sink`** — the PipeWire-native way to capture monitor
   ports. Combined with `target.object` (sink name without `.monitor` suffix),
   PipeWire automatically routes audio from the correct output device.

4. **`pw::channel` for lifecycle** — clean cross-thread signaling. The
   `Sender` is stored in the recorder struct; `stop_loopback()` sends `()`
   to quit the MainLoop. No process killing or flag polling needed.

### Files Changed
- `src/recording/loopback.rs` — replaced `parec` subprocess with native
  PipeWire `StreamBox` capture

### Dependencies
- `pipewire = "0.9"` (already in Cargo.toml, now actively used)
- System: `libpipewire-0.3-dev` (build-time), PipeWire daemon (runtime)
- `pactl` still used for source discovery (optional — falls back to default)

## Consequences

### Positive
- ✅ No more `parec` subprocess spawning/killing
- ✅ No `pulseaudio-utils` runtime dependency for audio capture
- ✅ Native F32LE format — no i16→f32 sample conversion
- ✅ PipeWire resampling to 16 kHz (higher quality than parec)
- ✅ Clean lifecycle via channel-based signaling
- ✅ Simpler code (~300 vs ~140 lines, but no subprocess management)

### Negative
- ⚠️ Requires PipeWire daemon (won't work on pure PulseAudio systems)
- ⚠️ Build-time dependency on `libpipewire-0.3-dev`
- ⚠️ Still uses `pactl` for initial source discovery

### Future Work
- Replace `pactl` discovery with PipeWire Registry API enumeration
- Add fallback for pure PulseAudio systems (detect PipeWire availability)
- Test on multiple distributions (Ubuntu 22.04+, Fedora 34+, Arch)
