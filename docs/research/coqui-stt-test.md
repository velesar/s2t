# Coqui STT (Mozilla DeepSpeech) - Тестування

## Мета
Протестувати Coqui STT як альтернативу Whisper, особливо для української мови.

## Інформація про Coqui STT

**Що це:**
- Форк Mozilla DeepSpeech (тепер підтримується Coqui)
- Оптимізований для edge devices
- Підтримує багато мов через Common Voice

**Rust bindings:**
- `coqui-stt` crate (0.1.0)
- Автор: tazz4843 (той самий що whisper-rs)
- Останнє оновлення: липень 2022

**Моделі:**
- Доступні на https://coqui.ai/models
- Українська модель: `coqui-stt-uk-v0.9.0` (якщо доступна)
- Розміри: зазвичай менші ніж Whisper

## Тестові сценарії

### 1. Порівняння з Whisper

**Тестові записи:**
- Короткий запис (10 секунд) українською
- Середній запис (1 хвилина) українською
- Запис з шумом
- Запис з акцентом

**Метрики:**
- [ ] Час обробки (секунди)
- [ ] Real-time factor (RTF)
- [ ] Word Error Rate (WER)
- [ ] Розмір моделі
- [ ] CPU usage
- [ ] Пам'ять

### 2. Якість транскрипції

**Тестовий набір:**
- 10 записів українською різної складності
- Reference transcripts

**Порівняння:**
- Whisper base vs Coqui STT
- Whisper base-q5_0 vs Coqui STT
- Якість на різних типах аудіо

### 3. Швидкість обробки

**Бенчмарки:**
```rust
use coqui_stt::Model;

fn benchmark_coqui(model_path: &str, audio: &[f32]) -> f64 {
    let model = Model::new(model_path).unwrap();
    
    let start = Instant::now();
    let result = model.speech_to_text(audio).unwrap();
    let duration = start.elapsed();
    
    // Calculate RTF
    // ...
}
```

**Очікувані результати:**
- Coqui STT може бути швидший на CPU
- Менші моделі = швидша обробка
- Streaming support для real-time

### 4. Підтримка української мови

**Перевірка:**
- [ ] Чи є українська модель доступна?
- [ ] Якість на українській мові
- [ ] Порівняння з Whisper для української

**Моделі для тестування:**
- `coqui-stt-uk-v0.9.0` (якщо доступна)
- Або загальна багатомовна модель

## Інтеграція

### Додавання залежності

```toml
# Cargo.toml
[dependencies]
coqui-stt = "0.1"
```

### Базовий приклад використання

```rust
use coqui_stt::Model;

pub struct CoquiSTT {
    model: Model,
}

impl CoquiSTT {
    pub fn new(model_path: &str) -> Result<Self> {
        let model = Model::new(model_path)?;
        Ok(Self { model })
    }
    
    pub fn transcribe(&self, samples: &[f32]) -> Result<String> {
        // Coqui STT потребує 16kHz mono audio
        let text = self.model.speech_to_text(samples)?;
        Ok(text)
    }
}
```

### Адаптер для сумісності

```rust
// Створити trait для абстракції
pub trait SpeechToText {
    fn transcribe(&self, samples: &[f32], language: Option<&str>) -> Result<String>;
}

impl SpeechToText for WhisperSTT {
    // ...
}

impl SpeechToText for CoquiSTT {
    // ...
}
```

## Порівняльна таблиця (після тестування)

| Критерій | Whisper base | Whisper base-q5_0 | Coqui STT |
|----------|--------------|-------------------|-----------|
| Швидкість (RTF) | | | |
| Якість (WER) | | | |
| Розмір моделі | 150MB | ~80MB | ? |
| Підтримка української | ✅ | ✅ | ? |
| Streaming | ❌ | ❌ | ✅ |
| Активна підтримка | ✅ | ✅ | ⚠️ |

## Результати тестування

### Система: [Заповнити]
- CPU:
- RAM:
- OS:

### Результати:

| Модель | RTF | WER | Розмір | CPU % | Пам'ять |
|--------|-----|-----|--------|-------|---------|
| Whisper base | | | | | |
| Whisper base-q5_0 | | | | | |
| Coqui STT | | | | | |

### Висновки:
[Заповнити після тестування]

## Рекомендації

### Якщо Coqui STT швидший і достатньо якісний:
- Додати як альтернативу до Whisper
- Дозволити користувачу вибрати STT engine
- Використати для real-time transcription

### Якщо Coqui STT недостатньо якісний:
- Залишити Whisper як основну систему
- Можливо використати Coqui для streaming preview

### Якщо Coqui STT краще для української:
- Рекомендувати Coqui для української мови
- Whisper для інших мов

## Наступні кроки

1. Встановити `coqui-stt` crate
2. Завантажити українську модель (якщо доступна)
3. Запустити бенчмарки
4. Порівняти з Whisper
5. Оновити ADR-005 з результатами

## Посилання

- [Coqui STT Models](https://coqui.ai/models)
- [coqui-stt crate](https://crates.io/crates/coqui-stt)
- [Coqui STT Documentation](https://stt.readthedocs.io/)
