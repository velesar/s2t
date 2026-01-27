# Speaker Diarization - Тестування та дослідження

## Мета
Визначити найкращий підхід для розпізнавання мовців та анотації транскрипції в конференційних записах.

## Тестові сценарії

### 1. Channel-Based Diarization (Simple)

**Тестовий сценарій:**
- Записати конференцію з двома потоками:
  - Потік 1: Мікрофон користувача
  - Потік 2: Системний аудіо (інші учасники)

**Очікуваний результат:**
- Кожен потік транскрибується окремо
- Результат об'єднується з labels "[Ви]" та "[Учасник]"
- Точність розпізнавання мовців: 100% (оскільки джерела різні)

**Код для тестування:**
```rust
// Псевдокод
let user_audio = record_microphone();
let remote_audio = record_system_audio();

let user_text = whisper.transcribe(&user_audio)?;
let remote_text = whisper.transcribe(&remote_audio)?;

let annotated = format!("[Ви] {}\n[Учасник] {}", user_text, remote_text);
```

**Метрики:**
- [ ] Чи правильно розпізнаються два потоки?
- [ ] Чи синхронізовані timestamps?
- [ ] Чи правильно об'єднуються результати?

### 2. pyannote-rs Integration Test

**Статус бібліотеки (досліджено 2026-01-28):**
- Версія: **0.3.4** (released 2025-09-07)
- Crate: https://crates.io/crates/pyannote-rs
- GitHub: https://github.com/thewh1teagle/pyannote-rs

**Моделі:**
- **Segmentation**: `segmentation-3.0` — визначає коли є мова (до 10с chunks, sliding window)
- **Speaker ID**: `wespeaker-voxceleb-resnet34-LM` — ідентифікує мовців
- Inference: ONNX Runtime
- Продуктивність: **< 1 хвилина на 1 годину аудіо** (CPU)

**Залежності:**
```toml
pyannote-rs = "0.3"
# Feature flags: coreml, directml, load-dynamic
```

**Альтернатива (pure Rust):**
- Fork: https://github.com/RustedBytes/pyannote-rs
- Використовує `kaldi-native-fbank` замість C++ bindings
- Burn backend (CPU/GPU/Metal)

**Тестовий сценарій:**
- Записати конференцію з 3+ учасниками
- Використати pyannote-rs для diarization
- Транскрибувати через Whisper з анотацією

**Очікуваний результат:**
- pyannote визначає сегменти з різними мовцями
- Whisper транскрибує кожен сегмент
- Результат містить labels типу "[Speaker 1]", "[Speaker 2]", тощо

**Код для тестування:**
```rust
use pyannote_rs::DiarizationPipeline;

// 1. Завантажити модель (перший раз)
let pipeline = DiarizationPipeline::new()?;

// 2. Застосувати diarization
let diarization = pipeline.apply(&audio_samples)?;

// 3. Для кожного сегменту - транскрипція
for segment in diarization.segments {
    let segment_audio = extract_segment(&audio, segment.start, segment.end);
    let text = whisper.transcribe(&segment_audio, None)?;
    println!("[{}] {} ({:.2}s - {:.2}s)",
        segment.speaker, text, segment.start, segment.end);
}
```

**Метрики (TODO):**
- [ ] Чи завантажуються моделі правильно?
- [ ] Чи правильно визначаються сегменти?
- [ ] Чи правильно ідентифікуються мовці?
- [ ] Чи синхронізовані з Whisper timestamps?
- [ ] Час обробки (очікується < 1 хвилина на годину аудіо)

### 3. Edge Cases Testing

#### 3.1 Одночасна мова
**Сценарій**: Два мовці говорять одночасно

**Очікуваний результат:**
- Channel-based: Кожен потік обробляється окремо (працює)
- pyannote-rs: Може плутати або пропускати один з мовців

#### 3.2 Тиша та паузи
**Сценарій**: Довгі паузи між репліками

**Очікуваний результат:**
- VAD правильно визначає сегменти з мовою
- Тиша не включається в транскрипцію

#### 3.3 Фоновий шум
**Сценарій**: Фоновий шум або музика

**Очікуваний результат:**
- VAD не класифікує шум як мову
- Транскрипція не містить шуму

#### 3.4 Один мовець (тільки мікрофон)
**Сценарій**: Користувач говорить сам (немає системного аудіо)

**Очікуваний результат:**
- Channel-based: Тільки "[Ви]" сегменти
- Не має бути "[Учасник]" сегментів

### 4. Performance Testing

**Метрики:**
- Час обробки 1 години аудіо:
  - [ ] Channel-based: очікується ~5-10 хвилин (Whisper транскрипція)
  - [ ] pyannote-rs: очікується < 1 хвилина (diarization) + Whisper time
- Використання пам'яті:
  - [ ] Channel-based: мінімальне (тільки Whisper)
  - [ ] pyannote-rs: +100-200MB для моделей
- CPU usage:
  - [ ] Channel-based: високе під час транскрипції
  - [ ] pyannote-rs: високе під час diarization + транскрипції

### 3. parakeet-rs + Sortformer (NVIDIA) — досліджено 2026-01-28

**Огляд:**
parakeet-rs — Rust бібліотека для STT з підтримкою NVIDIA Sortformer для streaming diarization.

**Ключові переваги:**
- **Streaming diarization** — real-time обробка
- **Sortformer v2/v2.1** — SOTA модель від NVIDIA (2025)
- Підтримка до **4 мовців**
- Обробка довгих файлів (25+ хв) без memory issues
- Працює на **CPU** (швидко навіть на Mac M3)

**Залежності:**
```toml
parakeet-rs = { version = "0.3", features = ["sortformer"] }
# GPU acceleration:
parakeet-rs = { version = "0.3", features = ["sortformer", "cuda"] }
```

**Моделі (HuggingFace):**
- `nvidia/diar_streaming_sortformer_4spk-v2` — streaming, 4 speakers
- `nvidia/diar_streaming_sortformer_4spk-v2.1` — покращена версія
- Ліцензія моделей: **CC-BY-4.0** (NVIDIA)

**Код для тестування:**
```rust
use parakeet_rs::sortformer::{Sortformer, DiarizationConfig};

let mut sortformer = Sortformer::with_config(
    "diar_streaming_sortformer_4spk-v2.onnx",
    None,
    DiarizationConfig::callhome(), // or dihard3(), custom()
)?;

let segments = sortformer.diarize(audio, 16000, 1)?;
for seg in segments {
    println!("Speaker {} [{:.2}s - {:.2}s]", seg.speaker_id, seg.start, seg.end);
}
```

**Обмеження:**
- Максимум 4 мовці
- ONNX export має issues для деяких моделей (GitHub #15077)
- Моделі потрібно завантажувати окремо

**Метрики (TODO):**
- [ ] Встановити та протестувати на Fedora 41
- [ ] Порівняти швидкість з pyannote-rs
- [ ] Перевірити точність на українській мові

---

## Порівняльна таблиця (оновлено 2026-01-28)

| Критерій | Channel-Based | pyannote-rs | parakeet-rs (Sortformer) |
|----------|---------------|-------------|--------------------------|
| Складність | ⭐ Легко | ⭐⭐⭐ Середньо | ⭐⭐ Легко |
| Точність (2 мовці) | ⭐⭐⭐⭐⭐ 100% | ⭐⭐⭐⭐ 95%+ | ⭐⭐⭐⭐ 95%+ |
| Точність (3+ мовці) | ❌ Не працює | ⭐⭐⭐⭐ 90%+ | ⭐⭐⭐⭐⭐ SOTA |
| Streaming | ❌ | ❌ | ✅ Real-time |
| Швидкість | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Моделі | 0 MB | ~100-200 MB | ~50-100 MB |
| Max speakers | 2 (fixed) | Unlimited | 4 |
| Ліцензія | MIT | MIT | MIT + CC-BY-4.0 (models) |

## Рекомендації (оновлено 2026-01-28)

### Рекомендований підхід: Hybrid

**Phase 1 (MVP):** Channel-Based
- Найпростіша реалізація
- 100% точність для 2 учасників (mic vs system audio)
- Нульові додаткові залежності

**Phase 2 (Advanced):** parakeet-rs + Sortformer
- **Рекомендовано замість pyannote-rs** для streaming use case
- NVIDIA SOTA модель (2025)
- Real-time processing
- До 4 мовців

**Phase 3 (Optional):** pyannote-rs
- Якщо потрібно >4 мовців
- Offline batch processing

### Чому Sortformer краще для конференцій:
1. **Streaming** — real-time diarization під час запису
2. **Оптимізований для довгих записів** — 25+ хвилин без memory issues
3. **NVIDIA backing** — активна розробка, CC-BY-4.0 ліцензія
4. **Швидший на CPU** — важливо для offline використання

### Можливі проблеми:
- Sortformer max 4 speakers (достатньо для більшості конференцій)
- Моделі CC-BY-4.0 — потрібна атрибуція NVIDIA

## Наступні кроки

1. [x] Дослідити альтернативи (pyannote-rs, Sortformer, NeMo)
2. [ ] Реалізувати channel-based diarization (Phase 1)
3. [ ] Створити PoC з parakeet-rs + Sortformer
4. [ ] Протестувати на реальних конференційних записах
5. [ ] Порівняти pyannote-rs vs Sortformer на українській мові

## Приклади очікуваного виводу

### Channel-Based (Simple)
```
[Ви] Привіт, як справи?
[Учасник] Добре, дякую. А у тебе?
[Ви] Теж добре. Почнемо конференцію?
[Учасник] Так, почнемо.
```

### pyannote-rs (Advanced)
```
[Speaker 1] Привіт, як справи?
[Speaker 2] Добре, дякую. А у тебе?
[Speaker 1] Теж добре. Почнемо конференцію?
[Speaker 2] Так, почнемо.
[Speaker 3] Я також готовий.
```

---

## Додаткові дослідження (2026-01-28)

### NVIDIA NeMo (Python-based)
**Не рекомендовано для Rust проекту**, але корисно знати:
- Sortformer — end-to-end Transformer encoder model
- Cascaded pipeline: MarbleNet (VAD) + TitaNet (embeddings) + MSDD
- ONNX export має проблеми (GitHub #15077, #8765)
- Потребує Python integration

### WhisperX (Python)
- Whisper + pyannote-audio для diarization
- 70x realtime з batched inference
- Word-level timestamps через wav2vec2
- **Не підходить** — Python-based

### Інші Rust crates
- `whisper-rs` — тільки STT, без diarization
- `deepspeech-rs` — застаріле, не підтримується
- `transcribe-rs` — wrapper, не diarization

### Висновок
Для Rust проекту найкращі варіанти:
1. **parakeet-rs** (Sortformer) — streaming, SOTA, до 4 speakers
2. **pyannote-rs** — batch processing, unlimited speakers
3. **Channel-based** — простий MVP для 2 speakers

## References

- [parakeet-rs](https://github.com/altunenes/parakeet-rs)
- [pyannote-rs](https://github.com/thewh1teagle/pyannote-rs)
- [NVIDIA Sortformer Blog](https://developer.nvidia.com/blog/identify-speakers-in-meetings-calls-and-voice-apps-in-real-time-with-nvidia-streaming-sortformer/)
- [NeMo Speaker Diarization](https://docs.nvidia.com/nemo-framework/user-guide/latest/nemotoolkit/asr/speaker_diarization/intro.html)
- [Speaker Diarization Models Comparison](https://brasstranscripts.com/blog/speaker-diarization-models-comparison)
