# Refactoring Session 005: P1.2 Lock-Free Audio Ring Buffer

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — performance + reliability)
**Findings Addressed:** F-B1A4B2AE (HIGH performance), F-03F55E2A (HIGH performance)
**Files Changed:** `Cargo.toml`, `src/recording/microphone.rs`

---

## P1.2: Replace Mutex-Based Audio Buffer with Lock-Free SPSC Ring Buffer

**Findings:** F-B1A4B2AE (HIGH performance — mutex lock in real-time audio callback), F-03F55E2A (HIGH performance — heap allocations in real-time audio callback)
**Risk:** Audio glitches (dropouts, xruns) due to blocking and allocating in CPAL's real-time audio callback

### Problem

The CPAL audio callback in `microphone.rs` violated fundamental real-time audio programming rules:

1. **Mutex lock in callback**: `resampler.lock()` — acquires `parking_lot::Mutex` on every callback invocation
2. **Mutex lock for sample storage**: `samples.lock().extend(...)` — acquires another mutex to store resampled data
3. **Heap allocations**: `to_mono(data, channels)` allocates a new `Vec<f32>` every callback
4. **Heap allocations in resampler**: `vec![chunk.to_vec()]` and `padded.resize()` allocate on each chunk

In a real-time audio callback, any operation that may block (mutex) or trigger the allocator (Vec::new, Vec::push beyond capacity) can cause audio dropouts. The OS audio thread has a hard deadline — typically 5–20ms depending on buffer size — and any delay causes audible glitches.

### Architecture: Before

```
CPAL callback (real-time thread)
  ├── to_mono(data) → Vec<f32> allocation
  ├── resampler.lock() → MUTEX LOCK
  ├── vec![chunk.to_vec()] → allocation per chunk
  ├── resampler.process() → resampling
  └── samples.lock().extend() → MUTEX LOCK + potential reallocation
```

### Architecture: After

```
CPAL callback (real-time thread)          Consumer thread (non-RT)
  ├── to_mono_into(pre-allocated buf)     ├── consumer.read_chunk()
  ├── calculate_rms() (atomic store)      ├── resampler.process()
  └── producer.write_chunk() (lock-free)  └── samples.lock().extend()
         │                                       ▲
         └── rtrb SPSC ring buffer ──────────────┘
```

### Fix

**1. Added `rtrb` dependency** (real-time ring buffer) to `Cargo.toml`:
```toml
rtrb = "0.3"
```

**2. Created lock-free SPSC pipeline:**

The CPAL callback now only performs lock-free, allocation-free operations:

```rust
// CPAL callback — real-time safe
move |data: &[f32], _: &cpal::InputCallbackInfo| {
    // Zero-allocation mono conversion into pre-allocated buffer
    let mono_len = to_mono_into(data, channels, &mut mono_buf);
    let mono = &mono_buf[..mono_len];

    // Atomic amplitude update (lock-free)
    let amplitude = calculate_rms(mono);
    current_amplitude.store(amplitude.to_bits(), Ordering::Relaxed);

    // Lock-free write to SPSC ring buffer
    if let Ok(mut write_chunk) = producer.write_chunk(mono_len) {
        let (first, second) = write_chunk.as_mut_slices();
        first.copy_from_slice(&mono[..first.len()]);
        if !second.is_empty() {
            second.copy_from_slice(&mono[first.len()..]);
        }
        write_chunk.commit_all();
    }
    // If buffer full, samples are dropped (preferable to blocking)
}
```

A separate consumer thread handles all non-RT work:

```rust
// Consumer thread — reads ring buffer, resamples, stores
while is_recording.load(Ordering::SeqCst) {
    if let Ok(chunk) = consumer.read_chunk(available) {
        // Accumulate into resampler-sized chunks
        // Process through sinc resampler
        // Store into Arc<Mutex<Vec<f32>>> (safe — not in RT thread)
    }
}
```

**3. Added `to_mono_into()` — zero-allocation mono conversion:**

```rust
fn to_mono_into(data: &[f32], channels: usize, mono_buf: &mut [f32]) -> usize
```

Replaces `to_mono()` which allocated a new `Vec<f32>` on every call. The new function writes directly into a pre-allocated buffer owned by the callback closure (allocated once at recording start).

**4. Proper shutdown sequencing:**

```rust
// 1. Stop the CPAL stream (no more callbacks)
drop(stream);
// 2. Wait for consumer to drain remaining samples
consumer_handle.join();
// 3. Signal completion
completion_tx.send_blocking(());
```

This ensures all buffered samples are processed before the completion signal is sent, preventing data loss at recording end.

### Downstream Compatibility

The `Arc<Mutex<Vec<f32>>>` samples buffer interface is unchanged. All downstream consumers (segmentation monitor, `stop()` method, conference recorder) continue to work without modification. The change is fully internal to the audio callback path.

### Performance Impact

| Property | Before | After |
|----------|--------|-------|
| Locks in RT callback | 2 (resampler + samples) | 0 |
| Heap allocations in RT callback | 3+ per callback | 0 |
| Mono conversion | `Vec<f32>` alloc each time | Pre-allocated buffer (once) |
| Resampler ownership | `Arc<Mutex<SincFixedIn>>` | Owned by consumer thread |
| Ring buffer | N/A | 10s SPSC lock-free (rtrb) |
| Backpressure | Blocks on mutex | Drops samples (no glitch propagation) |
| Shutdown | Stream dropped, samples may be lost | Stream dropped → consumer drains → complete |

### Tests Added

3 new unit tests for `to_mono_into`:
- `test_to_mono_into_single_channel` — passthrough for mono input
- `test_to_mono_into_stereo` — correct averaging for stereo
- `test_to_mono_into_larger_buffer` — works with oversized output buffer

---

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 163/163 passed (3 new tests, 0 regressions)
```

## P1 Status

| ID | Task | Status |
|----|------|--------|
| P1.1 | Switch to parking_lot::Mutex | **DONE** (session 004) |
| P1.2 | Lock-free audio ring buffer (rtrb) | **DONE** (this session) |
| P1.3 | Drop implementations for resource cleanup | Pending |
| P1.4 | Fix thread::sleep blocking GTK main thread | Pending |
| P1.5 | Signal handlers for clean shutdown | Pending |
| P1.6 | Fix silent error swallowing | Pending |
| P1.7 | Fix lock ordering issues | Pending |
| P1.8 | Store JoinHandle for segmentation thread | Pending |
| P1.9 | Add timeout to segmented mode polling | Pending |
| P1.10 | Pre-allocate denoiser and reuse | Pending |
