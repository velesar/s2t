# STT Optimization - Тестування та бенчмарки

## Мета
Визначити оптимальний баланс між швидкістю та якістю транскрипції для української мови.

## Тестові сценарії

### 1. Порівняння моделей Whisper

**Тестові моделі:**
- `ggml-tiny.bin` (75MB) - найшвидша, найменш точна
- `ggml-base.bin` (150MB) - баланс
- `ggml-base-q5_0.bin` (quantized) - швидша версія base
- `ggml-base-q8_0.bin` (quantized) - менш квантована
- `ggml-small.bin` (500MB) - краща якість
- `ggml-small-q5_0.bin` (quantized) - швидша версія small

**Тестові аудіо:**
1. Короткий запис (10 секунд) - проста мова
2. Середній запис (1 хвилина) - звичайна мова
3. Довгий запис (5 хвилин) - складніша мова
4. Запис з шумом - перевірка стійкості
5. Запис з акцентом - перевірка точності

**Метрики:**
- [ ] Час обробки (секунди)
- [ ] Real-time factor (RTF) = час_обробки / час_аудіо
- [ ] Word Error Rate (WER) - порівняння з reference
- [ ] Розмір моделі (MB)
- [ ] CPU usage (%)
- [ ] Пам'ять (MB)

### 2. Бенчмарки швидкості

**Тестовий код:**
```rust
use std::time::Instant;

fn benchmark_model(model_path: &str, audio_samples: &[f32]) -> f64 {
    let whisper = WhisperSTT::new(model_path).unwrap();

    let start = Instant::now();
    let _result = whisper.transcribe(audio_samples, Some("uk")).unwrap();
    let duration = start.elapsed();

    let audio_duration_secs = audio_samples.len() as f64 / 16000.0;
    let rtf = duration.as_secs_f64() / audio_duration_secs;

    println!("Model: {}", model_path);
    println!("Audio duration: {:.2}s", audio_duration_secs);
    println!("Processing time: {:.2}s", duration.as_secs_f64());
    println!("Real-time factor: {:.2}x", rtf);

    rtf
}
```

**Очікувані результати:**
- Tiny: RTF < 0.1x (дуже швидко)
- Base: RTF ~0.3-0.5x (швидко)
- Base-Q5_0: RTF ~0.2-0.3x (швидше ніж base)
- Small: RTF ~1.0-2.0x (повільно)
- Small-Q5_0: RTF ~0.5-1.0x (швидше ніж small)

### 3. Порівняння якості

**Тестовий набір:**
- 10 записів українською мовою різної складності
- Reference transcripts (ручна транскрипція)

**Метрика якості:**
```rust
fn calculate_wer(reference: &str, hypothesis: &str) -> f64 {
    // Word Error Rate calculation
    // WER = (S + D + I) / N
    // S = substitutions, D = deletions, I = insertions, N = total words
}
```

**Очікувані результати:**
- Tiny: WER ~15-25% (низька якість)
- Base: WER ~5-10% (прийнятна якість)
- Base-Q5_0: WER ~6-12% (трохи гірше ніж base)
- Small: WER ~3-7% (висока якість)
- Small-Q5_0: WER ~4-8% (трохи гірше ніж small)

### 4. Тестування faster-whisper

**Якщо інтегруємо faster-whisper:**

**Тестовий код:**
```rust
// Псевдокод для faster-whisper-rs
use faster_whisper_rs::Whisper;

fn benchmark_faster_whisper(model_path: &str, audio: &[f32]) -> f64 {
    let model = Whisper::new(model_path).unwrap();

    let start = Instant::now();
    let result = model.transcribe(audio, "uk").unwrap();
    let duration = start.elapsed();

    // Calculate RTF
    // ...
}
```

**Метрики:**
- [ ] Порівняння швидкості з whisper-rs
- [ ] Порівняння якості
- [ ] Складність інтеграції

### 5. Тестування streaming transcription

**Якщо додаємо streaming:**

**Тестовий сценарій:**
- Записувати аудіо сегментами по 5 секунд
- Транскрибувати кожен сегмент окремо
- Об'єднувати результати

**Метрики:**
- [ ] Час до першого результату
- [ ] Загальний час обробки
- [ ] Якість порівняно з full transcription
- [ ] UX (чи користувач бачить результат швидше)

## Порівняльна таблиця

| Модель | Розмір | Швидкість (RTF) | Якість (WER) | Рекомендація |
|--------|--------|-----------------|--------------|--------------|
| tiny | 75MB | < 0.1x | 15-25% | Тільки для тестів |
| base | 150MB | 0.3-0.5x | 5-10% | Баланс (поточний) |
| base-q5_0 | ~80MB | 0.2-0.3x | 6-12% | **Рекомендовано для MVP** |
| base-q8_0 | ~120MB | 0.25-0.4x | 5-11% | Альтернатива |
| small | 500MB | 1.0-2.0x | 3-7% | Для високої якості |
| small-q5_0 | ~250MB | 0.5-1.0x | 4-8% | Для кращої якості |

## Рекомендації після тестування

### Якщо base-q5_0 достатньо якісний (WER < 10%):
- Використовувати як default модель
- Додати small-q5_0 як опцію для кращої якості
- Додати tiny як опцію для швидких тестів

### Якщо base-q5_0 недостатньо якісний:
- Використовувати base-q8_0 (менше квантування)
- Або small-q5_0 як default
- Додати опцію для вибору моделі

### Якщо потрібна ще більша швидкість:
- Дослідити faster-whisper інтеграцію
- Або whisper.apr для streaming

## Наступні кроки

1. Завантажити квантовані моделі з HuggingFace
2. Запустити бенчмарки на тестовому наборі
3. Записати результати в цей документ
4. Оновити ADR-005 з результатами
5. Реалізувати обраний підхід

## Приклади команд для завантаження моделей

```bash
# Base quantized models
curl -L -o ~/.local/share/whisper/ggml-base-q5_0.bin \
    https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_0.bin

curl -L -o ~/.local/share/whisper/ggml-base-q8_0.bin \
    https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q8_0.bin

# Small quantized
curl -L -o ~/.local/share/whisper/ggml-small-q5_0.bin \
    https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_0.bin
```

## Результати тестування

### Система: [Заповнити після тестування]
- CPU:
- RAM:
- OS:

### Результати:

| Модель | RTF | WER | Розмір | CPU % | Пам'ять |
|--------|-----|-----|--------|-------|---------|
| tiny | | | | | |
| base | | | | | |
| base-q5_0 | | | | | |
| base-q8_0 | | | | | |
| small | | | | | |
| small-q5_0 | | | | | |

### Висновки:
[Заповнити після тестування]
