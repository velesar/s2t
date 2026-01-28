# Compilation Warnings Root Cause Analysis

**Date:** 2026-01-28
**Total warnings:** 17

---

## Warning 1: unused import `Context` (diarization.rs:1)

```rust
use anyhow::{Context, Result};
```

**Root Cause:** Модуль diarization.rs був написаний для роботи з feature `sortformer`. Коли feature вимкнений, більшість коду не компілюється, включаючи виклики `.context()`. Імпорт `Context` залишився безумовним.

**Походження:** Копіювання стандартного патерну імпорту anyhow без врахування умовної компіляції.

**Fix:** Зробити імпорт умовним:
```rust
#[cfg(feature = "sortformer")]
use anyhow::Context;
use anyhow::Result;
```

---

## Warning 2: unreachable expression (diarization.rs:59)

```rust
#[cfg(not(feature = "sortformer"))]
{
    anyhow::bail!("Sortformer не доступний...");
}
Ok(())  // <- unreachable
```

**Root Cause:** Логічна помилка при написанні функції `load_model()`. Автор додав `Ok(())` в кінці функції для задоволення типу повернення `Result<()>`, не врахувавши що `bail!` вже повертає `Err` і виходить з функції.

**Походження:** Механічне додавання `Ok(())` без аналізу control flow.

**Fix:** Видалити `Ok(())` або реструктурувати функцію з окремими `return` для кожної гілки `#[cfg]`.

---

## Warning 3: unused variable `audio_samples` (diarization.rs:76)

```rust
pub fn diarize(&mut self, audio_samples: &[f32]) -> Result<...>
```

**Root Cause:** Параметр використовується тільки в `#[cfg(feature = "sortformer")]` блоці. При компіляції без feature, параметр ніде не використовується.

**Походження:** Та сама проблема що й #1 - код написаний для sortformer feature, без врахування компіляції без нього.

**Fix:** `_audio_samples: &[f32]`

---

## Warning 4: unused constant `SAMPLE_RATE` (diarization.rs:7)

```rust
const SAMPLE_RATE: u32 = 16000;
```

**Root Cause:** Константа використовується тільки в `sortformer.diarize()` виклику (рядок 82). Без feature - не використовується.

**Походження:** Та сама проблема.

**Fix:** `#[cfg(feature = "sortformer")] const SAMPLE_RATE: u32 = 16000;`

---

## Warning 5: dead code field `model_path` (diarization.rs:22)

```rust
pub struct DiarizationEngine {
    #[cfg(feature = "sortformer")]
    sortformer: Option<Sortformer>,
    model_path: Option<PathBuf>,  // завжди присутній
}
```

**Root Cause:** Поле `model_path` зберігається в структурі, але читається тільки в `load_model()` всередині `#[cfg(feature = "sortformer")]` блоку. Без feature поле записується в `new()`, але ніколи не читається.

**Походження:** Архітектурне рішення зберігати шлях для можливого повторного завантаження моделі, але getter `model_path()` теж ніколи не викликається.

**Fix:** Додати `#[cfg(feature = "sortformer")]` до поля, або додати `#[allow(dead_code)]`.

---

## Warning 6: unused variable `is_recording` (loopback.rs:48)

```rust
let is_recording = self.is_recording.clone();
let is_recording_for_loop = self.is_recording.clone();  // цей використовується
```

**Root Cause:** Є два клони `is_recording`: один для потенційного використання поза циклом, інший (`is_recording_for_loop`) для циклу читання. Перший ніколи не використовується.

**Походження:** Copy-paste з audio.rs де є схожий патерн, але там обидва клони використовуються. В loopback.rs другий clone був перейменований для ясності, але перший залишився.

**Fix:** Видалити `let is_recording = self.is_recording.clone();` (рядок 48).

---

## Warning 7: dead code fields `start_time`, `end_time` (continuous.rs:15-16)

```rust
pub struct AudioSegment {
    pub samples: Vec<f32>,
    pub start_time: Instant,
    pub end_time: Instant,
    pub segment_id: usize,
}
```

**Root Cause:** Поля заповнюються при створенні сегменту (рядки 153-160), але споживач (UI транскрипція) використовує тільки `samples` і `segment_id`. Timestamps призначались для логування/відладки тривалості сегментів.

**Походження:** Преждевчасна оптимізація - поля додані "на майбутнє" для можливого відображення часу сегменту в UI або для синхронізації з відео.

**Fix:** Або використати для відображення часу в UI, або видалити, або `#[allow(dead_code)]` з коментарем про призначення.

---

## Warning 8: dead code method `add_samples` (continuous.rs:239)

```rust
pub fn add_samples(&self, samples: &[f32]) {
    self.ring_buffer.write(samples);
}
```

**Root Cause:** Метод був частиною початкового дизайну де зовнішній код (audio callback) мав би пушити семпли в recorder. Але фінальна реалізація використовує інший підхід - monitoring thread сам читає з `recorder.samples` (рядки 101-104).

**Походження:** Зміна архітектури без видалення старого API. Спочатку планувалось push-based API, потім перейшли на poll-based.

**Fix:** Видалити метод - він не потрібен при поточній архітектурі.

---

## Warning 9: dead code field `text` (diarization.rs:15)

```rust
pub struct DiarizationSegment {
    pub speaker_id: usize,
    pub start_time: f64,
    pub end_time: f64,
    pub text: String,  // завжди порожній
}
```

**Root Cause:** Поле призначалось для зберігання транскрипції разом з diarization інформацією. Але в поточній реалізації транскрипція відбувається окремо від diarization - Sortformer повертає тільки speaker boundaries, а Whisper транскрибує окремо.

**Походження:** Оптимістичний дизайн - планувалось мати unified pipeline де diarization і transcription працюють разом. Реальність: це два окремі процеси.

**Fix:** Видалити поле - транскрипція ніколи не буде тут зберігатися при поточній архітектурі.

---

## Warning 10: dead code method `model_path` (diarization.rs:105)

```rust
pub fn model_path(&self) -> Option<&PathBuf> {
    self.model_path.as_ref()
}
```

**Root Cause:** Getter для поля `model_path`, але ніде не викликається. UI не показує шлях до Sortformer моделі.

**Походження:** Стандартний getter написаний "про всяк випадок" для потенційного використання в UI (показати шлях користувачу).

**Fix:** Видалити - якщо знадобиться, легко додати назад.

---

## Warning 11: dead code method `search` (history.rs:92)

```rust
pub fn search(&self, query: &str) -> Vec<&HistoryEntry> {
    let query_lower = query.to_lowercase();
    self.entries
        .iter()
        .filter(|e| e.text.to_lowercase().contains(&query_lower))
        .collect()
}
```

**Root Cause:** Метод пошуку реалізований в бекенді, але history_dialog.rs не має поля пошуку в UI.

**Походження:** Backend-first розробка - функціонал написаний в очікуванні UI, який так і не був реалізований.

**Fix:** Або додати search box в history_dialog.rs, або видалити метод до моменту реалізації UI.

---

## Warning 12: dead code constant `WHISPER_SAMPLE_RATE` (loopback.rs:7)

```rust
const WHISPER_SAMPLE_RATE: u32 = 16000;
```

**Root Cause:** Константа визначена для консистентності з іншими модулями (audio.rs має таку ж), але в loopback.rs значення 16000 використовується напряму в parec command (рядок 70: `--rate=16000`).

**Походження:** Copy-paste константи з audio.rs без її використання.

**Fix:** Або використати константу в parec аргументах, або видалити.

---

## Warning 13: dead code function `get_sortformer_model_info` (models.rs:198)

```rust
pub fn get_sortformer_model_info() -> ModelInfo {
    ModelInfo {
        filename: "diar_streaming_sortformer_4spk-v2.1.onnx".to_string(),
        // ...
    }
}
```

**Root Cause:** Функція створена для відображення інформації про Sortformer модель в model_dialog.rs, але діалог показує тільки Whisper моделі.

**Походження:** Планувалось додати Sortformer в UI моделей, але так і не інтегровано.

**Fix:** Інтегрувати в model_dialog.rs або видалити до реалізації.

---

## Warning 14: dead code function `download_sortformer_model` (models.rs:215)

```rust
pub async fn download_sortformer_model<F>(progress_callback: F) -> Result<()>
```

**Root Cause:** Функція завантаження Sortformer моделі написана, але кнопка завантаження в UI відсутня.

**Походження:** Та сама причина що й #13 - backend готовий, UI ні.

**Fix:** Додати кнопку в model_dialog.rs або видалити до реалізації.

---

## Warning 15: dead code methods `len`, `is_empty` (ring_buffer.rs:103)

```rust
pub fn len(&self) -> usize { ... }
pub fn is_empty(&self) -> bool { ... }
```

**Root Cause:** Стандартні методи колекції додані для повноти API, але фактично ring buffer використовується тільки через `write()`, `read_all()`, `peek_last()`, `clear()`.

**Походження:** Слідування Rust conventions - структура схожа на колекцію, тому має мати `len()` і `is_empty()`. Але практично вони не потрібні.

**Fix:** `#[allow(dead_code)]` для API completeness, або видалити.

---

## Warning 16: dead code field `min_speech_duration_ms` (vad.rs:13)

```rust
pub struct VoiceActivityDetector {
    vad: Arc<Mutex<Vad>>,
    silence_threshold_ms: u32,
    min_speech_duration_ms: u32,  // зберігається, але не читається
}
```

**Root Cause:** Поле використовується тільки в методі `detect_segments()` (рядки 76, 100, 112), який сам не використовується. Метод `detect_speech_end()`, який реально використовується, перевіряє тільки `silence_threshold_ms`.

**Походження:** `detect_segments()` був первинним batch-processing API. Потім для streaming continuous recording був написаний `detect_speech_end()`, який простіший і не потребує min_speech_duration (це вирішується на рівні UI - мінімальний розмір сегменту).

**Fix:** Або видалити поле і `detect_segments()`, або використати min_speech_duration в `detect_speech_end()` для валідації.

---

## Warning 17: dead code methods (vad.rs:37)

- `set_silence_threshold()`
- `set_min_speech_duration()`
- `detect_segments()`

**Root Cause:**
- **Setters:** Параметри VAD передаються через конструктор `with_thresholds()`. Runtime зміна не використовується.
- **detect_segments():** Batch API замінений на streaming `detect_speech_end()`.

**Походження:**
- Setters: Стандартний патерн "builder з setters", але виявилось що конструктора достатньо.
- detect_segments(): Еволюція архітектури від batch до streaming обробки.

**Fix:** Видалити setters. `detect_segments()` можна залишити якщо планується batch режим, або видалити.

---

## Summary by Root Cause Category

| Category | Count | Warnings |
|----------|-------|----------|
| **Conditional compilation (sortformer feature)** | 5 | #1, #2, #3, #4, #5 |
| **Unused/leftover code after refactoring** | 4 | #6, #8, #12, #17 |
| **Backend ready, UI not implemented** | 3 | #11, #13, #14 |
| **Premature API/fields for future use** | 4 | #7, #9, #10, #15 |
| **Architecture evolution (batch→streaming)** | 1 | #16 |

---

## Recommended Actions

### Immediate (простий fix)
1. #1, #3, #4, #5 - Додати `#[cfg(feature = "sortformer")]` або underscore prefix
2. #2 - Видалити unreachable `Ok(())`
3. #6 - Видалити невикористаний clone
4. #12 - Видалити або використати константу

### Cleanup (видалити мертвий код)
5. #8 - Видалити `add_samples()`
6. #9 - Видалити поле `text`
7. #10 - Видалити getter `model_path()`
8. #17 - Видалити setters і можливо `detect_segments()`

### UI Integration (потребує роботи)
9. #11 - Додати search в history dialog
10. #13, #14 - Додати Sortformer в model dialog

### Keep with justification
11. #7 - `start_time/end_time` - залишити для майбутнього, додати `#[allow(dead_code)]`
12. #15 - `len/is_empty` - API completeness, `#[allow(dead_code)]`
13. #16 - Або використати, або видалити разом з `detect_segments()`
