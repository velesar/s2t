# Refactoring Session 012: P1.9 Polling Timeout + P1.10 Denoiser Caching

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — reliability + performance)
**Findings Addressed:**
- F-4178C565 (MEDIUM reliability) — No timeout on segmented mode polling
- F-28A76F6A (MEDIUM performance) — Denoiser re-created on every call
- F-C782CACD (MEDIUM performance) — FFT resamplers re-created on every call
- F-C5DD03E6 (MEDIUM performance) — Frame buffers heap-allocated per frame

**Files Changed:** `src/ui/mic.rs`, `src/recording/denoise.rs`

---

## P1.9: Add Timeout to Segmented Mode Polling

### Problem

In `handle_segmented_stop()`, after stopping the recording, a polling loop waits for all transcription threads to complete by checking `SEGMENTS_COMPLETED >= SEGMENTS_SENT` every 100ms. There was no upper bound — if a transcription thread hung (model corruption, OOM, deadlocked resource), the polling loop would run indefinitely.

While a manual "Cancel" button was available, the user had to actively notice the stall and click it. There was no automatic safeguard.

**Before:**
```rust
loop {
    let sent = SEGMENTS_SENT.with(|c| c.get());
    let completed = SEGMENTS_COMPLETED.with(|c| c.get());

    if completed >= sent && sent > 0 {
        break;
    }

    if PROCESSING_CANCELLED.with(|c| c.get()) {
        was_cancelled = true;
        break;
    }

    glib::timeout_future(poll_interval).await;
}
```

### Fix

Added a 5-minute safety timeout using `std::time::Instant`:

```rust
let poll_timeout = std::time::Duration::from_secs(5 * 60);
let poll_start = std::time::Instant::now();
let mut was_timed_out = false;

loop {
    // ... existing completion and cancel checks ...

    if poll_start.elapsed() >= poll_timeout {
        was_timed_out = true;
        eprintln!(
            "Segment processing timed out after {:?}: {}/{} completed",
            poll_timeout, completed, sent
        );
        break;
    }

    glib::timeout_future(poll_interval).await;
}
```

The timeout result is displayed to the user with a distinct status message ("Тайм-аут обробки") and the partially accumulated text is still saved to history — the user doesn't lose work that was completed before the timeout.

### Why 5 minutes?

- Whisper large model can take ~30 seconds per 30-second segment on CPU
- A typical recording session might produce 10-20 segments
- 5 minutes provides ample margin for normal use while catching genuine hangs
- The manual cancel button remains available for faster intervention

---

## P1.10: Pre-allocate Denoiser Resamplers and Frame Buffers

### Problem

The `NnnoiselessDenoiser` had three performance issues in its hot path:

1. **FFT resamplers re-created every call:** `upsample()` and `downsample()` were static methods that constructed new `FftFixedIn` instances on every `denoise_buffer()` invocation. FFT resampler initialization includes computing FFT kernels — a one-time cost that was paid repeatedly.

2. **Frame buffers heap-allocated per frame:** Inside the RNNoise processing loop, `drain(..FRAME_SIZE).collect()` allocated a new `Vec<f32>` for `frame_in`, and `vec![0.0f32; FRAME_SIZE]` allocated a new `Vec<f32>` for `frame_out`, on every 480-sample frame. For a 30-second audio at 48kHz, this is ~3,000 allocations.

3. **No reuse across calls:** Each `denoise_buffer()` call started from scratch with fresh resamplers and buffers.

### Fix

**1. Cached resamplers in `DenoiseInner`:**

Added `upsampler: Option<FftFixedIn<f32>>` and `downsampler: Option<FftFixedIn<f32>>` as fields of `DenoiseInner`. Resamplers are lazily initialized on first use via `ensure_resamplers()` and reused across all subsequent calls.

```rust
struct DenoiseInner {
    state: Box<DenoiseState<'static>>,
    buffer: Vec<f32>,
    frame_in: Vec<f32>,      // NEW: pre-allocated
    frame_out: Vec<f32>,     // NEW: pre-allocated
    upsampler: Option<FftFixedIn<f32>>,    // NEW: cached
    downsampler: Option<FftFixedIn<f32>>,  // NEW: cached
}
```

The static `upsample()`/`downsample()` methods were replaced with `resample_up()`/`resample_down()` instance methods on `DenoiseInner`.

**2. Pre-allocated frame buffers:**

`frame_in` and `frame_out` are allocated once at construction (`vec![0.0f32; FRAME_SIZE]`) and reused via `copy_from_slice` in the RNNoise processing loop:

```rust
fn process_rnnoise(&mut self, upsampled: &[f32]) -> Vec<f32> {
    let DenoiseInner { ref mut state, ref mut buffer, ref mut frame_in, ref mut frame_out, .. } = *self;
    // ...
    while buffer.len() >= FRAME_SIZE {
        frame_in.copy_from_slice(&buffer[..FRAME_SIZE]);
        buffer.drain(..FRAME_SIZE);
        state.process_frame(frame_out, frame_in);
        denoised.extend_from_slice(frame_out);
    }
}
```

The destructuring pattern (`let DenoiseInner { ... } = *self`) is necessary to satisfy the borrow checker: `process_frame` needs `&mut state`, `&mut frame_out`, and `&frame_in` simultaneously, which requires split borrows on the struct fields.

**3. Updated `reset()`:**

Reset now clears cached resamplers (setting them to `None` for lazy re-creation) and fills frame buffers with zeros, in addition to the existing `DenoiseState` and buffer reset.

### Design decisions

**Why lazy initialization instead of eager?** `FftFixedIn::new()` can fail (returns `Result`). By making resamplers `Option`, the constructor `NnnoiselessDenoiser::new()` remains infallible, and errors are deferred to the first `denoise_buffer()` call where they can be properly propagated.

**Why not keep resamplers outside the mutex?** The original design had `upsample`/`downsample` as lock-free static methods called outside the mutex. Moving them inside the mutex increases lock hold time. However:
- `denoise_buffer()` is a batch operation on the full audio buffer, not a streaming per-sample call
- The resampling time is proportional to audio length, same as RNNoise processing
- Having a single mutex simplifies the design and eliminates any risk of lock ordering issues
- In practice, denoising is called once per recording stop or once per segment — not a hot path for concurrency

**Why drop resamplers on `reset()`?** `FftFixedIn` maintains internal overlap buffers that may contain audio data from the previous session. Dropping and re-creating ensures clean state. The cost of one FFT kernel computation per session is negligible.

---

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 165/165 passed (0 regressions)
```

## Findings Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-4178C565 (No timeout on segmented mode polling loop) | MEDIUM | Fixed |
| F-28A76F6A (Denoiser re-created on every call) | MEDIUM | Fixed |
| F-C782CACD (FFT resamplers re-created on every call) | MEDIUM | Fixed |
| F-C5DD03E6 (Frame buffers heap-allocated per frame in RNNoise loop) | MEDIUM | Fixed |

## Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 10 | 0 |
| P2 | 10 | 0 | 10 |
| P3 | 10 | 0 | 10 |
