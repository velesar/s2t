# Refactoring Session 007: P1.4 Fix thread::sleep Blocking GTK Main Thread

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — reliability + performance)
**Findings Addressed:** F-447B7279 (MEDIUM reliability), F-383A4B16 (MEDIUM performance)
**Files Changed:** `src/ui/mic.rs`, `src/ui/conference.rs`, `src/infrastructure/paste.rs`

---

## P1.4: Fix thread::sleep Blocking GTK Main Thread

**Findings:** F-447B7279 (MEDIUM reliability — UI freezes during auto-paste), F-383A4B16 (MEDIUM performance — blocking sleep in GTK async context)
**Risk:** GTK main thread blocked for 200ms+ during auto-paste, causing visible UI freeze (unresponsive window, stalled animations)

### Problem

Both `ui/mic.rs` and `ui/conference.rs` called `std::thread::sleep(100ms)` inside `glib::spawn_future_local` async blocks. Since `glib::spawn_future_local` runs on the GTK main thread, this `thread::sleep` blocked the entire GTK event loop:

```rust
// BEFORE: blocks GTK main thread for 200ms+
glib::spawn_future_local(async move {
    // ... transcription result handling ...
    if auto_paste_enabled {
        std::thread::sleep(std::time::Duration::from_millis(100)); // BLOCKS GTK
        crate::infrastructure::paste::paste_from_clipboard()?;     // BLOCKS GTK (another 100ms + xdotool)
    }
});
```

Additionally, `paste_from_clipboard()` itself contained another `thread::sleep(100ms)` plus a synchronous `xdotool` process spawn. Total GTK main thread blocking: ~200ms sleep + xdotool process time.

During this block:
- Window becomes unresponsive to user input
- Animations and progress indicators stall
- GTK event processing is paused

### Fix

**1. Replaced `thread::sleep` with `glib::timeout_future` (non-blocking):**

The 100ms clipboard readiness delay now uses `glib::timeout_future`, which yields to the GTK event loop instead of blocking it:

```rust
// AFTER: non-blocking delay, GTK event loop continues
glib::timeout_future(std::time::Duration::from_millis(100)).await;
```

**2. Moved `paste_from_clipboard()` to a background thread:**

The synchronous xdotool call is now spawned on a background thread with result communicated via async channel:

```rust
// AFTER: paste runs on background thread, UI stays responsive
let (paste_tx, paste_rx) = async_channel::bounded::<Option<String>>(1);
std::thread::spawn(move || {
    let err = crate::infrastructure::paste::paste_from_clipboard()
        .err()
        .map(|e| e.to_string());
    let _ = paste_tx.send_blocking(err);
});
if let Ok(Some(err)) = paste_rx.recv().await {
    eprintln!("...");
    ui.base.set_status(&format!("Готово! (помилка вставки: {})", err));
}
```

**3. Removed redundant sleep from `paste_from_clipboard()`:**

Since both callers now handle the clipboard readiness delay via `glib::timeout_future`, the internal `thread::sleep(100ms)` in `paste.rs` was redundant. Removed it with a comment explaining the caller's responsibility.

### Impact

| Property | Before | After |
|----------|--------|-------|
| GTK main thread block | ~200ms+ (sleep + xdotool) | 0ms (fully non-blocking) |
| Clipboard readiness delay | `thread::sleep` (blocking) | `glib::timeout_future` (async) |
| xdotool execution | On GTK thread | Background thread |
| Error handling | Same pattern | Same pattern (preserved) |

### Files Changed

- **`src/ui/mic.rs`** — replaced `thread::sleep` + blocking paste with `glib::timeout_future` + background thread
- **`src/ui/conference.rs`** — same fix
- **`src/infrastructure/paste.rs`** — removed redundant internal `thread::sleep(100ms)`

---

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 165/165 passed (0 regressions)
```

## P1 Status

| ID | Task | Status |
|----|------|--------|
| P1.1 | Switch to parking_lot::Mutex | **DONE** (session 004) |
| P1.2 | Lock-free audio ring buffer (rtrb) | **DONE** (session 005) |
| P1.3 | Drop implementations for resource cleanup | **DONE** (session 006) |
| P1.4 | Fix thread::sleep blocking GTK main thread | **DONE** (this session) |
| P1.5 | Signal handlers for clean shutdown | Pending |
| P1.6 | Fix silent error swallowing | Pending |
| P1.7 | Fix lock ordering issues | Pending |
| P1.8 | Store JoinHandle for segmentation thread | Pending |
| P1.9 | Add timeout to segmented mode polling | Pending |
| P1.10 | Pre-allocate denoiser and reuse | Pending |
