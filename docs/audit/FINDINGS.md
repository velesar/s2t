# Audit Findings

**Project:** Voice Dictation (s2t)
**Audit Date:** 2026-01-30
**Total Findings:** 113

---

## Summary

| Category | HIGH | MEDIUM | LOW | INFO | Total |
|----------|------|--------|-----|------|-------|
| Reliability | 5 | 15 | 5 | 0 | 25 |
| Maintainability | 3 | 11 | 4 | 0 | 18 |
| Security | 2 | 3 | 5 | 13 | 23 |
| Performance | 3 | 10 | 9 | 0 | 22 |
| Testability | 3 | 10 | 4 | 2 | 19 |
| Domain (VP-S02) | 0 | 2 | 0 | 0 | 2 |
| **Total** | **16** | **54** | **34** | **9** | **113** |

---

## HIGH Severity Findings

### Reliability

#### F-565811D6: Denoise module ABBA deadlock
- **File:** `src/recording/denoise.rs:144`
- **Description:** `denoise_buffer()` acquires state then buffer; `reset()` acquires buffer then state. Classic deadlock if called concurrently.
- **Fix:** Consolidate into single `Mutex<(DenoiseState, Vec<f32>)>` or enforce consistent lock ordering.

#### F-AE877B80: Loopback panic on odd byte count
- **File:** `src/recording/loopback.rs:83`
- **Description:** `.chunks(2)` with `chunk[1]` access panics if pipe read returns odd bytes.
- **Fix:** Use `.chunks_exact(2)` to safely skip trailing bytes.

#### F-C9E210CD: CPAL stream build/play unwrap panics
- **File:** `src/recording/microphone.rs:126`
- **Description:** `build_input_stream().unwrap()` and `stream.play().unwrap()` crash if audio device disappears mid-session.
- **Fix:** Return `Result` and propagate errors through the completion channel.

#### F-D0663E4B: Mutex poison cascade (70+ sites)
- **File:** `src/app/context.rs:62` (systemic)
- **Description:** 70+ `.lock().unwrap()` calls. One panicking thread poisons a mutex, cascading panics across the entire app.
- **Fix:** Use `parking_lot::Mutex` (no poisoning) or `.unwrap_or_else(|e| e.into_inner())`.

#### F-F00B80FC: Segmentation clones entire buffer every 500ms
- **File:** `src/recording/segmentation.rs:115`
- **Description:** Full `samples.clone()` grows with recording duration. At 30 min: ~115 MB cloned every 500ms.
- **Fix:** Track last-read offset, copy only new samples.

### Security

#### F-752F5717: No integrity verification for downloaded models
- **File:** `src/infrastructure/models.rs:125`
- **Description:** Model files downloaded over HTTPS loaded into native C/C++ code without SHA256 verification.
- **Fix:** Add checksum verification against known-good hashes stored in `ModelInfo`.

#### F-C69D1AA0: (Duplicate confirmation of F-752F5717)

### Performance

#### F-03F55E2A: Heap allocations in CPAL audio callback
- **File:** `src/recording/microphone.rs:83`
- **Description:** `to_mono()`, `to_vec()`, resampler buffers allocate on every ~5ms callback invocation.
- **Fix:** Pre-allocate reusable buffers outside the callback.

#### F-B1A4B2AE: Mutex lock blocks real-time audio thread
- **File:** `src/recording/microphone.rs:94`
- **Description:** Two mutex acquisitions in the callback can block when segmentation or stop_recording holds the lock.
- **Fix:** Use lock-free SPSC ring buffer (`ringbuf` or `rtrb` crate).

#### F-BC68140E: Full buffer clone in segmentation poll
- **File:** `src/recording/segmentation.rs:115`
- **Description:** Same root cause as reliability F-F00B80FC, from performance perspective.

### Maintainability

#### F-2AFBAE9C: models.rs exceeds 400-line threshold (535 lines)
- **File:** `src/infrastructure/models.rs:1`
- **Description:** Three model types with duplicated get/download/delete functions.
- **Fix:** Extract generic model manager parameterized by type.

#### F-0FAD129D: Triplicated HTTP download logic
- **File:** `src/infrastructure/models.rs:125`
- **Description:** `download_model()`, `download_sortformer_model()`, `download_tdt_model()` repeat ~50 lines each (~150 lines duplication).
- **Fix:** Extract shared `download_file()` helper.

#### F-2880938A: settings.rs is a single 429-line function
- **File:** `src/dialogs/settings.rs:8`
- **Description:** Monolithic `show_settings_dialog()` with 14 clone variables.
- **Fix:** Decompose into per-section builders + extracted save handler.

### Testability

#### F-789CDE98: No integration tests
- **File:** `tests/` (absent)
- **Description:** 152 unit tests but zero cross-module integration tests.
- **Fix:** Create `tests/` with CLI workflow, pipeline, and persistence round-trip tests.

#### F-31C9E22E: UI modules completely untested
- **File:** `src/ui/mod.rs`
- **Description:** 7 files (~700+ lines) with critical business logic have zero tests.
- **Fix:** Extract state machine logic from GTK handlers into testable functions.

#### F-2F3EE753: AppContext untestable
- **File:** `src/app/context.rs:55`
- **Description:** Constructor accesses real audio hardware, no mock-friendly alternative.
- **Fix:** Add `AppContext::for_testing()` that accepts pre-built mock services.

---

## MEDIUM Severity Findings (54)

### Reliability (15 MEDIUM)

| ID | Title | File | Line |
|----|-------|------|------|
| F-EDDDE3A1 | No Drop for audio recorders - streams/parec leak | recording/core.rs | 33 |
| F-6FC84772 | Tokio runtime unwrap in download/tray threads | dialogs/model/download.rs | 85 |
| F-AE91F1FF | Hotkey polling busy loop with no shutdown | main.rs | 254 |
| F-F7242D62 | Hotkey reload double-lock pattern (fragile ordering) | main.rs | 244 |
| F-11AA0296 | Conference mode dual-lock (transcription + diarization) | ui/conference.rs | 153 |
| F-CD32DE8A | Segmentation stop relies on sleep(100ms) not JoinHandle | recording/segmentation.rs | 196 |
| F-C485F5A8 | Transcription error swallowed silently in segmented mode | ui/mic.rs | 211 |
| F-EAEDFBCB | VAD creation failure silently ignored | recording/segmentation.rs | 104 |
| F-E90DACC3 | Window close hides - recording continues invisibly | ui/mod.rs | 137 |
| F-447B7279 | thread::sleep blocks GTK main thread (100ms) | ui/mic.rs | 321 |
| F-1E1D9EF0 | Download temp files not cleaned up on failure | infrastructure/models.rs | 135 |
| F-98D52FD7 | Tray model switch holds lock during slow load | infrastructure/tray.rs | 59 |
| F-EDBBC05E | No signal handler for SIGTERM/SIGINT | main.rs | 119 |
| F-4178C565 | Segmented mode stop polling can spin indefinitely | ui/mic.rs | 394 |
| F-53EA4617 | Model set-default holds config lock during model load | dialogs/model/list.rs | 108 |

### Performance (10 MEDIUM)

| ID | Title | File | Line |
|----|-------|------|------|
| F-85E44ECE | Samples buffer grows unboundedly during recording | recording/core.rs | 34 |
| F-3BB96A00 | Ring buffer write is sample-by-sample with mutex held | recording/ring_buffer.rs | 37 |
| F-C782CACD | Denoiser creates new resamplers per call | recording/denoise.rs | 50 |
| F-C5DD03E6 | Denoiser allocates intermediate Vec per frame | recording/denoise.rs | 151 |
| F-28A76F6A | New NnnoiselessDenoiser per transcription call | ui/mic.rs | 39 |
| F-FC3AE82A | Eager model loading blocks startup | main.rs | 164 |
| F-383A4B16 | thread::sleep blocks GTK main thread (auto-paste) | ui/mic.rs | 321 |
| F-A045CC87 | to_mono() allocates for single-channel audio | recording/core.rs | 19 |
| F-45913997 | WebRTC VAD allocates Vec<i16> on every is_speech | vad/webrtc.rs | 70 |
| F-BFCDC0B4 | Transcription mutex held for entire inference | ui/mic.rs | 209 |

### Maintainability (11 MEDIUM)

| ID | Title | File | Line |
|----|-------|------|------|
| F-29817E5B | Duplicated timer/level bar update loops | ui/mic.rs | 114 |
| F-1F5DAF71 | Duplicated post-transcription handling | ui/mic.rs | 306 |
| F-1A29C690 | Duplicated denoising pattern | ui/conference.rs | 129 |
| F-999055EE | Duplicated model resolution in CLI | cli/transcribe.rs | - |
| F-4A574FFF | Config struct approaching god object (18 fields) | app/config.rs | 7 |
| F-60997C54 | Domain layer imports from history (layering violation) | domain/types.rs | - |
| F-3C913733 | Inconsistent RMS function naming | recording/core.rs | - |
| F-E519DEFD | Dead code: NoOpDenoiser, create_denoiser | recording/denoise.rs | 197 |
| F-2E5359CF | cli/transcribe.rs at 625 lines | cli/transcribe.rs | 1 |
| F-5D347824 | ui/mic.rs at 448 lines | ui/mic.rs | 1 |
| F-9A631714 | history/mod.rs at 427 lines | history/mod.rs | 1 |

### Security (3 MEDIUM)

| ID | Title | File | Line |
|----|-------|------|------|
| F-15C26F89 | Path traversal in model filename parameter | infrastructure/models.rs | 108 |
| F-38F99208 | Config/history files world-readable (default umask) | app/config.rs | 204 |
| F-D50B6D79 | Config fields used without semantic validation | app/config.rs | 183 |

### Testability (10 MEDIUM)

| ID | Title | File | Line |
|----|-------|------|------|
| F-8FF6D3DF | No MockUIStateUpdater | test_support/mocks.rs | - |
| F-DDF32CB4 | Dialog modules have zero tests | dialogs/ | - |
| F-04B5B7B1 | Infrastructure mostly untested (hotkeys parsing) | infrastructure/hotkeys.rs | - |
| F-55386DF4 | LoopbackRecorder directly spawns subprocesses | recording/loopback.rs | - |
| F-0B7151CE | ConferenceRecorder hardcodes concrete types | recording/conference.rs | - |
| F-0EF51150 | RingBuffer has zero tests | recording/ring_buffer.rs | - |
| F-F6B2C631 | SegmentationMonitor has zero tests | recording/segmentation.rs | - |
| F-6A8E8BD6 | TranscriptionService conference methods untested | transcription/service.rs | - |
| F-1EC4A944 | thread_local! state prevents test isolation | ui/mic.rs | 29 |
| F-300742DF | CI lacks headless display (xvfb) | .github/workflows/ci.yml | - |

---

## LOW and INFO Findings

Low and INFO findings are available in the exported JSON at `docs/audit/findings-export.json`.

Key LOW findings include:
- Loopback recorder unvalidated monitor source name
- Temporary download files not cleaned up on crash
- Fallback to current directory when XDG dirs unavailable
- CLI accepts arbitrary file paths without sandboxing
- Tray thread has no graceful shutdown
- Settings dialog reads config lock 15+ times individually
- Excessive resampler quality settings for speech

Key INFO findings (positive observations):
- Subprocess calls use safe `.arg()` passing (no shell injection)
- HTTPS with TLS validation for all downloads
- No secrets/credentials in source code
- Strong mock infrastructure (6 mocks, 21 self-tests)
- Good domain/data test coverage

---

## Full Findings Data

All 113 findings are exported to `docs/audit/findings-export.json` and persisted in the mental model at `.audit/mental_model.yaml`.
