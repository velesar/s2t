# Remediation Plan

**Project:** Voice Dictation (s2t)
**Based on:** Audit of 2026-01-30 (113 findings)
**Focus:** Reliability-first remediation

---

## Overview

This plan addresses all 113 audit findings in 4 priority tiers. **Reliability is the primary focus** — the 5 HIGH reliability findings represent crash/deadlock risks in production use.

| Priority | Scope | Findings Addressed | Theme |
|----------|-------|-------------------|-------|
| **P0** | Fix Now | 8 HIGH | Crashes, deadlock, data integrity |
| **P1** | Next Sprint | 20 HIGH+MEDIUM | Mutex safety, audio performance, error handling |
| **P2** | Technical Debt | 30 MEDIUM | Duplication, coupling, testability |
| **P3** | Hardening | 55 LOW+INFO | Validation, docs, edge cases |

---

## P0: Fix Now (crash/data-integrity risk)

### P0.1: Fix ABBA deadlock in denoise.rs

**Finding:** F-565811D6 (HIGH reliability)
**File:** `src/recording/denoise.rs:144`
**Risk:** Deadlock when `denoise_buffer()` and `reset()` called concurrently

**Current code (deadlock pattern):**
```rust
// denoise_buffer(): state THEN buffer
fn denoise_buffer(&self, samples: &[f32]) -> Result<Vec<f32>> {
    let state = self.state.lock().unwrap();   // Lock 1
    let buffer = self.buffer.lock().unwrap();  // Lock 2
    // ...
}

// reset(): buffer THEN state  <-- REVERSED ORDER
fn reset(&self) {
    let buffer = self.buffer.lock().unwrap();  // Lock 2
    let state = self.state.lock().unwrap();    // Lock 1
}
```

**Fix (consolidate into single mutex):**
```rust
struct DenoiseInner {
    state: Box<DenoiseState>,
    buffer: Vec<f32>,
}

pub struct NnnoiselessDenoiser {
    inner: Mutex<DenoiseInner>,  // Single lock, no ordering issue
}
```

**Files to change:** `src/recording/denoise.rs`
**Tests:** Add concurrent stress test calling `denoise_buffer()` and `reset()` from separate threads.

---

### P0.2: Fix loopback panic on odd byte count

**Finding:** F-AE877B80 (HIGH reliability)
**File:** `src/recording/loopback.rs:83`
**Risk:** Index out of bounds panic if pipe read returns odd bytes

**Current code:**
```rust
let samples: Vec<i16> = buffer[..bytes_read]
    .chunks(2)
    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))  // panics if len=1
    .collect();
```

**Fix:**
```rust
let samples: Vec<i16> = buffer[..bytes_read]
    .chunks_exact(2)  // safely skips trailing odd byte
    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
    .collect();
```

**Files to change:** `src/recording/loopback.rs`
**Tests:** Add unit test with odd-length byte buffer.

---

### P0.3: Handle CPAL stream errors instead of unwrapping

**Finding:** F-C9E210CD (HIGH reliability)
**File:** `src/recording/microphone.rs:126`
**Risk:** Silent thread panic if audio device disappears; UI hangs forever

**Current code:**
```rust
let stream = device.build_input_stream(&config, data_callback, err_callback, None).unwrap();
stream.play().unwrap();
```

**Fix:**
```rust
let stream = match device.build_input_stream(&config, data_callback, err_callback, None) {
    Ok(s) => s,
    Err(e) => {
        eprintln!("Failed to build audio stream: {}", e);
        let _ = completion_tx.send(());
        return;
    }
};
if let Err(e) = stream.play() {
    eprintln!("Failed to start audio stream: {}", e);
    let _ = completion_tx.send(());
    return;
}
```

**Files to change:** `src/recording/microphone.rs`
**Tests:** Mock test with `AudioRecording` trait returning error.

---

### P0.4: Add SHA256 verification for downloaded models

**Finding:** F-752F5717, F-C69D1AA0 (HIGH security)
**File:** `src/infrastructure/models.rs:125`
**Risk:** Malicious model files loaded into native C/C++ code (whisper.cpp, ort)

**Implementation:**
1. Add `sha256` field to `ModelInfo` struct
2. After download, compute SHA256 of temp file
3. Compare against expected hash before rename
4. Abort with error if mismatch

```rust
use sha2::{Sha256, Digest};

fn verify_checksum(path: &Path, expected: &str) -> Result<()> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let hash = format!("{:x}", hasher.finalize());
    if hash != expected {
        bail!("Checksum mismatch: expected {}, got {}", expected, hash);
    }
    Ok(())
}
```

**Dependencies to add:** `sha2 = "0.10"` in Cargo.toml
**Files to change:** `src/infrastructure/models.rs`, `Cargo.toml`

---

### P0.5: Fix segmentation buffer clone

**Finding:** F-F00B80FC (HIGH reliability), F-BC68140E (HIGH performance)
**File:** `src/recording/segmentation.rs:115`
**Risk:** OOM on long recordings, severe CPU waste

**Current code:**
```rust
let current_samples = samples.lock().unwrap().clone();  // clones entire buffer
```

**Fix:**
```rust
let lock = samples.lock().unwrap();
let new_samples = if lock.len() > last_samples_len {
    lock[last_samples_len..].to_vec()  // only new samples
} else {
    vec![]
};
last_samples_len = lock.len();
drop(lock);
```

**Files to change:** `src/recording/segmentation.rs`
**Tests:** Add test verifying only incremental samples are read.

---

## P1: Next Sprint (reliability + performance)

### P1.1: Switch to parking_lot::Mutex

**Finding:** F-D0663E4B (HIGH reliability, systemic)
**Risk:** One panicking thread cascades panics across entire app via mutex poisoning

**Approach:**
1. Add `parking_lot = "0.12"` to Cargo.toml
2. Replace `use std::sync::Mutex` with `use parking_lot::Mutex` project-wide
3. Remove all `.unwrap()` after `.lock()` (parking_lot::Mutex::lock returns `MutexGuard` directly, not `Result`)
4. This is a mechanical find-and-replace (~70 sites)

**Files to change:** All files containing `Mutex::lock().unwrap()` (~15 files)
**Risk:** Low — parking_lot is a well-established crate with identical API minus poisoning.

---

### P1.2: Replace Arc<Mutex<Vec<f32>>> with lock-free ring buffer in audio callback

**Finding:** F-B1A4B2AE (HIGH performance), F-03F55E2A (HIGH performance)
**Risk:** Audio glitches (dropouts) due to blocking in real-time callback

**Approach:**
1. Add `rtrb = "0.3"` to Cargo.toml (real-time ring buffer)
2. In `microphone.rs`, create SPSC producer/consumer pair before `build_input_stream`
3. Audio callback writes to producer (non-blocking)
4. Consumer thread reads samples into the shared buffer
5. Pre-allocate mono buffer and resampler input buffer outside callback

```rust
let (mut producer, consumer) = rtrb::RingBuffer::new(16000 * 10); // 10 sec buffer

// In callback (real-time safe):
let mono = to_mono_preallocated(&data, channels, &mut mono_buf);
let resampled = resampler.process_into(&[mono], &mut resample_buf)?;
producer.write_chunk_uninit(resampled.len())
    .map(|chunk| chunk.fill_from_iter(resampled.iter().copied()));
```

**Files to change:** `src/recording/microphone.rs`, `src/recording/core.rs`
**Dependencies to add:** `rtrb = "0.3"`

---

### P1.3: Add Drop implementations for resource cleanup

**Finding:** F-EDDDE3A1 (MEDIUM reliability)

**Implement Drop for:**
- `RecordingCore` — set `is_recording` to false
- `LoopbackRecorder` — kill parec child process
- `HotkeyManager` — unregister hotkey
- Store CPAL `Stream` handle in `AudioRecorder` for cleanup

**Files to change:** `src/recording/core.rs`, `src/recording/loopback.rs`, `src/infrastructure/hotkeys.rs`, `src/recording/microphone.rs`

---

### P1.4: Fix thread::sleep blocking GTK main thread

**Finding:** F-447B7279, F-383A4B16 (MEDIUM reliability + performance)
**File:** `src/ui/mic.rs:321`, `src/ui/conference.rs:185`

**Fix:** Replace `std::thread::sleep(Duration::from_millis(100))` with `glib::timeout_future(Duration::from_millis(100)).await` in all `glib::spawn_future_local` blocks.

---

### P1.5: Add signal handlers for clean shutdown

**Finding:** F-EDBBC05E (MEDIUM reliability)

**Approach:**
1. Install SIGTERM/SIGINT handler in `main.rs`
2. Set shutdown `AtomicBool` flag
3. Stop any active recording, save history, kill child processes
4. Use `ctrlc` crate or `signal-hook` for safe signal handling

---

### P1.6: Fix silent error swallowing

**Findings:** F-C485F5A8, F-EAEDFBCB (MEDIUM reliability)

- `ts.transcribe().unwrap_or_default()` in segmented mode: propagate errors through result channel, display count of failed segments
- `create_vad(&config).ok()`: log warning when VAD creation fails, notify user through UI

---

### P1.7: Fix lock ordering issues

**Findings:** F-F7242D62, F-11AA0296, F-98D52FD7, F-53EA4617 (MEDIUM reliability)

**Principle:** Never hold two locks simultaneously. Extract data from first lock, drop it, then acquire second.

- `main.rs:244` hotkey reload: clone config values, drop lock, then lock hotkey_manager
- `ui/conference.rs:153`: clone transcription result, drop lock, then lock diarization
- `infrastructure/tray.rs:59`: create model instance outside lock, briefly lock to swap
- `dialogs/model/list.rs:108`: drop config lock before acquiring transcription lock

---

### P1.8: Store JoinHandle for segmentation thread

**Finding:** F-CD32DE8A (MEDIUM reliability)
**File:** `src/recording/segmentation.rs:196`

Replace `sleep(100ms)` with stored `JoinHandle::join()` for deterministic thread synchronization.

---

### P1.9: Add timeout to segmented mode polling

**Finding:** F-4178C565 (MEDIUM reliability)
**File:** `src/ui/mic.rs:394`

Add maximum timeout (e.g., 5 minutes) that breaks the polling loop with a warning if transcription threads fail to complete.

---

### P1.10: Pre-allocate denoiser and reuse

**Findings:** F-28A76F6A, F-C782CACD, F-C5DD03E6 (MEDIUM performance)

- Create denoiser once in `AppContext`, call `reset()` between uses
- Cache FFT resamplers as fields (create in `new()`, reuse across calls)
- Pre-allocate frame_in/frame_out buffers, reuse via `copy_from_slice`

---

## P2: Technical Debt (maintainability + testability)

### P2.1: Extract shared download_file() in models.rs

**Findings:** F-0FAD129D, F-2AFBAE9C (HIGH maintainability)

Extract ~50 lines of duplicated HTTP download logic into:
```rust
async fn download_file(
    url: &str,
    dir: &Path,
    filename: &str,
    progress: impl Fn(u64, u64),
) -> Result<PathBuf>
```

**Eliminates:** ~100 lines of duplication, reduces models.rs from 535 to ~435 lines.

---

### P2.2: Decompose show_settings_dialog()

**Finding:** F-2880938A (HIGH maintainability)

Split into:
- `build_language_section()` -> language combo
- `build_backend_section()` -> STT backend, model selection
- `build_recording_section()` -> mode, VAD, denoise
- `build_hotkey_section()` -> hotkey toggle and binding
- `build_history_section()` -> limits and cleanup
- `save_settings()` -> extracted save handler
- `SettingsState` struct -> replaces 14 clone variables

---

### P2.3: Add AppContext::for_testing()

**Finding:** F-2F3EE753 (HIGH testability)

```rust
impl AppContext {
    #[cfg(test)]
    pub fn for_testing(
        config: Arc<Mutex<Config>>,
        history: Arc<Mutex<History>>,
        audio: Arc<AudioService>,
        transcription: Arc<Mutex<TranscriptionService>>,
    ) -> Self { ... }
}
```

---

### P2.4: Add MockUIStateUpdater

**Finding:** F-8FF6D3DF (MEDIUM testability)

Add to `test_support/mocks.rs`:
```rust
pub struct MockUIStateUpdater {
    pub status_calls: RefCell<Vec<String>>,
    pub recording_calls: RefCell<Vec<String>>,
    // ...
}
impl UIStateUpdater for MockUIStateUpdater { ... }
```

---

### P2.5: Add RingBuffer and SegmentationMonitor tests

**Findings:** F-0EF51150, F-F6B2C631 (MEDIUM testability)

- RingBuffer: test wrap-around, write overflow, peek_last, read_all
- SegmentationMonitor: test segment emission timing, VAD-based splitting

---

### P2.6: Create integration test suite

**Finding:** F-789CDE98 (HIGH testability)

Create `tests/`:
- `tests/cli_transcribe.rs` — CLI workflow with mock WAV
- `tests/config_roundtrip.rs` — Config save/load
- `tests/history_roundtrip.rs` — History persistence
- `tests/audio_pipeline.rs` — AudioService -> TranscriptionService with mocks

---

### P2.7: Extract duplicated UI patterns

**Findings:** F-29817E5B, F-1F5DAF71, F-1A29C690 (MEDIUM maintainability)

- Extract shared `start_timer_loop()`, `start_level_loop()` from mic/conference/conference_file
- Extract `handle_post_transcription()` from mic.rs and conference.rs
- Move `maybe_denoise()` to shared location

---

### P2.8: Add path traversal guards

**Finding:** F-15C26F89 (MEDIUM security)

```rust
fn sanitize_model_filename(filename: &str) -> Result<&str> {
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        bail!("Invalid model filename: {}", filename);
    }
    Ok(filename)
}
```

Apply in `get_model_path()`, `delete_model()`, `download_model()`.

---

### P2.9: Add config validation

**Finding:** F-D50B6D79 (MEDIUM security)

Add `Config::validate()` called after `load_config()`:
- `segment_interval_secs >= 1`
- `history_max_entries` in range `1..=10000`
- `default_model` contains no path separators
- Model paths point to expected directories

---

### P2.10: Set restrictive file permissions

**Finding:** F-38F99208 (MEDIUM security)

```rust
#[cfg(unix)]
fn set_restrictive_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}
```

Apply after writing config.toml and history.json.

---

## P3: Hardening

### P3.1: Clean up dead code (12 clippy warnings)
Remove unused `NoOpDenoiser`, `create_denoiser()`, `WhisperSTT::model_path`, `VadEngine::as_str()`, etc.

### P3.2: Fix domain layering violation
Move `HistoryEntry` import out of `domain/types.rs` or make `SharedHistory` generic.

### P3.3: Add xvfb to CI
Add `xvfb-run` wrapper to `cargo test` in `.github/workflows/ci.yml` for future GTK tests.

### P3.4: Clean up stale .downloading files on startup
Add startup check for `.downloading` temp files older than 1 hour and remove them.

### P3.5: Return error instead of fallback to '.' for XDG dirs
Replace `PathBuf::from('.')` fallback with `anyhow!("XDG directory not found")`.

### P3.6: Load models asynchronously
Show GTK window immediately with loading indicator; load Whisper/TDT/Sortformer in background thread.

### P3.7: Use glib::timeout_future for auto-paste delay
Replace all `thread::sleep` in async GTK contexts.

### P3.8: Reduce resampler quality for speech
Reduce `sinc_len` from 256 to 128 and `oversampling_factor` from 256 to 128.

### P3.9: Add doc comments to public APIs
Cover `config.rs` (8 functions) and `models.rs` (7 functions).

### P3.10: Pre-allocate sample buffers
Use `Vec::with_capacity(16000 * 300)` for recording buffers (5 min). Use `std::mem::take()` instead of `.clone()` in `stop()`.

---

## Tracking

All findings are tracked in the mental model at `.audit/mental_model.yaml` and exported to `docs/audit/findings-export.json`.

### Verification Checklist

After each priority tier, verify:

```bash
# P0: Basic correctness
cargo clippy -- -D warnings
cargo test

# P1: Run with active recording for 5+ minutes, verify no crashes
cargo run --release

# P2: Verify test coverage improved
cargo test -- --nocapture 2>&1 | grep -c "test result"

# P3: Final quality gate
cargo clippy -- -D warnings -W clippy::all
cargo test
```

### Metrics to Track

| Metric | Current | After P0 | After P1 | After P2 | Target |
|--------|---------|----------|----------|----------|--------|
| HIGH findings | 16 | 8 | 0 | 0 | 0 |
| MEDIUM findings | 54 | 54 | 34 | 4 | 0 |
| Deadlock risks | 1 confirmed | 0 | 0 | 0 | 0 |
| Panic-on-error sites | 70+ | 70+ | 0 | 0 | 0 |
| Integration tests | 0 | 0 | 0 | 4+ | 10+ |
| UI test coverage | 0% | 0% | 0% | >0% | >30% |
| Largest file (LOC) | 625 | 625 | 625 | <500 | <500 |
| Code duplication | ~150 lines | ~150 | ~150 | ~0 | 0 |
