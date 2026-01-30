# Refactoring Session 017: P2.6 Create Integration Test Suite

**Date:** 2026-01-30
**Priority:** P2 (Technical Debt — testability)
**Findings Addressed:** F-789CDE98 (HIGH testability)
**Files Changed:** `src/lib.rs` (new), `tests/config_roundtrip.rs` (new), `tests/history_roundtrip.rs` (new), `tests/cli_transcribe.rs` (new)

---

## P2.6: Create Integration Test Suite

### Problem

The project had zero integration tests. All 198 tests were unit tests (internal `#[cfg(test)] mod tests` blocks). This meant:
- No validation that serialization formats are compatible across modules (Config TOML, History JSON)
- No verification that the CLI binary handles argument parsing and error cases correctly
- No cross-module testing of History operations (add → search → cleanup → trim → export)
- No file I/O round-trip testing through the full persist/load path

Integration tests (in `tests/`) are compiled as separate crates, so they exercise the public API surface — catching visibility issues, broken re-exports, and API contract violations that unit tests cannot.

### Structural Change: src/lib.rs

The project was binary-only (`src/main.rs`). Integration tests in `tests/` require a library crate to import from. Added `src/lib.rs` with public module re-exports:

```rust
pub mod app;
pub mod cli;
pub mod domain;
pub mod history;
pub mod infrastructure;
pub mod recording;
pub mod transcription;
pub mod vad;

#[cfg(test)]
pub mod test_support;
```

`dialogs` and `ui` modules are excluded — they depend on GTK4 runtime initialization and are not suitable for headless integration testing.

### Test 1: Config Round-Trip (`tests/config_roundtrip.rs`) — 6 tests

| Test | Behavior Verified |
|------|-------------------|
| `config_save_load_roundtrip` | Default Config → TOML file → load → all 21 fields match |
| `config_custom_values_roundtrip` | Non-default values preserved through serialize/deserialize |
| `config_partial_toml_uses_defaults` | Minimal TOML (2 fields) fills remaining fields with serde defaults |
| `config_unknown_fields_are_ignored` | Forward-compatible: extra TOML keys don't cause errors |
| `config_empty_toml_fails` | Empty string fails (required fields `default_model`, `language` missing) |
| `config_clone_preserves_all_fields` | Clone produces identical TOML serialization |

### Test 2: History Round-Trip (`tests/history_roundtrip.rs`) — 8 tests

| Test | Behavior Verified |
|------|-------------------|
| `history_persistence_roundtrip` | Mixed entries (dictation, conference, UTF-8) survive JSON file round-trip |
| `history_search_after_roundtrip` | Case-insensitive search works on deserialized history |
| `history_cleanup_and_trim_after_roundtrip` | `cleanup_old()` and `trim_to_limit()` via HistoryRepository trait |
| `history_remove_by_id_after_roundtrip` | UUID-based removal after deserialization |
| `history_filter_by_date_range` | Date filtering with specific timestamps (Q1-Q2 range) |
| `history_export_to_text_after_roundtrip` | JSON → load → export_to_text → verify Ukrainian output format |
| `history_empty_roundtrip` | Empty history serializes/deserializes correctly |
| `history_large_roundtrip` | 1000 entries round-trip without data loss, ordering preserved |

### Test 3: CLI Integration (`tests/cli_transcribe.rs`) — 7 tests

These tests run the compiled binary as a subprocess (`std::process::Command`), validating the CLI interface without requiring Whisper models or audio hardware.

| Test | Behavior Verified |
|------|-------------------|
| `cli_help_flag` | `--help` exits 0, prints usage info |
| `cli_version_flag` | `--version` exits 0, prints binary name |
| `cli_transcribe_help` | `transcribe --help` shows `--model`, `--language`, `--backend` options |
| `cli_transcribe_missing_input` | `transcribe` without file argument exits with error |
| `cli_transcribe_nonexistent_file` | Nonexistent WAV path exits with error (no panic) |
| `cli_models_subcommand` | `models` command runs without panic (exit code is defined) |
| `cli_invalid_subcommand` | Unknown subcommand exits with error |

### Design decisions

**Why lib.rs instead of feature flags?** The binary crate's modules are declared as `mod foo` (private) in `main.rs`. Integration tests need `pub mod foo` in `lib.rs`. Cargo supports having both `src/main.rs` and `src/lib.rs` simultaneously — the binary can use `mod` declarations while the library provides `pub mod` re-exports. This is the standard Rust pattern for integration testing binary crates.

**Why exclude dialogs/ and ui/ from lib.rs?** These modules call GTK4 initialization functions (`gtk4::init()`, `Application::new()`) at import time. Including them in the library crate would require GTK4 runtime in headless CI and test environments, which defeats the purpose of integration testing.

**Why subprocess-based CLI tests?** CLI tests exercise the actual binary's argument parsing and error handling. This catches issues that library-level testing misses: exit codes, stderr output format, clap configuration errors, and binary packaging correctness. These tests run against the debug binary produced by `cargo test`.

**Why not mock-based audio pipeline tests?** `test_support` is `#[cfg(test)]` in `main.rs`, making it unavailable to integration tests (separate crate). Changing this to a feature flag or dev-dependency would require architectural changes beyond the scope of P2.6. The existing 198 unit tests (including 24 ring_buffer + segmentation tests from P2.5, and 52 mock tests) provide sufficient audio pipeline coverage at the unit level.

---

## Metrics

| Metric | Before | After |
|--------|--------|-------|
| Integration tests | 0 | 21 |
| Integration test files | 0 | 3 |
| Unit tests | 198 | 198 (unchanged) |
| Total tests | 198 | 219 |
| New files | 0 | 4 (lib.rs + 3 test files) |
| Lines added | 0 | ~380 |

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code + 3 private_interfaces warnings unchanged)
cargo test    — 219 tests passed (198 unit + 21 integration), 0 failures
```

## Findings Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-789CDE98 (Zero integration tests) | HIGH | Fixed |

## Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 10 | 0 |
| P2 | 10 | 6 (P2.1–P2.6) | 4 |
| P3 | 10 | 0 | 10 |
