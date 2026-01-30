# Refactoring Session 011: P1.8 Store JoinHandle for Segmentation Thread

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — reliability)
**Finding Addressed:** F-CD32DE8A (MEDIUM reliability)
**Files Changed:** `src/recording/segmentation.rs`

---

## P1.8: Store JoinHandle for Segmentation Thread

### Problem

In `SegmentationMonitor::start()`, `std::thread::spawn()` was called without storing the returned `JoinHandle`. In `stop()`, a `thread::sleep(Duration::from_millis(100))` was used as a best-effort wait for the monitoring thread to finish its current iteration.

**Before:**
```rust
pub fn start(&self, samples_buffer: Arc<Mutex<Vec<f32>>>, segment_tx: Sender<AudioSegment>) {
    // ...
    std::thread::spawn(move || {
        // monitoring loop with 500ms check intervals
        while is_running.load(Ordering::SeqCst) {
            std::thread::sleep(check_interval);  // 500ms
            // ... process samples, emit segments ...
        }
    });
    // JoinHandle dropped — no way to wait for thread completion
}

pub fn stop(&self, samples_buffer: &Arc<Mutex<Vec<f32>>>) {
    self.is_running.store(false, Ordering::SeqCst);

    // Give the monitoring thread a moment to finish its current iteration
    std::thread::sleep(Duration::from_millis(100));  // RACE CONDITION

    // Drain remaining audio from ring buffer...
}
```

**The 100ms sleep is a race condition:**

The monitoring thread's main loop sleeps for 500ms between iterations. After setting `is_running` to false, the thread may still be in the middle of a 500ms sleep or processing samples. A 100ms wait is insufficient — if the thread is at the beginning of its sleep, it won't see the flag for up to ~500ms. This means `stop()` may begin draining the ring buffer while the monitoring thread is still writing to it, potentially losing the last batch of samples.

### Fix

1. Added a `thread_handle: Mutex<Option<JoinHandle<()>>>` field to `SegmentationMonitor`
2. In `start()`, store the `JoinHandle` returned by `thread::spawn`
3. In `stop()`, replace `thread::sleep(100ms)` with `handle.join()`

**After:**
```rust
pub struct SegmentationMonitor {
    // ... existing fields ...
    thread_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

pub fn start(&self, ...) {
    // ...
    let handle = std::thread::spawn(move || {
        // ... same monitoring loop ...
    });
    *self.thread_handle.lock() = Some(handle);
}

pub fn stop(&self, samples_buffer: &Arc<Mutex<Vec<f32>>>) {
    self.is_running.store(false, Ordering::SeqCst);

    // Wait for the monitoring thread to finish its current iteration
    if let Some(handle) = self.thread_handle.lock().take() {
        if let Err(e) = handle.join() {
            eprintln!("Segmentation thread panicked: {:?}", e);
        }
    }
    // ... drain remaining audio ...
}
```

### Why this is correct

- **Deterministic synchronization:** `join()` blocks until the thread exits, guaranteeing the monitoring loop has fully stopped before we drain the ring buffer.
- **No data race:** After `join()` returns, the ring buffer is no longer being written to by the monitoring thread, so `read_all()` is safe.
- **Panic propagation:** If the monitoring thread panicked, `join()` returns `Err` with the panic payload. We log it and continue with cleanup (draining the buffer, sending the final segment, closing the channel).
- **Worst-case latency:** The monitoring thread sleeps for 500ms between iterations. After `is_running` is set to false, the thread will see the flag on its next wakeup and exit. Maximum wait time is ~500ms + processing time, which is acceptable for a stop operation.
- **`Mutex<Option<JoinHandle>>` via `take()`:** Using `.take()` moves the `JoinHandle` out of the `Option`, consuming it. This ensures `join()` can only be called once per start/stop cycle. The `Mutex` is `parking_lot::Mutex` (no poisoning), consistent with the rest of the codebase.

---

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 165/165 passed (0 regressions)
```

## Finding Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-CD32DE8A (Segmentation thread JoinHandle not stored, sleep-based synchronization) | MEDIUM | Fixed |

## Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 8 (P1.1-P1.8) | 2 (P1.9-P1.10) |
| P2 | 10 | 0 | 10 |
| P3 | 10 | 0 | 10 |
