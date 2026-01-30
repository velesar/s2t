# Refactoring Session 013: P2.1 Extract shared download_file()

**Date:** 2026-01-30
**Priority:** P2 (Technical Debt — maintainability)
**Findings Addressed:** F-0FAD129D, F-2AFBAE9C (HIGH maintainability)
**Files Changed:** `src/infrastructure/models.rs`

---

## P2.1: Extract shared download_file() in models.rs

### Problem

Three public download functions contained nearly identical HTTP download logic:

1. **`download_model()`** (Whisper models) — single file download with progress
2. **`download_sortformer_model()`** (Sortformer ONNX) — single file download with progress
3. **`download_tdt_model()`** (TDT encoder + decoder + vocab) — multi-file download with cumulative progress

Each function independently implemented the same ~40-line pattern:
- Create `reqwest::Client` and send GET request
- Check HTTP status
- Stream response chunks to a `.downloading` temp file
- Report progress via callback
- Verify SHA256 checksum (if available)
- Atomic rename from temp to final path

This triplication meant any bug fix or behavior change (e.g., adding retry logic, timeout, or User-Agent header) would need to be applied in three places.

### Fix

Extracted a private `download_file()` function that encapsulates the entire download-verify-rename pipeline:

```rust
async fn download_file(
    url: &str,
    dir: &Path,
    filename: &str,
    expected_sha256: Option<&str>,
    downloaded_offset: u64,
    total_size_override: u64,
    progress_callback: &(dyn Fn(u64, u64) + Send + Sync),
) -> Result<PathBuf>
```

**Parameters:**
- `url` — full download URL
- `dir` — target directory (created if missing)
- `filename` — final filename within dir
- `expected_sha256` — optional checksum for verification
- `downloaded_offset` — starting byte count for progress (supports multi-file cumulative progress)
- `total_size_override` — total size for progress denominator (0 = use Content-Length)
- `progress_callback` — reports `(downloaded_bytes, total_bytes)`

**Returns:** `PathBuf` to the final downloaded file.

### How each caller was simplified

**`download_model()`** — Looks up the model's SHA256, constructs URL, delegates to `download_file()` with offset=0, override=0.

**`download_sortformer_model()`** — Reduced from ~45 lines to ~15. Gets model info, constructs URL, delegates.

**`download_tdt_model()`** — Reduced from ~50 lines to ~20. Loops over 3 model files, passes cumulative `total_downloaded` as offset and `get_tdt_total_size()` as total override, reads actual file size after each download to update the offset.

### Design decisions

**Why `&dyn Fn` instead of generic `F`?** The shared function uses a trait object reference (`&dyn Fn(u64, u64)`) rather than a generic parameter. This avoids monomorphization of the download logic for each call site — there's no benefit to specializing HTTP I/O code per callback type. The public wrappers remain generic for caller ergonomics.

**Why `downloaded_offset` and `total_size_override`?** TDT downloads 3 files sequentially but reports cumulative progress to the UI as a single operation. The offset lets the callback produce monotonically increasing byte counts across files, and the total override ensures the denominator reflects the combined size rather than per-file Content-Length.

**Why read `fs::metadata` for actual file size?** After `download_file()` returns, `download_tdt_model()` reads the actual downloaded file size via `fs::metadata()` rather than trusting the `model_file.size_bytes` estimate. This ensures the cumulative offset is accurate even if the catalog size is slightly wrong.

**Trait bound change: `Send + 'static` → `Send + Sync + 'static`:** The `&dyn Fn` reference inside `download_file()` requires `Sync` on the callback (it's shared by reference across an async boundary). All existing callers pass closures capturing `async_channel::Sender<T>`, which is `Sync`, so this is backwards-compatible. The `Clone` bound on `download_tdt_model`'s `F` was also removed — it was only needed when the callback was moved into each loop iteration; now the shared function borrows it.

---

## Metrics

| Metric | Before | After |
|--------|--------|-------|
| File lines (models.rs) | 669 | 622 |
| Net lines changed | — | -46 (86 added, 132 removed) |
| Download code instances | 3 | 1 (shared) + 3 (thin wrappers) |
| Duplicated download lines | ~120 | 0 |

## Verification

```
cargo clippy  — 0 new warnings (13 pre-existing dead_code warnings unchanged)
cargo test    — 165/165 passed (0 regressions)
```

## Findings Addressed

| Finding | Severity | Status |
|---------|----------|--------|
| F-0FAD129D (Triplicated download logic in models.rs) | HIGH | Fixed |
| F-2AFBAE9C (Code duplication across model download functions) | HIGH | Fixed |

## Cumulative Progress

| Priority | Total | Completed | Remaining |
|----------|-------|-----------|-----------|
| P0 | 5 | 5 | 0 |
| P1 | 10 | 10 | 0 |
| P2 | 10 | 1 (P2.1) | 9 |
| P3 | 10 | 0 | 10 |
