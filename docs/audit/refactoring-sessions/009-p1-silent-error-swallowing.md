# Refactoring Session 009: P1.6 Fix Silent Error Swallowing

**Date:** 2026-01-30
**Priority:** P1 (Next Sprint — reliability)
**Findings Addressed:** F-C485F5A8 (MEDIUM reliability), F-EAEDFBCB (MEDIUM reliability)
**Files Changed:** `src/ui/mic.rs`, `src/recording/segmentation.rs`

---

## P1.6: Fix Silent Error Swallowing

### Problem

Two sites in the codebase silently discarded errors, hiding failures from both the user and logs:

**1. Segmented transcription (`src/ui/mic.rs:211`):**

```rust
let result = ts.transcribe(&segment_samples, &lang);
let text = result.unwrap_or_default();  // Error silently becomes ""
```

If `transcribe()` fails (model corruption, OOM, systematic backend error), every segment returns an empty string. The only hint is a Ukrainian stderr message about "empty result" — which is also triggered by genuinely silent audio. The user sees successful completion with no text, with no indication that the model is broken.

**2. VAD creation (`src/recording/segmentation.rs:105`):**

```rust
create_vad(&config).ok()  // Error silently becomes None
```

If VAD initialization fails (e.g., Silero ONNX model not found, runtime error), the segmentation silently falls back to fixed-interval mode. The user configured VAD but it's not active — completely invisible.

### Fix 1: Propagate transcription errors through the result channel

Changed the segment transcription pipeline to pass `Result<String, String>` instead of plain `String`:

```rust
// Channel type changed from (usize, String) to (usize, Result<String, String>)
let (result_tx, result_rx) =
    async_channel::unbounded::<(usize, Result<String, String>)>();
```

The worker thread now sends the full Result:

```rust
std::thread::spawn(move || {
    let result = ts
        .transcribe(&segment_samples, &lang)
        .map_err(|e| e.to_string());
    let _ = tx.send_blocking((segment_id, result));
});
```

The result processor handles errors explicitly:

- **Error segments** get a visual "✗" indicator with `segment-error` CSS class
- **Failed segment count** is tracked and shown in the status bar: `"Транскрибовано: 5 сегментів (2 з помилками)"`
- **Errors are logged** to stderr with the segment ID and error message
- **Successful but empty segments** are silently skipped (as before — this is normal for silent audio)

### Fix 2: Log VAD initialization failure

Replaced `.ok()` with explicit `match`:

```rust
match create_vad(&config) {
    Ok(v) => Some(v),
    Err(e) => {
        eprintln!(
            "VAD initialization failed ({:?}), falling back to fixed-interval segmentation: {}",
            vad_engine, e
        );
        None
    }
}
```

The fallback to fixed-interval segmentation is still the correct behavior (the recording should continue), but now the failure is logged with the engine type and error message. This makes debugging VAD configuration issues possible.

### Why not bubble the VAD error to the UI?

The VAD creation happens inside a spawned thread (`std::thread::spawn`), which has no access to GTK widgets. Sending a notification to the UI would require adding a new async channel just for VAD status. The eprintln approach is proportionate:

- VAD failure is not critical — the recording continues with fixed intervals
- The user gets a degraded but functional experience
- The log message enables debugging when users report "VAD doesn't seem to work"
- A UI notification could be added later as a P3 improvement

### Changes

**`src/ui/mic.rs`:**
- Changed result channel type from `(usize, String)` to `(usize, Result<String, String>)`
- Worker thread sends full `Result` instead of `unwrap_or_default()`
- Result processor tracks `failed_count`, shows error count in status
- Failed segments get "✗" visual indicator with `segment-error` CSS class

**`src/recording/segmentation.rs`:**
- Replaced `create_vad(&config).ok()` with explicit `match` that logs the error

---

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 165/165 passed (0 regressions)
```

## Findings Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-C485F5A8 (Transcription error silently discarded in segmented mode) | MEDIUM | Fixed |
| F-EAEDFBCB (VAD creation failure silently ignored) | MEDIUM | Fixed |

## Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 6 (P1.1-P1.6) | 4 (P1.7-P1.10) |
| P2 | 10 | 0 | 10 |
| P3 | 10 | 0 | 10 |
