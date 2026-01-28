# Audit Findings

**Project:** Voice Dictation (s2t)
**Audit Date:** 2026-01-28

---

## Summary

| Category | Critical | High | Medium | Low | Info | Total |
|----------|----------|------|--------|-----|------|-------|
| Security | 0 | 0 | 0 | 1 | 2 | 3 |
| Maintainability | 0 | 0 | 4 | 2 | 0 | 6 |
| **Total** | **0** | **0** | **4** | **3** | **2** | **9** |

---

## Security Findings

### F-0E5EAC4D: Vulnerable dependency: atty 0.2.14

| Field | Value |
|-------|-------|
| **Severity** | LOW |
| **Category** | Security |
| **File** | Cargo.lock:108 |
| **Rule ID** | GHSA-g98v-hv3f-hcfr |
| **Viewpoint** | VP-Q01 |

**Description:**
The `atty` crate has a potential unaligned pointer dereference on Windows (GHSA-g98v-hv3f-hcfr). This is a transitive dependency brought in by another crate. The vulnerability has low severity and only affects Windows, while this application targets Linux.

**Risk:**
- Linux: No impact
- Windows: Potential crash with custom allocators (unlikely scenario)

**Recommendation:**
Remove the `atty` dependency or replace with `std::io::IsTerminal` which is stable since Rust 1.70.0. Check which dependency brings in `atty` using `cargo tree -i atty`.

---

### F-76F6B43D: External command execution in paste.rs

| Field | Value |
|-------|-------|
| **Severity** | INFO |
| **Category** | Security |
| **File** | src/paste.rs:11 |
| **Rule ID** | external-command |
| **Viewpoint** | VP-Q01 |

**Description:**
Uses `std::process::Command` to execute `xdotool key ctrl+v` for auto-paste functionality. The command is hardcoded and does not accept user input, making it safe from command injection.

**Code:**
```rust
let output = std::process::Command::new("xdotool")
    .arg("key")
    .arg("ctrl+v")
    .output()
```

**Risk:**
None - command is hardcoded with no user-controllable parameters.

**Recommendation:**
Document `xdotool` as a runtime dependency in README. Note Wayland compatibility limitations.

---

### F-C1708BE0: External command execution in loopback.rs

| Field | Value |
|-------|-------|
| **Severity** | INFO |
| **Category** | Security |
| **File** | src/loopback.rs:52 |
| **Rule ID** | external-command |
| **Viewpoint** | VP-Q01 |

**Description:**
Uses `pactl list sources short` to enumerate audio sources and `parec` to capture system audio. Both commands are hardcoded without user input.

**Code:**
```rust
let monitor_source = std::process::Command::new("pactl")
    .args(&["list", "sources", "short"])
    .output()
// ...
let mut child = std::process::Command::new("parec")
    .arg("--format=s16le")
    .arg(format!("--rate={}", WHISPER_SAMPLE_RATE))
    .arg("--channels=1")
    .arg("--device")
    .arg(&monitor_source)  // From pactl output, not user input
    .spawn()
```

**Risk:**
None - commands are hardcoded, `monitor_source` is from system output.

**Recommendation:**
Consider implementing native PipeWire API (`pipewire` crate is already in dependencies) for better integration and performance. The current CLI approach is an MVP solution.

---

## Maintainability Findings

### F-EECA451D: Functions with too many arguments in ui.rs

| Field | Value |
|-------|-------|
| **Severity** | MEDIUM |
| **Category** | Maintainability |
| **File** | src/ui.rs:914 |
| **Rule ID** | clippy::too_many_arguments |
| **Viewpoint** | VP-Q02 |

**Description:**
Multiple functions in ui.rs have excessive parameter counts:

| Function | Parameters | Location |
|----------|------------|----------|
| `setup_record_button` | 21 | Line 914 |
| `handle_stop_continuous` | 15 | Line 255 |
| `handle_stop_conference` | 14 | Line 1377 |
| `handle_start_continuous` | 13 | Line 39 |
| `handle_stop_recording` | 12 | Line 1148 |
| `build_ui` | 10 | Line 431 |

**Impact:**
- Difficult to understand function signatures
- Easy to make mistakes when calling
- Hard to add new parameters
- Indicates missing abstractions

**Recommendation:**
Create context structs to group related parameters:

```rust
// Before
fn setup_record_button(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    // ... 17 more parameters
) { }

// After
struct RecordingContext {
    button: Button,
    status_label: Label,
    result_text_view: TextView,
    timer_label: Label,
    // ...
}

fn setup_record_button(ctx: &RecordingContext) { }
```

---

### F-6FF78A81: Large UI module (1555 lines)

| Field | Value |
|-------|-------|
| **Severity** | MEDIUM |
| **Category** | Maintainability |
| **File** | src/ui.rs:1 |
| **Rule ID** | file-size |
| **Viewpoint** | VP-S02 |

**Description:**
The ui.rs file contains 1555 lines of code and 473 symbols, making it the largest file in the project by far. It includes a nested module `ui_continuous` and handles multiple recording modes (dictation, continuous, conference).

**Module Size Comparison:**
| File | Lines | % of Total |
|------|-------|------------|
| ui.rs | 1,555 | 27.8% |
| model_dialog.rs | 532 | 9.5% |
| history_dialog.rs | 418 | 7.5% |
| All others | 3,089 | 55.2% |

**Impact:**
- Difficult to navigate and understand
- Higher chance of merge conflicts
- Harder to test individual components
- Cognitive overload for maintainers

**Recommendation:**
Split into focused modules:
- `src/ui/mod.rs` - Common UI utilities and re-exports
- `src/ui/recording.rs` - Dictation mode recording
- `src/ui/continuous.rs` - Continuous mode (extract from nested module)
- `src/ui/conference.rs` - Conference mode recording
- `src/ui/widgets.rs` - Custom widget builders

---

### F-B7926CF8: High module coupling

| Field | Value |
|-------|-------|
| **Severity** | MEDIUM |
| **Category** | Maintainability |
| **File** | src/main.rs:1 |
| **Rule ID** | high-coupling |
| **Viewpoint** | VP-S02 |

**Description:**
Both `main.rs` and `ui.rs` have direct dependencies on all 19 other modules in the project:

```
main.rs depends on: audio, config, conference_recorder, continuous,
    diarization, history, history_dialog, hotkeys, loopback,
    model_dialog, models, paste, recordings, ring_buffer,
    settings_dialog, tray, ui, vad, whisper

ui.rs depends on: (same 19 modules)
```

**Impact:**
- Changes in any module may affect main.rs and ui.rs
- Difficult to reason about dependencies
- Harder to unit test in isolation
- Indicates thin/missing application layer

**Recommendation:**
Introduce service facades to group related functionality:

```rust
// src/services/audio_service.rs
pub struct AudioService {
    recorder: AudioRecorder,
    continuous: ContinuousRecorder,
    loopback: LoopbackRecorder,
    vad: VadDetector,
}

// src/services/transcription_service.rs
pub struct TranscriptionService {
    whisper: WhisperSTT,
    diarization: DiarizationEngine,
}
```

---

### F-F1F2F032: Arc with non-Send/Sync type

| Field | Value |
|-------|-------|
| **Severity** | MEDIUM |
| **Category** | Maintainability |
| **File** | src/vad.rs:29 |
| **Rule ID** | clippy::arc_with_non_send_sync |
| **Viewpoint** | VP-Q02 |

**Description:**
The `VadDetector` struct wraps `Vad` in `Arc<Mutex<Vad>>`, but the underlying `Vad` type from `webrtc-vad` is not `Send` or `Sync`.

**Code:**
```rust
pub struct VadDetector {
    vad: Arc<Mutex<Vad>>,  // Vad is not Send+Sync
}

impl VadDetector {
    pub fn new() -> Result<Self> {
        let vad = Vad::new()?;
        Ok(Self {
            vad: Arc::new(Mutex::new(vad)),  // Warning here
        })
    }
}
```

**Impact:**
- `Arc` is designed for multi-threaded sharing
- If `Vad` is not thread-safe, `Arc` provides false sense of security
- May compile but cause undefined behavior if actually shared between threads

**Recommendation:**
If `VadDetector` is only used in a single thread:
```rust
pub struct VadDetector {
    vad: Rc<RefCell<Vad>>,  // Single-threaded alternative
}
```

If multi-threaded access is needed, verify `Vad` safety or use a thread-local pattern.

---

### F-3BC3393E: Complex return type in conference_recorder

| Field | Value |
|-------|-------|
| **Severity** | LOW |
| **Category** | Maintainability |
| **File** | src/conference_recorder.rs:42 |
| **Rule ID** | clippy::type_complexity |
| **Viewpoint** | VP-Q02 |

**Description:**
Function returns a 4-element tuple which is difficult to understand:

```rust
pub fn stop_conference(&self) -> (
    Vec<f32>,                    // mic_samples
    Vec<f32>,                    // loopback_samples
    Option<Receiver<()>>,        // mic_completion
    Option<Receiver<()>>,        // loopback_completion
) {
```

**Recommendation:**
Extract into a named struct:

```rust
pub struct ConferenceRecordingResult {
    pub mic_samples: Vec<f32>,
    pub loopback_samples: Vec<f32>,
    pub mic_completion: Option<Receiver<()>>,
    pub loopback_completion: Option<Receiver<()>>,
}
```

---

### F-0F12223F: Redundant closure in continuous.rs

| Field | Value |
|-------|-------|
| **Severity** | LOW |
| **Category** | Maintainability |
| **File** | src/continuous.rs:153 |
| **Rule ID** | clippy::redundant_closure |
| **Viewpoint** | VP-Q02 |

**Description:**
Using a closure where the function can be passed directly:

```rust
// Current code
let start_time = last_segment_time.lock().unwrap()
    .unwrap_or_else(|| Instant::now());

// Suggested fix
let start_time = last_segment_time.lock().unwrap()
    .unwrap_or_else(Instant::now);
```

**Impact:**
Minor - slightly more verbose than necessary.

**Recommendation:**
Replace `|| Instant::now()` with `Instant::now` for cleaner code.

---

## Additional Clippy Warnings

The following clippy warnings were detected but not elevated to findings:

| Rule | Count | Files |
|------|-------|-------|
| `writeln_empty_string` | 3 | history.rs |
| `map_flatten` | 1 | history_dialog.rs |
| `collapsible_else_if` | 1 | ui.rs |
| `needless_borrows_for_generic_args` | 1 | loopback.rs |
| `single_char_add_str` | 1 | whisper.rs |

These are style improvements that can be addressed when modifying the affected files.
