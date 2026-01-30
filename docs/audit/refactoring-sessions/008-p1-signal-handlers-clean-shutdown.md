# Refactoring Session 008: P1.5 Signal Handlers for Clean Shutdown

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — reliability)
**Finding Addressed:** F-EDBBC05E (MEDIUM reliability)
**Files Changed:** `Cargo.toml`, `src/main.rs`

---

## P1.5: Add Signal Handlers for Clean Shutdown

### Problem

The application had no signal handlers. When receiving SIGINT (Ctrl+C) or SIGTERM (e.g., from `systemctl stop` or `kill`), the process terminated immediately without:
- Saving unsaved transcription history to disk
- Properly shutting down the GTK main loop
- Allowing Drop implementations to run (hotkey unregistration, recording cleanup)

This could result in data loss (recent transcriptions not persisted) and resource leaks (orphaned `parec` processes, registered but uncleared global hotkeys).

### Solution

Added a three-layer shutdown mechanism:

1. **Signal handler (`ctrlc` crate):** Catches SIGINT/SIGTERM, immediately saves history from the signal thread, then sends a shutdown message via `async_channel` to the GTK main loop.

2. **GTK main loop listener (`glib::spawn_future_local`):** Receives the shutdown signal, saves history again from the main thread (belt-and-suspenders — the signal handler may have been interrupted), and calls `app.quit()` to initiate orderly GTK shutdown.

3. **GTK `connect_shutdown` callback:** Saves history on every GTK application shutdown, regardless of trigger (window close, tray Quit, signal, etc.). This covers the normal exit path that didn't previously persist history.

### Why `ctrlc` instead of `signal-hook`

- `ctrlc` is simpler (one function call vs manual signal set management)
- It handles both SIGINT and SIGTERM by default on Unix
- No need for the advanced features of `signal-hook` (signal masking, async iterators)
- Well-maintained, 50M+ downloads, used by many CLI tools in the Rust ecosystem

### Why save history in multiple places

The signal handler runs on a dedicated signal thread — it's a "best effort" save. The `glib::spawn_future_local` listener is more reliable but requires the GTK main loop to be running. The `connect_shutdown` callback covers the normal exit path. The `parking_lot::Mutex` (from session 004) ensures all three paths can safely acquire the lock without poisoning concerns.

### Changes

**`Cargo.toml`:**
- Added `ctrlc = "3.5"` dependency

**`src/main.rs`:**
- Added `ctrlc::set_handler()` that saves history and sends shutdown signal
- Added `glib::spawn_future_local` listener that calls `app.quit()` on signal
- Added `app.connect_shutdown()` callback that saves history on GTK shutdown

### Verification

```
cargo clippy  — 13 pre-existing dead_code warnings, no new warnings
cargo test    — 165 tests passed, 0 failed
```

### Findings Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-EDBBC05E (No signal handlers for clean shutdown) | MEDIUM | Fixed |

### Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 5 (P1.1-P1.5) | 5 (P1.6-P1.10) |
| P2 | 10 | 0 | 10 |
| P3 | 10 | 0 | 10 |
