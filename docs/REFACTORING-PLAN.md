# Comprehensive Refactoring Plan

**Project:** Voice Dictation (s2t)
**Created:** 2026-01-28
**Methodology:** Clean Architecture (Robert C. Martin)
**Status:** Planning

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current State Assessment](#current-state-assessment)
3. [Architecture Vision](#architecture-vision)
4. [Refactoring Phases](#refactoring-phases)
   - [Phase 0: Quick Wins](#phase-0-quick-wins-1-2-days)
   - [Phase 1: Complete Services Migration](#phase-1-complete-services-migration-1-week)
   - [Phase 2: Dependency Inversion](#phase-2-dependency-inversion-1-2-weeks)
   - [Phase 3: Recording Mode Polymorphism](#phase-3-recording-mode-polymorphism-1-week)
   - [Phase 4: Domain Layer Extraction](#phase-4-domain-layer-extraction-2-weeks)
   - [Phase 5: Testing Infrastructure](#phase-5-testing-infrastructure-ongoing)
5. [Detailed Task Breakdown](#detailed-task-breakdown)
6. [Risk Assessment](#risk-assessment)
7. [Success Metrics](#success-metrics)
8. [Appendices](#appendices)

---

## Executive Summary

### Problem Statement

The codebase has undergone partial refactoring, resulting in a **hybrid state** where:
- New architectural structures exist (services, context, UI modules)
- Legacy code paths remain active
- Services layer is defined but largely unused
- `#[allow(dead_code)]` masks incomplete migration

### Goal

Transform the codebase into a **Clean Architecture** compliant system with:
- Clear layer boundaries (Presentation → Application → Domain → Infrastructure)
- Dependency Inversion through traits
- Single Responsibility modules
- Zero clippy warnings
- Comprehensive test coverage

### Scope

| In Scope | Out of Scope |
|----------|--------------|
| Complete services migration | New features |
| Fix all identified violations | Performance optimization |
| Introduce trait abstractions | UI redesign |
| Thread safety fixes | New recording modes |
| Code cleanup and documentation | Database migration |

---

## Current State Assessment

### Architecture Fitness Scores

| Fitness Function | Score | Target | Gap |
|-----------------|-------|--------|-----|
| FF-1: Dependency Direction | 60% | 95% | -35% |
| FF-2: Component Instability | 70% | 90% | -20% |
| FF-3: Hotspot Coverage | 50% | 80% | -30% |
| FF-4: Module Cohesion | 85% | 95% | -10% |
| FF-5: Acyclic Dependencies | 70% | 100% | -30% |

### Key Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Max file LOC | 532 | < 400 |
| `#[allow(dead_code)]` count | ~10 | 0 |
| Clippy warnings | 8 | 0 |
| Direct recorder access points | 15+ | 0 |
| Trait abstractions | 0 | 5+ |
| Test coverage | ~10% | > 60% |

### Identified Issues

| ID | Issue | Severity | Phase |
|----|-------|----------|-------|
| I-01 | Hybrid migration state | High | 1 |
| I-02 | `Arc<Mutex<Vad>>` thread safety | Medium | 0 |
| I-03 | Complex tuple return type | Low | 0 |
| I-04 | 8 clippy warnings | Low | 0 |
| I-05 | `context.rs` violates SRP | High | 2 |
| I-06 | No trait abstractions (DIP) | High | 2 |
| I-07 | Conditional recording mode logic | Medium | 3 |
| I-08 | Mixed domain/infrastructure | Medium | 4 |
| I-09 | Implicit cyclic dependencies | Medium | 1-2 |
| I-10 | Low test coverage | High | 5 |

---

## Architecture Vision

### Target Layer Structure

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         PRESENTATION LAYER                               │
│                                                                          │
│  src/ui/                                                                 │
│  ├── mod.rs           Window setup, widget creation                     │
│  ├── state.rs         UI state structs (UIContext, etc.)                │
│  ├── recording.rs     Dictation mode UI handler                         │
│  ├── continuous.rs    Continuous mode UI handler                        │
│  └── conference.rs    Conference mode UI handler                        │
│                                                                          │
│  src/dialogs/                                                            │
│  ├── history.rs       History browser                                   │
│  ├── model.rs         Model management                                  │
│  └── settings.rs      Settings configuration                            │
│                                                                          │
│  Depends on: Application Layer (via traits)                             │
├─────────────────────────────────────────────────────────────────────────┤
│                         APPLICATION LAYER                                │
│                                                                          │
│  src/app/                                                                │
│  ├── context.rs       AppContext (thin DI container)                    │
│  ├── commands.rs      Application commands/use cases                    │
│  └── events.rs        Application events                                │
│                                                                          │
│  src/services/                                                           │
│  ├── audio.rs         AudioService (recording orchestration)            │
│  └── transcription.rs TranscriptionService (STT orchestration)          │
│                                                                          │
│  Depends on: Domain Layer (via traits)                                  │
├─────────────────────────────────────────────────────────────────────────┤
│                           DOMAIN LAYER                                   │
│                                                                          │
│  src/domain/                                                             │
│  ├── traits.rs        Core abstractions (Recording, Transcription)      │
│  ├── audio.rs         Audio processing logic                            │
│  ├── transcription.rs STT domain logic                                  │
│  ├── history.rs       History entity and repository trait               │
│  └── config.rs        Configuration entity                              │
│                                                                          │
│  Depends on: Nothing (pure domain logic)                                │
├─────────────────────────────────────────────────────────────────────────┤
│                       INFRASTRUCTURE LAYER                               │
│                                                                          │
│  src/infra/                                                              │
│  ├── audio/                                                              │
│  │   ├── cpal.rs      CPAL microphone implementation                    │
│  │   ├── loopback.rs  System audio capture                              │
│  │   └── vad.rs       Voice activity detection                          │
│  ├── stt/                                                                │
│  │   ├── whisper.rs   Whisper.cpp integration                           │
│  │   └── diarization.rs Speaker diarization                             │
│  ├── storage/                                                            │
│  │   ├── config.rs    TOML config persistence                           │
│  │   ├── history.rs   JSON history persistence                          │
│  │   └── models.rs    Model file management                             │
│  └── system/                                                             │
│      ├── tray.rs      System tray integration                           │
│      ├── hotkeys.rs   Global hotkey handling                            │
│      └── clipboard.rs Clipboard/paste integration                       │
│                                                                          │
│  Depends on: Domain Layer (implements traits)                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### Dependency Flow

```
Presentation → Application → Domain ← Infrastructure
     │              │           ↑           │
     │              │           │           │
     └──────────────┴───────────┴───────────┘
                    All depend on Domain traits
```

---

## Refactoring Phases

### Phase 0: Quick Wins (1-2 days)

**Goal:** Fix low-hanging fruit without architectural changes.

#### P0.1: Fix Clippy Warnings

| Warning | File | Fix |
|---------|------|-----|
| `empty_line_after_outer_attr` | services/audio.rs:51 | Remove empty line |
| `type_complexity` | conference_recorder.rs:42 | Use `ConferenceRecording` struct |
| `redundant_closure` | continuous.rs:153 | Replace with `Instant::now` |
| `writeln_empty_string` | history.rs:135,148,152 | Use `writeln!(file)` |
| `map_flatten` | history_dialog.rs:199 | Use `and_then()` |
| `too_many_arguments` | history_dialog.rs:323 | Create parameter struct |

```bash
# Verification
cargo clippy -- -D warnings
```

#### P0.2: Fix Thread Safety Issue

**File:** `src/vad.rs`

**Current:**
```rust
pub struct VoiceActivityDetector {
    vad: Arc<Mutex<Vad>>,  // Vad is !Send
}
```

**Solution A (Preferred - Single Thread):**
```rust
use std::cell::RefCell;
use std::rc::Rc;

pub struct VoiceActivityDetector {
    vad: Rc<RefCell<Vad>>,
    silence_threshold_ms: u32,
}
```

**Solution B (If cross-thread needed):**
```rust
/// # Thread Safety
/// VoiceActivityDetector is designed for single-threaded use.
/// Create separate instances for each thread if needed.
#[allow(clippy::arc_with_non_send_sync)]
pub struct VoiceActivityDetector {
    vad: Arc<Mutex<Vad>>,
}
```

#### P0.3: Apply ConferenceRecording Struct

**File:** `src/conference_recorder.rs`

**Current:**
```rust
pub fn stop_conference(&self) -> (
    Vec<f32>, Vec<f32>, Option<Receiver<()>>, Option<Receiver<()>>
)
```

**Refactored:**
```rust
use crate::services::audio::ConferenceRecording;

pub fn stop_conference(&self) -> ConferenceRecording {
    let (mic, loopback, mic_rx, loopback_rx) = self.stop_internal();
    ConferenceRecording {
        mic_samples: mic,
        loopback_samples: loopback,
        mic_completion: mic_rx,
        loopback_completion: loopback_rx,
    }
}
```

**Move struct to shared location:**
```rust
// src/types.rs (new file)
pub struct ConferenceRecording {
    pub mic_samples: Vec<f32>,
    pub loopback_samples: Vec<f32>,
    pub mic_completion: Option<Receiver<()>>,
    pub loopback_completion: Option<Receiver<()>>,
}
```

#### P0.4: Fix Parameter Count

**File:** `src/history_dialog.rs:323`

**Current:**
```rust
fn create_history_row(
    entry: &HistoryEntry,
    list_box: &ListBox,
    history: Arc<Mutex<History>>,
    result_text_view: &TextView,
    // ... 8 parameters total
) -> ListBoxRow
```

**Refactored:**
```rust
struct HistoryRowContext<'a> {
    entry: &'a HistoryEntry,
    list_box: &'a ListBox,
    history: Arc<Mutex<History>>,
    result_text_view: &'a TextView,
    // ... remaining fields
}

fn create_history_row(ctx: &HistoryRowContext) -> ListBoxRow
```

---

### Phase 1: Complete Services Migration (1 week)

**Goal:** Make services layer the only path to recorders. Remove hybrid state.

#### P1.1: Audit Current Service Usage

**Files to migrate:**

| File | Current Pattern | Target Pattern |
|------|-----------------|----------------|
| ui/recording.rs | Direct `AudioRecorder` access | `ctx.audio.start_dictation()` |
| ui/continuous.rs | Direct `ContinuousRecorder` access | `ctx.audio.start_continuous()` |
| ui/conference.rs | Direct `ConferenceRecorder` access | `ctx.audio.start_conference()` |
| main.rs | Creates recorders directly | Use `AppContext` only |

#### P1.2: Migrate Recording Handler

**File:** `src/ui/recording.rs`

**Before:**
```rust
pub fn handle_start(ctx: &AppContext, rec_ctx: &RecordingContext, ui: &DictationUI) {
    let recorder = ctx.audio.mic_recorder();  // Legacy accessor
    if let Err(e) = recorder.start_recording() {
        // ...
    }
}
```

**After:**
```rust
pub fn handle_start(ctx: &AppContext, rec_ctx: &RecordingContext, ui: &DictationUI) {
    if let Err(e) = ctx.audio.start_dictation() {
        // ...
    }
}

pub fn handle_stop(ctx: &AppContext, rec_ctx: &RecordingContext, ui: &DictationUI) {
    let (samples, completion) = ctx.audio.stop_dictation();
    // Process samples...
}
```

#### P1.3: Migrate Continuous Handler

**File:** `src/ui/continuous.rs`

**Key changes:**
```rust
// Before
let continuous_recorder = ctx.audio.continuous_recorder();
continuous_recorder.start_continuous(segment_tx)?;

// After
ctx.audio.start_continuous(segment_tx)?;
```

#### P1.4: Migrate Conference Handler

**File:** `src/ui/conference.rs`

**Key changes:**
```rust
// Before
let conference_recorder = ctx.audio.conference_recorder();
conference_recorder.start_conference()?;
let (mic, loopback, _, _) = conference_recorder.stop_conference();

// After
ctx.audio.start_conference()?;
let result = ctx.audio.stop_conference();
// Use result.mic_samples, result.loopback_samples
```

#### P1.5: Remove Legacy Accessors

**File:** `src/services/audio.rs`

**Delete these methods:**
```rust
// DELETE these legacy accessors
pub fn mic_recorder(&self) -> &Arc<AudioRecorder> { ... }
pub fn conference_recorder(&self) -> &Arc<ConferenceRecorder> { ... }
pub fn continuous_recorder(&self) -> &Arc<ContinuousRecorder> { ... }
```

#### P1.6: Remove Dead Code Annotations

After migration, remove all `#[allow(dead_code)]` from:
- `src/services/audio.rs`
- `src/services/transcription.rs`
- `src/context.rs`
- `src/ui/state.rs`

**Verification:**
```bash
# Should compile without dead_code warnings
cargo build 2>&1 | grep -c "dead_code"
# Expected: 0
```

#### P1.7: Simplify AppContext

**File:** `src/context.rs`

**Remove convenience methods that bypass services:**
```rust
// DELETE - these bypass the service layer
pub fn config_arc(&self) -> Arc<Mutex<Config>> { ... }
pub fn history_arc(&self) -> Arc<Mutex<History>> { ... }
pub fn diarization_arc(&self) -> Arc<Mutex<DiarizationEngine>> { ... }
```

**Keep only essential accessors:**
```rust
impl AppContext {
    // Services (primary interface)
    pub fn audio(&self) -> &AudioService { &self.audio }
    pub fn transcription(&self) -> &TranscriptionService { ... }

    // Configuration (read-only convenience)
    pub fn language(&self) -> String { ... }
    pub fn recording_mode(&self) -> String { ... }
    pub fn continuous_mode(&self) -> bool { ... }
}
```

---

### Phase 2: Dependency Inversion (1-2 weeks)

**Goal:** Introduce traits for core abstractions, enabling testability and flexibility.

#### P2.1: Create Domain Traits Module

**New file:** `src/traits.rs`

```rust
//! Core domain traits for dependency inversion.
//!
//! These traits define the contracts between layers without
//! depending on concrete implementations.

use anyhow::Result;

/// Audio recording abstraction
pub trait AudioRecording: Send + Sync {
    /// Start recording audio
    fn start(&self) -> Result<()>;

    /// Stop recording and return samples
    fn stop(&self) -> (Vec<f32>, Option<async_channel::Receiver<()>>);

    /// Get current amplitude (0.0 - 1.0)
    fn amplitude(&self) -> f32;

    /// Check if currently recording
    fn is_recording(&self) -> bool;
}

/// Speech-to-text transcription abstraction
pub trait Transcription: Send + Sync {
    /// Transcribe audio samples to text
    fn transcribe(&self, samples: &[f32], language: &str) -> Result<String>;

    /// Check if a model is loaded
    fn is_loaded(&self) -> bool;

    /// Get the name of the loaded model
    fn model_name(&self) -> Option<String>;
}

/// Voice activity detection abstraction
pub trait VoiceDetection {
    /// Check if audio frame contains speech
    fn is_speech(&self, samples: &[f32]) -> Result<bool>;

    /// Detect end of speech (silence after speech)
    fn detect_speech_end(&self, samples: &[f32]) -> Result<bool>;
}

/// History storage abstraction
pub trait HistoryRepository: Send + Sync {
    /// Add a new entry
    fn add(&mut self, entry: HistoryEntry) -> Result<()>;

    /// Get all entries
    fn entries(&self) -> &[HistoryEntry];

    /// Search entries by text
    fn search(&self, query: &str) -> Vec<&HistoryEntry>;

    /// Remove old entries
    fn cleanup(&mut self, max_age_days: u32);

    /// Persist to storage
    fn save(&self) -> Result<()>;
}

/// Configuration abstraction
pub trait ConfigProvider: Send + Sync {
    fn language(&self) -> String;
    fn default_model(&self) -> String;
    fn auto_copy(&self) -> bool;
    fn auto_paste(&self) -> bool;
    fn continuous_mode(&self) -> bool;
    fn recording_mode(&self) -> String;
}
```

#### P2.2: Implement Traits for Existing Types

**File:** `src/audio.rs`

```rust
use crate::traits::AudioRecording;

impl AudioRecording for AudioRecorder {
    fn start(&self) -> Result<()> {
        self.start_recording()
    }

    fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.stop_recording()
    }

    fn amplitude(&self) -> f32 {
        self.get_amplitude()
    }

    fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}
```

**File:** `src/whisper.rs`

```rust
use crate::traits::Transcription;

impl Transcription for WhisperSTT {
    fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        self.transcribe(samples, language)
    }

    fn is_loaded(&self) -> bool {
        true // WhisperSTT is only created when model loads
    }

    fn model_name(&self) -> Option<String> {
        Some(self.model_path.clone())
    }
}
```

#### P2.3: Update Services to Use Traits

**File:** `src/services/audio.rs`

```rust
use crate::traits::AudioRecording;

pub struct AudioService {
    mic: Arc<dyn AudioRecording>,
    continuous: Arc<ContinuousRecorder>,
    conference: Arc<ConferenceRecorder>,
}

impl AudioService {
    pub fn new(mic: Arc<dyn AudioRecording>, ...) -> Self { ... }

    // For production
    pub fn with_default_recorders(config: ContinuousConfig) -> Result<Self> {
        Self::new(
            Arc::new(AudioRecorder::new()),
            // ...
        )
    }
}
```

#### P2.4: Create Mock Implementations for Testing

**New file:** `src/test_support/mocks.rs`

```rust
use crate::traits::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct MockAudioRecorder {
    is_recording: AtomicBool,
    samples_to_return: Vec<f32>,
}

impl MockAudioRecorder {
    pub fn new() -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            samples_to_return: vec![0.0; 16000], // 1 second of silence
        }
    }

    pub fn with_samples(samples: Vec<f32>) -> Self {
        Self {
            is_recording: AtomicBool::new(false),
            samples_to_return: samples,
        }
    }
}

impl AudioRecording for MockAudioRecorder {
    fn start(&self) -> Result<()> {
        self.is_recording.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.is_recording.store(false, Ordering::SeqCst);
        (self.samples_to_return.clone(), None)
    }

    fn amplitude(&self) -> f32 { 0.5 }
    fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}

pub struct MockTranscription {
    result: String,
}

impl MockTranscription {
    pub fn returning(text: &str) -> Self {
        Self { result: text.to_string() }
    }
}

impl Transcription for MockTranscription {
    fn transcribe(&self, _: &[f32], _: &str) -> Result<String> {
        Ok(self.result.clone())
    }

    fn is_loaded(&self) -> bool { true }
    fn model_name(&self) -> Option<String> { Some("mock".to_string()) }
}
```

#### P2.5: Update AppContext to Use Traits

**File:** `src/context.rs`

```rust
use crate::traits::{AudioRecording, Transcription, HistoryRepository};

pub struct AppContext {
    audio: Arc<AudioService>,
    transcription: Arc<dyn Transcription>,
    history: Arc<Mutex<dyn HistoryRepository>>,
    config: Arc<Mutex<Config>>,
    channels: Arc<UIChannels>,
}
```

---

### Phase 3: Recording Mode Polymorphism (1 week)

**Goal:** Replace conditional logic with trait-based polymorphism.

#### P3.1: Define Recording Mode Trait

**New file:** `src/modes/mod.rs`

```rust
pub mod dictation;
pub mod continuous;
pub mod conference;

use crate::context::AppContext;
use crate::ui::state::RecordingContext;
use anyhow::Result;

/// Recording mode abstraction
pub trait RecordingMode {
    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Start recording
    fn start(&self, ctx: &AppContext, rec_ctx: &RecordingContext) -> Result<()>;

    /// Stop recording and return transcription
    fn stop(&self, ctx: &AppContext, rec_ctx: &RecordingContext) -> Result<String>;

    /// Get current amplitude for UI
    fn amplitude(&self, ctx: &AppContext) -> f32;

    /// Check if this mode supports continuous operation
    fn is_continuous(&self) -> bool { false }
}
```

#### P3.2: Implement Mode Structs

**File:** `src/modes/dictation.rs`

```rust
use super::RecordingMode;

pub struct DictationMode;

impl RecordingMode for DictationMode {
    fn name(&self) -> &'static str { "dictation" }

    fn start(&self, ctx: &AppContext, rec_ctx: &RecordingContext) -> Result<()> {
        ctx.audio.start_dictation()?;
        rec_ctx.start_recording();
        Ok(())
    }

    fn stop(&self, ctx: &AppContext, rec_ctx: &RecordingContext) -> Result<String> {
        let (samples, _) = ctx.audio.stop_dictation();
        rec_ctx.start_processing();

        let language = ctx.language();
        let text = ctx.transcription.lock().unwrap()
            .transcribe(&samples, &language)?;

        rec_ctx.finish();
        Ok(text)
    }

    fn amplitude(&self, ctx: &AppContext) -> f32 {
        ctx.audio.get_dictation_amplitude()
    }
}
```

**File:** `src/modes/continuous.rs`

```rust
pub struct ContinuousMode {
    segment_tx: Sender<AudioSegment>,
}

impl ContinuousMode {
    pub fn new(segment_tx: Sender<AudioSegment>) -> Self {
        Self { segment_tx }
    }
}

impl RecordingMode for ContinuousMode {
    fn name(&self) -> &'static str { "continuous" }

    fn is_continuous(&self) -> bool { true }

    fn start(&self, ctx: &AppContext, rec_ctx: &RecordingContext) -> Result<()> {
        ctx.audio.start_continuous(self.segment_tx.clone())?;
        rec_ctx.start_recording();
        Ok(())
    }

    // ... etc
}
```

#### P3.3: Create Mode Factory

**File:** `src/modes/factory.rs`

```rust
use super::*;

pub fn create_mode(
    mode_name: &str,
    continuous_enabled: bool,
    segment_tx: Option<Sender<AudioSegment>>,
) -> Box<dyn RecordingMode> {
    match mode_name {
        "conference" => Box::new(ConferenceMode),
        "dictation" if continuous_enabled => {
            Box::new(ContinuousMode::new(segment_tx.unwrap()))
        }
        _ => Box::new(DictationMode),
    }
}
```

#### P3.4: Simplify UI Handler

**File:** `src/ui/mod.rs`

**Before (conditional logic):**
```rust
match rec_ctx.state.get() {
    AppState::Idle => {
        if is_conference {
            conference::handle_start(&ctx, &rec_ctx, &conference_ui);
        } else if is_continuous {
            continuous::handle_start(&ctx, &rec_ctx, &continuous_ui);
        } else {
            recording::handle_start(&ctx, &rec_ctx, &dictation_ui);
        }
    }
    // ...
}
```

**After (polymorphic dispatch):**
```rust
let mode = modes::create_mode(
    &ctx.recording_mode(),
    ctx.continuous_mode(),
    Some(segment_tx.clone()),
);

match rec_ctx.state.get() {
    AppState::Idle => {
        if let Err(e) = mode.start(&ctx, &rec_ctx) {
            show_error(&ui, &e);
        }
    }
    AppState::Recording => {
        match mode.stop(&ctx, &rec_ctx) {
            Ok(text) => ui.set_result_text(&text),
            Err(e) => show_error(&ui, &e),
        }
    }
    // ...
}
```

---

### Phase 4: Domain Layer Extraction (2 weeks)

**Goal:** Separate pure domain logic from infrastructure concerns.

#### P4.1: Create Domain Module Structure

```bash
mkdir -p src/domain
touch src/domain/mod.rs
touch src/domain/audio.rs
touch src/domain/transcription.rs
touch src/domain/history.rs
touch src/domain/config.rs
```

#### P4.2: Extract History Domain Entity

**File:** `src/domain/history.rs`

```rust
//! History domain entity and business rules.
//!
//! This module contains pure domain logic with no external dependencies.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// A transcription history entry (domain entity)
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    id: Uuid,
    text: String,
    timestamp: DateTime<Utc>,
    duration_secs: f32,
    mode: RecordingMode,
    word_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordingMode {
    Dictation,
    Continuous,
    Conference,
}

impl HistoryEntry {
    pub fn new(text: String, duration_secs: f32, mode: RecordingMode) -> Self {
        let word_count = text.split_whitespace().count();
        Self {
            id: Uuid::new_v4(),
            text,
            timestamp: Utc::now(),
            duration_secs,
            mode,
            word_count,
        }
    }

    pub fn id(&self) -> Uuid { self.id }
    pub fn text(&self) -> &str { &self.text }
    pub fn timestamp(&self) -> DateTime<Utc> { self.timestamp }
    pub fn duration_secs(&self) -> f32 { self.duration_secs }
    pub fn mode(&self) -> RecordingMode { self.mode }
    pub fn word_count(&self) -> usize { self.word_count }

    /// Preview of text (first N characters)
    pub fn preview(&self, max_chars: usize) -> &str {
        if self.text.len() <= max_chars {
            &self.text
        } else {
            &self.text[..max_chars]
        }
    }

    /// Check if entry matches search query
    pub fn matches(&self, query: &str) -> bool {
        self.text.to_lowercase().contains(&query.to_lowercase())
    }

    /// Check if entry is older than given days
    pub fn is_older_than(&self, days: u32) -> bool {
        let age = Utc::now() - self.timestamp;
        age.num_days() > days as i64
    }
}

/// History collection with business rules
pub struct History {
    entries: Vec<HistoryEntry>,
}

impl History {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn add(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry); // Most recent first
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        self.entries.iter()
            .filter(|e| e.matches(query))
            .collect()
    }

    /// Remove entries older than max_age_days
    pub fn cleanup_old(&mut self, max_age_days: u32) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| !e.is_older_than(max_age_days));
        before - self.entries.len()
    }

    /// Trim to maximum entries
    pub fn trim_to_limit(&mut self, max_entries: usize) -> usize {
        if self.entries.len() <= max_entries {
            return 0;
        }
        let removed = self.entries.len() - max_entries;
        self.entries.truncate(max_entries);
        removed
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_word_count() {
        let entry = HistoryEntry::new(
            "Hello world test".to_string(),
            1.5,
            RecordingMode::Dictation,
        );
        assert_eq!(entry.word_count(), 3);
    }

    #[test]
    fn test_entry_matches_case_insensitive() {
        let entry = HistoryEntry::new(
            "Hello World".to_string(),
            1.0,
            RecordingMode::Dictation,
        );
        assert!(entry.matches("hello"));
        assert!(entry.matches("WORLD"));
    }

    #[test]
    fn test_history_search() {
        let mut history = History::new();
        history.add(HistoryEntry::new("apple".to_string(), 1.0, RecordingMode::Dictation));
        history.add(HistoryEntry::new("banana".to_string(), 1.0, RecordingMode::Dictation));
        history.add(HistoryEntry::new("apple pie".to_string(), 1.0, RecordingMode::Dictation));

        let results = history.search("apple");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_history_trim() {
        let mut history = History::new();
        for i in 0..10 {
            history.add(HistoryEntry::new(
                format!("entry {}", i),
                1.0,
                RecordingMode::Dictation,
            ));
        }

        let removed = history.trim_to_limit(5);
        assert_eq!(removed, 5);
        assert_eq!(history.len(), 5);
    }
}
```

#### P4.3: Create Infrastructure Adapter

**File:** `src/infra/storage/history.rs`

```rust
//! JSON-based history persistence.

use crate::domain::history::{History, HistoryEntry, RecordingMode};
use crate::traits::HistoryRepository;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub struct JsonHistoryRepository {
    history: History,
    path: PathBuf,
}

impl JsonHistoryRepository {
    pub fn new(path: PathBuf) -> Self {
        Self {
            history: History::new(),
            path,
        }
    }

    pub fn load(path: PathBuf) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read history from {:?}", path))?;

        let history: History = serde_json::from_str(&content)
            .with_context(|| "Failed to parse history JSON")?;

        Ok(Self { history, path })
    }
}

impl HistoryRepository for JsonHistoryRepository {
    fn add(&mut self, entry: HistoryEntry) -> Result<()> {
        self.history.add(entry);
        self.save()
    }

    fn entries(&self) -> &[HistoryEntry] {
        self.history.entries()
    }

    fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        self.history.search(query)
    }

    fn cleanup(&mut self, max_age_days: u32) {
        self.history.cleanup_old(max_age_days);
    }

    fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self.history.entries())
            .context("Failed to serialize history")?;

        fs::write(&self.path, json)
            .with_context(|| format!("Failed to write history to {:?}", self.path))?;

        Ok(())
    }
}
```

#### P4.4: Reorganize Module Structure

**Final structure after Phase 4:**

```
src/
├── main.rs                 # Entry point only
├── traits.rs               # Core abstractions
├── types.rs                # Shared types
│
├── domain/                 # Pure business logic (no deps)
│   ├── mod.rs
│   ├── history.rs          # History entity
│   ├── audio.rs            # Audio processing rules
│   └── config.rs           # Config entity
│
├── app/                    # Application layer
│   ├── mod.rs
│   ├── context.rs          # AppContext (DI)
│   └── services/
│       ├── audio.rs        # AudioService
│       └── transcription.rs
│
├── ui/                     # Presentation layer
│   ├── mod.rs
│   ├── state.rs
│   ├── recording.rs
│   ├── continuous.rs
│   ├── conference.rs
│   └── dialogs/
│       ├── history.rs
│       ├── model.rs
│       └── settings.rs
│
├── modes/                  # Recording mode strategies
│   ├── mod.rs
│   ├── dictation.rs
│   ├── continuous.rs
│   └── conference.rs
│
└── infra/                  # Infrastructure implementations
    ├── mod.rs
    ├── audio/
    │   ├── cpal.rs         # Microphone (impl AudioRecording)
    │   ├── continuous.rs
    │   ├── loopback.rs
    │   └── vad.rs
    ├── stt/
    │   ├── whisper.rs      # impl Transcription
    │   └── diarization.rs
    ├── storage/
    │   ├── config.rs       # TOML persistence
    │   ├── history.rs      # JSON persistence
    │   └── models.rs       # Model file management
    └── system/
        ├── tray.rs
        ├── hotkeys.rs
        └── clipboard.rs
```

---

### Phase 5: Testing Infrastructure (Ongoing)

**Goal:** Achieve >60% test coverage with meaningful tests.

#### P5.1: Unit Tests for Domain Layer

```rust
// src/domain/history.rs - tests already included above

// src/domain/audio.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amplitude_calculation() {
        let samples = vec![0.5, -0.5, 0.3, -0.3];
        let amplitude = calculate_amplitude(&samples);
        assert!((amplitude - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_resample_preserves_duration() {
        let input = vec![0.0; 44100]; // 1 second at 44.1kHz
        let output = resample_to_16khz(&input, 44100);
        assert_eq!(output.len(), 16000); // 1 second at 16kHz
    }
}
```

#### P5.2: Integration Tests

**File:** `tests/integration_tests.rs`

```rust
use voice_dictation::*;
use voice_dictation::test_support::mocks::*;

#[test]
fn test_dictation_workflow() {
    // Setup
    let recorder = Arc::new(MockAudioRecorder::with_samples(
        generate_sine_wave(16000, 1.0) // 1 second
    ));
    let transcriber = Arc::new(MockTranscription::returning("hello world"));

    let service = AudioService::new(recorder, ...);

    // Execute
    service.start_dictation().unwrap();
    let (samples, _) = service.stop_dictation();
    let text = transcriber.transcribe(&samples, "en").unwrap();

    // Verify
    assert_eq!(text, "hello world");
    assert_eq!(samples.len(), 16000);
}

#[test]
fn test_history_persistence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("history.json");

    // Create and save
    let mut repo = JsonHistoryRepository::new(path.clone());
    repo.add(HistoryEntry::new("test".to_string(), 1.0, RecordingMode::Dictation)).unwrap();

    // Load and verify
    let loaded = JsonHistoryRepository::load(path).unwrap();
    assert_eq!(loaded.entries().len(), 1);
    assert_eq!(loaded.entries()[0].text(), "test");
}
```

#### P5.3: CI/CD Pipeline

**File:** `.github/workflows/ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libgtk-4-dev libadwaita-1-dev libasound2-dev

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Check formatting
        run: cargo fmt --check

      - name: Clippy (strict)
        run: cargo clippy -- -D warnings

      - name: Build
        run: cargo build --release

      - name: Run tests
        run: cargo test --all-features

      - name: Generate coverage
        run: |
          cargo install cargo-tarpaulin --locked || true
          cargo tarpaulin --out Xml

      - name: Upload coverage
        uses: codecov/codecov-action@v4
        with:
          files: cobertura.xml

  architecture-fitness:
    runs-on: ubuntu-latest
    needs: check
    steps:
      - uses: actions/checkout@v4

      - name: Install rust-analyzer
        run: |
          curl -L https://github.com/rust-lang/rust-analyzer/releases/latest/download/rust-analyzer-x86_64-unknown-linux-gnu.gz | gunzip -c - > ~/.cargo/bin/rust-analyzer
          chmod +x ~/.cargo/bin/rust-analyzer

      - name: Build SCIP index
        run: |
          cargo install scip-rust || true
          scip-rust index

      - name: Check architecture fitness
        run: |
          # Custom script to verify architecture rules
          ./scripts/check-architecture.sh
```

---

## Detailed Task Breakdown

### Phase 0 Tasks

| Task ID | Description | Est. Hours | Dependencies |
|---------|-------------|------------|--------------|
| P0.1.1 | Fix empty_line_after_outer_attr in services/audio.rs | 0.25 | - |
| P0.1.2 | Fix type_complexity in conference_recorder.rs | 0.5 | - |
| P0.1.3 | Fix redundant_closure in continuous.rs | 0.25 | - |
| P0.1.4 | Fix writeln_empty_string in history.rs (3 places) | 0.25 | - |
| P0.1.5 | Fix map_flatten in history_dialog.rs | 0.25 | - |
| P0.1.6 | Fix too_many_arguments in history_dialog.rs | 1.0 | - |
| P0.2.1 | Analyze VAD threading requirements | 0.5 | - |
| P0.2.2 | Refactor VoiceActivityDetector to Rc<RefCell> | 1.0 | P0.2.1 |
| P0.3.1 | Move ConferenceRecording to types.rs | 0.5 | - |
| P0.3.2 | Update conference_recorder.rs to return struct | 0.5 | P0.3.1 |
| P0.4.1 | Verify all clippy warnings resolved | 0.5 | P0.1.* |

### Phase 1 Tasks

| Task ID | Description | Est. Hours | Dependencies |
|---------|-------------|------------|--------------|
| P1.1.1 | Audit service usage in ui/recording.rs | 1.0 | P0.* |
| P1.1.2 | Audit service usage in ui/continuous.rs | 1.0 | P0.* |
| P1.1.3 | Audit service usage in ui/conference.rs | 1.0 | P0.* |
| P1.2.1 | Migrate recording.rs to AudioService | 2.0 | P1.1.1 |
| P1.3.1 | Migrate continuous.rs to AudioService | 3.0 | P1.1.2 |
| P1.4.1 | Migrate conference.rs to AudioService | 3.0 | P1.1.3 |
| P1.5.1 | Remove legacy accessors from AudioService | 1.0 | P1.2-4 |
| P1.6.1 | Remove all #[allow(dead_code)] | 1.0 | P1.5.1 |
| P1.7.1 | Simplify AppContext, remove legacy methods | 2.0 | P1.6.1 |
| P1.8.1 | Integration testing of migrated code | 4.0 | P1.7.1 |

### Phase 2-5 Tasks

*(Detailed breakdown to be refined after Phase 1 completion)*

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Breaking changes during migration | High | Medium | Incremental changes, comprehensive testing |
| Hidden dependencies not in codegraph | Medium | High | Manual code review before changes |
| GTK threading issues | Medium | High | Document threading model, review async patterns |
| Performance regression | Low | Medium | Benchmark critical paths before/after |
| Incomplete migration | Medium | High | Clear milestones, regular checkpoints |

---

## Success Metrics

### After Phase 0

- [ ] `cargo clippy -- -D warnings` passes
- [ ] No `Arc<Mutex<T>>` with non-Send types
- [ ] All complex return types use named structs

### After Phase 1

- [ ] Zero `#[allow(dead_code)]` in services layer
- [ ] All recording operations go through `AudioService`
- [ ] `AppContext` has no `_arc()` legacy accessors
- [ ] All tests pass

### After Phase 2

- [ ] Core traits defined in `src/traits.rs`
- [ ] All services use trait bounds
- [ ] Mock implementations available for testing
- [ ] Test coverage > 40%

### After Phase 3

- [ ] Recording mode selection is polymorphic
- [ ] No conditional mode logic in UI handlers
- [ ] Easy to add new recording modes

### After Phase 4

- [ ] Clear layer separation in directory structure
- [ ] Domain layer has zero external dependencies
- [ ] Test coverage > 60%

### After Phase 5

- [ ] CI/CD pipeline running
- [ ] Architecture fitness checks automated
- [ ] All fitness functions passing

---

## Appendices

### Appendix A: File Change Summary

| File | Phase | Change Type |
|------|-------|-------------|
| src/services/audio.rs | 0, 1 | Modify |
| src/conference_recorder.rs | 0 | Modify |
| src/continuous.rs | 0 | Modify |
| src/history.rs | 0 | Modify |
| src/history_dialog.rs | 0 | Modify |
| src/vad.rs | 0 | Modify |
| src/types.rs | 0 | Create |
| src/ui/recording.rs | 1 | Modify |
| src/ui/continuous.rs | 1 | Modify |
| src/ui/conference.rs | 1 | Modify |
| src/context.rs | 1, 2 | Modify |
| src/traits.rs | 2 | Create |
| src/test_support/mocks.rs | 2 | Create |
| src/audio.rs | 2 | Modify |
| src/whisper.rs | 2 | Modify |
| src/modes/ | 3 | Create |
| src/domain/ | 4 | Create |
| src/infra/ | 4 | Create |
| tests/ | 5 | Create |
| .github/workflows/ci.yml | 5 | Create |

### Appendix B: Commands Reference

```bash
# Check all clippy warnings
cargo clippy -- -D warnings

# Run all tests
cargo test --all-features

# Check for dead code
cargo build 2>&1 | grep "dead_code"

# Generate documentation
cargo doc --no-deps --open

# Count lines by file
find src -name "*.rs" -exec wc -l {} \; | sort -n

# Check for #[allow(dead_code)]
grep -r "allow(dead_code)" src/
```

### Appendix C: References

- Martin, R. C. (2017). *Clean Architecture: A Craftsman's Guide to Software Structure and Design*
- Martin, R. C. (2008). *Clean Code: A Handbook of Agile Software Craftsmanship*
- docs/architecture-fitness-methodology.md
- docs/audit/RECOMMENDATIONS.md
- docs/audit/architecture-overview-and-design-findings.md

---

*Document Version: 1.0*
*Last Updated: 2026-01-28*
