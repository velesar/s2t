# Architecture Overview and Design Findings

**Project:** Voice Dictation (s2t)
**Audit Date:** 2026-01-28

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Architecture Pattern](#architecture-pattern)
3. [Module Structure](#module-structure)
4. [Data Flow](#data-flow)
5. [Dependency Analysis](#dependency-analysis)
6. [Layer Architecture](#layer-architecture)
7. [Hotspot Analysis](#hotspot-analysis)
8. [Design Strengths](#design-strengths)
9. [Design Weaknesses](#design-weaknesses)
10. [Architectural Recommendations](#architectural-recommendations)

---

## System Overview

Voice Dictation is a **desktop GUI application** for offline speech-to-text transcription on Linux. It operates as a system tray application with three recording modes:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Voice Dictation                             │
│                                                                  │
│  ┌──────────┐    ┌──────────┐    ┌──────────────────┐          │
│  │ System   │    │   GTK4   │    │    Whisper.cpp   │          │
│  │   Tray   │◄──►│   GUI    │◄──►│  Speech Engine   │          │
│  └──────────┘    └──────────┘    └──────────────────┘          │
│        │              │                   │                     │
│        ▼              ▼                   ▼                     │
│  ┌──────────┐    ┌──────────┐    ┌──────────────────┐          │
│  │  Global  │    │  Audio   │    │     History      │          │
│  │ Hotkeys  │    │ Pipeline │    │     Storage      │          │
│  └──────────┘    └──────────┘    └──────────────────┘          │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Key Characteristics

| Aspect | Description |
|--------|-------------|
| **Type** | Desktop GUI Application |
| **Platform** | Linux (Fedora optimized) |
| **Connectivity** | Fully offline capable |
| **State Management** | Shared state via Arc<Mutex<T>> |
| **Concurrency** | Multi-threaded with async channels |
| **Distribution** | Single binary + Whisper models |

---

## Architecture Pattern

### Primary Pattern: Component-Based GUI Application

The application follows a **component-based architecture** typical of GTK applications, with shared state managed through thread-safe smart pointers.

```
┌─────────────────────────────────────────────────────────────────┐
│                         main.rs                                  │
│                    (Application Orchestrator)                    │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   Config    │  │   History   │  │   Whisper Model         │ │
│  │ Arc<Mutex>  │  │ Arc<Mutex>  │  │   Arc<Mutex<Option>>    │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│         │               │                     │                 │
│         └───────────────┼─────────────────────┘                 │
│                         │                                       │
│                         ▼                                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                      UI Layer                                ││
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐   ││
│  │  │ Main UI  │  │ History  │  │  Model   │  │ Settings │   ││
│  │  │ (ui.rs)  │  │  Dialog  │  │  Dialog  │  │  Dialog  │   ││
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘   ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

### State Sharing Pattern

```rust
// Shared state initialized in main.rs
let whisper: Arc<Mutex<Option<WhisperSTT>>> = Arc::new(Mutex::new(None));
let config: Arc<Mutex<Config>> = Arc::new(Mutex::new(load_config()?));
let history: Arc<Mutex<History>> = Arc::new(Mutex::new(load_history()?));

// Cloned and passed to UI components
build_ui(&app, whisper.clone(), config.clone(), history.clone(), ...);
```

### Async Communication Pattern

```rust
// Inter-component communication via async channels
let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();
let (hotkey_tx, hotkey_rx) = async_channel::unbounded::<HotkeyEvent>();
let (tray_tx, tray_rx) = async_channel::unbounded::<TrayAction>();
```

---

## Module Structure

### Module Overview (20 modules, 5,594 LOC)

```
src/
├── main.rs              (266 LOC)  Application entry, orchestration
├── ui.rs              (1,555 LOC)  Main UI, recording handlers [LARGE]
├── model_dialog.rs      (532 LOC)  Model download/management UI
├── history_dialog.rs    (418 LOC)  History browser UI
├── settings_dialog.rs   (375 LOC)  Settings configuration UI
├── models.rs            (355 LOC)  Model metadata and paths
├── history.rs           (289 LOC)  History storage/retrieval
├── continuous.rs        (247 LOC)  Continuous recording mode
├── config.rs            (222 LOC)  Configuration management
├── audio.rs             (196 LOC)  Microphone recording
├── whisper.rs           (188 LOC)  Whisper STT integration
├── tray.rs              (177 LOC)  System tray service
├── hotkeys.rs           (153 LOC)  Global hotkey handling
├── loopback.rs          (139 LOC)  System audio capture
├── ring_buffer.rs       (113 LOC)  Circular audio buffer
├── diarization.rs       (111 LOC)  Speaker diarization
├── vad.rs                (85 LOC)  Voice activity detection
├── conference_recorder.rs (76 LOC) Conference mode coordinator
├── recordings.rs         (74 LOC)  Recording file management
└── paste.rs              (23 LOC)  Auto-paste functionality
```

### Module Size Distribution

```
Lines of Code Distribution
==========================

ui.rs            ████████████████████████████ 1555 (27.8%)
model_dialog.rs  █████████░░░░░░░░░░░░░░░░░░░  532 (9.5%)
history_dialog.rs ███████░░░░░░░░░░░░░░░░░░░░  418 (7.5%)
settings_dialog.rs ██████░░░░░░░░░░░░░░░░░░░░  375 (6.7%)
models.rs        ██████░░░░░░░░░░░░░░░░░░░░░░  355 (6.3%)
history.rs       █████░░░░░░░░░░░░░░░░░░░░░░░  289 (5.2%)
main.rs          ████░░░░░░░░░░░░░░░░░░░░░░░░  266 (4.8%)
continuous.rs    ████░░░░░░░░░░░░░░░░░░░░░░░░  247 (4.4%)
config.rs        ███░░░░░░░░░░░░░░░░░░░░░░░░░  222 (4.0%)
other (11 files) ████████████░░░░░░░░░░░░░░░░ 1335 (23.9%)
```

### Module Categories

| Category | Modules | Purpose |
|----------|---------|---------|
| **Core** | main.rs, config.rs | Application lifecycle, configuration |
| **UI** | ui.rs, *_dialog.rs | User interface components |
| **Audio** | audio.rs, continuous.rs, ring_buffer.rs, vad.rs, loopback.rs, conference_recorder.rs, recordings.rs | Audio capture and processing |
| **Speech** | whisper.rs, diarization.rs | Speech recognition and analysis |
| **System** | tray.rs, hotkeys.rs, paste.rs | OS integration |
| **Data** | history.rs, models.rs | Persistence and model management |

---

## Data Flow

### Recording Data Flow

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ Microphone  │────►│   CPAL      │────►│   Rubato    │────►│   Whisper   │
│ (Hardware)  │     │ (Capture)   │     │ (Resample)  │     │   (STT)     │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                          │                   │                    │
                    44.1/48 kHz          16 kHz              Transcription
                                                                   │
                                                                   ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Clipboard  │◄────│  Auto-Copy  │◄────│   History   │◄────│    Text     │
│  (System)   │     │ (Optional)  │     │  (Storage)  │     │   Output    │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
```

### Continuous Mode Data Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        Continuous Recording Mode                          │
│                                                                           │
│  ┌─────────┐    ┌─────────────┐    ┌─────────────┐    ┌──────────────┐  │
│  │  Mic    │───►│ Ring Buffer │───►│     VAD     │───►│   Segment    │  │
│  │ Input   │    │  (30 sec)   │    │ (Detection) │    │   Channel    │  │
│  └─────────┘    └─────────────┘    └─────────────┘    └──────────────┘  │
│                                                              │            │
│                                          ┌───────────────────┘            │
│                                          ▼                                │
│                                   ┌─────────────┐                         │
│                                   │  Whisper    │                         │
│                                   │ (per segment)│                        │
│                                   └─────────────┘                         │
│                                          │                                │
│                                          ▼                                │
│                                   ┌─────────────┐                         │
│                                   │   Append    │                         │
│                                   │  to Output  │                         │
│                                   └─────────────┘                         │
└──────────────────────────────────────────────────────────────────────────┘
```

### Conference Mode Data Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        Conference Recording Mode                          │
│                                                                           │
│  ┌─────────┐                                      ┌──────────────────┐   │
│  │   Mic   │─────────────────────────────────────►│                  │   │
│  │ (User)  │                                      │   Transcription  │   │
│  └─────────┘                                      │   + Diarization  │   │
│                                                   │                  │   │
│  ┌─────────┐    ┌─────────────┐                  │   Speaker 1: ... │   │
│  │Loopback │───►│   parec     │─────────────────►│   Speaker 2: ... │   │
│  │(System) │    │  (Capture)  │                  │                  │   │
│  └─────────┘    └─────────────┘                  └──────────────────┘   │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Dependency Analysis

### Module Dependency Graph

```
                              main.rs
                                 │
        ┌────────────────────────┼────────────────────────┐
        │                        │                        │
        ▼                        ▼                        ▼
    ┌───────┐              ┌─────────┐              ┌─────────┐
    │config │◄─────────────│  ui.rs  │─────────────►│ tray.rs │
    └───────┘              └─────────┘              └─────────┘
        │                        │                        │
        │    ┌───────────────────┼───────────────────┐    │
        │    │                   │                   │    │
        ▼    ▼                   ▼                   ▼    ▼
    ┌───────────┐         ┌───────────┐         ┌───────────┐
    │  audio    │         │  whisper  │         │  history  │
    │continuous │         │diarization│         │  models   │
    │ loopback  │         └───────────┘         └───────────┘
    │   vad     │
    └───────────┘
```

### High Coupling Modules

| Module | Dependencies | Dependents | Coupling Score |
|--------|--------------|------------|----------------|
| main.rs | 19 | 0 | High (orchestrator) |
| ui.rs | 19 | 1 | High (god object risk) |
| config.rs | 0 | 16 | Low (leaf dependency) |
| history.rs | 1 | 8 | Medium |
| whisper.rs | 0 | 6 | Low |

### Dependency Matrix (Simplified)

```
             main  ui  audio whis hist conf tray hotk dial
main.rs       -    X    X     X    X    X    X    X    X
ui.rs         X    -    X     X    X    X    -    X    X
audio.rs      -    -    -     -    -    X    -    -    -
whisper.rs    -    -    -     -    -    -    -    -    -
history.rs    -    -    -     -    -    X    -    -    -
config.rs     -    -    -     -    -    -    -    -    -
tray.rs       -    -    -     -    -    -    -    -    -
hotkeys.rs    -    -    -     -    -    X    -    -    -
*_dialog.rs   -    -    -     X    X    X    -    -    -
```

---

## Layer Architecture

### Current Layer Structure

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         PRESENTATION LAYER                               │
│                                                                          │
│  ui.rs (1555)  history_dialog.rs (418)  model_dialog.rs (532)          │
│  settings_dialog.rs (375)  tray.rs (177)                                │
│                                                                          │
│  Responsibility: User interface, event handling, display                 │
├─────────────────────────────────────────────────────────────────────────┤
│                         APPLICATION LAYER                                │
│                                                                          │
│  main.rs (266)  config.rs (222)  hotkeys.rs (153)                       │
│                                                                          │
│  Responsibility: Application lifecycle, configuration, orchestration    │
├─────────────────────────────────────────────────────────────────────────┤
│                           DOMAIN LAYER                                   │
│                                                                          │
│  audio.rs (196)  whisper.rs (188)  history.rs (289)                     │
│  continuous.rs (247)  diarization.rs (111)  vad.rs (85)                 │
│                                                                          │
│  Responsibility: Core business logic, audio processing, transcription   │
├─────────────────────────────────────────────────────────────────────────┤
│                       INFRASTRUCTURE LAYER                               │
│                                                                          │
│  models.rs (355)  loopback.rs (139)  recordings.rs (74)                 │
│  paste.rs (23)  ring_buffer.rs (113)  conference_recorder.rs (76)       │
│                                                                          │
│  Responsibility: External systems, file I/O, OS integration             │
└─────────────────────────────────────────────────────────────────────────┘
```

### Layer Violations Identified

| Violation | Source | Target | Description |
|-----------|--------|--------|-------------|
| V1 | ui.rs | models.rs | Presentation directly accesses infrastructure |
| V2 | ui.rs | recordings.rs | Presentation directly manages file storage |
| V3 | main.rs | all layers | Orchestrator bypasses application layer |

### Ideal Layer Flow

```
Presentation → Application → Domain → Infrastructure
     ↓              ↓           ↓            ↓
   Events      Use Cases    Entities     External
```

---

## Hotspot Analysis

### Code Hotspots (Symbols with 5+ References)

| Symbol | File | References | Type |
|--------|------|------------|------|
| `Config` | config.rs | 28 | Struct |
| `AppState` | ui.rs | 30 | Enum |
| `WhisperSTT` | whisper.rs | 23 | Struct |
| `History` | history.rs | 22 | Struct |
| `HistoryEntry` | history.rs | 20 | Struct |
| `ModelInfo` | models.rs | 15 | Struct |
| `AudioRecorder` | audio.rs | 15 | Struct |
| `TrayAction` | tray.rs | 15 | Enum |

### Complexity Hotspots

| File | Cyclomatic Complexity | Reason |
|------|----------------------|--------|
| ui.rs | High | Multiple recording modes, state transitions |
| model_dialog.rs | Medium | Download state management |
| history_dialog.rs | Medium | Search and filtering logic |
| continuous.rs | Medium | VAD-based segmentation |

---

## Design Strengths

### 1. Clean Error Handling

```rust
// Consistent use of anyhow for error propagation
pub fn load_config() -> Result<Config> {
    let content = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    toml::from_str(&content).with_context(|| "Failed to parse config")
}
```

### 2. Thread-Safe State Management

```rust
// Proper use of Arc<Mutex<T>> for shared state
let whisper: Arc<Mutex<Option<WhisperSTT>>> = Arc::new(Mutex::new(None));
```

### 3. Async Communication

```rust
// Non-blocking communication between components
let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();
```

### 4. Configuration Flexibility

```rust
// Serde-based configuration with sensible defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_language")]
    pub language: String,
    // ...
}
```

### 5. Modular Audio Pipeline

```rust
// Separation of concerns in audio processing
AudioRecorder (capture) → Rubato (resample) → WhisperSTT (transcribe)
```

### 6. Good Test Coverage for Core Logic

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_config_serialization() { ... }
    #[test]
    fn test_history_cleanup() { ... }
}
```

---

## Design Weaknesses

### 1. God Object: ui.rs (1555 lines)

**Problem:** ui.rs handles all three recording modes, state management, and UI updates in a single file.

**Impact:**
- Difficult to navigate
- High merge conflict risk
- Hard to test in isolation

**Recommendation:** Split into focused modules.

### 2. High Coupling in Orchestration

**Problem:** Both main.rs and ui.rs depend on all 19 other modules.

**Impact:**
- Changes ripple across the codebase
- Difficult to reason about dependencies
- Tight coupling limits reusability

**Recommendation:** Introduce service facades.

### 3. Thin Application Layer

**Problem:** main.rs directly wires all components without intermediate services.

**Impact:**
- Business logic mixed with wiring
- No clear use case boundaries
- Hard to add cross-cutting concerns

**Recommendation:** Create service layer.

### 4. Inconsistent Concurrency Primitives

**Problem:** Mix of `Arc<Mutex<T>>`, `Rc<RefCell<T>>`, and `Arc<Atomic*>` without clear rationale.

**Impact:**
- Cognitive overhead
- Risk of misuse (Arc with non-Send types)

**Recommendation:** Document concurrency strategy, standardize patterns.

### 5. CLI Tool Dependencies

**Problem:** Uses `xdotool`, `pactl`, `parec` via Command::new.

**Impact:**
- Runtime dependencies not in Cargo.toml
- Subprocess overhead
- Error handling complexity

**Recommendation:** Native API integration where possible.

---

## Architectural Recommendations

### Short-Term (1-2 Sprints)

#### R1: Split ui.rs into Focused Modules

```
src/ui/
├── mod.rs           # Re-exports
├── state.rs         # AppState enum
├── context.rs       # Context structs
├── recording.rs     # Dictation mode
├── continuous.rs    # Continuous mode
├── conference.rs    # Conference mode
└── widgets.rs       # Shared widgets
```

#### R2: Create Context Structs

```rust
pub struct RecordingContext<'a> {
    pub ui: &'a UiWidgets,
    pub state: &'a SharedState,
    pub audio: &'a AudioService,
}
```

### Medium-Term (3-4 Sprints)

#### R3: Introduce Service Layer

```rust
// src/services/mod.rs
pub struct AudioService { ... }
pub struct TranscriptionService { ... }
pub struct HistoryService { ... }

// Reduces main.rs to:
let audio = AudioService::new()?;
let transcription = TranscriptionService::new()?;
let history = HistoryService::new()?;

build_ui(&app, &audio, &transcription, &history);
```

#### R4: Document Concurrency Strategy

```rust
// doc/CONCURRENCY.md
// 1. Arc<Mutex<T>> for cross-thread shared state
// 2. Rc<RefCell<T>> for single-thread shared state
// 3. async_channel for message passing
// 4. AtomicBool/AtomicU32 for simple flags
```

### Long-Term (Future)

#### R5: Consider Event-Driven Architecture

```rust
enum AppEvent {
    RecordingStarted,
    RecordingStopped(Vec<f32>),
    TranscriptionComplete(String),
    ConfigChanged(Config),
}

// Central event bus
let (event_tx, event_rx) = async_channel::unbounded::<AppEvent>();
```

#### R6: Native PipeWire Integration

Replace CLI tools with native `pipewire` crate APIs for system audio capture.

---

## Conclusion

The Voice Dictation application demonstrates **solid Rust engineering practices** with appropriate use of the type system, error handling, and concurrency primitives. The main architectural concerns are:

1. **Code concentration** in ui.rs (28% of codebase)
2. **High coupling** between orchestration and all modules
3. **Thin application layer** missing service abstractions

These issues can be addressed incrementally without major architectural rewrites. The current design is suitable for an MVP/early-stage application, but should be refactored as the feature set grows.

**Overall Architecture Rating:** 7/10 (Good foundation, needs modularization)
