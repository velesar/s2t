# Audit Recommendations

**Project:** Voice Dictation (s2t)
**Audit Date:** 2026-01-30
**Supersedes:** Previous recommendations from 2026-01-28

---

## Priority Matrix

| Priority | Category | Findings | Key Actions |
|----------|----------|----------|-------------|
| **P0** | Fix Now | 8 HIGH | Deadlock, panics, SHA256 verification, buffer clone |
| **P1** | Next Sprint | 20 MEDIUM | parking_lot, lock-free audio, Drop impls, signal handlers |
| **P2** | Tech Debt | 30 MEDIUM | Download dedup, settings split, tests, validation |
| **P3** | Hardening | 55 LOW+INFO | Dead code, docs, async model loading, permissions |

---

## P0: Must Fix (crash/data-integrity)

| # | Action | Finding | File | Effort |
|---|--------|---------|------|--------|
| 0.1 | Fix ABBA deadlock in denoise | F-565811D6 | recording/denoise.rs | Low |
| 0.2 | Use chunks_exact(2) in loopback | F-AE877B80 | recording/loopback.rs | Low |
| 0.3 | Handle CPAL stream errors | F-C9E210CD | recording/microphone.rs | Low |
| 0.4 | Add SHA256 model verification | F-752F5717 | infrastructure/models.rs | Medium |
| 0.5 | Fix segmentation buffer clone | F-F00B80FC | recording/segmentation.rs | Low |

## P1: Should Fix (reliability + performance)

| # | Action | Finding | File | Effort |
|---|--------|---------|------|--------|
| 1.1 | Switch to parking_lot::Mutex | F-D0663E4B | All (~15 files) | Medium |
| 1.2 | Lock-free ring buffer for audio | F-B1A4B2AE | recording/microphone.rs | Medium |
| 1.3 | Add Drop implementations | F-EDDDE3A1 | core.rs, loopback.rs, hotkeys.rs | Low |
| 1.4 | Fix thread::sleep on GTK thread | F-447B7279 | ui/mic.rs, ui/conference.rs | Low |
| 1.5 | Add signal handlers | F-EDBBC05E | main.rs | Medium |
| 1.6 | Fix silent error swallowing | F-C485F5A8 | ui/mic.rs, segmentation.rs | Low |
| 1.7 | Fix lock ordering issues | F-F7242D62 | main.rs, conference.rs, tray.rs | Low |
| 1.8 | Store JoinHandle for segmentation | F-CD32DE8A | recording/segmentation.rs | Low |
| 1.9 | Add timeout to polling loop | F-4178C565 | ui/mic.rs | Low |
| 1.10 | Pre-allocate and reuse denoiser | F-28A76F6A | ui/mic.rs, recording/denoise.rs | Medium |

## P2: Should Fix (maintainability + testability)

| # | Action | Finding | File | Effort |
|---|--------|---------|------|--------|
| 2.1 | Extract shared download_file() | F-0FAD129D | infrastructure/models.rs | Medium |
| 2.2 | Decompose settings dialog | F-2880938A | dialogs/settings.rs | Medium |
| 2.3 | Add AppContext::for_testing() | F-2F3EE753 | app/context.rs | Low |
| 2.4 | Add MockUIStateUpdater | F-8FF6D3DF | test_support/mocks.rs | Low |
| 2.5 | Add RingBuffer/Segmentation tests | F-0EF51150 | recording/ | Medium |
| 2.6 | Create integration test suite | F-789CDE98 | tests/ | Medium |
| 2.7 | Extract duplicated UI patterns | F-29817E5B | ui/mic.rs, conference.rs | Medium |
| 2.8 | Add path traversal guards | F-15C26F89 | infrastructure/models.rs | Low |
| 2.9 | Add config validation | F-D50B6D79 | app/config.rs | Low |
| 2.10 | Set restrictive file permissions | F-38F99208 | app/config.rs | Low |

## P3: Nice to Have (hardening)

| # | Action | Finding | Effort |
|---|--------|---------|--------|
| 3.1 | Clean up dead code (12 items) | F-E519DEFD | Low |
| 3.2 | Fix domain layering violation | F-60997C54 | Low |
| 3.3 | Add xvfb to CI | F-300742DF | Low |
| 3.4 | Clean stale .downloading files | F-1E1D9EF0 | Low |
| 3.5 | Error on missing XDG dirs | multiple | Low |
| 3.6 | Async model loading | F-FC3AE82A | Medium |
| 3.7 | Reduce resampler quality | F-202485F0 | Low |
| 3.8 | Add public API doc comments | multiple | Low |
| 3.9 | Pre-allocate sample buffers | F-85E44ECE | Low |
| 3.10 | Extract UI business logic for testing | F-31C9E22E | High |

---

## Detailed Plan

See [REMEDIATION-PLAN.md](./REMEDIATION-PLAN.md) for implementation details, code examples, and verification steps for each action item.

---

## Metrics to Track

| Metric | Current | Target |
|--------|---------|--------|
| HIGH findings | 16 | 0 |
| MEDIUM findings | 54 | 0 |
| Deadlock risks | 1 | 0 |
| Panic-on-error sites | 70+ | 0 |
| Integration tests | 0 | 10+ |
| Largest file (LOC) | 625 | < 500 |
| Code duplication | ~150 lines | 0 |
