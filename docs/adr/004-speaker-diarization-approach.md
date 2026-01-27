# ADR-004: Speaker Diarization Implementation Approach

## Status
Proposed

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

#### Option B: WhisperX + NVIDIA NeMo
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

### 4. Hybrid Approach for Conference Use Case

**Рекомендований підхід для конференцій:**

**Phase 1 (MVP)**: Channel-Based Diarization (Option C)
- Найпростіша реалізація
- Достатньо для базового випадку
- Швидкий time-to-market

**Phase 2 (Enhanced)**: Додати pyannote-rs для складніших випадків
- Коли потрібно розрізняти кількох учасників у системному аудіо
- Для записів з більш ніж 2 мовцями
- Опціональна функція (advanced mode)

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

### Option 3: Hybrid (Recommended)
**Підхід**: Channel-based для MVP, з можливістю додати pyannote-rs пізніше.

**Реалізація:**
```rust
pub enum DiarizationMode {
    Simple,      // Channel-based (2 speakers)
    Advanced,    // pyannote-rs (multiple speakers)
}

pub struct ConferenceTranscriber {
    mode: DiarizationMode,
    pyannote: Option<DiarizationPipeline>,  // тільки якщо Advanced
}
```

**Переваги:**
- ✅ Гнучкість
- ✅ Простий старт (Simple mode)
- ✅ Можливість покращення (Advanced mode)

**Недоліки:**
- ⚠️ Потрібна підтримка двох режимів
- ⚠️ Трохи складніша архітектура

## Recommended Decision

**Для MVP (Phase 1):**
**Option 1** - Channel-Based Diarization

**Обґрунтування:**
- Найпростіша реалізація
- Достатньо для базового випадку конференцій (користувач + інші)
- Не потребує додаткових залежностей
- Швидкий time-to-market

**Для Production (Phase 2):**
**Option 3** - Hybrid approach

**Обґрунтування:**
- Починаємо з Simple mode
- Додаємо Advanced mode як опцію
- Користувач може вибрати режим в налаштуваннях

## Implementation Plan

### Phase 1: Channel-Based Diarization
1. Модифікувати `WhisperSTT` для підтримки сегментів з timestamps
2. Створити `ConferenceTranscriber` для обробки двох потоків
3. Об'єднати результати з labels "[Ви]" та "[Учасник]"
4. Додати UI для відображення анотованої транскрипції

### Phase 2: Advanced Diarization (Optional)
1. Додати `pyannote-rs` залежність
2. Створити `AdvancedDiarization` модуль
3. Додати налаштування для вибору режиму
4. Інтегрувати з існуючою системою транскрипції

## Testing Requirements

1. **Simple mode**: Тестувати з 2 мовцями (мікрофон + системний аудіо)
2. **Advanced mode**: Тестувати з 3+ мовцями
3. **Edge cases**: Одночасна мова, тиша, шум
4. **Accuracy**: Порівняти з ручною анотацією

## Consequences

### Positive
- ✅ Можливість розпізнавати мовців
- ✅ Анотована транскрипція
- ✅ Гнучкість в обранні підходу

### Negative
- ⚠️ Додаткова складність в коді
- ⚠️ Можливі проблеми з точністю (особливо в Simple mode)
- ⚠️ Потрібне тестування на різних типах записів
- ⚠️ Advanced mode потребує додаткових ресурсів (моделі)

## Related Files
- [docs/backlog/conference-recording.md](../backlog/conference-recording.md) - Feature description
- [src/whisper.rs](../../src/whisper.rs) - Current Whisper integration
- [Cargo.toml](../../Cargo.toml) - Dependencies

## References
1. [Whisper Speaker Diarization Guide](https://brasstranscripts.com/blog/whisper-speaker-diarization-guide)
2. [pyannote-rs on crates.io](https://crates.io/crates/pyannote-rs)
3. [Energy-Based VAD Tutorial](https://superkogito.github.io/blog/2020/02/09/naive_vad.html)
4. [WhisperX with NeMo](https://learnopencv.com/automatic-speech-recognition/)
