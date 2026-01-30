# Refactoring Session 018: P2.7 Extract Duplicated UI Patterns

**Date:** 2026-01-30
**Priority:** P2 (Technical Debt -- maintainability)
**Findings Addressed:** F-29817E5B (MEDIUM), F-1F5DAF71 (MEDIUM), F-1A29C690 (MEDIUM)
**Files Changed:** `src/ui/shared.rs` (new), `src/ui/mic.rs`, `src/ui/conference.rs`, `src/ui/conference_file.rs`, `src/ui/mod.rs`

---

## P2.7: Extract Duplicated UI Patterns

### Problem

Three recording mode handlers (`mic.rs`, `conference.rs`, `conference_file.rs`) contained duplicated code for:

1. **Timer update loop** -- identical 12-line `glib::timeout_add_local` block in all 3 modules
2. **Conference level bar loop** -- identical 12-line `glib::timeout_add_local` block in `conference.rs` and `conference_file.rs`
3. **Post-transcription actions** -- 30-line auto-copy/auto-paste/history-save sequence duplicated in `mic.rs` and `conference.rs`
4. **Audio denoising wrapper** -- `maybe_denoise()` defined in `mic.rs`, duplicated inline (with per-channel expansion) in `conference.rs`

Total duplicated code: ~150 lines across 3 files.

### Solution

Created `src/ui/shared.rs` with 4 shared functions:

| Function | Replaces | Callers |
|----------|----------|---------|
| `start_timer_loop(rec, base)` | 3 identical inline timer loops | mic.rs, conference.rs, conference_file.rs |
| `start_conference_level_loop(ctx, rec, ui)` | 2 identical inline level bar loops | conference.rs, conference_file.rs |
| `maybe_denoise(samples, enabled)` | 1 function + 1 inline copy (26 lines) | mic.rs (2 call sites), conference.rs (1 call site) |
| `handle_post_transcription(ctx, base, ...)` | 2 duplicated 30-line blocks | mic.rs, conference.rs |

### Changes by file

**`src/ui/shared.rs` (new, 134 lines)**
- `start_timer_loop()`: Takes `&RecordingContext` and `&UIContext`, sets up 1-second glib timer
- `start_conference_level_loop()`: Takes `&Arc<AppContext>`, `&RecordingContext`, `&ConferenceUI`, sets up 50ms level bar update
- `maybe_denoise()`: Wraps `NnnoiselessDenoiser::new()` + `denoise_buffer()` with fallback
- `handle_post_transcription()`: Async function handling auto-copy, auto-paste (with async xdotool spawn), and history save. Accepts optional `recording_file` and `speakers` for conference metadata.

**`src/ui/mic.rs` (507 -> 456 lines, -51)**
- Removed local `maybe_denoise()` (12 lines), replaced with `use shared::maybe_denoise`
- Removed local `start_timer_loop()` (12 lines), replaced with `shared::start_timer_loop()`
- Replaced 30-line post-transcription block in `handle_simple_stop()` with `shared::handle_post_transcription()` call
- `start_level_loop()` and `start_vad_loop()` remain local (mic-specific: single level bar + VAD indicator)
- Segmented mode `handle_segmented_stop()` keeps its own history logic (accumulated text from multiple segments, not a single transcription result)

**`src/ui/conference.rs` (229 -> 149 lines, -80)**
- Removed 12-line inline timer loop, replaced with `shared::start_timer_loop()`
- Removed 12-line inline level loop, replaced with `shared::start_conference_level_loop()`
- Removed 26-line inline per-channel denoise, replaced with 2 `maybe_denoise()` calls
- Removed 38-line post-transcription block, replaced with `shared::handle_post_transcription()` with recording file path and speaker metadata
- Removed imports: `NnnoiselessDenoiser`, `save_history`, `HistoryEntry`, `HistoryRepository`

**`src/ui/conference_file.rs` (121 -> 97 lines, -24)**
- Removed 12-line inline timer loop, replaced with `shared::start_timer_loop()`
- Removed 12-line inline level loop, replaced with `shared::start_conference_level_loop()`
- No post-transcription changes (this mode saves files, not transcription text)

**`src/ui/mod.rs`**
- Added `pub(crate) mod shared;` module declaration

### Design decisions

**Why `UIContext` instead of `impl UIStateUpdater`?** The timer loop and post-transcription functions need a `Clone + 'static` type to move into closures. Using `impl UIStateUpdater + Clone + 'static` creates ambiguous type syntax in Rust. Since all callers pass `UIContext` (the concrete type), using it directly avoids unnecessary generics while maintaining clarity. If a new UI backend is added, these functions can be parameterized then.

**Why keep `start_level_loop` and `start_vad_loop` local to mic.rs?** The mic-specific level loop reads `ctx.audio.mic_amplitude()` and updates a single `LevelBar`. The conference version reads both `get_mic_amplitude()` and `get_loopback_amplitude()` and updates two bars via `update_levels()`. These are structurally different enough that forcing them through a single generic function would add complexity without reducing duplication.

**Why keep segmented mode history logic local?** The segmented stop path saves accumulated text from the UI result text view (which may be partially filled from cancelled/timed-out segment processing). This is fundamentally different from the simple "transcription result -> history" flow. Forcing it through `handle_post_transcription()` would require adding conditional logic that makes the shared function harder to understand.

**Why does `handle_post_transcription` take `recording_file` and `speakers`?** Conference mode saves recording files and speaker metadata. Dictation mode passes `None` and `vec![]`. The function dispatches to `HistoryEntry::new()` vs `HistoryEntry::new_with_recording()` based on these parameters, keeping both callers simple.

---

## Metrics

| Metric | Before | After |
|--------|--------|-------|
| mic.rs lines | 507 | 456 (-51) |
| conference.rs lines | 229 | 149 (-80) |
| conference_file.rs lines | 121 | 97 (-24) |
| shared.rs lines | 0 | 134 (new) |
| Total UI handler lines | 857 | 836 (-21) |
| Duplicated code blocks | 6 | 0 |
| Duplicated lines eliminated | ~150 | 0 |

## Verification

```
cargo clippy  -- 0 new warnings (pre-existing dead_code + private_interfaces warnings unchanged)
cargo test    -- 219 tests passed (198 unit + 21 integration), 0 failures
```

## Findings Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-29817E5B (Duplicated timer/level loops across modes) | MEDIUM | Fixed |
| F-1F5DAF71 (Duplicated post-transcription handler) | MEDIUM | Fixed |
| F-1A29C690 (maybe_denoise duplicated in conference.rs) | MEDIUM | Fixed |

## Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 10 | 0 |
| P2 | 10 | 7 (P2.1--P2.7) | 3 |
| P3 | 10 | 0 | 10 |
