# Compilation Warnings - Remaining Issues

**Date:** 2026-01-28
**Total warnings:** 6

---

## Summary

| # | Warning | File | Line | Category |
|---|---------|------|------|----------|
| 1 | fields `start_time`, `end_time` never read | continuous.rs | 15 | Future use |
| 2 | field `model_path` never read | diarization.rs | 24 | Conditional compilation |
| 3 | method `model_path` never used | diarization.rs | 106 | Conditional compilation |
| 4 | method `search` never used | history.rs | 92 | UI not implemented |
| 5 | function `get_sortformer_model_info` never used | models.rs | 198 | UI not implemented |
| 6 | function `download_sortformer_model` never used | models.rs | 215 | UI not implemented |

---

## Detailed Analysis

### 1. Dead code: `start_time`, `end_time` (continuous.rs:15)

```rust
pub struct AudioSegment {
    pub samples: Vec<f32>,
    pub start_time: Instant,  // never read
    pub end_time: Instant,    // never read
    pub segment_id: usize,
}
```

**Root Cause:** Поля додані для майбутнього використання - відображення часу сегменту в UI або синхронізація з відео.

**Рішення:** Залишити з `#[allow(dead_code)]` або видалити якщо не планується використовувати.

---

### 2-3. Dead code: `model_path` field/method (diarization.rs:24, 106)

```rust
pub struct DiarizationEngine {
    #[cfg(feature = "sortformer")]
    sortformer: Option<Sortformer>,
    model_path: Option<PathBuf>,  // never read without sortformer
}

pub fn model_path(&self) -> Option<&PathBuf> { ... }  // never called
```

**Root Cause:** Поле та getter використовуються тільки з `--features sortformer`. Без feature - dead code.

**Рішення:** Додати `#[cfg(feature = "sortformer")]` до поля та методу.

---

### 4. Dead code: `search` method (history.rs:92)

```rust
pub fn search(&self, query: &str) -> Vec<&HistoryEntry> { ... }
```

**Root Cause:** Backend-функціонал без UI. History dialog не має поля пошуку.

**Рішення:** Додати search box в history_dialog.rs або видалити метод.

---

### 5-6. Dead code: Sortformer functions (models.rs:198, 215)

```rust
pub fn get_sortformer_model_info() -> ModelInfo { ... }
pub async fn download_sortformer_model<F>(...) -> Result<()> { ... }
```

**Root Cause:** Функції для завантаження Sortformer моделі, але UI (model_dialog.rs) показує тільки Whisper моделі.

**Рішення:** Інтегрувати Sortformer в model_dialog.rs або видалити до реалізації.

---

## Recommended Actions

### Keep with `#[allow(dead_code)]`
- #1 `start_time/end_time` - для майбутнього використання

### Add conditional compilation
- #2, #3 `model_path` - додати `#[cfg(feature = "sortformer")]`

### Requires UI work or removal
- #4 `search` - або UI, або видалити
- #5, #6 Sortformer functions - або UI, або видалити

---

## Fixed Warnings (for reference)

| Original # | Warning | Fix |
|------------|---------|-----|
| 1 | unused import `Context` | Conditional import |
| 2 | unreachable expression | Split function by #[cfg] |
| 3 | unused `audio_samples` | Underscore prefix |
| 4 | unused `SAMPLE_RATE` | Conditional constant |
| 6 | unused `is_recording` clone | Removed |
| 8 | unused `add_samples()` | Removed |
| 9 | unused `text` field | Removed |
| 12 | unused `WHISPER_SAMPLE_RATE` | Used in parec args |
| 15 | unused `len/is_empty` | Removed |
| 16 | unused `min_speech_duration_ms` | Removed |
| 17 | unused VAD methods | Removed |
