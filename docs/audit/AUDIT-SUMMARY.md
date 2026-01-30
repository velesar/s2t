# Code Audit Summary

**Project:** Voice Dictation (s2t)
**Version:** 0.3.0
**Commit:** `0d54993`
**Audit Date:** 2026-01-30
**Auditor:** Claude Code (Automated Analysis)
**Methodology:** 11-viewpoint architecture fitness audit (Foundation, Structure, Quality)

---

## Executive Summary

Voice Dictation is a Rust GTK4 desktop application (~10,929 LOC across 57 files) for offline speech-to-text on Linux. This audit identified **113 findings** across 5 quality categories, with **16 HIGH severity** issues requiring immediate attention. The most critical problems are in the **audio pipeline reliability** (deadlock, panics, mutex poisoning cascade) and **performance** (heap allocations and mutex locks in real-time audio callbacks).

### Key Metrics

| Metric | Value |
|--------|-------|
| Total Source Files | 57 (.rs) |
| Total Lines of Code | 10,929 |
| Total Symbols (SCIP) | 1,246 |
| Dependencies | 25+ direct |
| Unit Tests | 152 |
| Integration Tests | 0 |

### Findings Overview

| Severity | Count | Description |
|----------|-------|-------------|
| HIGH | 16 | Deadlock, panics, no integrity checks, untested code |
| MEDIUM | 54 | Resource leaks, silent errors, coupling, duplication |
| LOW | 34 | Minor code style, documentation gaps, edge cases |
| INFO | 9 | Positive observations, acceptable patterns |
| **Total** | **113** | |

### By Category

| Category | Count | Key Issues |
|----------|-------|------------|
| Reliability | 25 | Deadlock in denoise, panic on odd bytes, mutex poison cascade |
| Maintainability | 24 | Triplicated download code, 429-line function, 535-line file |
| Security | 23 | No model integrity verification, path traversal, world-readable files |
| Performance | 22 | Heap allocs in audio callback, mutex in real-time thread, unbounded buffers |
| Testability | 19 | No integration tests, UI untested, AppContext untestable |

### Risk Assessment

| Category | Risk Level | Notes |
|----------|------------|-------|
| Reliability | **HIGH** | ABBA deadlock, panic in audio thread, mutex poison cascade |
| Performance | **HIGH** | Real-time audio callback violates lock-free requirements |
| Security | **MEDIUM** | No model checksum verification, path traversal possible |
| Maintainability | **MEDIUM** | Significant code duplication, oversized files/functions |
| Testability | **MEDIUM** | Zero integration tests, UI completely untested |

---

## Technology Stack

- **Language:** Rust 2021 Edition (1.93.0)
- **GUI Framework:** GTK4 0.9 + glib/gio 0.20
- **Audio:** CPAL 0.15 (capture) + Rubato 0.16 (resampling) + nnnoiseless 0.5 (denoise)
- **Speech Recognition:** whisper-rs 0.12 (whisper.cpp) + parakeet-rs 0.2 (NVIDIA TDT)
- **Async Runtime:** Tokio 1.x + async-channel 2.3
- **System Tray:** ksni 0.3 (StatusNotifierItem)
- **Hotkeys:** global-hotkey 0.5
- **Configuration:** TOML via serde
- **Platform:** Linux (Fedora optimized)

---

## Viewpoints Analyzed

| ID | Viewpoint | Status | Key Findings |
|----|-----------|--------|--------------|
| VP-F01 | Tech Stack | Completed | v0.3.0, 25+ deps, release with LTO |
| VP-F02 | Repository Structure | Completed | 57 files, 10,929 LOC, 7 files over 400 lines |
| VP-F03 | Build & Deploy | Completed | GitHub Actions CI, install.sh, no packaging |
| VP-S01 | Architecture | Completed | Layered with hexagonal elements, AppContext DI |
| VP-S02 | Domain Model | Completed | 7 domain traits, 3 recording modes |
| VP-S03 | Interface Surface | Completed | CLI + GUI, xdotool/pactl/parec IPC, HTTPS downloads |
| VP-Q01 | Security | Completed | 23 findings (1 HIGH, 3 MEDIUM, 5 LOW, 14 INFO) |
| VP-Q02 | Reliability | Completed | 25 findings (5 HIGH, 15 MEDIUM, 5 LOW) |
| VP-Q03 | Maintainability | Completed | 24 findings (3 HIGH, 11 MEDIUM, 4 LOW) |
| VP-Q04 | Performance | Completed | 22 findings (3 HIGH, 10 MEDIUM, 9 LOW) |
| VP-Q05 | Testability | Completed | 19 findings (3 HIGH, 10 MEDIUM, 4 LOW, 2 INFO) |

---

## Root Causes

The synthesis identified 5 root cause clusters:

| ID | Root Cause | Findings | Key Driver |
|----|------------|----------|------------|
| RC-6DEBFCED | Reliability Issues | 25 | Unwrap-heavy error handling, mutex misuse in concurrent audio pipeline |
| RC-0437BA7A | Maintainability Issues | 24 | Code duplication (models.rs), monolithic functions (settings.rs) |
| RC-F7E29DCB | Security Issues | 23 | Missing input validation, no download integrity checks |
| RC-2C63AEA2 | Performance Issues | 22 | Real-time audio path uses heap allocs + mutexes, unbounded buffer growth |
| RC-02650BF4 | Testability Issues | 19 | No integration tests, UI logic coupled to GTK, AppContext requires hardware |

---

## Architecture Overview

```
Voice Dictation v0.3.0 Architecture
====================================

+-----------------------------------------------------------------+
|                    Presentation Layer                            |
|  ui/ (7 files)    | dialogs/ (8 files) | cli/ (4 files)        |
|  state, dispatch  | settings, models   | transcribe, args      |
|  mic, conference  | history, export    | denoise-eval          |
+---------------------------+-------------------------------------+
|                    Application Layer                             |
|  app/context.rs (DI)  | app/channels.rs  | app/config.rs       |
+---------------------------+-------------------------------------+
|                    Domain / Contract Layer                       |
|  domain/traits.rs (7 traits) | domain/types.rs (AppState, etc) |
+---------------------------+-------------------------------------+
|                    Infrastructure Layer                          |
|  recording/ (8 files)    | transcription/ (4 files)            |
|  mic, loopback, denoise  | whisper, tdt, diarization           |
|  segmentation, ring_buf  | service                             |
|  infrastructure/ (5 files) | vad/ (3 files)                    |
|  hotkeys, tray, paste    | webrtc, silero                     |
|  models, recordings      |                                     |
+-----------------------------------------------------------------+
```

---

## Conclusion

The project has a well-designed trait-based architecture with clear separation of concerns. However, the audio pipeline has critical reliability and performance issues that could cause crashes, deadlocks, and audio glitches in production use. The most urgent work is fixing the ABBA deadlock in `denoise.rs`, the panic on odd bytes in `loopback.rs`, and replacing `Arc<Mutex<Vec<f32>>>` with a lock-free ring buffer in the CPAL audio callback.

**Recommended Actions (Priority Order):**
1. **P0:** Fix deadlock, panics, and model integrity verification
2. **P1:** Switch to `parking_lot::Mutex`, lock-free audio buffers, pre-allocated callbacks
3. **P2:** Extract shared download logic, decompose settings, add test constructors
4. **P3:** Create integration tests, add signal handlers, hardening

See [FINDINGS.md](./FINDINGS.md) for detailed findings and [REMEDIATION-PLAN.md](./REMEDIATION-PLAN.md) for the comprehensive fix plan.
