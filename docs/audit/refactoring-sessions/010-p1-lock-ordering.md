# Refactoring Session 010: P1.7 Fix Lock Ordering Issues

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — reliability)
**Findings Addressed:** F-F7242D62, F-11AA0296, F-98D52FD7, F-53EA4617 (MEDIUM reliability)
**Files Changed:** `src/main.rs`, `src/ui/conference.rs`, `src/dialogs/model/list.rs`

---

## P1.7: Fix Lock Ordering Issues

### Problem

The remediation plan identified 4 sites where multiple mutexes were held simultaneously, creating potential deadlock risk:

1. **`main.rs:244-250`** — Initial hotkey registration: `config.lock()` then `hotkey_manager.lock()` held simultaneously
2. **`main.rs:256-263`** — Hotkey reload handler: same pattern as above
3. **`ui/conference.rs:153-154`** — Conference transcription: `transcription.lock()` then `diarization.lock()` held simultaneously
4. **`infrastructure/tray.rs:49-66`** — Model selection: already properly sequenced (locks not held simultaneously)
5. **`dialogs/model/list.rs:107-121`** — Set default model: `config.lock()` held while `transcription.lock()` acquired

### Analysis

**Principle:** Never hold two mutex locks simultaneously unless absolutely necessary. When two locks must be held, document and enforce a consistent ordering.

**Site 1 & 2 (main.rs):** The config lock was held while acquiring the hotkey_manager lock. Since `register_from_config()` only reads config values (`&Config`), we can clone the Config, drop the lock, then acquire hotkey_manager.

**Site 3 (conference.rs):** `transcribe_conference()` requires both `&TranscriptionService` and `&mut DiarizationEngine` simultaneously — the function signature demands both. We cannot eliminate holding two locks here, but we can document and enforce a consistent ordering (diarization before transcription) to prevent ABBA deadlock.

**Site 4 (tray.rs):** Already correctly implemented — config lock is dropped in a block scope before transcription lock is acquired. No change needed.

**Site 5 (list.rs):** Config lock was held across `save_config()` and then into `transcription.lock()`. Fixed by scoping the config lock to just the save operation, dropping it before acquiring the transcription lock.

### Fix 1: main.rs — Clone config, drop lock before hotkey registration

```rust
// Before: two locks held simultaneously
let cfg = config.lock();
let mut hk = hotkey_manager.lock();
hk.register_from_config(&cfg)?;

// After: clone config, release lock, then acquire hotkey lock
let cfg_snapshot = config.lock().clone();
let mut hk = hotkey_manager.lock();
hk.register_from_config(&cfg_snapshot)?;
```

Applied to both the initial registration (line 244) and the reload handler (line 258). Config implements `Clone` via derive, so the snapshot is a straightforward value copy.

### Fix 2: conference.rs — Document lock ordering (diarization before transcription)

```rust
// Before: transcription locked first, then diarization
let ts = ctx_for_thread.transcription.lock();
let mut engine_guard = ctx_for_thread.diarization.lock();

// After: diarization locked first (consistent ordering documented)
// Lock ordering: diarization before transcription.
let mut engine_guard = ctx_for_thread.diarization.lock();
let ts = ctx_for_thread.transcription.lock();
```

Both locks must be held simultaneously because `transcribe_conference()` requires references to both services. No other code path acquires both locks, so there is no ABBA risk. The comment documents the ordering convention for future maintainers.

### Fix 3: list.rs — Scope config lock, drop before transcription lock

```rust
// Before: config lock held across save AND model load
let mut cfg = config_clone.lock();
cfg.default_model = filename_owned.clone();
save_config(&cfg)?;
// config still locked here!
let mut ts = transcription_clone.lock();  // second lock acquired
ts.load_model(&model_path)?;

// After: config lock scoped to save only
{
    let mut cfg = config_clone.lock();
    cfg.default_model = filename_owned.clone();
    save_config(&cfg)?;
}  // config lock dropped here

let mut ts = transcription_clone.lock();
ts.load_model(&model_path)?;
drop(ts);
```

This matches the existing pattern in `tray.rs::select_model()`, which already correctly sequences these same operations.

### Why tray.rs was already correct

The remediation plan listed `infrastructure/tray.rs:59` as a site to fix. On inspection, `select_model()` already uses proper lock scoping:

```rust
fn select_model(&mut self, filename: &str) {
    {
        let mut cfg = self.config.lock();  // config locked
        cfg.default_model = filename.to_string();
        save_config(&cfg)?;
    }  // config lock dropped

    let mut ts = self.transcription.lock();  // transcription locked separately
    ts.load_model(&model_path)?;
}
```

No changes needed.

---

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 165/165 passed (0 regressions)
```

## Findings Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-F7242D62 (main.rs hotkey reload holds two locks) | MEDIUM | Fixed |
| F-11AA0296 (conference.rs transcription+diarization held simultaneously) | MEDIUM | Fixed (documented ordering) |
| F-98D52FD7 (tray.rs model creation inside lock) | MEDIUM | Already correct |
| F-53EA4617 (model list config+transcription held simultaneously) | MEDIUM | Fixed |

## Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 7 (P1.1-P1.7) | 3 (P1.8-P1.10) |
| P2 | 10 | 0 | 10 |
| P3 | 10 | 0 | 10 |
