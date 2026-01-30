# Refactoring Session 002: P0.3 CPAL Stream Errors + P0.5 Segmentation Buffer Clone

**Date:** 2026-01-30
**Priority:** P0 (Fix Now — crash/resource risk)
**Findings Addressed:** F-C9E210CD (HIGH), F-F00B80FC (HIGH), F-BC68140E (HIGH)
**Files Changed:** `src/recording/microphone.rs`, `src/recording/segmentation.rs`

---

## P0.3: Handle CPAL Stream Errors in microphone.rs

**Finding:** F-C9E210CD (HIGH reliability)
**Risk:** Silent thread panic if audio device disappears or is busy; UI hangs forever waiting for completion signal

### Problem

In `microphone.rs`, the recording thread called `.unwrap()` on both `build_input_stream()` and `stream.play()`. If the audio device is unavailable, busy, or disappears mid-session, these calls panic inside a spawned thread. The main thread never receives the completion signal, causing the UI to hang indefinitely.

```rust
// BEFORE: panics on device error, UI hangs forever
let stream = device.build_input_stream(/* ... */).unwrap();
stream.play().unwrap();
```

### Fix

Replaced `.unwrap()` with `match`/`if let` that logs the error and sends the completion signal before returning:

```rust
// AFTER: graceful error handling, UI receives completion signal
let stream = match device.build_input_stream(/* ... */) {
    Ok(s) => s,
    Err(e) => {
        eprintln!("Не вдалося створити аудіопотік: {}", e);
        let _ = completion_tx.send_blocking(());
        return;
    }
};

if let Err(e) = stream.play() {
    eprintln!("Не вдалося запустити аудіопотік: {}", e);
    let _ = completion_tx.send_blocking(());
    return;
}
```

This ensures:
1. No panic in the recording thread
2. The completion channel always receives a signal (UI doesn't hang)
3. Error is logged for diagnostics

---

## P0.5: Fix Segmentation Buffer Clone

**Findings:** F-F00B80FC (HIGH reliability), F-BC68140E (HIGH performance)
**Risk:** OOM on long recordings, severe CPU waste from full buffer cloning

### Problem

The segmentation monitor's background thread cloned the entire shared samples buffer every 500ms:

```rust
// BEFORE: clones entire buffer every 500ms — O(n) growing allocation
let current_samples = {
    let samples = samples_buffer.lock().unwrap();
    samples.clone()  // clones ALL accumulated samples
};

let new_samples_count = current_samples.len().saturating_sub(last_samples_len);
if new_samples_count > 0 {
    let new_samples = &current_samples[last_samples_len..];
    ring_buffer.write(new_samples);
    last_samples_len = current_samples.len();
}
```

For a 10-minute recording at 16kHz, this copies 9.6M samples (38MB) every 500ms — pure waste since only the new samples since the last check are needed.

The same pattern existed in the `stop()` method where the full buffer was cloned even when not needed.

### Fix

**Monitoring loop:** Read only new samples while holding the lock, no allocation:

```rust
// AFTER: reads only new samples, zero allocation, minimal lock hold time
{
    let samples = samples_buffer.lock().unwrap();
    let current_len = samples.len();
    if current_len > last_samples_len {
        ring_buffer.write(&samples[last_samples_len..]);
        last_samples_len = current_len;
    }
}
```

**Stop method:** Only clone when the ring buffer fallback is actually needed:

```rust
// AFTER: only clones if ring buffer is empty (rare fallback path)
let mut remaining = ring_remaining;
if remaining.is_empty() {
    let recorder_samples = samples_buffer.lock().unwrap();
    if !recorder_samples.is_empty() {
        let last_segment_samples = (WHISPER_SAMPLE_RATE as usize) * 5;
        let start = recorder_samples.len().saturating_sub(last_segment_samples);
        remaining = recorder_samples[start..].to_vec();
    }
}
```

### Performance Impact

| Metric | Before | After |
|--------|--------|-------|
| Memory per 500ms tick (10min recording) | ~38MB clone | 0 bytes (slice reference) |
| Lock hold time | Clone duration (~ms) | Slice write (~μs) |
| OOM risk on long recordings | Yes (quadratic growth) | No |

---

## Verification

```
cargo clippy  — 0 new warnings (12 pre-existing dead_code warnings unchanged)
cargo test    — 156/156 passed (no regressions)
```

## Remaining P0 Items

| ID | Task | Status |
|----|------|--------|
| P0.1 | Fix ABBA deadlock in denoise.rs | **DONE** (session 001) |
| P0.2 | Fix loopback panic on odd bytes | **DONE** (session 001) |
| P0.3 | Handle CPAL stream errors (microphone.rs) | **DONE** (this session) |
| P0.4 | Add SHA256 model verification | Pending |
| P0.5 | Fix segmentation buffer clone | **DONE** (this session) |
