# Refactoring Session 003: P0.4 SHA256 Model Verification

**Date:** 2026-01-30
**Priority:** P0 (Fix Now — data integrity risk)
**Findings Addressed:** F-752F5717 (HIGH security), F-C69D1AA0 (HIGH security)
**Files Changed:** `src/infrastructure/models.rs`, `Cargo.toml`

---

## P0.4: Add SHA256 Verification for Downloaded Models

**Findings:** F-752F5717, F-C69D1AA0 (HIGH security)
**Risk:** Malicious or corrupted model files loaded into native C/C++ code (whisper.cpp, ONNX Runtime) without any integrity verification

### Problem

All three download functions (`download_model()`, `download_sortformer_model()`, `download_tdt_model()`) accepted downloaded files without any integrity verification. A corrupted download, MITM attack, or compromised CDN could deliver a malicious model file that gets loaded directly into native code (whisper.cpp or ONNX Runtime), potentially leading to arbitrary code execution.

### Fix

**1. Added `sha2` dependency** to `Cargo.toml`:
```toml
sha2 = "0.10"
```

**2. Added `sha256` field to `ModelInfo`:**
```rust
pub struct ModelInfo {
    pub filename: String,
    pub display_name: String,
    pub size_bytes: u64,
    pub description: String,
    pub sha256: Option<String>,  // NEW: SHA256 hash for integrity verification
}
```

Using `Option<String>` allows gradual adoption — models with known hashes are verified, others emit a warning.

**3. Created `verify_checksum()` function:**
```rust
fn verify_checksum(path: &Path, expected: &str) -> Result<()> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    let hash = format!("{:x}", hasher.finalize());
    if hash != expected {
        bail!("Checksum mismatch for {}: expected {}, got {}", ...);
    }
    Ok(())
}
```

**4. Integrated verification into all download pipelines:**

Each download function now:
1. Closes the file handle (`drop(file)`) before verification
2. Computes SHA256 of the temp file
3. If hash is known and mismatches — deletes the temp file and returns error
4. If hash is unknown (`None`) — logs a warning and proceeds
5. Only renames temp → final after successful verification

**5. Added SHA256 hashes for all 11 Whisper models** sourced from the HuggingFace LFS API (`huggingface.co/api/models/ggerganov/whisper.cpp/tree/main`). Also corrected `size_bytes` to exact values from HuggingFace.

Sortformer and TDT models have `sha256: None` (third-party, hashes TBD).

### Models with SHA256 Hashes

| Model | SHA256 (first 16 chars) |
|-------|------------------------|
| ggml-tiny-q5_1.bin | `818710568da3ca15...` |
| ggml-base-q5_1.bin | `422f1ae452ade6f3...` |
| ggml-base-q8_0.bin | `c577b9a86e7e048a...` |
| ggml-small-q5_1.bin | `ae85e4a935d7a567...` |
| ggml-small-q8_0.bin | `49c8fb02b65e6049...` |
| ggml-medium-q5_0.bin | `19fea4b380c3a618...` |
| ggml-tiny.bin | `be07e048e1e599ad...` |
| ggml-base.bin | `60ed5bc3dd14eea8...` |
| ggml-small.bin | `1be3a9b2063867b9...` |
| ggml-medium.bin | `6c14d5adee5f8639...` |
| ggml-large-v3.bin | `64d182b440b98d52...` |

### Tests Added

4 new tests:
- `test_all_whisper_models_have_sha256` — all Whisper models have valid 64-char hex SHA256
- `test_verify_checksum_valid` — correct hash for known content passes
- `test_verify_checksum_mismatch` — wrong hash returns error with descriptive message
- `test_verify_checksum_file_not_found` — missing file returns error

---

## Verification

```
cargo clippy  — 0 new warnings (12 pre-existing dead_code warnings unchanged)
cargo test    — 160/160 passed (4 new tests added)
```

## P0 Status: COMPLETE

| ID | Task | Status |
|----|------|--------|
| P0.1 | Fix ABBA deadlock in denoise.rs | **DONE** (session 001) |
| P0.2 | Fix loopback panic on odd bytes | **DONE** (session 001) |
| P0.3 | Handle CPAL stream errors (microphone.rs) | **DONE** (session 002) |
| P0.4 | Add SHA256 model verification | **DONE** (this session) |
| P0.5 | Fix segmentation buffer clone | **DONE** (session 002) |

All P0 items are now resolved. Next priority tier is **P1** (parking_lot migration, lock-free audio buffer, Drop impls, etc.).
