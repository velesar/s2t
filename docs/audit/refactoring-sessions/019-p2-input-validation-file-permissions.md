# Session 019: Input Validation and File Permission Hardening (P2.8, P2.9, P2.10)

**Date:** 2026-01-30
**Scope:** P2.8 (path traversal guards), P2.9 (config validation), P2.10 (restrictive file permissions)
**Status:** Completed

---

## Summary

This session addresses the final three items in the P2 (Technical Debt) tier, all related to input validation and security hardening at system boundaries. These three items form a cohesive unit: preventing malicious filenames from escaping the models directory (P2.8), validating configuration values after loading from disk (P2.9), and restricting file permissions on sensitive user data (P2.10).

---

## Changes

### P2.8: Path Traversal Guards (`src/infrastructure/models.rs`)

**Findings addressed:** F-15C26F89 (MEDIUM security)

Added `sanitize_model_filename()` that rejects filenames containing:
- Path separators (`/`, `\`)
- Parent directory references (`..`)
- Null bytes (`\0`)
- Empty strings

Applied to:
- `is_model_downloaded()` — returns `false` for invalid filenames (no panic)
- `delete_model()` — returns `Err` before any filesystem operation
- `download_model()` — returns `Err` before initiating HTTP request

Added 7 tests covering valid filenames, path traversal, separators, empty strings, and null bytes.

### P2.9: Config Validation (`src/app/config.rs`)

**Findings addressed:** F-D50B6D79 (MEDIUM security)

Added `Config::validate()` method called after `load_config()` deserialization:

| Field | Validation | Action |
|-------|-----------|--------|
| `default_model` | No `/`, `\`, `..` | Reject (bail) |
| `segment_interval_secs` | [1, 300] | Clamp |
| `history_max_entries` | [1, 10_000] | Clamp |
| `history_max_age_days` | [1, 3650] | Clamp |
| `silero_threshold` | [0.0, 1.0] | Clamp |
| `recording_mode` | dictation/conference/conference_file | Reset to default |
| `stt_backend` | whisper/tdt | Reset to default |
| `vad_engine` | webrtc/silero | Reset to default |

Design decision: numeric fields are clamped (tolerant) while path-like fields are rejected (strict). Enum-like string fields reset to defaults with no error, matching serde's `#[serde(default)]` philosophy.

Added 11 tests covering all validation rules.

### P2.10: Restrictive File Permissions (`src/app/config.rs`, `src/history/persistence.rs`)

**Findings addressed:** F-38F99208 (MEDIUM security)

Added `set_owner_only_permissions()` that sets `0o600` (owner read/write only) on Unix. No-op on non-Unix platforms. Applied after:
- `save_config()` writes `config.toml`
- `save_history()` writes `history.json`

---

## Files Modified

| File | Lines Changed | Nature |
|------|--------------|--------|
| `src/infrastructure/models.rs` | +60 | `sanitize_model_filename()` + guards in 3 functions + 7 tests |
| `src/app/config.rs` | +100 | `Config::validate()` + `set_owner_only_permissions()` + 11 tests |
| `src/history/persistence.rs` | +2 | Call `set_owner_only_permissions()` after save |

---

## Verification

```
cargo clippy  -> No new warnings (only pre-existing dead_code from P3.1)
cargo test    -> 216 unit tests + 21 integration tests, 0 failures
```

---

## Findings Resolved

| Finding ID | Severity | Category | Title |
|-----------|----------|----------|-------|
| F-15C26F89 | MEDIUM | Security | Path traversal in model filename handling |
| F-D50B6D79 | MEDIUM | Security | No config validation after loading |
| F-38F99208 | MEDIUM | Security | World-readable config and history files |

---

## P2 Tier Status: COMPLETE

All 10 P2 items have been completed across sessions 013-019:

| Item | Session | Status |
|------|---------|--------|
| P2.1: Extract download_file | 013 | Done |
| P2.2: Decompose settings dialog | 014 | Done |
| P2.3: AppContext::for_testing | 015 | Done |
| P2.4: MockUIStateUpdater | 015 | Done |
| P2.5: RingBuffer/SegmentationMonitor tests | 016 | Done |
| P2.6: Integration test suite | 017 | Done |
| P2.7: Extract UI patterns | 018 | Done |
| P2.8: Path traversal guards | 019 | Done |
| P2.9: Config validation | 019 | Done |
| P2.10: File permissions | 019 | Done |
