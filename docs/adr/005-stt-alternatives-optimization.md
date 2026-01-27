# ADR-005: STT Alternatives and Optimization Approaches

## Status
Proposed

## Context
Поточна реалізація використовує Whisper через `whisper-rs` (обгортка навколо whisper.cpp). Проблеми:
- **Великі якісні моделі** (medium, large): працюють занадто довго на CPU
- **Малі моделі** (tiny, base): недостатньо точні, особливо для української мови
- Потрібен баланс між швидкістю та якістю

Потрібно дослідити:
1. Альтернативні STT моделі/бібліотеки
2. Оптимізації для Whisper (quantization, acceleration)
3. Гібридні підходи (швидка модель для простого, повільна для складного)

## Research Findings

### 1. Whisper Optimization Options

#### Option A: Quantization
**Підхід**: Використати квантовані моделі Whisper для зменшення розміру та прискорення.

**Техніки:**
- **INT8 quantization**: 2x швидше, мінімальна втрата якості
- **INT4 quantization**: 4x швидше, помітна втрата якості
- **Half-Quadratic Quantization (HQQ)**: 6x швидше з мінімальною втратою якості

**whisper.cpp підтримка:**
- Підтримує Q4_0, Q4_1, Q5_0, Q5_1, Q8_0 quantization
- Моделі доступні на HuggingFace (ggml-format)
- Можна конвертувати з PyTorch моделей

**Переваги:**
- ✅ Використовує існуючу інфраструктуру
- ✅ Менший розмір моделей
- ✅ Швидша обробка
- ✅ Мінімальні зміни в коді

**Недоліки:**
- ⚠️ Дещо нижча якість (особливо INT4)
- ⚠️ Потрібні квантовані моделі

**Реалізація:**
```rust
// Використовувати квантовані моделі (Q5_0 або Q8_0)
// Завантажити з HuggingFace: ggml-base-q5_0.bin
// whisper-rs автоматично підтримує квантовані моделі
```

#### Option B: faster-whisper (CTranslate2)
**Підхід**: Використати faster-whisper через Rust bindings.

**Техніка:**
- Використовує CTranslate2 inference engine
- До 4x швидше ніж оригінальний Whisper
- Підтримує quantization (INT8, INT4)
- Оптимізований для CPU та GPU

**Rust bindings:**
- `faster-whisper-rs` (0.2.0) - обгортка через PyO3
- `ctranslate2-rs` - прямі bindings до CTranslate2

**Переваги:**
- ✅ Значно швидше (до 4x)
- ✅ Підтримує quantization
- ✅ Активно розвивається

**Недоліки:**
- ⚠️ `faster-whisper-rs` потребує Python (через PyO3)
- ⚠️ `ctranslate2-rs` потребує більше налаштування
- ⚠️ Може потребувати конвертації моделей

**Залежності:**
```toml
faster-whisper-rs = "0.2"  # або
ctranslate2-rs = "0.1"
```

#### Option C: whisper.apr (Pure Rust)
**Підхід**: Використати whisper.apr - pure Rust реалізацію.

**Особливості:**
- Pure Rust (без C++ залежностей)
- WASM-first дизайн
- INT4/INT8 quantization (4x-8x зменшення розміру)
- Streaming transcription
- Custom `.apr` формат моделей

**Продуктивність:**
- Real-time factor на M1 MacBook: 0.3x (tiny) до 2.0x (large)
- Оптимізований для edge deployment

**Переваги:**
- ✅ Pure Rust (без C++ залежностей)
- ✅ Streaming support
- ✅ Оптимізований для CPU
- ✅ Менший розмір моделей

**Недоліки:**
- ⚠️ Потребує конвертації моделей в .apr формат
- ⚠️ Менш зрілий проект
- ⚠️ Може мати обмежену підтримку мов

**Залежності:**
```toml
whisper-apr = "0.1"
```

### 2. Alternative STT Models

#### Option D: Wav2Vec 2.0
**Підхід**: Використати Wav2Vec 2.0 для STT.

**Особливості:**
- Ефективний для low-resource мов
- Може бути швидший на CPU
- Потребує fine-tuning для конкретної мови

**Rust реалізація:**
- Немає готових Rust bindings
- Потрібна інтеграція через Python або ONNX

**Переваги:**
- ✅ Може бути швидший
- ✅ Ефективний для української

**Недоліки:**
- ⚠️ Немає готових Rust реалізацій
- ⚠️ Потребує fine-tuning
- ⚠️ Складніша інтеграція

**Висновок**: Не рекомендовано через відсутність Rust реалізацій.

#### Option E: Coqui STT (Mozilla DeepSpeech)
**Підхід**: Використати Coqui STT (раніше Mozilla DeepSpeech).

**Особливості:**
- Оптимізований для edge devices
- Підтримує багато мов (включно з українською через Common Voice)
- Може бути швидший на CPU
- Менші моделі ніж Whisper
- Streaming support

**Rust реалізація:**
- `coqui-stt` crate (0.1.0) - Rust bindings до Coqui STT C library
- Підтримується tazz4843 (той самий автор що whisper-rs)
- Останнє оновлення: липень 2022

**Переваги:**
- ✅ Rust bindings доступні
- ✅ Може бути швидший на CPU
- ✅ Менші моделі
- ✅ Streaming transcription
- ✅ Підтримка української через Common Voice моделі

**Недоліки:**
- ⚠️ Менш активно підтримується (останнє оновлення 2022)
- ⚠️ Може бути нижча якість ніж Whisper
- ⚠️ Потребує окремі моделі (не сумісні з Whisper)
- ⚠️ Немає прямих бенчмарків порівняння з Whisper

**Залежності:**
```toml
coqui-stt = "0.1"
```

**Висновок**: Варто протестувати як альтернативу, особливо для української мови через Common Voice підтримку.

#### Option E2: Mozilla DeepSpeech (Archived)
**Статус**: Проект заархівований (archived March 2022)

**Rust bindings:**
- `deepspeech-rs` (RustAudio) - заархівований
- `deepspeech-sys` - низькорівневі FFI bindings
- `ds-transcriber` - високорівнева обгортка

**Висновок**: Не рекомендовано - проект не підтримується, краще використовувати Coqui STT (форк DeepSpeech).

### 3. Hybrid Approaches

#### Option F: Adaptive Model Selection
**Підхід**: Використовувати різні моделі залежно від складності аудіо.

**Стратегія:**
1. Спочатку спробувати швидку модель (tiny/base quantized)
2. Якщо впевненість низька або помилки - перетранскрибувати з великою моделлю
3. Або: використовувати швидку модель для real-time preview, велику для фінального результату

**Переваги:**
- ✅ Швидкий feedback для користувача
- ✅ Висока якість фінального результату
- ✅ Економія ресурсів для простого аудіо

**Недоліки:**
- ⚠️ Складніша логіка
- ⚠️ Може потребувати дві транскрипції

#### Option G: Streaming Transcription
**Підхід**: Транскрибувати в real-time під час запису.

**Техніка:**
- Обробляти аудіо сегментами (наприклад, по 5 секунд)
- Показувати результат поступово
- Користувач бачить результат швидше

**Переваги:**
- ✅ Швидший feedback
- ✅ Менше очікування
- ✅ Кращий UX

**Недоліки:**
- ⚠️ Може бути менш точним (контекст обмежений)
- ⚠️ Складніша реалізація
- ⚠️ Потребує streaming support в моделі

## Decision Options

### Option 1: Quantized Whisper Models (Recommended for MVP)
**Підхід**: Використати квантовані моделі Whisper (Q5_0 або Q8_0).

**Реалізація:**
- Завантажити квантовані моделі з HuggingFace
- Використати `base-q5_0` або `small-q5_0` замість звичайних
- Мінімальні зміни в коді (whisper-rs підтримує автоматично)

**Переваги:**
- ✅ Найпростіша реалізація
- ✅ 2-3x швидше
- ✅ Менший розмір моделей
- ✅ Мінімальна втрата якості (Q5_0)

**Недоліки:**
- ⚠️ Все ще може бути повільно для великих моделей
- ⚠️ Дещо нижча якість ніж оригінал

### Option 2: faster-whisper Integration
**Підхід**: Інтегрувати faster-whisper через Rust bindings.

**Реалізація:**
- Використати `faster-whisper-rs` або `ctranslate2-rs`
- Конвертувати моделі в CTranslate2 формат
- Замінити whisper-rs на faster-whisper

**Переваги:**
- ✅ До 4x швидше
- ✅ Підтримує quantization
- ✅ Активно розвивається

**Недоліки:**
- ⚠️ Потребує Python (faster-whisper-rs) або більше налаштування (ctranslate2-rs)
- ⚠️ Потрібна конвертація моделей
- ⚠️ Більше змін в коді

### Option 3: whisper.apr (Pure Rust)
**Підхід**: Перейти на whisper.apr.

**Реалізація:**
- Замінити whisper-rs на whisper-apr
- Конвертувати моделі в .apr формат
- Використати streaming transcription

**Переваги:**
- ✅ Pure Rust
- ✅ Streaming support
- ✅ Оптимізований для CPU

**Недоліки:**
- ⚠️ Потребує конвертації моделей
- ⚠️ Менш зрілий проект
- ⚠️ Великі зміни в коді

### Option 5: Coqui STT (Mozilla DeepSpeech)
**Підхід**: Перейти на Coqui STT замість Whisper.

**Реалізація:**
- Замінити whisper-rs на coqui-stt
- Завантажити Coqui STT моделі (включно з українською)
- Використати streaming transcription

**Переваги:**
- ✅ Може бути швидший на CPU
- ✅ Менші моделі
- ✅ Streaming support
- ✅ Підтримка української через Common Voice
- ✅ Rust bindings доступні

**Недоліки:**
- ⚠️ Менш активно підтримується
- ⚠️ Може бути нижча якість
- ⚠️ Потрібні окремі моделі
- ⚠️ Немає бенчмарків порівняння

**Висновок**: Варто протестувати, особливо для української мови.

### Option 4: Hybrid Approach (Recommended for Production)
**Підхід**: Комбінувати квантовані моделі з adaptive selection.

**Стратегія:**
1. Використовувати `base-q5_0` за замовчуванням (швидко + достатньо якісно)
2. Додати опцію для вибору моделі (tiny/base/small/medium)
3. Можливість автоматичного вибору на основі довжини запису

**Реалізація:**
```rust
pub enum ModelSize {
    Tiny,    // Для швидких тестів
    Base,    // Баланс швидкості/якості (рекомендовано)
    Small,   // Краща якість
    Medium,  // Найкраща якість (повільно)
}

pub struct AdaptiveSTT {
    current_model: ModelSize,
    // Можна мати кілька моделей завантажених
}
```

**Переваги:**
- ✅ Гнучкість для користувача
- ✅ Баланс швидкості/якості
- ✅ Можна почати з простої реалізації

**Недоліки:**
- ⚠️ Потрібна підтримка кількох моделей
- ⚠️ Більше коду

## Recommended Decision

**Для MVP (Phase 1):**
**Option 1** - Quantized Whisper Models

**Обґрунтування:**
- Найпростіша реалізація
- Мінімальні зміни в коді
- 2-3x прискорення
- Достатньо для початку

**Конкретні кроки:**
1. Завантажити `ggml-base-q5_0.bin` замість `ggml-base.bin`
2. Тестувати якість на українській мові
3. Якщо якість достатня - використовувати як default
4. Додати опцію для вибору моделі в налаштуваннях

**Для Production (Phase 2):**
**Option 4** - Hybrid Approach з квантованими моделями

**Обґрунтування:**
- Гнучкість для користувача
- Можна вибрати баланс швидкості/якості
- Можна додати faster-whisper пізніше як опцію

## Implementation Plan

### Phase 1: Quantized Models
1. Додати підтримку квантованих моделей в `models.rs`
2. Завантажити `base-q5_0` та `small-q5_0` моделі
3. Додати вибір моделі в налаштуваннях
4. Тестувати якість на українській мові

### Phase 2: Performance Testing
1. Бенчмарки різних моделей (tiny/base/small quantized)
2. Порівняння якості на тестовому наборі
3. Визначити оптимальну модель за замовчуванням

### Phase 3: Advanced Optimization (Optional)
1. Дослідити faster-whisper інтеграцію
2. Додати як опцію для advanced users
3. Або перейти на whisper.apr якщо стане зрілим

## Testing Requirements

1. **Якість транскрипції**: Порівняти різні моделі на українській мові
2. **Швидкість**: Бенчмарки часу обробки для різних довжин аудіо
3. **Розмір моделей**: Порівняти розміри файлів
4. **CPU usage**: Моніторинг використання ресурсів

## Consequences

### Positive
- ✅ Швидша обробка
- ✅ Менший розмір моделей
- ✅ Кращий UX (менше очікування)
- ✅ Гнучкість в обранні моделі

### Negative
- ⚠️ Можлива втрата якості (особливо для складних випадків)
- ⚠️ Потрібне тестування на різних типах аудіо
- ⚠️ Потрібна підтримка кількох моделей

## Related Files
- [src/whisper.rs](../../src/whisper.rs) - Current Whisper integration
- [src/models.rs](../../src/models.rs) - Model management
- [src/config.rs](../../src/config.rs) - Configuration (можна додати вибір моделі)

## Additional Research
- [STT Optimization Test Plan](../research/stt-optimization-test.md) - Тестування та бенчмарки

## References
1. [Whisper Quantization Blog](https://dropbox.github.io/whisper-static-cache-blog/)
2. [faster-whisper-rs on crates.io](https://crates.io/crates/faster-whisper-rs)
3. [whisper.apr on lib.rs](https://lib.rs/crates/whisper-apr)
4. [whisper.cpp Quantization](https://github.com/ggerganov/whisper.cpp#quantization)
5. [coqui-stt crate](https://crates.io/crates/coqui-stt)
6. [Coqui STT Models](https://coqui.ai/models)
7. [Mozilla DeepSpeech (archived)](https://github.com/mozilla/DeepSpeech)
