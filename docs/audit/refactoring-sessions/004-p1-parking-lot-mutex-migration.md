# Refactoring Session 004: P1.1 parking_lot::Mutex Migration

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — reliability)
**Finding Addressed:** F-D0663E4B (HIGH reliability, systemic)
**Files Changed:** 25 files (Cargo.toml + Cargo.lock + 23 source files)

---

## P1.1: Switch to parking_lot::Mutex

**Finding:** F-D0663E4B (HIGH reliability — systemic, affects ~70 sites)
**Risk:** One panicking thread cascades panics across entire application via mutex poisoning

### Problem

The codebase used `std::sync::Mutex` exclusively (~70 lock sites across 23 files). `std::sync::Mutex::lock()` returns `Result<MutexGuard, PoisonError>` — if any thread panics while holding a lock, the mutex becomes "poisoned" and all subsequent `.lock().unwrap()` calls will also panic, cascading the failure across the entire application.

In an audio application where recording threads, transcription threads, and the GTK UI thread all share mutexes, a single panic in any thread would cascade:

1. Recording thread panics → `samples` mutex poisoned
2. Segmentation thread calls `samples.lock().unwrap()` → panic
3. UI thread calls `samples.lock().unwrap()` → panic
4. Application crashes completely

### Fix

Replaced `std::sync::Mutex` with `parking_lot::Mutex` project-wide:

1. **Added dependency:** `parking_lot = "0.12"` to `Cargo.toml`
2. **Replaced imports** in 16 files:
   - `use std::sync::{Arc, Mutex}` → `use std::sync::Arc; use parking_lot::Mutex;`
   - `use std::sync::Mutex` → `use parking_lot::Mutex`
3. **Removed 83 `.unwrap()` calls** after `.lock()` across all source files
4. **Fixed 3 multi-line `.unwrap()` patterns** (chained on separate lines)
5. **Simplified `tdt.rs`** — collapsed 4-line `.lock().map_err(...)` block into single `.lock()` call
6. **Updated `main.rs`** — both fully-qualified `std::sync::Mutex` references in function signatures and the scoped import inside `run_gui()`

### Benefits

| Property | std::sync::Mutex | parking_lot::Mutex |
|----------|------------------|--------------------|
| Poisoning | Yes — cascading panics | No — other threads continue |
| `.lock()` return | `Result<MutexGuard>` | `MutexGuard` directly |
| Performance | Good | Better (faster uncontested locking) |
| Size | 40 bytes (Linux) | 1 byte |
| API | Identical (drop-in replacement) | Identical |

### Scope of Changes

| Category | Count |
|----------|-------|
| Files modified | 25 |
| Import lines changed | 19 |
| `.lock().unwrap()` → `.lock()` | 83 (same-line) + 3 (multi-line) |
| `.map_err()` blocks removed | 1 (tdt.rs) |
| Fully-qualified paths updated | 3 (main.rs) |
| Net lines changed | -13 (122 insertions, 109 deletions) |

### Files Changed

**Build:**
- `Cargo.toml` — added `parking_lot = "0.12"`

**Domain:**
- `src/domain/types.rs` — `SharedHistory` type alias

**Application:**
- `src/app/context.rs` — `AppContext` struct fields and all convenience methods

**Recording (8 files):**
- `src/recording/core.rs` — `RecordingCore`, `RecordingHandles`
- `src/recording/microphone.rs` — `AudioRecorder`, resampler mutex in callback
- `src/recording/loopback.rs` — samples buffer in reader thread
- `src/recording/conference.rs` — `ConferenceRecorder`, `start_time` mutex
- `src/recording/segmentation.rs` — `SegmentationMonitor` (6 mutex fields)
- `src/recording/ring_buffer.rs` — `RingBuffer` internal state
- `src/recording/denoise.rs` — `NnnoiselessDenoiser` inner mutex
- `src/recording/service.rs` — `AudioService` mic_samples buffer

**Transcription:**
- `src/transcription/tdt.rs` — `ParakeetSTT` model mutex (+ `.map_err()` removal)

**Infrastructure:**
- `src/infrastructure/tray.rs` — `DictationTray` config/transcription fields

**UI (4 files):**
- `src/ui/mod.rs` — `build_ui()` type annotations
- `src/ui/widgets.rs` — `build_main_widgets()` config parameter
- `src/ui/mic.rs` — transcription and history lock sites
- `src/ui/conference.rs` — transcription and diarization lock sites

**Dialogs (4 files):**
- `src/dialogs/settings.rs` — 14 config lock sites
- `src/dialogs/model/mod.rs` — config/transcription parameters
- `src/dialogs/model/list.rs` — config and transcription lock sites
- `src/dialogs/history/list.rs` — history lock site
- `src/dialogs/history/export.rs` — history lock site

**Tests:**
- `src/test_support/mocks.rs` — `MockAudioRecorder`, `MockTranscription` internal mutexes

**Entry point:**
- `src/main.rs` — function signatures, hotkey manager, config/history initialization

---

## Verification

```
cargo clippy  — 0 new warnings (12 pre-existing dead_code warnings unchanged)
cargo test    — 160/160 passed (0 regressions)
```

## P1 Status

| ID | Task | Status |
|----|------|--------|
| P1.1 | Switch to parking_lot::Mutex | **DONE** (this session) |
| P1.2 | Lock-free audio ring buffer (rtrb) | Pending |
| P1.3 | Drop implementations for resource cleanup | Pending |
| P1.4 | Fix thread::sleep blocking GTK main thread | Pending |
| P1.5 | Signal handlers for clean shutdown | Pending |
| P1.6 | Fix silent error swallowing | Pending |
| P1.7 | Fix lock ordering issues | Pending |
| P1.8 | Store JoinHandle for segmentation thread | Pending |
| P1.9 | Add timeout to segmented mode polling | Pending |
| P1.10 | Pre-allocate denoiser and reuse | Pending |
