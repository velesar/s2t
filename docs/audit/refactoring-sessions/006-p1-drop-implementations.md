# Refactoring Session 006: P1.3 Drop Implementations for Resource Cleanup

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — reliability)
**Finding Addressed:** F-EDDDE3A1 (MEDIUM reliability)
**Files Changed:** `src/recording/core.rs`, `src/infrastructure/hotkeys.rs`

---

## P1.3: Add Drop Implementations for Resource Cleanup

**Finding:** F-EDDDE3A1 (MEDIUM reliability)
**Risk:** Resource leaks if structs are dropped without explicit `stop()` / `unregister()` calls (e.g., on early return, panic unwinding, or forgotten cleanup)

### Problem

Several key structs held system resources but had no `Drop` implementation:

1. **RecordingCore** — owns `is_recording: Arc<AtomicBool>` that controls spawned recording threads. If `RecordingCore` is dropped without calling `stop()`, the background threads continue spinning forever on `while is_recording.load(SeqCst)`.

2. **LoopbackRecorder** — spawns a `parec` child process controlled by `is_recording`. Without cleanup, the process lingers as a zombie.

3. **HotkeyManager** — registers a global system hotkey. Without cleanup, the hotkey stays registered after the application exits, potentially blocking other applications from using the same key binding.

4. **AudioRecorder** — CPAL stream and consumer thread, also controlled by `is_recording`.

### Fix

**1. RecordingCore::Drop — set `is_recording` to false:**

```rust
impl Drop for RecordingCore {
    fn drop(&mut self) {
        self.is_recording.store(false, Ordering::SeqCst);
    }
}
```

This is the keystone fix. Since both `AudioRecorder` and `LoopbackRecorder` compose `RecordingCore`, this single Drop cascades to all recording types:

- **AudioRecorder**: The recording thread sees `is_recording == false`, exits its polling loop, drops the CPAL stream (stopping callbacks), joins the consumer thread, and sends the completion signal.
- **LoopbackRecorder**: The reader thread sees `is_recording == false`, exits its loop, calls `child.kill()` on the parec process, and sends completion.

No separate Drop is needed for `AudioRecorder` or `LoopbackRecorder` — the `core` field's Drop handles everything.

**2. HotkeyManager::Drop — unregister hotkey:**

```rust
impl Drop for HotkeyManager {
    fn drop(&mut self) {
        if let Some(hotkey) = self.current_hotkey.take() {
            let _ = self.manager.unregister(hotkey);
        }
    }
}
```

This ensures the global hotkey is released when the manager is dropped. The `let _ =` pattern intentionally ignores errors during drop — there's nothing useful to do if unregistration fails during cleanup.

### Design Decision: Why Not Store CPAL Stream Handle?

The remediation plan originally suggested storing the CPAL `Stream` handle in `AudioRecorder` for explicit cleanup. After analysis, this was rejected because:

1. `cpal::Stream` is not `Send` on all platforms, making it difficult to store in a struct that's shared across threads.
2. The current architecture already handles stream cleanup correctly through the `is_recording` flag — the stream lives inside the spawned thread and is dropped there.
3. The `RecordingCore::Drop` fix ensures the flag is set to `false` on all drop paths, which triggers the same cleanup chain as `stop()`.

### Tests Added

2 new tests in `recording::core::tests`:

- **`test_drop_clears_is_recording`** — verifies that preparing a recording and then dropping `RecordingCore` without calling `stop()` works without panic.
- **`test_drop_signals_thread_to_stop`** — spawns a background thread that polls `is_recording`, drops `RecordingCore` without calling `stop()`, and asserts the thread exits. This simulates the actual resource leak scenario.

---

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 165/165 passed (2 new tests, 0 regressions)
```

## P1 Status

| ID | Task | Status |
|----|------|--------|
| P1.1 | Switch to parking_lot::Mutex | **DONE** (session 004) |
| P1.2 | Lock-free audio ring buffer (rtrb) | **DONE** (session 005) |
| P1.3 | Drop implementations for resource cleanup | **DONE** (this session) |
| P1.4 | Fix thread::sleep blocking GTK main thread | Pending |
| P1.5 | Signal handlers for clean shutdown | Pending |
| P1.6 | Fix silent error swallowing | Pending |
| P1.7 | Fix lock ordering issues | Pending |
| P1.8 | Store JoinHandle for segmentation thread | Pending |
| P1.9 | Add timeout to segmented mode polling | Pending |
| P1.10 | Pre-allocate denoiser and reuse | Pending |
