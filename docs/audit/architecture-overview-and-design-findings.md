# Architecture Overview and Design Findings

**Project:** Voice Dictation (s2t)
**Initial Audit Date:** 2026-01-28
**Last Updated:** 2026-01-28 (post-Phase 2-5 refactor)
**Methodology:** Architecture Fitness Functions (see `docs/architecture-fitness-methodology.md`)

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Architecture Pattern](#architecture-pattern)
3. [Module Structure](#module-structure)
4. [Data Flow](#data-flow)
5. [Dependency Analysis](#dependency-analysis)
6. [Layer Architecture](#layer-architecture)
7. [Architecture Fitness Assessment](#architecture-fitness-assessment)
8. [Hotspot Analysis](#hotspot-analysis)
9. [Design Strengths](#design-strengths)
10. [Design Weaknesses](#design-weaknesses)
11. [Architectural Recommendations](#architectural-recommendations)

---

## System Overview

Voice Dictation is a **desktop GUI application** for offline speech-to-text transcription on Linux using Whisper.

```
┌─────────────────────────────────────────────────────────────────┐
│                      Voice Dictation                            │
│                                                                 │
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
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Key Characteristics

| Aspect | Description |
|--------|-------------|
| **Type** | Desktop GUI Application |
| **Platform** | Linux (Fedora optimized) |
| **Connectivity** | Fully offline capable |
| **State Management** | Shared state via `Arc<Mutex<T>>` |
| **Concurrency** | Multi-threaded with async channels |
| **Distribution** | Single binary + Whisper models |
| **Codebase Size** | 36 files, 7,086 LOC, 788 symbols |

---

## Architecture Pattern

### Primary Pattern: Service-Oriented GTK Application with AppContext

The application has evolved from a flat component-based architecture to a **service-oriented** pattern centered on `AppContext` — a dependency injection container that bundles all services and shared state.

```
┌─────────────────────────────────────────────────────────────────────┐
│                           main.rs                                   │
│                      (Composition Root)                             │
│                                                                     │
│  Creates: Config, History, TranscriptionService, DiarizationEngine │
│                              │                                      │
│                              ▼                                      │
│                       ┌─────────────┐                               │
│                       │ AppContext   │                               │
│                       │ (DI Container)│                              │
│                       └──────┬──────┘                               │
│               ┌──────────────┼──────────────┐                       │
│               ▼              ▼              ▼                       │
│  ┌─────────────────┐  ┌──────────┐  ┌──────────────┐              │
│  │  AudioService   │  │ Transcr. │  │  UIChannels  │              │
│  │ (Mic/Cont/Conf) │  │ Service  │  │ (async msgs) │              │
│  └─────────────────┘  └──────────┘  └──────────────┘              │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    Presentation Layer                         │  │
│  │  ┌────────┐  ┌─────────┐  ┌─────────┐  ┌──────────┐        │  │
│  │  │ ui/    │  │dialogs/ │  │ tray.rs │  │hotkeys.rs│        │  │
│  │  │mod.rs  │  │history  │  │         │  │          │        │  │
│  │  │state   │  │model    │  │         │  │          │        │  │
│  │  │record  │  │settings │  │         │  │          │        │  │
│  │  │contin. │  │         │  │         │  │          │        │  │
│  │  │confer. │  │         │  │         │  │          │        │  │
│  │  └────────┘  └─────────┘  └─────────┘  └──────────┘        │  │
│  └──────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

### State Sharing Pattern (Current)

```rust
// AppContext bundles all shared state
let ctx = Arc::new(AppContext::new(config, history, transcription, diarization)?);

// Passed as single dependency to UI
app.connect_activate(move |app| {
    ui::build_ui(app, ctx.clone());
});
```

### Async Communication Pattern

```rust
// Centralized UIChannels for inter-component messaging
pub struct UIChannels {
    toggle_recording: (Sender<()>, Receiver<()>),
    reload_hotkeys: (Sender<()>, Receiver<()>),
    open_models: (Sender<()>, Receiver<()>),
    open_history: (Sender<()>, Receiver<()>),
    open_settings: (Sender<()>, Receiver<()>),
}
```

---

## Module Structure

### Module Overview (36 files, 7,086 LOC, 788 symbols)

```
src/
├── main.rs                   (289 LOC,  67 sym)  Composition root
├── context.rs                (127 LOC,  33 sym)  AppContext DI container
├── traits.rs                 (179 LOC,  81 sym)  Core domain traits
├── types.rs                  ( ~50 LOC)          Shared type aliases
├── channels.rs               ( ~80 LOC)          UIChannels
│
├── ui/                       UI layer (split from monolithic ui.rs)
│   ├── mod.rs                ( 76 sym)           Window setup, build_ui
│   ├── state.rs              (126 sym)           UIContext, AppState
│   ├── recording.rs                              Dictation mode handler
│   ├── continuous.rs         ( 92 sym)           Continuous mode handler
│   └── conference.rs                             Conference mode handler
│
├── dialogs/                  Dialog windows
│   ├── history.rs            (152 sym)           History browser
│   ├── model.rs              (156 sym)           Model download/management
│   └── settings.rs           ( 95 sym)           Settings configuration
│
├── services/                 Service layer
│   ├── mod.rs                                    Re-exports
│   ├── audio.rs              ( 69 sym)           AudioService facade
│   └── transcription.rs      ( 47 sym)           TranscriptionService facade
│
├── test_support/             Test infrastructure
│   ├── mod.rs
│   └── mocks.rs                                  Mock implementations
│
├── config.rs                 ( 59 sym)           TOML configuration
├── history.rs                (148 sym)           History storage/retrieval
├── audio.rs                  ( 82 sym)           Microphone recording (CPAL)
├── whisper.rs                ( 55 sym)           Whisper STT integration
├── continuous.rs             ( 80 sym)           Continuous recording mode
├── conference_recorder.rs                        Conference mode coordinator
├── diarization.rs                                Speaker diarization
├── vad.rs                    ( 30 sym)           Voice activity detection
├── loopback.rs                                   System audio capture (parec)
├── ring_buffer.rs                                Circular audio buffer
├── recordings.rs                                 Recording file management
├── models.rs                                     Model metadata and paths
├── tray.rs                   ( 58 sym)           System tray service (ksni)
├── hotkeys.rs                ( 28 sym)           Global hotkey handling
└── paste.rs                                      Auto-paste (xdotool)
```

### Module Categories

| Category | Modules | Symbols | Purpose |
|----------|---------|---------|---------|
| **Core / DI** | main.rs, context.rs, traits.rs, types.rs, channels.rs | 181 | Application lifecycle, DI, contracts |
| **UI** | ui/*, dialogs/* | 697 | User interface, event handling |
| **Services** | services/* | 116 | Service facades (audio, transcription) |
| **Audio** | audio.rs, continuous.rs, conference_recorder.rs, ring_buffer.rs, vad.rs, loopback.rs, recordings.rs | ~250 | Audio capture and processing |
| **Speech** | whisper.rs, diarization.rs | ~85 | Speech recognition |
| **System** | tray.rs, hotkeys.rs, paste.rs | ~86 | OS integration |
| **Data** | history.rs, config.rs, models.rs | ~260 | Persistence, model management |
| **Test** | test_support/* | ~30 | Mock implementations |

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

### Service Layer Data Flow (Current)

```
┌──────────────────────────────────────────────────────┐
│                    AppContext                          │
│                                                       │
│  UI Handler ──► AudioService ──► AudioRecorder        │
│       │              │               │                │
│       │              ▼               ▼                │
│       │         stop_dictation()  (samples)           │
│       │              │                                │
│       ▼              ▼                                │
│  TranscriptionService ──► WhisperSTT.transcribe()     │
│       │                                               │
│       ▼                                               │
│  History.add(entry)                                   │
└──────────────────────────────────────────────────────┘
```

### Continuous Mode Data Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        Continuous Recording Mode                         │
│                                                                          │
│  ┌─────────┐    ┌─────────────┐    ┌─────────────┐    ┌──────────────┐  │
│  │  Mic    │───►│ Ring Buffer │───►│     VAD     │───►│   Segment    │  │
│  │ Input   │    │  (30 sec)   │    │ (Detection) │    │   Channel    │  │
│  └─────────┘    └─────────────┘    └─────────────┘    └──────────────┘  │
│                                                              │           │
│                                          ┌───────────────────┘           │
│                                          ▼                               │
│                                   ┌─────────────┐                        │
│                                   │  Whisper    │                        │
│                                   │ (per segment)│                       │
│                                   └─────────────┘                        │
└──────────────────────────────────────────────────────────────────────────┘
```

### Conference Mode Data Flow

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        Conference Recording Mode                         │
│                                                                          │
│  ┌─────────┐                                      ┌──────────────────┐  │
│  │   Mic   │─────────────────────────────────────►│                  │  │
│  │ (User)  │                                      │   Transcription  │  │
│  └─────────┘                                      │   + Diarization  │  │
│                                                   │                  │  │
│  ┌─────────┐    ┌─────────────┐                  │   Speaker 1: ... │  │
│  │Loopback │───►│   parec     │─────────────────►│   Speaker 2: ... │  │
│  │(System) │    │  (Capture)  │                  │                  │  │
│  └─────────┘    └─────────────┘                  └──────────────────┘  │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Dependency Analysis

### Module Dependency Overview (from codegraph)

Due to Rust's flat crate structure (all modules are siblings in the same crate), codegraph reports 31-32 bidirectional connections per module. This is a structural artifact of the single-crate layout, not true coupling. The meaningful dependencies are the **import-level** dependencies analyzed below.

### Effective Dependency Graph

```
                              main.rs
                         (Composition Root)
                                │
        ┌───────────────────────┼───────────────────────────┐
        │                       │                           │
        ▼                       ▼                           ▼
   ┌─────────┐           ┌───────────┐              ┌───────────┐
   │ context  │◄──────────│  ui/mod   │              │  tray.rs  │
   │ (DI)     │           │ (build_ui)│              │ (ksni)    │
   └────┬─────┘           └─────┬─────┘              └─────┬─────┘
        │                       │                          │
   ┌────┼──────────────────────┼──────────────────────────┤
   │    │    ┌─────────────────┼───────────────────┐      │
   │    ▼    ▼                 ▼                   ▼      ▼
   │  ┌───────────┐     ┌───────────┐         ┌───────────┐
   │  │ services/ │     │ dialogs/  │         │ whisper   │
   │  │ audio     │     │ hist/mod/ │         │ (direct!) │
   │  │ transcr.  │     │ settings  │         └───────────┘
   │  └─────┬─────┘     └─────┬─────┘
   │        │                  │
   │        ▼                  ▼
   │  ┌───────────┐     ┌───────────┐     ┌───────────┐
   │  │  audio    │     │  config   │◄────│  history  │
   │  │continuous │     │           │     │           │
   │  │ loopback  │     └───────────┘     └───────────┘
   │  │   vad     │
   │  └───────────┘
   │
   │  ┌───────────┐
   └─►│  traits   │  (defined but not fully wired)
      └───────────┘
```

### Instability Metrics (I = Ce / (Ca + Ce))

| Module | Ce (out) | Ca (in) | I | Classification |
|--------|----------|---------|---|----------------|
| config.rs | 0 | 16 | 0.00 | **Maximally Stable** |
| traits.rs | 1 | 2 | 0.33 | Stable |
| history.rs | 2 | 8 | 0.20 | Stable |
| whisper.rs | 2 | 6 | 0.25 | Stable |
| audio.rs | 3 | 5 | 0.38 | Moderate |
| context.rs | 6 | 8 | 0.43 | Moderate |
| services/audio.rs | 5 | 3 | 0.63 | Unstable |
| services/transcription.rs | 3 | 2 | 0.60 | Unstable |
| ui/state.rs | 5 | 7 | 0.42 | **Violation** (see below) |
| ui/mod.rs | 12 | 1 | 0.92 | Unstable (expected) |
| dialogs/model.rs | 8 | 1 | 0.89 | Unstable (expected) |
| dialogs/history.rs | 7 | 1 | 0.88 | Unstable (expected) |
| dialogs/settings.rs | 6 | 1 | 0.86 | Unstable (expected) |
| main.rs | 14 | 0 | 1.00 | Maximally Unstable (expected) |

**Stable Dependencies Principle Violation:** `ui/state.rs` (I=0.42, 126 symbols) is depended upon by 7 modules but is itself an unstable module containing UI-specific state (`UIContext`, `AppState`). This creates fragility — changes to UI state structures ripple across the entire UI layer.

---

## Layer Architecture

### Current Layer Structure (Post-Refactoring)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         PRESENTATION LAYER                              │
│                                                                         │
│  ui/mod.rs (76)  ui/state.rs (126)  ui/recording.rs                    │
│  ui/continuous.rs (92)  ui/conference.rs                                │
│  dialogs/history.rs (152)  dialogs/model.rs (156)                      │
│  dialogs/settings.rs (95)  tray.rs (58)                                │
│                                                                         │
│  Depends on: AppContext (services, config, history — concrete types)    │
├─────────────────────────────────────────────────────────────────────────┤
│                         APPLICATION LAYER                               │
│                                                                         │
│  context.rs (33)  channels.rs  services/audio.rs (69)                  │
│  services/transcription.rs (47)  hotkeys.rs (28)                       │
│                                                                         │
│  Depends on: Domain concrete types (not traits)                        │
├─────────────────────────────────────────────────────────────────────────┤
│                      DOMAIN / CONTRACT LAYER                            │
│                                                                         │
│  traits.rs (81) — AudioRecording, Transcription, VoiceDetection,       │
│                   HistoryRepository, ConfigProvider                      │
│  types.rs — shared type aliases                                         │
│                                                                         │
│  Status: DEFINED but NOT WIRED (traits exist, impls missing)           │
├─────────────────────────────────────────────────────────────────────────┤
│                       INFRASTRUCTURE LAYER                              │
│                                                                         │
│  audio.rs (82)  whisper.rs (55)  history.rs (148)  config.rs (59)      │
│  continuous.rs (80)  vad.rs (30)  loopback.rs  diarization.rs          │
│  conference_recorder.rs  ring_buffer.rs  recordings.rs                 │
│  models.rs  paste.rs                                                    │
│                                                                         │
│  Status: Does NOT implement domain traits                              │
└─────────────────────────────────────────────────────────────────────────┘
```

### Layer Violations Identified

| ID | Violation | Source → Target | Severity |
|----|-----------|-----------------|----------|
| V1 | Traits defined but not implemented | traits.rs → (nothing) | **HIGH** |
| V2 | Tray bypasses AppContext | main.rs:161 → WhisperSTT (duplicate) | HIGH |
| V3 | AppContext leaks internals | context.rs `config_arc()`, `history_arc()` | MEDIUM |
| V4 | Dialogs use concrete types | dialogs/* → Config, History directly | MEDIUM |
| V5 | Services use concrete types | services/* → AudioRecorder directly | MEDIUM |
| V6 | No layer enforcement | Flat `mod` in main.rs, no crate boundaries | LOW |

### Target Layer Flow

```
Presentation → Application → Domain ← Infrastructure
     │              │           ↑           │
     │              │           │           │
     └──────────────┴───────────┴───────────┘
                    All depend on Domain traits
```

---

## Architecture Fitness Assessment

### Overall Score: 2.6 / 5.0

| Fitness Function | Score | Status | Details |
|-----------------|-------|--------|---------|
| **FF-1:** Dependency Direction | 2/5 | **FAIL** | Traits defined but not implemented by concrete types |
| **FF-2:** Component Instability | 3/5 | MIXED | ui/state.rs violates Stable Dependencies Principle |
| **FF-3:** Hotspot Risk | 3/5 | WARNING | 241 symbols with ≥5 callers; UI state unprotected |
| **FF-4:** Module Size / Cohesion | 3/5 | WARNING | 3 modules approaching 200-symbol limit |
| **FF-5:** Cyclic Dependencies | 2/5 | INCONCLUSIVE | Flat crate prevents enforcement |

### FF-1: Dependency Direction — FAIL

**Principle:** Dependencies must point inward, toward higher-level policies.

**Finding:** `src/traits.rs` defines 5 trait abstractions:
- `AudioRecording` — ✅ `TestRecorder` in tests only
- `Transcription` — ❌ `WhisperSTT` does not implement it
- `VoiceDetection` — ❌ `VoiceActivityDetector` does not implement it
- `HistoryRepository` — ❌ `History` does not implement it
- `ConfigProvider` — ❌ `Config` does not implement it

The traits are **aspirational architecture** — they document intent but don't enforce it. The codebase still depends on concrete types throughout.

### FF-2: Component Instability — MIXED

**Principle:** Stable components should be depended upon. Unstable components should not be heavily depended upon.

**Violation:** `ui/state.rs` (I=0.42, 126 symbols) is depended on by 7 modules but contains presentation-specific structures (`UIContext`, `AppState`, status labels, button references). Any change to UI state structures forces changes in all dependent UI handlers.

**Healthy pattern:** `config.rs` (I=0.00, 59 symbols) — maximally stable, depended on by 16 modules, zero outgoing deps.

### FF-3: Hotspot Risk — WARNING

**Top hotspot symbols (callers ≥ 20):**

| Symbol | Module | Callers | Risk |
|--------|--------|---------|------|
| `History#entries` | history.rs | 33 | Medium |
| `History` (struct) | history.rs | 31 | Medium |
| `HistoryEntry` | history.rs | 31 | Medium |
| `History::add` | history.rs | 30 | Medium |
| `Config` (struct) | config.rs | 27 | Medium |
| `UIContext#status_label` | ui/state.rs | 27 | **HIGH** (unstable module) |
| `ContinuousUI#base` | ui/continuous.rs | 26 | Medium |
| `ModelInfo#filename` | models.rs | 23 | Low |
| `WhisperSTT` | whisper.rs | 21 | Low |
| `AppContext` | context.rs | 21 | Low |
| `AudioRecorder` | audio.rs | 20 | Medium |
| `TranscriptionService` | services/transcription.rs | 20 | Low |
| `UIContext#button` | ui/state.rs | 20 | **HIGH** (unstable module) |

**Critical concern:** `audio.rs` contains local variables with 40-95 internal callers, indicating extremely long functions that need decomposition.

### FF-4: Module Size / Cohesion — WARNING

| Module | Symbols | Status |
|--------|---------|--------|
| dialogs/model.rs | 156 | ⚠️ Approaching 200 limit |
| dialogs/history.rs | 152 | ⚠️ Approaching 200 limit |
| history.rs | 148 | ⚠️ Approaching 200 limit |
| ui/state.rs | 126 | OK |
| ui/continuous.rs | 92 | OK |
| dialogs/settings.rs | 95 | OK |
| traits.rs | 81 | OK |

No module exceeds 200, but three are within 25% of the threshold.

### FF-5: Cyclic Dependencies — INCONCLUSIVE

The flat `mod` structure (22 `mod` declarations in `main.rs`) means all modules exist as siblings in the same crate. Rust's module system prevents true circular imports, but semantic dependencies may still form cycles. Without crate-level boundaries, this fitness function cannot be meaningfully evaluated.

---

## Hotspot Analysis

### Structural Hotspots (Codegraph, ≥10 callers)

| Symbol | File | Callers | Risk Level |
|--------|------|---------|------------|
| `History#entries` | history.rs | 33 | Medium |
| `HistoryEntry` | history.rs | 31 | Medium |
| `History::add` | history.rs | 30 | Medium |
| `Config` | config.rs | 27 | Low (stable) |
| `UIContext#status_label` | ui/state.rs | 27 | **High** |
| `ContinuousUI#base` | ui/continuous.rs | 26 | Medium |
| `ModelInfo#filename` | models.rs | 23 | Low |
| `WhisperSTT` | whisper.rs | 21 | Low |
| `AppContext` | context.rs | 21 | Low |
| `UIContext#button` | ui/state.rs | 20 | **High** |
| `TranscriptionService` | services/transcription.rs | 20 | Low |
| `AudioRecorder` | audio.rs | 20 | Medium |

### Complexity Hotspots

| File | Issue | Evidence |
|------|-------|----------|
| audio.rs | Extremely long functions | Local variables with 40-95 internal callers |
| dialogs/model.rs | High symbol density | 156 symbols, multi-concern (download, listing, UI) |
| dialogs/history.rs | High symbol density | 152 symbols, search + display + management |
| ui/state.rs | Unstable hotspot | 126 symbols, depended on by 7 modules |

---

## Design Strengths

### 1. AppContext Dependency Injection

The introduction of `AppContext` as a central DI container is a significant improvement over the previous pattern of passing 5-8 `Arc<Mutex<T>>` parameters.

```rust
pub struct AppContext {
    pub audio: Arc<AudioService>,
    pub transcription: Arc<Mutex<TranscriptionService>>,
    pub config: Arc<Mutex<Config>>,
    pub history: Arc<Mutex<History>>,
    pub diarization: Arc<Mutex<DiarizationEngine>>,
    pub channels: Arc<UIChannels>,
}
```

### 2. Trait Abstractions (Defined)

`traits.rs` defines clean, well-documented contracts for all core concerns:
- `AudioRecording` — audio capture abstraction
- `Transcription` — STT abstraction
- `VoiceDetection` — VAD abstraction
- `HistoryRepository` — persistence abstraction
- `ConfigProvider` — configuration abstraction

### 3. UI Module Split

The monolithic `ui.rs` (1,555 LOC) has been split into focused modules:
- `ui/mod.rs` — window setup
- `ui/state.rs` — shared UI state
- `ui/recording.rs` — dictation handler
- `ui/continuous.rs` — continuous handler
- `ui/conference.rs` — conference handler

### 4. Service Layer

`services/audio.rs` and `services/transcription.rs` provide facade patterns over lower-level implementations, reducing direct coupling.

### 5. Centralized Channel Management

`UIChannels` consolidates all async communication channels, replacing scattered channel creation in `main.rs`.

### 6. Clean Error Handling

Consistent use of `anyhow::Result` with `.context()` for error propagation.

### 7. Test Infrastructure

`test_support/mocks.rs` provides mock implementations, and `traits.rs` includes its own test module with `TestRecorder`.

---

## Design Weaknesses

### 1. Incomplete Trait Adoption (CRITICAL)

**Problem:** Five domain traits are defined in `traits.rs` but only `AudioRecording` has a test implementation. No production types implement these traits. The codebase still depends on concrete types everywhere.

**Impact:**
- Dependency Inversion Principle not enforced
- Services cannot be swapped or mocked in production code
- Architecture fitness FF-1 fails

**Evidence:** `AppContext.transcription` is `Arc<Mutex<TranscriptionService>>`, not `Arc<Mutex<dyn Transcription>>`.

### 2. Tray Duplication (main.rs:161-177)

**Problem:** `main.rs` creates a duplicate `WhisperSTT` instance for the tray, bypassing `AppContext` entirely.

```rust
// main.rs:159-177 — loads the Whisper model TWICE
let whisper_for_tray: Arc<Mutex<Option<WhisperSTT>>> = {
    // Re-load model for tray (tray needs its own mutable reference)
    let cfg = config.lock().unwrap();
    if let Some(model_path) = find_model_path(&cfg) {
        match WhisperSTT::new(&model_path) { ... }
    }
};
```

**Impact:** Doubles memory usage for the Whisper model (~75-500MB depending on model size). Creates inconsistency if models are reloaded.

### 3. AppContext Leaks Internal State

**Problem:** `AppContext` provides `config_arc()` and `history_arc()` methods that return raw `Arc<Mutex<T>>` handles, allowing callers to bypass the service layer.

**Impact:** Defeats the purpose of the DI container. Any caller can lock and mutate config/history directly.

### 4. ui/state.rs as Unstable Hotspot

**Problem:** `ui/state.rs` (I=0.42, 126 symbols) is depended upon by 7 modules. It contains presentation-specific structs (`UIContext` with GTK widget references). Changes to widget layout force cascading changes.

**Impact:** High coupling in the UI layer. Widget-level details leak across module boundaries.

### 5. Oversized Dialog Modules

**Problem:** `dialogs/model.rs` (156 symbols) and `dialogs/history.rs` (152 symbols) are approaching the 200-symbol cohesion threshold. Each handles multiple responsibilities (UI, data loading, user interaction, file management).

### 6. Long Functions in audio.rs

**Problem:** `audio.rs` contains functions with local variables referenced 40-95 times internally, indicating functions that are hundreds of lines long.

**Impact:** Hard to test, hard to reason about, high cyclomatic complexity.

### 7. Flat Module Hierarchy (No Layer Enforcement)

**Problem:** All 22 modules are declared as flat siblings in `main.rs`:

```rust
mod audio;
mod channels;
mod conference_recorder;
mod config;
mod context;
// ... 17 more flat mods
```

**Impact:** Any module can import any other module. Layer boundaries exist only by convention, not by the type system or module visibility.

---

## Architectural Recommendations

### Priority 0: Complete Trait Adoption

**Goal:** Wire the existing traits to concrete implementations.

**Steps:**
1. Implement `Transcription` for `WhisperSTT`
2. Implement `HistoryRepository` for `History`
3. Implement `ConfigProvider` for `Config`
4. Implement `VoiceDetection` for `VoiceActivityDetector`
5. Update `AppContext` to use `dyn Trait` bounds
6. Update services to accept trait objects

**Verification:** `AppContext.transcription` becomes `Arc<Mutex<dyn Transcription>>`.

### Priority 1: Tame UI State Hotspot

**Goal:** Reduce coupling on `ui/state.rs` by extracting stable interfaces.

**Steps:**
1. Extract a `UIActions` trait from `UIContext` (set_status, enable_button, etc.)
2. Have UI handlers depend on the trait, not the struct
3. Move `AppState` enum to `types.rs` (it's domain-level, not UI-level)

### Priority 2: Fix Tray Duplication

**Goal:** Eliminate the duplicate Whisper model load in `main.rs:161-177`.

**Steps:**
1. Migrate tray to use `AppContext` (the TODO at line 160 acknowledges this)
2. Share the `TranscriptionService` via `Arc` instead of creating a separate `WhisperSTT`
3. Remove `whisper_for_tray` entirely

### Priority 3: Decompose Oversized Modules

**Goal:** Keep all modules under 200 symbols.

**Steps:**
1. Split `dialogs/model.rs` into `model_list.rs` + `model_download.rs`
2. Split `dialogs/history.rs` into `history_list.rs` + `history_detail.rs`
3. Extract search/filter logic from `history.rs` into a separate module

### Priority 4: Enforce Layer Boundaries

**Goal:** Make layer violations compile-time errors.

**Steps (long-term):**
1. Consider workspace crates: `s2t-domain`, `s2t-services`, `s2t-ui`, `s2t-infra`
2. Or use Rust module visibility (`pub(crate)`, `pub(super)`) to restrict access
3. Add architecture fitness checks to CI

---

## Conclusion

The Voice Dictation application has undergone significant architectural improvement since the initial audit. The introduction of `AppContext`, `traits.rs`, `services/`, UI module split, and `UIChannels` demonstrates clear architectural intent toward Clean Architecture.

However, the refactoring is **incomplete**. The most critical gap is that domain traits exist but are not wired to concrete implementations — the architecture is aspirational rather than enforced. The codebase sits in a transitional state where new structures coexist with legacy patterns.

### Current State Summary

| Aspect | Status |
|--------|--------|
| AppContext DI container | ✅ Implemented |
| UI module split | ✅ Implemented |
| Service layer | ✅ Implemented |
| UIChannels | ✅ Implemented |
| Domain traits defined | ✅ Implemented |
| Domain traits wired | ❌ Not done |
| Tray uses AppContext | ❌ Not done (duplicate model) |
| Layer enforcement | ❌ Not done (flat hierarchy) |
| Legacy accessors removed | ❌ Not done (config_arc, history_arc) |

**Overall Architecture Rating:** 5/10 (Good intent, incomplete execution — up from 7/10 initial rating adjusted to account for the hybrid state penalty)
