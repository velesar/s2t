# Multi-Language / Code-Switching STT Research

**Дата:** 2026-01-29
**Статус:** Complete
**Проблема:** Користувачі часто змішують мови в розмовах (UA+EN, UA+RU). Поточна реалізація передає фіксовану мову з конфіга.

## Зміст

1. [Поточна реалізація](#поточна-реалізація)
2. [Whisper і code-switching](#whisper-і-code-switching)
3. [Українська мова в Whisper](#українська-мова-в-whisper)
4. [Підходи до вирішення](#підходи-до-вирішення)
5. [Рекомендований підхід](#рекомендований-підхід)
6. [План імплементації](#план-імплементації)

---

## Поточна реалізація

### Як працює зараз (src/whisper.rs)

```rust
pub fn transcribe(&self, samples: &[f32], language: Option<&str>) -> Result<String> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    // Мова передається явно з конфіга
    if let Some(lang) = language {
        params.set_language(Some(lang));  // "uk", "en", тощо
    }
    // ...
}
```

### Конфігурація (src/config.rs)

```toml
# config.toml
language = "uk"  # Фіксована мова для всіх записів
```

### Проблеми

1. **Фіксована мова** - не адаптується до контенту
2. **Code-switching ігнорується** - англійські слова транскрибуються як українські
3. **Немає language detection** - не використовується автодетекція

---

## Whisper і code-switching

### Обмеження архітектури Whisper

```
Whisper Processing Flow:
┌───────────────────────────────────────────────────────────────┐
│                                                               │
│  Audio Input (up to 30s chunk)                               │
│       │                                                       │
│       ▼                                                       │
│  ┌─────────────────────────────────────────┐                 │
│  │         Mel Spectrogram                  │                 │
│  │         (First 30s)                      │                 │
│  └────────────────┬────────────────────────┘                 │
│                   │                                           │
│                   ▼                                           │
│  ┌─────────────────────────────────────────┐                 │
│  │       Language Detection                 │                 │
│  │   (Based on first 30s ONLY!)            │ ← ПРОБЛЕМА!     │
│  │   Returns: "uk" with 85% confidence      │                 │
│  └────────────────┬────────────────────────┘                 │
│                   │                                           │
│                   ▼                                           │
│  ┌─────────────────────────────────────────┐                 │
│  │    Encoder-Decoder Transcription         │                 │
│  │    (Uses detected language for ALL)      │ ← ПРОБЛЕМА!     │
│  └─────────────────────────────────────────┘                 │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

**Ключові обмеження:**

| Обмеження | Опис |
|-----------|------|
| **Single language assumption** | Whisper припускає одну мову на весь файл |
| **First 30s detection** | Мова визначається по перших 30 секундах |
| **No mid-stream switching** | Не перемикається між мовами динамічно |
| **Trained on monolingual data** | Тренувався переважно на одномовних записах |

### Що відбувається при code-switching

**Сценарій 1: UA → EN**
```
Input:  "Привіт, let's discuss the project"
Output: "Привіт, летс діскас зе проджект"  ❌
        (англійські слова транскрибуються як українські)
```

**Сценарій 2: EN → UA (language=en)**
```
Input:  "Hello, давай обговоримо проект"
Output: "Hello, davai obgovorimo proekt"  ❌
        (українські слова транслітеруються)
```

**Сценарій 3: Auto-detect**
```
Input:  [50% UA, 50% EN audio]
Detect: "uk" (based on first speaker)
Output: Все транскрибується як українське  ❌
```

---

## Українська мова в Whisper

### Базова продуктивність Whisper

| Модель | WER (Common Voice UA) | Примітки |
|--------|----------------------|----------|
| Whisper base | ~35-40% | Без fine-tuning |
| Whisper small | ~30% | Без fine-tuning |
| Whisper medium | ~25% | Без fine-tuning |
| Whisper large-v2 | ~20% | Без fine-tuning |
| Whisper large-v3 | ~15-18% | Покращено для low-resource |

### Fine-tuned моделі для української

| Модель | WER | Джерело |
|--------|-----|---------|
| `Yehor/whisper-small-ukrainian` | 27% | HuggingFace |
| `mitchelldehaven/whisper-medium-uk` | ~20% | HuggingFace |
| `Yehor/whisper-large-v2-quantized-uk` | ~15% | HuggingFace |
| ElevenLabs Scribe (commercial) | 3.1-5.5% | FLEURS/CV benchmarks |

### Ресурси для української

- **Датасети:**
  - Mozilla Common Voice Ukrainian (~100+ hours)
  - M-AILABS Ukrainian Corpus
  - VoA Ukrainian (~390 hours)

- **Інструменти:**
  - [speech-recognition-uk](https://github.com/egorsmkv/speech-recognition-uk) - каталог ресурсів
  - [whisper-ukrainian](https://github.com/egorsmkv/whisper-ukrainian) - скрипти для fine-tuning

### Ukrainian + Russian code-switching

Є дослідження "Automatic Recognition of mixed Ukrainian-Russian Speech":
- Оскільки українська фонетика включає всі російські фонеми
- Можливо використовувати acoustic model на базі української для розпізнавання російської
- Підхід працює для UA-RU code-switching

**Проблема:** Для UA-EN такого готового рішення немає.

---

## Підходи до вирішення

### Підхід 1: Auto-detect mode

**Ідея:** Не вказувати мову явно, дозволити Whisper визначати.

```rust
// Замість
params.set_language(Some("uk"));

// Використовувати
params.set_language(None);  // або не викликати взагалі
```

**Pros:**
- Найпростіша зміна
- Працює для монолінгвальних записів

**Cons:**
- Не вирішує code-switching
- Перша мова застосовується до всього
- Може давати гірші результати для української

### Підхід 2: VAD-based segmentation + per-segment detection

**Ідея:** Сегментувати аудіо по VAD, визначати мову кожного сегменту.

```
┌─────────────────────────────────────────────────────────────┐
│                   VAD-BASED APPROACH                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Audio → VAD → [Segment 1] [Segment 2] [Segment 3]         │
│                     │           │           │               │
│                     ▼           ▼           ▼               │
│              Lang Detect  Lang Detect  Lang Detect          │
│                  "uk"        "en"         "uk"              │
│                     │           │           │               │
│                     ▼           ▼           ▼               │
│              Transcribe   Transcribe   Transcribe           │
│              (lang=uk)    (lang=en)    (lang=uk)            │
│                     │           │           │               │
│                     └─────┬─────┴─────┬─────┘               │
│                           ▼                                 │
│                    Merged Output                            │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Pros:**
- Адаптується до зміни мов
- Працює для inter-sentential code-switching
- Можна реалізувати з поточними інструментами

**Cons:**
- Не працює для intra-sentential (мікс в одному реченні)
- Додаткова latency на language detection
- Більше обчислень

### Підхід 3: Speaker diarization + per-speaker language

**Ідея:** Різні спікери можуть говорити різними мовами.

```
┌─────────────────────────────────────────────────────────────┐
│              DIARIZATION-BASED APPROACH                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Audio → Diarization → [Speaker A] [Speaker B]             │
│                             │           │                   │
│                             ▼           ▼                   │
│                      Lang: "uk"    Lang: "en"              │
│                             │           │                   │
│                             ▼           ▼                   │
│                      Transcribe   Transcribe                │
│                             │           │                   │
│                             ▼           ▼                   │
│                   [Спікер 1: ...]  [Спікер 2: ...]         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Pros:**
- Природний для конференцій
- Вже є diarization в s2t (channel-based, Sortformer)
- Мова визначається один раз per speaker

**Cons:**
- Не працює коли один спікер говорить двома мовами
- Потребує diarization

### Підхід 4: Multi-pass transcription

**Ідея:** Транскрибувати двічі (UA і EN), об'єднати результати.

```
┌─────────────────────────────────────────────────────────────┐
│               MULTI-PASS APPROACH                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Audio ──┬──→ Transcribe (lang=uk) → [UA text + confidence]│
│          │                                                  │
│          └──→ Transcribe (lang=en) → [EN text + confidence]│
│                         │                                   │
│                         ▼                                   │
│                    Merge by segment confidence              │
│                         │                                   │
│                         ▼                                   │
│                    Mixed output                             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Pros:**
- Може працювати для intra-sentential
- Не потребує додаткових моделей

**Cons:**
- 2x час обробки
- Складне злиття результатів
- Може давати дивні результати

### Підхід 5: DiCoW (Diarization-Conditioned Whisper)

**Ідея:** Використовувати спеціалізовану модель для multi-speaker multilingual.

**Pros:**
- State-of-the-art результати
- Спеціально розроблено для цього

**Cons:**
- Python-only (не whisper-rs)
- Більші обчислювальні вимоги
- Складна інтеграція

### Підхід 6: Primary + fallback languages

**Ідея:** Якщо confidence низький - спробувати іншу мову.

```rust
fn transcribe_adaptive(&self, samples: &[f32]) -> Result<String> {
    // Спочатку пробуємо основну мову (з конфіга)
    let (text_primary, conf_primary) = self.transcribe_with_confidence(samples, "uk")?;

    if conf_primary > 0.8 {
        return Ok(text_primary);
    }

    // Низький confidence - пробуємо fallback
    let (text_fallback, conf_fallback) = self.transcribe_with_confidence(samples, "en")?;

    if conf_fallback > conf_primary {
        Ok(text_fallback)
    } else {
        Ok(text_primary)
    }
}
```

**Pros:**
- Простіше за multi-pass
- Не завжди потребує другого проходу

**Cons:**
- whisper-rs не повертає confidence напряму
- Все одно не вирішує intra-sentential

---

## Рекомендований підхід

### Для s2t: Hybrid VAD + Adaptive Language

**Phase 1: Immediate (Quick Win)**

Додати `language = "auto"` опцію в конфіг:

```toml
# config.toml
language = "auto"  # або "uk", "en", "uk+en"
```

```rust
fn get_language_param(config_lang: &str) -> Option<&str> {
    match config_lang {
        "auto" => None,  // Whisper auto-detect
        lang => Some(lang),
    }
}
```

**Phase 2: VAD-based segment detection**

При continuous mode - визначати мову кожного сегменту:

```rust
fn transcribe_segment(&self, samples: &[f32], hint_lang: Option<&str>) -> Result<TranscriptSegment> {
    // 1. Detect language if no hint
    let detected_lang = if hint_lang.is_none() {
        self.detect_language(samples)?  // Перші кілька секунд
    } else {
        hint_lang.unwrap().to_string()
    };

    // 2. Transcribe with detected language
    let text = self.transcribe(samples, Some(&detected_lang))?;

    Ok(TranscriptSegment {
        text,
        language: detected_lang,
    })
}
```

**Phase 3: Per-speaker language (Conference mode)**

При diarization - зберігати мову per speaker:

```rust
struct SpeakerProfile {
    speaker_id: usize,
    detected_language: String,
    segments_count: usize,
}

// При transcribe_with_diarization:
// 1. Визначити мову першого сегменту кожного спікера
// 2. Використовувати цю мову для всіх сегментів цього спікера
```

### Конфігурація

```toml
[transcription]
# Основна мова (використовується як fallback і hint)
primary_language = "uk"

# Режим визначення мови
# - "fixed" - завжди primary_language
# - "auto" - автовизначення Whisper
# - "adaptive" - auto з fallback на primary
# - "per_segment" - визначення для кожного сегменту
language_mode = "adaptive"

# Додаткові мови для fallback (якщо adaptive)
fallback_languages = ["en", "ru"]

# Мінімальний confidence для прийняття результату
min_confidence = 0.7
```

---

## План імплементації

### Phase 1: Auto-detect option (1 день)

1. Оновити `Config`:
   ```rust
   pub language: String,  // "uk", "en", "auto"
   ```

2. Оновити `WhisperSTT::transcribe`:
   ```rust
   if language != "auto" {
       params.set_language(Some(language));
   }
   // else: Whisper auto-detects
   ```

3. Оновити UI Settings:
   - Додати "Auto" до dropdown мов

### Phase 2: Language detection API (2-3 дні)

1. Дослідити whisper-rs API для `detect_language`
   - Можливо потрібно PR до whisper-rs

2. Або: використати перший короткий транскрипт для детекції

3. Implement:
   ```rust
   fn detect_language(&self, samples: &[f32]) -> Result<String>;
   ```

### Phase 3: Per-segment language (3-5 днів)

1. В continuous mode:
   - Визначати мову кожного VAD сегменту
   - Передавати в Whisper

2. Зберігати мову в `AudioSegment`:
   ```rust
   pub struct AudioSegment {
       pub samples: Vec<f32>,
       pub detected_language: Option<String>,
       // ...
   }
   ```

### Phase 4: Per-speaker language (2-3 дні)

1. При diarization:
   - Визначити мову першого сегменту кожного спікера
   - Кешувати та використовувати для наступних

### Checklist

- [ ] Phase 1: Auto-detect
  - [ ] Config option `language = "auto"`
  - [ ] Update whisper.rs to handle "auto"
  - [ ] UI dropdown update
  - [ ] Test with mixed audio

- [ ] Phase 2: Language detection
  - [ ] Research whisper-rs detect_language API
  - [ ] Implement detection function
  - [ ] Add confidence scores

- [ ] Phase 3: Per-segment
  - [ ] Integrate with continuous mode
  - [ ] Per-segment language tracking
  - [ ] UI indication of detected language

- [ ] Phase 4: Per-speaker
  - [ ] Speaker language profiles
  - [ ] Integration with diarization
  - [ ] Caching speaker preferences

---

## Обмеження

### Що НЕ буде працювати добре

1. **Intra-sentential code-switching**
   ```
   "Я думаю що this is a good idea"
   ```
   - Whisper не підтримує нативно
   - Потребує спеціальних моделей (не існують для UA-EN)

2. **Швидке перемикання мов**
   - Кожен detect + transcribe займає час
   - При швидкому перемиканні latency зростає

3. **Рідкісні мови**
   - Whisper має обмежену підтримку
   - UA краще ніж багато інших, але гірше за EN

### Очікувані покращення

| Сценарій | До | Після |
|----------|-----|-------|
| Чиста українська | Добре | Добре |
| Чиста англійська (lang=en) | Добре | Добре |
| UA speaker + EN speaker | Погано | Добре (per-speaker) |
| One speaker switching | Погано | Краще (per-segment) |
| Mixed in one sentence | Погано | Погано* |

*Потребує спеціальних моделей

---

## Джерела

- [Whisper Multiple Languages Discussion](https://github.com/openai/whisper/discussions/49)
- [DiCoW: Diarization-Conditioned Whisper](https://github.com/BUTSpeechFIT/DiCoW)
- [faster-whisper](https://github.com/SYSTRAN/faster-whisper)
- [WhisperX](https://github.com/m-bain/whisperX)
- [Ukrainian ASR Resources](https://github.com/egorsmkv/speech-recognition-uk)
- [whisper-ukrainian fine-tuning](https://github.com/egorsmkv/whisper-ukrainian)
- [Ukrainian-Russian Code-Switching ASR Paper](https://lt4all.elra.info/media/papers/P2/51.pdf)
- [Semantic VAD Paper](https://www.isca-archive.org/interspeech_2023/shi23c_interspeech.pdf)
