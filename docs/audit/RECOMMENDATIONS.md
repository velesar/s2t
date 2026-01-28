# Audit Recommendations

**Project:** Voice Dictation (s2t)
**Audit Date:** 2026-01-28

---

## Priority Matrix

| Priority | Category | Effort | Impact | Items |
|----------|----------|--------|--------|-------|
| P1 | Must Fix | Low-Medium | High | 3 |
| P2 | Should Fix | Medium | Medium | 3 |
| P3 | Nice to Have | Low-High | Low | 4 |

---

## Priority 1: Must Fix

These issues should be addressed in the next development cycle.

### 1.1 Create Context Structs for UI Functions

**Related Finding:** F-EECA451D
**Effort:** Medium
**Files:** src/ui.rs

**Problem:**
Functions with 10-21 parameters are error-prone and hard to maintain.

**Solution:**
Create grouped context structs:

```rust
// src/ui/context.rs

/// Shared UI components for recording operations
pub struct RecordingUiContext {
    pub button: Button,
    pub status_label: Label,
    pub result_text_view: TextView,
    pub timer_label: Label,
    pub level_bar: LevelBar,
}

/// State shared across recording operations
pub struct RecordingState {
    pub whisper: Arc<Mutex<Option<WhisperSTT>>>,
    pub config: Arc<Mutex<Config>>,
    pub history: Arc<Mutex<History>>,
    pub app_state: Rc<Cell<AppState>>,
    pub recording_start_time: Rc<Cell<Option<Instant>>>,
}

/// Continuous mode specific components
pub struct ContinuousModeContext {
    pub vad_indicator: Label,
    pub segment_indicators_box: GtkBox,
    pub segment_row: GtkBox,
    pub continuous_recorder: Arc<ContinuousRecorder>,
}
```

**Refactored function signature:**
```rust
// Before: 21 parameters
fn setup_record_button(button: &Button, ...) { }

// After: 3-4 context parameters
fn setup_record_button(
    ui: &RecordingUiContext,
    state: &RecordingState,
    continuous: Option<&ContinuousModeContext>,
) { }
```

---

### 1.2 Fix Arc Usage for Non-Thread-Safe Type

**Related Finding:** F-F1F2F032
**Effort:** Low
**Files:** src/vad.rs

**Problem:**
`Arc<Mutex<Vad>>` is used but `Vad` is not `Send + Sync`.

**Solution:**
Determine actual threading model and fix accordingly:

**Option A: Single-threaded (if VadDetector stays on main thread)**
```rust
use std::rc::Rc;
use std::cell::RefCell;

pub struct VadDetector {
    vad: Rc<RefCell<Vad>>,
}
```

**Option B: Thread-local (if created per-thread)**
```rust
thread_local! {
    static VAD: RefCell<Option<Vad>> = RefCell::new(None);
}
```

**Option C: Verify thread safety (if webrtc-vad is actually safe)**
```rust
// Add safety documentation if Vad can be safely shared
// despite missing Send+Sync bounds
#[allow(clippy::arc_with_non_send_sync)]
```

---

### 1.3 Address Vulnerable Dependency

**Related Finding:** F-0E5EAC4D
**Effort:** Low
**Files:** Cargo.toml, Cargo.lock

**Problem:**
`atty 0.2.14` has GHSA-g98v-hv3f-hcfr (low severity, Windows-only).

**Solution:**

1. Find which dependency brings in `atty`:
```bash
cargo tree -i atty
```

2. Options based on findings:
   - If direct dependency: remove or replace with `std::io::IsTerminal`
   - If transitive: check for updated version of parent crate
   - If unavoidable: document as known low-risk issue

**Mitigation:**
Since the application targets Linux and the vulnerability affects Windows with custom allocators, the actual risk is minimal. However, `atty` is unmaintained, so migration is recommended.

---

## Priority 2: Should Fix

These improve code quality and maintainability.

### 2.1 Split ui.rs into Smaller Modules

**Related Finding:** F-6FF78A81
**Effort:** Medium-High
**Files:** src/ui.rs (1555 lines)

**Proposed Structure:**
```
src/
├── ui/
│   ├── mod.rs              # Re-exports, common utilities
│   ├── context.rs          # Context structs (from 1.1)
│   ├── recording.rs        # Dictation mode handlers
│   ├── continuous.rs       # Continuous mode (move from nested module)
│   ├── conference.rs       # Conference mode handlers
│   ├── widgets.rs          # Custom widget builders
│   └── state.rs            # AppState enum and transitions
```

**Migration Steps:**
1. Create `src/ui/` directory
2. Move `AppState` enum to `src/ui/state.rs`
3. Extract nested `ui_continuous` module to `src/ui/continuous.rs`
4. Move conference handlers to `src/ui/conference.rs`
5. Create `src/ui/mod.rs` with re-exports
6. Update imports in main.rs

---

### 2.2 Reduce Module Coupling

**Related Finding:** F-B7926CF8
**Effort:** Medium
**Files:** src/main.rs, src/ui.rs

**Problem:**
Both orchestration modules depend on all 19 other modules.

**Solution:**
Introduce service layer facades:

```rust
// src/services/mod.rs
pub mod audio;
pub mod transcription;
pub mod storage;

// src/services/audio.rs
pub struct AudioService {
    mic_recorder: AudioRecorder,
    continuous_recorder: ContinuousRecorder,
    loopback_recorder: LoopbackRecorder,
    conference_recorder: ConferenceRecorder,
    vad_detector: VadDetector,
    ring_buffer: RingBuffer,
}

impl AudioService {
    pub fn start_dictation(&mut self) -> Result<()> { ... }
    pub fn start_continuous(&mut self) -> Result<()> { ... }
    pub fn stop(&mut self) -> Vec<f32> { ... }
}
```

This reduces main.rs dependencies from 19 to ~5 services.

---

### 2.3 Extract Complex Return Types

**Related Finding:** F-3BC3393E
**Effort:** Low
**Files:** src/conference_recorder.rs

**Before:**
```rust
fn stop_conference(&self) -> (
    Vec<f32>,
    Vec<f32>,
    Option<Receiver<()>>,
    Option<Receiver<()>>,
)
```

**After:**
```rust
pub struct ConferenceRecordingResult {
    pub mic_samples: Vec<f32>,
    pub loopback_samples: Vec<f32>,
    pub mic_completion: Option<Receiver<()>>,
    pub loopback_completion: Option<Receiver<()>>,
}

fn stop_conference(&self) -> ConferenceRecordingResult
```

---

## Priority 3: Nice to Have

These are improvements for future consideration.

### 3.1 Implement Native PipeWire API

**Related Finding:** F-C1708BE0
**Effort:** High
**Files:** src/loopback.rs

**Current State:**
Uses `pactl` and `parec` CLI tools via `std::process::Command`.

**Improvement:**
The `pipewire` crate (0.9) is already in dependencies. Implement native API:

```rust
use pipewire::{Context, MainLoop, stream::Stream};

impl LoopbackRecorder {
    pub fn start_loopback_native(&self) -> Result<()> {
        // Use PipeWire stream API directly
        // Benefits: No subprocess overhead, better error handling
    }
}
```

**Note:** The CLI approach works well as an MVP. Native API is an optimization.

---

### 3.2 Add CI/CD Pipeline

**Effort:** Medium
**Files:** .github/workflows/ (new)

**Suggested Workflow:**
```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - name: Check formatting
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy -- -D warnings

      - name: Build
        run: cargo build --release

      - name: Test
        run: cargo test
```

---

### 3.3 Add Integration Tests

**Effort:** Medium
**Files:** tests/ (new)

**Areas to Test:**
- Config serialization/deserialization
- Audio resampling pipeline
- History management
- Model path resolution

**Example:**
```rust
// tests/integration_test.rs

#[test]
fn test_audio_resampling_preserves_duration() {
    let input_samples = generate_sine_wave(44100, 1.0);  // 1 second at 44.1kHz
    let output = resample_to_16khz(&input_samples);

    let expected_samples = 16000;  // 1 second at 16kHz
    assert_eq!(output.len(), expected_samples);
}
```

---

### 3.4 Document External Dependencies

**Related Findings:** F-76F6B43D, F-C1708BE0
**Effort:** Low
**Files:** README.md

**Add Section:**
```markdown
## Runtime Dependencies

The following system tools are used at runtime:

| Tool | Package | Used For | Required |
|------|---------|----------|----------|
| xdotool | `xdotool` | Auto-paste feature | Optional |
| pactl | `pulseaudio-utils` | Audio source detection | For conference mode |
| parec | `pulseaudio-utils` | System audio capture | For conference mode |

Install on Fedora:
```bash
sudo dnf install xdotool pulseaudio-utils
```
```

---

## Implementation Order

Suggested order for addressing recommendations:

```
Week 1-2:
├── 1.3 Address vulnerable dependency (quick win)
├── 2.3 Extract complex return types (quick win)
└── 1.2 Fix Arc usage (targeted fix)

Week 3-4:
├── 1.1 Create context structs (enables 2.1)
└── 3.4 Document dependencies (documentation)

Week 5-8:
├── 2.1 Split ui.rs (major refactor)
└── 3.2 Add CI/CD (infrastructure)

Future:
├── 2.2 Reduce module coupling (architecture)
├── 3.1 Native PipeWire (optimization)
└── 3.3 Integration tests (quality)
```

---

## Metrics to Track

After implementing recommendations, verify improvements:

| Metric | Current | Target |
|--------|---------|--------|
| Largest file (LOC) | 1,555 | < 500 |
| Max function parameters | 21 | < 7 |
| Clippy warnings | 20 | 0 |
| Module dependencies (main.rs) | 19 | < 8 |
| Dependency vulnerabilities | 1 | 0 |
| Test coverage | ~10% | > 60% |
