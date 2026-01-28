# Comprehensive Refactoring Plan

**Project:** Voice Dictation (s2t)
**Created:** 2026-01-28
**Last Updated:** 2026-01-28 (post-fitness assessment)
**Methodology:** Clean Architecture (Robert C. Martin) + Architecture Fitness Functions
**Status:** Phases 0-5 partially complete; reassessed and restructured

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current State Assessment](#current-state-assessment)
3. [What Has Been Completed](#what-has-been-completed)
4. [Architecture Vision](#architecture-vision)
5. [Refactoring Phases (Revised)](#refactoring-phases-revised)
   - [Phase A: Wire Domain Traits](#phase-a-wire-domain-traits)
   - [Phase B: Eliminate Tray Duplication](#phase-b-eliminate-tray-duplication)
   - [Phase C: Tame UI State Hotspot](#phase-c-tame-ui-state-hotspot)
   - [Phase D: Decompose Oversized Modules](#phase-d-decompose-oversized-modules)
   - [Phase E: Layer Enforcement](#phase-e-layer-enforcement)
   - [Phase F: Testing & CI](#phase-f-testing--ci)
6. [Risk Assessment](#risk-assessment)
7. [Success Metrics](#success-metrics)
8. [Appendices](#appendices)

---

## Executive Summary

### Background

The original refactoring plan (Phases 0-5) has been **partially executed**. Significant structural improvements have been made — `AppContext`, `traits.rs`, `services/`, UI module split, `UIChannels`, `types.rs`, and `test_support/` all exist. However, an Architecture Fitness Assessment reveals the refactoring is incomplete, leaving the codebase in a **hybrid state** where new architectural structures coexist with legacy patterns.

### Problem Statement

The architecture scores **2.6/5.0** on fitness functions:
- **FF-1 FAIL (2/5):** Domain traits defined but not implemented by concrete types
- **FF-2 MIXED (3/5):** `ui/state.rs` violates the Stable Dependencies Principle
- **FF-3 WARNING (3/5):** 241 hotspot symbols; UI state fields have ≥27 callers in unstable module
- **FF-4 WARNING (3/5):** 3 modules approaching the 200-symbol cohesion limit
- **FF-5 INCONCLUSIVE (2/5):** Flat module hierarchy prevents layer enforcement

### Goal

Complete the transition to Clean Architecture by:
1. **Wiring** the existing traits to concrete implementations (the #1 gap)
2. **Eliminating** the tray duplication that bypasses AppContext
3. **Stabilizing** the UI state hotspot
4. **Decomposing** oversized modules before they cross thresholds
5. **Enforcing** layer boundaries structurally

### Target Fitness Scores

| Fitness Function | Current | Target |
|-----------------|---------|--------|
| FF-1: Dependency Direction | 2/5 | 5/5 |
| FF-2: Component Instability | 3/5 | 4/5 |
| FF-3: Hotspot Risk | 3/5 | 4/5 |
| FF-4: Module Cohesion | 3/5 | 5/5 |
| FF-5: Cyclic Dependencies | 2/5 | 4/5 |
| **Overall** | **2.6** | **4.4** |

---

## Current State Assessment

### Codebase Metrics

| Metric | Value |
|--------|-------|
| Files | 36 |
| Lines of Code | 7,086 |
| Symbols | 788 |
| Max symbols/module | 156 (dialogs/model.rs) |
| Trait abstractions defined | 5 |
| Trait abstractions wired | 0 (production) |
| `#[allow(dead_code)]` annotations | ~5-10 |
| Hotspot symbols (≥20 callers) | 13 |

### Architecture Fitness Scorecard

| Fitness Function | Score | Status | Key Issue |
|-----------------|-------|--------|-----------|
| FF-1: Dependency Direction | 2/5 | **FAIL** | Traits defined, not implemented |
| FF-2: Component Instability | 3/5 | MIXED | ui/state.rs (I=0.42) has 7 dependents |
| FF-3: Hotspot Risk | 3/5 | WARNING | UIContext#status_label (27 callers) in unstable module |
| FF-4: Module Cohesion | 3/5 | WARNING | dialogs/model.rs (156 sym) nearing 200 limit |
| FF-5: Cyclic Dependencies | 2/5 | INCONCLUSIVE | Flat crate, no enforceable boundaries |

### Key Violations

| ID | Violation | Location | Phase |
|----|-----------|----------|-------|
| V1 | Traits not implemented by concrete types | traits.rs → (nothing) | A |
| V2 | Duplicate WhisperSTT for tray | main.rs:161-177 | B |
| V3 | AppContext leaks `config_arc()`, `history_arc()` | context.rs:85-92 | A |
| V4 | ui/state.rs is unstable hotspot (I=0.42, 7 deps) | ui/state.rs | C |
| V5 | Dialogs use concrete types, not traits | dialogs/* | A |
| V6 | audio.rs has 40-95 caller local vars (long functions) | audio.rs | D |
| V7 | No compile-time layer enforcement | main.rs (22 flat mods) | E |

---

## What Has Been Completed

### From Original Phase 0 (Quick Wins)

| Task | Status | Notes |
|------|--------|-------|
| P0.1 Fix clippy warnings | ✅ Mostly done | Some may remain; verify with `cargo clippy` |
| P0.2 Fix `Arc<Mutex<Vad>>` | ⚠️ Partially | vad.rs still uses `Arc<Mutex<Vad>>` |
| P0.3 ConferenceRecording struct | ✅ Done | `types.rs` created with shared types |
| P0.4 Fix parameter count | ⚠️ Unknown | Needs verification |

### From Original Phase 1 (Services Migration)

| Task | Status | Notes |
|------|--------|-------|
| P1.1 Service layer created | ✅ Done | `services/audio.rs`, `services/transcription.rs` exist |
| P1.2 Migrate recording handler | ✅ Done | `ui/recording.rs` uses AppContext |
| P1.3 Migrate continuous handler | ✅ Done | `ui/continuous.rs` uses AppContext |
| P1.4 Migrate conference handler | ✅ Done | `ui/conference.rs` uses AppContext |
| P1.5 Remove legacy accessors | ❌ Not done | `config_arc()`, `history_arc()` still exist |
| P1.6 Remove dead_code annotations | ❌ Not done | Still present |
| P1.7 Simplify AppContext | ❌ Not done | Legacy methods remain |

### From Original Phase 2 (Dependency Inversion)

| Task | Status | Notes |
|------|--------|-------|
| P2.1 Create domain traits | ✅ Done | `traits.rs` with 5 traits |
| P2.2 Implement traits for existing types | ❌ **Not done** | **Critical gap** |
| P2.3 Update services to use traits | ❌ Not done | Services use concrete types |
| P2.4 Create mock implementations | ✅ Partial | `test_support/mocks.rs` exists |
| P2.5 Update AppContext to use traits | ❌ Not done | Uses concrete types |

### From Original Phase 3 (Recording Mode Polymorphism)

| Task | Status | Notes |
|------|--------|-------|
| P3.1 UI module split | ✅ Done | `ui/` directory with 5 files |
| P3.2 Recording mode handlers | ✅ Done | Separate files per mode |
| P3.3 Mode factory / polymorphism | ❌ Not done | Still conditional logic |

### From Original Phase 4 (Domain Layer Extraction)

| Task | Status | Notes |
|------|--------|-------|
| P4.1 Domain module structure | ❌ Not done | No `domain/` directory |
| P4.2 Extract entities | ❌ Not done | |
| P4.3 Infrastructure adapters | ❌ Not done | |

### From Original Phase 5 (Testing Infrastructure)

| Task | Status | Notes |
|------|--------|-------|
| P5.1 Test support module | ✅ Done | `test_support/` exists |
| P5.2 Trait tests | ✅ Partial | `traits.rs` has test module |
| P5.3 CI/CD pipeline | ❌ Not done | |

### Summary

**Completed:** AppContext, UI split, services layer, traits definition, channels, types, test support structure.

**Not completed (critical):** Trait implementations, AppContext using trait objects, legacy accessor removal, tray migration, layer enforcement.

---

## Architecture Vision

### Target State

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         PRESENTATION LAYER                              │
│                                                                         │
│  ui/ — Window, widgets, event handlers                                 │
│  dialogs/ — Modal dialog windows                                       │
│  tray.rs — System tray (uses AppContext, not raw WhisperSTT)           │
│                                                                         │
│  Depends on: Application Layer via AppContext                          │
├─────────────────────────────────────────────────────────────────────────┤
│                         APPLICATION LAYER                               │
│                                                                         │
│  context.rs — AppContext (DI container using trait objects)             │
│  services/ — AudioService, TranscriptionService                        │
│  channels.rs — UIChannels                                              │
│  hotkeys.rs — Global hotkey management                                 │
│                                                                         │
│  Depends on: Domain traits (not concrete types)                        │
├─────────────────────────────────────────────────────────────────────────┤
│                           DOMAIN LAYER                                  │
│                                                                         │
│  traits.rs — AudioRecording, Transcription, VoiceDetection,            │
│              HistoryRepository, ConfigProvider                          │
│  types.rs — Shared types (ConferenceRecording, etc.)                   │
│                                                                         │
│  Depends on: Nothing                                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                       INFRASTRUCTURE LAYER                              │
│                                                                         │
│  audio.rs — impl AudioRecording for AudioRecorder                      │
│  whisper.rs — impl Transcription for WhisperSTT                        │
│  history.rs — impl HistoryRepository for History                       │
│  config.rs — impl ConfigProvider for Config                            │
│  vad.rs — impl VoiceDetection for VoiceActivityDetector                │
│  continuous.rs, loopback.rs, diarization.rs, etc.                      │
│                                                                         │
│  Depends on: Domain traits (implements them)                           │
└─────────────────────────────────────────────────────────────────────────┘
```

### Dependency Rule

```
Presentation → Application → Domain ← Infrastructure
                                ↑           │
                                └───────────┘
                          Infrastructure implements Domain traits
```

---

## Refactoring Phases (Revised)

The original 6-phase plan (0-5) is replaced by 6 focused phases (A-F) based on the fitness assessment findings. Each phase targets a specific fitness function improvement.

### Phase A: Wire Domain Traits

**Targets:** FF-1 (Dependency Direction), Violations V1, V3, V5
**Priority:** P0 — CRITICAL (the #1 architectural gap)

#### A.1: Implement `Transcription` for `WhisperSTT`

**File:** `src/whisper.rs`

```rust
use crate::traits::Transcription;

impl Transcription for WhisperSTT {
    fn transcribe(&self, samples: &[f32], language: &str) -> Result<String> {
        self.transcribe(samples, language)  // delegate to existing method
    }

    fn is_loaded(&self) -> bool {
        true  // WhisperSTT only exists when model is loaded
    }

    fn model_name(&self) -> Option<String> {
        Some(self.model_path.clone())
    }
}
```

#### A.2: Implement `HistoryRepository` for `History`

**File:** `src/history.rs`

```rust
use crate::traits::HistoryRepository;

impl HistoryRepository for History {
    type Entry = HistoryEntry;

    fn add(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
    }

    fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    fn search(&self, query: &str) -> Vec<&HistoryEntry> {
        self.entries.iter()
            .filter(|e| e.text.to_lowercase().contains(&query.to_lowercase()))
            .collect()
    }

    fn cleanup_old(&mut self, max_age_days: u32) -> usize {
        self.cleanup_old_entries(max_age_days)
    }

    fn trim_to_limit(&mut self, max_entries: usize) -> usize {
        self.trim_to_limit(max_entries)
    }

    fn save(&self) -> Result<()> {
        crate::history::save_history(self)
    }
}
```

#### A.3: Implement `ConfigProvider` for `Config`

**File:** `src/config.rs`

```rust
use crate::traits::ConfigProvider;

impl ConfigProvider for Config {
    fn language(&self) -> String { self.language.clone() }
    fn default_model(&self) -> String { self.default_model.clone() }
    fn auto_copy(&self) -> bool { self.auto_copy }
    fn auto_paste(&self) -> bool { self.auto_paste }
    fn continuous_mode(&self) -> bool { self.continuous_mode }
    fn recording_mode(&self) -> String { self.recording_mode.clone() }
}
```

#### A.4: Implement `VoiceDetection` for `VoiceActivityDetector`

**File:** `src/vad.rs`

```rust
use crate::traits::VoiceDetection;

impl VoiceDetection for VoiceActivityDetector {
    fn is_speech(&mut self, samples: &[f32]) -> Result<bool> {
        self.is_speech(samples)
    }

    fn detect_speech_end(&mut self, samples: &[f32]) -> Result<bool> {
        self.detect_speech_end(samples)
    }

    fn reset(&mut self) {
        self.reset()
    }
}
```

#### A.5: Update `TranscriptionService` to Use Trait

**File:** `src/services/transcription.rs`

Change internal storage from `Option<WhisperSTT>` to `Option<Box<dyn Transcription>>` or keep `WhisperSTT` but expose via trait interface. The key change is that `AppContext.transcription` uses the `Transcription` trait bound.

#### A.6: Remove Legacy Accessors from AppContext

**File:** `src/context.rs`

Remove:
```rust
// DELETE these
pub fn config_arc(&self) -> Arc<Mutex<Config>> { ... }
pub fn history_arc(&self) -> Arc<Mutex<History>> { ... }
```

Update callers (dialogs) to receive what they need via parameters or through `AppContext` service methods.

#### A.7: Verification

```bash
# All trait implementations compile
cargo build

# Tests pass
cargo test

# No more dead_code warnings for trait methods
cargo build 2>&1 | grep -c "dead_code"
```

**Success criteria:** Every trait in `traits.rs` has at least one production `impl`. `AppContext` no longer exposes raw `Arc<Mutex<T>>` handles for config/history.

---

### Phase B: Eliminate Tray Duplication

**Targets:** FF-3 (Hotspot Risk), Violation V2
**Priority:** P1 — HIGH (memory waste, architectural bypass)

#### B.1: Audit Tray's WhisperSTT Usage

Understand exactly what `DictationTray` does with `WhisperSTT`. The tray likely needs:
- Check if model is loaded (for status display)
- Possibly trigger transcription from tray

#### B.2: Migrate Tray to Use AppContext

**File:** `src/main.rs`

Replace:
```rust
// DELETE: Lines 159-177 (duplicate model loading)
let whisper_for_tray: Arc<Mutex<Option<WhisperSTT>>> = { ... };
```

With:
```rust
// Tray uses shared TranscriptionService via AppContext
let tray_handle = DictationTray::spawn_service(tray_tx, ctx.clone());
```

#### B.3: Update DictationTray API

**File:** `src/tray.rs`

Change `spawn_service` signature to accept `Arc<AppContext>` instead of separate `config` + `whisper` parameters:

```rust
pub fn spawn_service(
    tx: Sender<TrayAction>,
    ctx: Arc<AppContext>,
) -> TrayHandle {
    // Use ctx.transcription for model status
    // Use ctx.config for configuration
}
```

#### B.4: Verification

```bash
# Build succeeds without whisper_for_tray
cargo build

# Memory usage reduced (no duplicate model)
# Tray correctly shows model status
cargo run --release
```

**Success criteria:** `whisper_for_tray` eliminated from `main.rs`. Only one `WhisperSTT` instance exists at runtime.

---

### Phase C: Tame UI State Hotspot

**Targets:** FF-2 (Component Instability), FF-3 (Hotspot Risk), Violation V4
**Priority:** P2 — MEDIUM

#### C.1: Extract Stable Interface from UIContext

`UIContext` has fields like `status_label` (27 callers) and `button` (20 callers) that are implementation details (GTK widgets) leaking across 7 modules.

**New trait in `ui/state.rs`:**

```rust
/// Stable interface for UI state updates.
/// Handlers depend on this trait, not on UIContext's widget fields.
pub trait UIStateUpdater {
    fn set_status(&self, text: &str);
    fn set_button_label(&self, text: &str);
    fn set_button_sensitive(&self, sensitive: bool);
    fn set_result_text(&self, text: &str);
    fn show_error(&self, message: &str);
}
```

#### C.2: Implement UIStateUpdater for UIContext

```rust
impl UIStateUpdater for UIContext {
    fn set_status(&self, text: &str) {
        self.status_label.set_text(text);
    }
    fn set_button_label(&self, text: &str) {
        self.button.set_label(text);
    }
    // ... etc
}
```

#### C.3: Migrate UI Handlers to Use Trait

Update `ui/recording.rs`, `ui/continuous.rs`, `ui/conference.rs` to accept `&dyn UIStateUpdater` instead of `&UIContext` where possible. This decouples handlers from specific GTK widget types.

#### C.4: Move `AppState` to `types.rs`

`AppState` (Idle, Recording, Processing) is a domain concept, not a UI concept. Move it to `types.rs` to reduce `ui/state.rs` symbol count and coupling.

**Success criteria:** UI handlers depend on `UIStateUpdater` trait. Direct field access to `status_label`, `button` is confined to `UIContext` impl. `AppState` is in `types.rs`.

---

### Phase D: Decompose Oversized Modules

**Targets:** FF-4 (Module Cohesion), Violation V6
**Priority:** P3 — MEDIUM

#### D.1: Split `dialogs/model.rs` (156 symbols)

Current responsibilities:
- Model listing/display
- Model downloading
- Model file management

Split into:
```
dialogs/
├── model/
│   ├── mod.rs          # Re-exports, show_dialog()
│   ├── list.rs         # Model listing and display
│   └── download.rs     # Download progress and management
```

#### D.2: Split `dialogs/history.rs` (152 symbols)

Current responsibilities:
- History listing
- Search/filter
- Entry detail display
- Entry management (delete, export)

Split into:
```
dialogs/
├── history/
│   ├── mod.rs          # Re-exports, show_dialog()
│   ├── list.rs         # History listing and search
│   └── detail.rs       # Entry detail view and actions
```

#### D.3: Decompose Long Functions in `audio.rs`

`audio.rs` has local variables with 40-95 internal callers, indicating monolithic functions. Extract:
- `setup_audio_stream()` — device selection and stream configuration
- `process_audio_buffer()` — sample processing and resampling
- `manage_recording_state()` — start/stop state transitions

#### D.4: Verification

```bash
# Check no module exceeds 150 symbols after split
# (use codegraph get_file_symbols for each)
```

**Success criteria:** All modules under 150 symbols. No function in `audio.rs` with more than 30 internal variable references.

---

### Phase E: Layer Enforcement

**Targets:** FF-5 (Cyclic Dependencies), Violation V7
**Priority:** P4 — LOW (long-term)

#### E.1: Module Visibility Restrictions

Use `pub(crate)` and `pub(super)` to restrict cross-layer access:

```rust
// Infrastructure modules should not be directly accessible from UI
// audio.rs
pub(crate) struct AudioRecorder { ... }  // Only services/ can use this

// UI should only access through AppContext
// context.rs
pub struct AppContext { ... }  // Public interface
```

#### E.2: Consider Workspace Crates (Future)

For strict enforcement, split into workspace crates:
```
s2t/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── s2t-domain/         # traits.rs, types.rs (zero deps)
│   ├── s2t-infra/          # audio, whisper, history, config impls
│   ├── s2t-services/       # AudioService, TranscriptionService
│   └── s2t-app/            # main.rs, ui/, dialogs/, tray
```

This makes layer violations into **compile errors**. However, this is a major restructuring and should only be done when the trait-based architecture is fully wired.

#### E.3: Architecture Fitness CI Check

Add a CI step that runs codegraph analysis and fails if:
- Any module exceeds 200 symbols
- Any new trait is defined without at least one impl
- Any domain module imports from UI

---

### Phase F: Testing & CI

**Targets:** All fitness functions (validation)
**Priority:** P5 — ONGOING

#### F.1: Unit Tests for Trait Implementations

After Phase A, add tests that exercise concrete types through trait interfaces:

```rust
#[test]
fn test_whisper_implements_transcription_trait() {
    // This test verifies the trait contract, not WhisperSTT internals
    let stt: Box<dyn Transcription> = Box::new(WhisperSTT::new("model.bin")?);
    assert!(stt.is_loaded());
}
```

#### F.2: Integration Tests with Mocks

```rust
#[test]
fn test_dictation_workflow_with_mocks() {
    let mock_recorder = MockAudioRecorder::with_samples(sine_wave(16000));
    let mock_transcriber = MockTranscription::returning("hello world");
    // ... exercise full workflow through AppContext
}
```

#### F.3: CI Pipeline

```yaml
# .github/workflows/ci.yml
jobs:
  check:
    steps:
      - cargo fmt --check
      - cargo clippy -- -D warnings
      - cargo build --release
      - cargo test --all-features

  architecture:
    steps:
      - # Run architecture fitness checks
      - # Verify all traits have implementations
      - # Check module sizes
```

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Trait implementation breaks existing API | Medium | High | Add `impl` blocks without changing existing method signatures |
| Tray migration breaks system tray | Medium | Medium | Test tray independently before integration |
| UI state refactoring breaks handlers | Medium | Medium | Incremental migration, one handler at a time |
| Dialog split changes public API | Low | Low | Re-export from `mod.rs` for backwards compat |
| Workspace split is too disruptive | High | High | Defer to Phase E; use visibility restrictions first |

### Migration Safety Rules

1. **One phase at a time.** Complete Phase A before starting Phase B.
2. **Green builds between steps.** Every step within a phase must leave `cargo build` and `cargo test` passing.
3. **No feature changes.** These are purely structural refactorings — zero behavioral changes.
4. **Trait impls are additive.** Adding `impl Transcription for WhisperSTT` does not change any existing code. It only adds a new capability.
5. **Measure before and after.** Run architecture fitness assessment at the start and end of each phase.

---

## Success Metrics

### After Phase A (Wire Domain Traits)

- [ ] `impl Transcription for WhisperSTT` compiles
- [ ] `impl HistoryRepository for History` compiles
- [ ] `impl ConfigProvider for Config` compiles
- [ ] `impl VoiceDetection for VoiceActivityDetector` compiles
- [ ] `AppContext` no longer exposes `config_arc()` / `history_arc()`
- [ ] FF-1 score: 4/5 or higher
- [ ] `cargo clippy -- -D warnings` passes

### After Phase B (Tray Fix)

- [ ] `whisper_for_tray` removed from `main.rs`
- [ ] `DictationTray::spawn_service` accepts `Arc<AppContext>`
- [ ] Only one `WhisperSTT` instance at runtime
- [ ] System tray still shows correct model status

### After Phase C (UI State)

- [ ] `UIStateUpdater` trait defined and implemented
- [ ] UI handlers use trait, not widget fields directly
- [ ] `AppState` moved to `types.rs`
- [ ] `ui/state.rs` callers reduced from 7 to ≤3
- [ ] FF-2 score: 4/5

### After Phase D (Module Decomposition)

- [ ] All modules under 150 symbols
- [ ] `dialogs/model/` directory with ≥2 files
- [ ] `dialogs/history/` directory with ≥2 files
- [ ] `audio.rs` functions all under 100 lines
- [ ] FF-4 score: 5/5

### After Phase E (Layer Enforcement)

- [ ] `pub(crate)` visibility on infrastructure types
- [ ] No direct UI → Infrastructure imports
- [ ] FF-5 score: 4/5

### After Phase F (Testing & CI)

- [ ] CI pipeline running on GitHub Actions
- [ ] Test coverage > 40%
- [ ] Architecture fitness checks automated
- [ ] All fitness functions ≥ 4/5

### Overall Target

| Metric | Before | After |
|--------|--------|-------|
| Architecture Fitness | 2.6/5.0 | 4.4/5.0 |
| Trait implementations | 0 (production) | 5 |
| Max module symbols | 156 | < 150 |
| WhisperSTT instances | 2 | 1 |
| Legacy accessors | 2 | 0 |
| Test coverage | ~10% | > 40% |

---

## Appendices

### Appendix A: Phase-File Impact Matrix

| File | Phase A | Phase B | Phase C | Phase D | Phase E |
|------|---------|---------|---------|---------|---------|
| src/whisper.rs | Modify | | | | |
| src/history.rs | Modify | | | | |
| src/config.rs | Modify | | | | |
| src/vad.rs | Modify | | | | |
| src/services/transcription.rs | Modify | | | | |
| src/context.rs | Modify | | | | Modify |
| src/main.rs | | Modify | | | |
| src/tray.rs | | Modify | | | |
| src/ui/state.rs | | | Modify | | |
| src/ui/recording.rs | | | Modify | | |
| src/ui/continuous.rs | | | Modify | | |
| src/ui/conference.rs | | | Modify | | |
| src/types.rs | | | Modify | | |
| src/dialogs/model.rs | | | | Split | |
| src/dialogs/history.rs | | | | Split | |
| src/audio.rs | | | | Modify | Modify |
| src/traits.rs | Verify | | Modify | | |

### Appendix B: Verification Commands

```bash
# Full verification after each phase
cargo fmt --check
cargo clippy -- -D warnings
cargo build --release
cargo test --all-features

# Check for remaining dead_code
cargo build 2>&1 | grep "dead_code"

# Check for remaining legacy accessors
grep -r "config_arc\|history_arc" src/

# Check for duplicate WhisperSTT creation
grep -rn "WhisperSTT::new" src/

# Architecture fitness (manual via codegraph)
# Run find_hotspot_symbols(min_callers=10)
# Run get_file_symbols for largest modules
# Run get_module_deps for all key modules
```

### Appendix C: References

- Martin, R. C. (2017). *Clean Architecture: A Craftsman's Guide to Software Structure and Design*
- Martin, R. C. (2002). *Agile Software Development: Principles, Patterns, and Practices*
- `docs/architecture-fitness-methodology.md` — Fitness function definitions
- `docs/audit/architecture-overview-and-design-findings.md` — Current state assessment
- `docs/audit/RECOMMENDATIONS.md` — Previous audit recommendations

---

*Document Version: 2.0*
*Last Updated: 2026-01-28*
