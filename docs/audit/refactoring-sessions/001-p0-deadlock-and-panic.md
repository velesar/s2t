# Refactoring Session 001: P0.1 ABBA Deadlock + P0.2 Loopback Panic

**Date:** 2026-01-30
**Priority:** P0 (Fix Now — crash/deadlock risk)
**Findings Addressed:** F-565811D6 (HIGH), F-AE877B80 (HIGH)
**Files Changed:** `src/recording/denoise.rs`, `src/recording/loopback.rs`

---

## P0.1: Fix ABBA Deadlock in denoise.rs

**Finding:** F-565811D6 (HIGH reliability)
**Risk:** Deadlock when `denoise_buffer()` and `reset()` called concurrently

### Problem

`NnnoiselessDenoiser` had two separate `Mutex` fields (`state` and `buffer`) that were locked in opposite order by different methods:

- `denoise_buffer()`: locked `state` first, then `buffer`
- `reset()`: locked `buffer` first, then `state`

This is a classic ABBA deadlock pattern. If thread A calls `denoise_buffer()` and acquires `state`, while thread B calls `reset()` and acquires `buffer`, both threads will block forever waiting for the other's lock.

### Fix

Consolidated both fields into a single `DenoiseInner` struct protected by one `Mutex`:

```rust
struct DenoiseInner {
    state: Box<DenoiseState<'static>>,
    buffer: Vec<f32>,
}

pub struct NnnoiselessDenoiser {
    inner: Mutex<DenoiseInner>,  // Single lock eliminates ordering issue
}
```

Both `denoise_buffer()` and `reset()` now acquire a single lock, making ABBA deadlock structurally impossible.

Additionally, `denoise_buffer()` performs resampling (upsample/downsample) outside the lock scope, minimizing lock hold time.

### Test Added

`test_concurrent_denoise_and_reset_no_deadlock` — spawns two threads that concurrently call `denoise_buffer()` and `reset()` in a tight loop. Before the fix, this would deadlock; after the fix, it completes reliably.

---

## P0.2: Fix Loopback Panic on Odd Byte Count

**Finding:** F-AE877B80 (HIGH reliability)
**Risk:** Index out of bounds panic if pipe read returns odd number of bytes

### Problem

In `loopback.rs`, raw bytes from `parec` stdout were converted to `i16` samples using `.chunks(2)`. If the pipe read returned an odd number of bytes, the last chunk would have length 1, causing `i16::from_le_bytes([chunk[0], chunk[1]])` to panic with index out of bounds.

```rust
// BEFORE: panics on odd byte count
.chunks(2)
.map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
```

### Fix

Replaced `.chunks(2)` with `.chunks_exact(2)`, which safely skips any trailing incomplete chunk:

```rust
// AFTER: safely skips trailing odd byte
.chunks_exact(2)
.map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
```

### Tests Added

Three tests in the new `loopback::tests` module:
- `test_i16_conversion_even_bytes` — baseline: correct conversion of complete byte pairs
- `test_i16_conversion_odd_bytes_does_not_panic` — regression: odd byte count does not panic
- `test_i16_conversion_single_byte_does_not_panic` — edge case: single byte produces empty result

---

## Verification

```
cargo clippy  — 0 new warnings (12 pre-existing dead_code warnings unchanged)
cargo test    — 156/156 passed (4 new tests added: 1 denoise, 3 loopback)
```

## Remaining P0 Items

| ID | Task | Status |
|----|------|--------|
| P0.1 | Fix ABBA deadlock in denoise.rs | **DONE** |
| P0.2 | Fix loopback panic on odd bytes | **DONE** |
| P0.3 | Handle CPAL stream errors (microphone.rs) | Pending |
| P0.4 | Add SHA256 model verification | Pending |
| P0.5 | Fix segmentation buffer clone | Pending |
