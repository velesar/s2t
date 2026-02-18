# ADR-004: Speaker Diarization Implementation Approach

## Status
**Accepted** (Updated 2026-01-28)

## Context
Для функції запису конференцій потрібно:
1. **Speaker Diarization**: Розпізнавати різних мовців у записі
2. **Annotated Transcription**: Транскрипція з анотацією хто що сказав (наприклад, "[Ви] текст" або "[Speaker 1] текст")

Це складна задача, оскільки:
- Whisper (поточна система транскрипції) **не підтримує** speaker diarization нативно
- Потрібно комбінувати транскрипцію з розпізнаванням мовців
- Для конференцій є спрощений варіант: розрізняти тільки два джерела (мікрофон vs системний аудіо)

## Research Findings

### 1. Whisper Limitations
**Ключовий факт**: OpenAI Whisper **не має вбудованої підтримки** speaker diarization.[1]

**Що Whisper робить:**
- Точна транскрипція тексту
- Word-level timestamps (якщо увімкнено)
- Мультимовна підтримка

**Що Whisper НЕ робить:**
- Не ідентифікує різних мовців
- Не позначає хто що сказав
- Не розрізняє голоси

**Висновок**: Потрібна окрема система для speaker diarization, яку потім інтегрувати з Whisper.

### 2. Professional Solutions

#### Option A: Whisper + Pyannote.audio
**Підхід**: Комбінувати Whisper для транскрипції з Pyannote для diarization.

**Як працює:**
1. Pyannote обробляє аудіо і визначає сегменти з різними мовцями
2. Whisper транскрибує кожен сегмент
3. Результати об'єднуються з timestamps та speaker labels

**Rust реалізація**: `pyannote-rs` (0.3.4 на crates.io)[2]

**Технічні деталі:**
- Використовує два моделі:
  - **Segmentation model** (segmentation-3.0): визначає коли є мова
  - **Speaker identification model** (wespeaker-voxceleb-resnet34-LM): ідентифікує мовців
- Обробляє 1 годину аудіо за менше ніж хвилину на CPU
- Використовує ONNX Runtime для inference
- Мінімальні залежності (~3-10MB)

**Переваги:**
- ✅ Висока точність
- ✅ Працює з багатьма мовцями
- ✅ Rust реалізація доступна
- ✅ Швидка обробка

**Недоліки:**
- ⚠️ Потребує завантаження моделей (~100-200MB)
- ⚠️ Додаткова складність інтеграції
- ⚠️ Потрібна синхронізація з Whisper timestamps

**Залежності:**
```toml
pyannote-rs = "0.3"  # або
native-pyannote-rs = "0.3"  # pure Rust версія
```

#### Option B: WhisperX + NVIDIA NeMo (Python)
**Підхід**: Використовувати WhisperX (обгортка навколо Whisper) з NeMo для diarization.

**Переваги:**
- Висока точність
- Word-level alignment
- GPU acceleration

**Недоліки:**
- ⚠️ Python-based (потребує Python integration)
- ⚠️ Потребує NVIDIA GPU для оптимальної продуктивності
- ⚠️ Складніша інтеграція в Rust додаток

**Висновок**: Не підходить для нашого випадку (Rust додаток, CPU-only).

#### Option E: parakeet-rs + NVIDIA Sortformer ⭐ NEW (2026-01-28)
**Підхід**: Використовувати `parakeet-rs` crate з NVIDIA Streaming Sortformer для real-time diarization.

**Що таке Sortformer:**
- NVIDIA SOTA модель для speaker diarization (2025)
- Transformer encoder-based end-to-end модель
- Streaming architecture — real-time обробка
- Sort Loss замість традиційного Permutation Invariant Loss

**Як працює:**
1. Аудіо обробляється в ~10s chunks
2. Sortformer визначає speaker labels для кожного frame
3. Speaker cache забезпечує консистентність між chunks
4. Результат: сегменти з speaker_id та timestamps

**Rust реалізація**: `parakeet-rs` (v0.3 на crates.io)

**Технічні деталі:**
- Streaming diarization до **4 мовців**
- Обробка довгих файлів (25+ хв) без memory issues
- ONNX Runtime для inference
- Підтримка CPU, CUDA, TensorRT, CoreML, DirectML

**Моделі (HuggingFace):**
- `nvidia/diar_streaming_sortformer_4spk-v2` (~50MB)
- `nvidia/diar_streaming_sortformer_4spk-v2.1` (покращена версія)
- Ліцензія: **CC-BY-4.0** (потрібна атрибуція NVIDIA)

**Переваги:**
- ✅ **Streaming** — real-time diarization під час запису
- ✅ **SOTA точність** — найкраща модель 2025 року
- ✅ **Швидкий на CPU** — оптимізований для offline
- ✅ **Простий API** — легша інтеграція ніж pyannote-rs
- ✅ **Native Rust** — без Python dependencies

**Недоліки:**
- ⚠️ Максимум 4 мовці (достатньо для більшості конференцій)
- ⚠️ Моделі CC-BY-4.0 — потрібна атрибуція
- ⚠️ ONNX export має відомі issues для деяких моделей

**Залежності:**
```toml
parakeet-rs = { version = "0.3", features = ["sortformer"] }
# З GPU:
parakeet-rs = { version = "0.3", features = ["sortformer", "cuda"] }
```

**Приклад коду:**
```rust
use parakeet_rs::sortformer::{Sortformer, DiarizationConfig};

let mut sortformer = Sortformer::with_config(
    "diar_streaming_sortformer_4spk-v2.onnx",
    None,
    DiarizationConfig::callhome(),
)?;

let segments = sortformer.diarize(audio, 16000, 1)?;
for seg in segments {
    println!("Speaker {} [{:.2}s - {:.2}s]", seg.speaker_id, seg.start, seg.end);
}
```

**Висновок**: **Рекомендовано для Production** — найкращий баланс точності, швидкості та простоти інтеграції.

### 3. Simple Approaches for Conference Recording

#### Option C: Channel-Based Diarization (Simplified)
**Підхід**: Для конференцій використати спрощений підхід:
- **Канал 1 (мікрофон)**: Користувач ("Ви")
- **Канал 2 (системний аудіо)**: Інші учасники ("Учасник" або "Remote")

**Як працює:**
1. Записуємо два окремі потоки (мікрофон + loopback)
2. Транскрибуємо кожен потік окремо через Whisper
3. Об'єднуємо з labels на основі джерела

**Переваги:**
- ✅ Найпростіша реалізація
- ✅ Не потребує додаткових моделей
- ✅ Працює одразу з існуючою системою
- ✅ Достатньо для базового випадку (2 мовці)

**Недоліки:**
- ⚠️ Не розрізняє кількох учасників у системному аудіо
- ⚠️ Якщо кілька людей говорять одночасно, буде плутанина
- ⚠️ Не працює для складних сценаріїв

#### Option D: Energy-Based VAD + Simple Clustering
**Підхід**: Використати Voice Activity Detection (VAD) для сегментації + простий clustering.

**Як працює:**
1. VAD визначає сегменти з мовою
2. Для кожного сегменту обчислюємо audio features (energy, spectral)
3. Простий clustering (наприклад, k-means з k=2) розділяє на два кластери
4. Whisper транскрибує кожен сегмент з label

**Переваги:**
- ✅ Простіша ніж pyannote
- ✅ Не потребує великих моделей
- ✅ Може працювати в real-time

**Недоліки:**
- ⚠️ Менша точність ніж pyannote
- ⚠️ Працює добре тільки з 2-3 мовцями
- ⚠️ Потребує налаштування thresholds

**Залежності:**
```toml
webrtc-vad = "0.4"  # для VAD
# + простий clustering алгоритм
```

### 4. Hybrid Approach for Conference Use Case (Updated 2026-01-28)

**Рекомендований підхід для конференцій:**

**Phase 1 (MVP)**: Channel-Based Diarization (Option C)
- Найпростіша реалізація
- Достатньо для базового випадку (2 учасники)
- Швидкий time-to-market
- Нульові додаткові залежності

**Phase 2 (Production)**: parakeet-rs + Sortformer (Option E) ⭐ RECOMMENDED
- Streaming real-time diarization
- До 4 мовців
- NVIDIA SOTA модель
- Простіша інтеграція ніж pyannote-rs

**Phase 3 (Optional)**: pyannote-rs (Option A)
- Тільки якщо потрібно >4 мовців
- Batch processing для офлайн аналізу
- Unlimited speakers

### 5. Comparison Table (2026-01-28)

| Критерій | Channel-Based | Sortformer | pyannote-rs |
|----------|---------------|------------|-------------|
| Складність | ⭐ Легко | ⭐⭐ Легко | ⭐⭐⭐ Середньо |
| Max speakers | 2 (fixed) | 4 | Unlimited |
| Streaming | ✅ | ✅ Real-time | ❌ Batch |
| Точність | 100% (channels) | SOTA 95%+ | 90-95% |
| Моделі | 0 MB | ~50-100 MB | ~100-200 MB |
| Ліцензія | MIT | MIT + CC-BY-4.0 | MIT |
| CPU speed | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ |

## Decision Options

### Option 1: Channel-Based Only (Simplest)
**Підхід**: Використовувати тільки channel-based diarization.

**Реалізація:**
```rust
pub struct ConferenceTranscription {
    user_segments: Vec<TranscriptionSegment>,  // з мікрофона
    remote_segments: Vec<TranscriptionSegment>, // з системного аудіо
}

impl ConferenceTranscription {
    pub fn transcribe_with_speakers(
        &self,
        input_audio: &[f32],
        output_audio: &[f32],
        whisper: &WhisperSTT,
    ) -> Result<String> {
        // Транскрибувати кожен потік окремо
        let user_text = whisper.transcribe(input_audio, None)?;
        let remote_text = whisper.transcribe(output_audio, None)?;

        // Об'єднати з labels
        format!("[Ви] {}\n[Учасник] {}", user_text, remote_text)
    }
}
```

**Переваги:**
- ✅ Найпростіша реалізація
- ✅ Не потребує додаткових залежностей
- ✅ Працює одразу

**Недоліки:**
- ⚠️ Не розрізняє кількох учасників
- ⚠️ Якщо кілька людей говорять одночасно - плутанина

### Option 2: pyannote-rs Integration (Most Accurate)
**Підхід**: Інтегрувати pyannote-rs для повноцінного speaker diarization.

**Реалізація:**
```rust
use pyannote_rs::DiarizationPipeline;

pub fn transcribe_with_diarization(
    audio: &[f32],
    whisper: &WhisperSTT,
) -> Result<Vec<AnnotatedSegment>> {
    // 1. Diarization через pyannote
    let pipeline = DiarizationPipeline::new()?;
    let diarization = pipeline.apply(audio)?;

    // 2. Для кожного сегменту - транскрипція через Whisper
    let mut result = Vec::new();
    for segment in diarization.segments {
        let audio_segment = extract_segment(audio, segment.start, segment.end);
        let text = whisper.transcribe(&audio_segment, None)?;
        result.push(AnnotatedSegment {
            speaker: segment.speaker,
            text,
            start: segment.start,
            end: segment.end,
        });
    }

    Ok(result)
}
```

**Переваги:**
- ✅ Висока точність
- ✅ Працює з багатьма мовцями
- ✅ Професійний підхід

**Недоліки:**
- ⚠️ Потребує завантаження моделей
- ⚠️ Більша складність
- ⚠️ Додаткова залежність

### Option 3: Hybrid with Sortformer ⭐ RECOMMENDED (Updated 2026-01-28)
**Підхід**: Channel-based для MVP, Sortformer для production, pyannote-rs як fallback.

**Реалізація:**
```rust
pub enum DiarizationMode {
    Simple,      // Channel-based (2 speakers, mic vs system)
    Streaming,   // Sortformer (up to 4 speakers, real-time)
    Advanced,    // pyannote-rs (unlimited speakers, batch)
}

pub struct ConferenceTranscriber {
    mode: DiarizationMode,
    sortformer: Option<Sortformer>,      // для Streaming mode
    pyannote: Option<DiarizationPipeline>, // для Advanced mode
}

impl ConferenceTranscriber {
    pub fn transcribe_streaming(
        &mut self,
        audio_chunk: &[f32],
    ) -> Result<Vec<DiarizedSegment>> {
        match self.mode {
            DiarizationMode::Streaming => {
                let sortformer = self.sortformer.as_mut().unwrap();
                sortformer.diarize(audio_chunk, 16000, 1)
            }
            // ...
        }
    }
}
```

**Переваги:**
- ✅ Гнучкість — три режими для різних потреб
- ✅ Простий старт (Simple mode)
- ✅ Production-ready (Streaming mode з Sortformer)
- ✅ Fallback для >4 speakers (Advanced mode)

**Недоліки:**
- ⚠️ Потрібна підтримка трьох режимів
- ⚠️ Додаткові optional dependencies

## Recommended Decision (Updated 2026-01-28)

### ✅ ACCEPTED: Hybrid with Sortformer (Option 3)

**Для MVP (Phase 1):**
**Channel-Based Diarization**

**Обґрунтування:**
- Найпростіша реалізація
- 100% точність для 2 учасників (mic vs system audio)
- Нульові додаткові залежності
- Швидкий time-to-market

**Для Production (Phase 2):**
**parakeet-rs + Sortformer** ⭐

**Обґрунтування:**
- **Streaming** — real-time diarization під час запису
- **SOTA точність** — NVIDIA модель 2025 року
- **Простіша інтеграція** ніж pyannote-rs
- **Швидкий на CPU** — важливо для offline використання
- До 4 мовців — достатньо для більшості конференцій

**Для Advanced (Phase 3, Optional):**
**pyannote-rs**

**Обґрунтування:**
- Тільки якщо потрібно >4 мовців
- Batch processing для офлайн аналізу

### Чому Sortformer замість pyannote-rs для Production:

| Аспект | Sortformer | pyannote-rs |
|--------|------------|-------------|
| Real-time | ✅ Streaming | ❌ Batch only |
| API simplicity | ✅ Простіший | ⚠️ Складніший |
| Long recordings | ✅ Native support | ⚠️ Memory issues |
| NVIDIA backing | ✅ Active development | ⚠️ Community |
| Speed (CPU) | ✅ Faster | ⚠️ Slower |

## Implementation Plan (Updated 2026-01-28)

### Phase 1: Channel-Based Diarization (MVP)
1. Модифікувати `WhisperSTT` для підтримки сегментів з timestamps
2. Створити `ConferenceTranscriber` для обробки двох потоків
3. Об'єднати результати з labels "[Ви]" та "[Учасник]"
4. Додати UI для відображення анотованої транскрипції

### Phase 2: Sortformer Integration (Production) ⭐
1. Додати `parakeet-rs` залежність з feature flag:
   ```toml
   parakeet-rs = { version = "0.3", features = ["sortformer"], optional = true }
   ```
2. Створити `StreamingDiarizer` модуль для real-time processing
3. Завантажити Sortformer модель з HuggingFace
4. Інтегрувати streaming diarization з conference recording
5. Додати UI toggle для вибору режиму (Simple/Streaming)

### Phase 3: pyannote-rs Fallback (Optional)
1. Додати `pyannote-rs` залежність (optional feature)
2. Створити `BatchDiarizer` для offline processing
3. Використовувати тільки якщо >4 мовців
4. Додати в налаштування як "Advanced mode"

### Feature Flags
```toml
[features]
default = []
diarization-streaming = ["parakeet-rs/sortformer"]
diarization-advanced = ["pyannote-rs"]
```

## Testing Requirements

### Phase 1 (Channel-Based)
1. Тестувати з 2 мовцями (мікрофон + системний аудіо)
2. Перевірити синхронізацію timestamps

### Phase 2 (Sortformer)
1. Тестувати streaming diarization з 2-4 мовцями
2. Перевірити точність на українській мові
3. Benchmark швидкість на CPU vs GPU
4. Тестувати довгі записи (25+ хвилин)
5. Порівняти з pyannote-rs на тих же записах

### Edge Cases
1. Одночасна мова (overlapping speech)
2. Довгі паузи та тиша
3. Фоновий шум
4. Один мовець (без системного аудіо)

### Accuracy Metrics
1. Diarization Error Rate (DER)
2. Порівняти з ручною анотацією
3. Word-level accuracy з Whisper timestamps

## Consequences

### Positive
- ✅ Можливість розпізнавати мовців (до 4 з Sortformer, unlimited з pyannote)
- ✅ Анотована транскрипція з speaker labels
- ✅ **Real-time streaming** з Sortformer
- ✅ Гнучкість — три режими для різних потреб
- ✅ NVIDIA SOTA модель для production
- ✅ Native Rust — без Python dependencies

### Negative
- ⚠️ Sortformer обмежений 4 мовцями
- ⚠️ Моделі потребують завантаження (~50-100MB для Sortformer)
- ⚠️ CC-BY-4.0 ліцензія для Sortformer моделей (атрибуція NVIDIA)
- ⚠️ Три режими = більше коду для підтримки
- ⚠️ Потрібне тестування на українській мові

### Risks
- ⚠️ ONNX export issues для деяких NeMo моделей (GitHub #15077)
- ⚠️ parakeet-rs ще молодий проект (v0.3)

## Related Files
- [docs/research/speaker-diarization-test.md](../research/speaker-diarization-test.md) - Testing plan
- [docs/adr/003-loopback-recording-approach.md](003-loopback-recording-approach.md) - Loopback recording ADR
- [src/whisper.rs](../../src/whisper.rs) - Current Whisper integration
- [Cargo.toml](../../Cargo.toml) - Dependencies

## References

### Rust Libraries
1. [parakeet-rs on crates.io](https://crates.io/crates/parakeet-rs) - NVIDIA Parakeet + Sortformer
2. [parakeet-rs GitHub](https://github.com/altunenes/parakeet-rs)
3. [pyannote-rs on crates.io](https://crates.io/crates/pyannote-rs)
4. [pyannote-rs GitHub](https://github.com/thewh1teagle/pyannote-rs)

### NVIDIA Sortformer
5. [NVIDIA Streaming Sortformer Blog](https://developer.nvidia.com/blog/identify-speakers-in-meetings-calls-and-voice-apps-in-real-time-with-nvidia-streaming-sortformer/)
6. [Sortformer Paper (arXiv)](https://arxiv.org/abs/2409.06656)
7. [nvidia/diar_streaming_sortformer_4spk-v2 (HuggingFace)](https://huggingface.co/nvidia/diar_streaming_sortformer_4spk-v2)
8. [NeMo Speaker Diarization Docs](https://docs.nvidia.com/nemo-framework/user-guide/latest/nemotoolkit/asr/speaker_diarization/intro.html)

### Comparisons
9. [Speaker Diarization Models Comparison 2026](https://brasstranscripts.com/blog/speaker-diarization-models-comparison)
10. [Whisper Speaker Diarization Guide](https://brasstranscripts.com/blog/whisper-speaker-diarization-guide)

### Other
11. [Energy-Based VAD Tutorial](https://superkogito.github.io/blog/2020/02/09/naive_vad.html)
