# Refactoring Session 014: P2.2 Decompose show_settings_dialog()

**Date:** 2026-01-30
**Priority:** P2 (Technical Debt — maintainability)
**Findings Addressed:** F-2880938A (HIGH maintainability)
**Files Changed:** `src/dialogs/settings.rs`

---

## P2.2: Decompose show_settings_dialog()

### Problem

`show_settings_dialog()` was a 430-line monolithic function responsible for:
- Building all UI sections (language, backend, recording mode, diarization, checkboxes, hotkeys, history)
- Wiring inter-widget dependencies (continuous mode toggling VAD, hotkey enabled toggling entry)
- Cloning 14 widget references for the save closure
- Reading all widget values and writing them back to Config

This made the function difficult to understand, modify, or test. Adding a new setting required navigating the entire function to find the right insertion point, adding yet another clone, and updating the save handler.

### Fix

#### 1. `SettingsWidgets` struct

Replaced 14 individual widget clones with a single struct that holds all widget references needed by the save handler:

```rust
struct SettingsWidgets {
    language_combo: ComboBoxText,
    backend_combo: ComboBoxText,
    mode_combo: ComboBoxText,
    // ... 10 more fields
}
```

The struct owns an `apply_to_config(&self, cfg: &mut Config)` method that reads all widget values and writes them into the config. This eliminated the save closure's 14 clones and ~35-line value extraction block.

#### 2. `RecordingWidgets` struct

The recording section returns 7 widgets (mode combo, diarization combo, 5 checkboxes). Rather than a 7-element tuple, a named struct provides clarity at the call site:

```rust
let recording = build_recording_section(&main_box, &cfg);
// ...
mode_combo: recording.mode_combo,
denoise_check: recording.denoise_check,
```

#### 3. Section builder functions

Extracted 5 builder functions, each taking `(&GtkBox, &Config)` and returning the widgets needed by the save handler:

| Function | Returns | Lines |
|----------|---------|-------|
| `build_language_section()` | `ComboBoxText` | 30 |
| `build_backend_section()` | `ComboBoxText` | 28 |
| `build_recording_section()` | `RecordingWidgets` | 92 |
| `build_hotkey_section()` | `(CheckButton, Entry)` | 29 |
| `build_history_section()` | `(SpinButton, SpinButton)` | 39 |

#### 4. Single config snapshot

The original code locked the config mutex 12 separate times to read initial values. The refactored version locks once:

```rust
let cfg = config.lock().clone();
```

Then passes the snapshot `&cfg` to all section builders. This eliminates 11 redundant lock acquisitions and guarantees a consistent view of settings even if another thread modifies config concurrently.

#### 5. `combo_to_value()` helper

Extracted a reusable helper that maps a `ComboBoxText` active index to a string value via a mapping table, replacing 3 instances of the same `if combo.active() == Some(1) { "x" } else { "y" }` pattern.

### Design decisions

**Why not split into separate files?** The settings dialog is a single GTK window. The section builders are private helpers that only make sense in this context. Splitting them into separate files (e.g., `settings/language.rs`, `settings/hotkeys.rs`) would add module boilerplate without meaningful encapsulation benefit — no other code needs these builders.

**Why clone Config instead of passing `&Config` from the lock guard?** The lock guard (`MutexGuard<Config>`) has a lifetime tied to the Mutex. Since builders append GTK widgets (no lifetime constraint), the borrow would complicate the code. Cloning Config (18 fields, mostly small strings) is negligible and gives a cleaner API.

**Why `RecordingWidgets` struct but tuples for other sections?** The recording section returns 7 items — a tuple that large is unreadable. Other sections return 1-2 items where tuples are fine.

---

## Metrics

| Metric | Before | After |
|--------|--------|-------|
| `show_settings_dialog()` lines | 430 | 80 |
| Longest function in file | 430 | 92 (`build_recording_section`) |
| Widget clones in save handler | 14 | 0 (struct moved) |
| Config lock acquisitions | 12 | 1 |
| Total file lines | 430 | 414 |
| Net lines changed | — | -16 (286 added, 302 removed) |

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 165/165 passed (0 regressions)
```

## Findings Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-2880938A (429-line show_settings_dialog function) | HIGH | Fixed |

## Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 10 | 0 |
| P2 | 10 | 2 (P2.1, P2.2) | 8 |
| P3 | 10 | 0 | 10 |
