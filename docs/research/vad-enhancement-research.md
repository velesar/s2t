# VAD Enhancement Research - Silero VAD + nnnoiseless

**Дата:** 2026-01-29
**Статус:** In Progress
**Гіпотеза:** Заміна WebRTC VAD на Silero VAD та додавання nnnoiseless для denoising значно покращить якість детекції мовлення.

## Зміст

1. [Поточна реалізація](#поточна-реалізація)
2. [Проблеми WebRTC VAD](#проблеми-webrtc-vad)
3. [Silero VAD](#silero-vad)
4. [nnnoiseless (RNNoise)](#nnnoiseless-rnnoise)
5. [Semantic VAD концепція](#semantic-vad-концепція)
6. [Архітектура покращення](#архітектура-покращення)
7. [Rust crates та інтеграція](#rust-crates-та-інтеграція)
8. [План імплементації](#план-імплементації)

---

## Поточна реалізація

### Поточний стек (src/vad.rs)

```rust
// Поточна реалізація використовує webrtc_vad
use webrtc_vad::{Vad, VadMode};

pub struct VoiceActivityDetector {
    vad: RefCell<Vad>,
    silence_threshold_ms: u32,
}

// Налаштування:
// - Sample rate: 16kHz
// - Frame size: 30ms (480 samples)
// - Mode: VadMode::Aggressive
```

### Як використовується (src/continuous.rs)

```rust
// VAD створюється в окремому thread
let vad = VoiceActivityDetector::with_thresholds(
    vad_silence_threshold_ms,  // default: 1000ms
    vad_min_speech_ms          // default: 500ms
);

// Перевірка кожні 500ms
let speech_now = vad.is_speech(&recent_samples)?;
let should_segment = vad.detect_speech_end(&samples)?;
```

### Trait interface (src/traits.rs)

```rust
pub trait VoiceDetection {
    fn is_speech(&self, samples: &[f32]) -> Result<bool>;
    fn detect_speech_end(&self, samples: &[f32]) -> Result<bool>;
    fn reset(&self);
}
```

---

## Проблеми WebRTC VAD

### Бенчмарки точності

| Метрика | WebRTC VAD | Silero VAD |
|---------|------------|------------|
| TPR @ 5% FPR | 50% | 87.7% |
| TPR @ 1% FPR | <20% | 80.4% |
| Помилок у 4x більше | ✓ | baseline |

**Джерело:** [Picovoice VAD Benchmark](https://picovoice.ai/docs/benchmark/vad/)

### Відомі проблеми

1. **Noise vs Speech confusion**
   - WebRTC добре відділяє silence від noise
   - Погано відділяє speech від noise
   - Багато false positives на фоновому шумі

2. **GMM-based limitations**
   - Gaussian Mixture Model - стара технологія
   - Не враховує контекст
   - Фіксовані features, не learned

3. **Binary output**
   - Тільки true/false
   - Немає probability score
   - Складно налаштувати threshold

### Симптоми в s2t

- Передчасне завершення сегментів при паузах у мовленні
- Пропуск тихого мовлення
- False triggers від keyboard/mouse clicks
- Погана робота з фоновою музикою/TV

---

## Silero VAD

### Огляд

**Silero VAD** - нейромережевий VAD на базі DNN, тренований на 6000+ мовах.

| Характеристика | Значення |
|----------------|----------|
| Модель | ONNX (2 MB) |
| Точність | 87.7% TPR @ 5% FPR |
| Швидкість | < 1ms на CPU (30ms chunk) |
| Sample rates | 8kHz, 16kHz |
| Мови | 6000+ (universal) |
| Ліцензія | MIT |

### Ключові переваги

1. **Probability output** (0.0 - 1.0)
   ```rust
   let prob: f32 = vad.predict(samples)?;
   if prob > 0.5 { /* speech detected */ }
   ```

2. **Context-aware** - враховує попередні frames

3. **Noise robust** - тренований на noisy data

4. **Streaming ready** - підтримує chunk-by-chunk processing

### Порівняння архітектур

```
WebRTC VAD (GMM):
┌─────────────┐    ┌─────────┐    ┌────────────┐
│ Audio Frame │ → │ Features│ → │ GMM Model │ → true/false
└─────────────┘    │ (fixed) │    │ (static)  │
                   └─────────┘    └────────────┘

Silero VAD (DNN):
┌─────────────┐    ┌─────────┐    ┌────────────┐    ┌──────────┐
│ Audio Frame │ → │ Features│ → │ DNN Model │ → │Probability│
└─────────────┘    │(learned)│    │ (ONNX)    │    │ 0.0-1.0  │
                   └─────────┘    └────────────┘    └──────────┘
                                       ↑
                             Hidden state (context)
```

---

## nnnoiseless (RNNoise)

### Огляд

**nnnoiseless** - Rust port of RNNoise для noise suppression.

| Характеристика | Значення |
|----------------|----------|
| Розмір | 135 KB crate |
| Швидкість | Real-time на CPU |
| Sample rate | 48kHz (native) |
| Frame size | 10ms |
| Ліцензія | BSD-3-Clause |

### Двойна функціональність

RNNoise надає **дві** функції одночасно:

1. **Noise suppression** - видалення фонового шуму
2. **VAD probability** - ймовірність мовлення

```rust
// RNNoise внутрішньо має VAD модуль
// Можна отримати VAD probability разом з denoised audio
struct RNNoiseOutput {
    denoised_samples: Vec<f32>,
    vad_probability: f32,  // 0.0 - 1.0
}
```

### Архітектура RNNoise

```
┌─────────────────────────────────────────────────────────────┐
│                     RNNoise Architecture                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Input Audio Frame (10ms @ 48kHz)                          │
│         │                                                   │
│         ▼                                                   │
│  ┌─────────────────┐                                       │
│  │ Feature Extract │  22 Bark-scale bands                  │
│  │ (Spectral)      │                                       │
│  └────────┬────────┘                                       │
│           │                                                 │
│           ▼                                                 │
│  ┌─────────────────────────────────────────┐               │
│  │              GRU Network                 │               │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐ │               │
│  │  │VAD Unit │  │ Noise   │  │ Gain    │ │               │
│  │  │         │  │Estimator│  │ Output  │ │               │
│  │  └────┬────┘  └────┬────┘  └────┬────┘ │               │
│  └───────┼────────────┼────────────┼──────┘               │
│          │            │            │                       │
│          ▼            ▼            ▼                       │
│    VAD Prob      Noise Est     22 Gains                   │
│    (0.0-1.0)                   (per band)                 │
│                                    │                       │
│                                    ▼                       │
│                           Denoised Audio                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### nnnoiseless API

```rust
use nnnoiseless::DenoiseState;

// Створення state
let mut state = DenoiseState::new();

// Обробка frame (480 samples @ 48kHz = 10ms)
let mut output = [0.0f32; 480];
state.process_frame(&mut output, &input);

// Примітка: поточний nnnoiseless API НЕ експортує VAD probability
// Потрібно або модифікувати crate, або використовувати окремий VAD
```

---

## Semantic VAD концепція

### Що таке Semantic VAD

**Semantic VAD** - VAD, що розуміє семантику мовлення:
- Розрізняє паузу всередині речення від кінця речення
- Використовує punctuation prediction
- Менша затримка при сегментації

### Традиційний VAD vs Semantic VAD

```
Традиційний VAD:
"Hello, my name is... [700ms silence] ...John"
                      ↑
              Split тут (неправильно!)

Semantic VAD:
"Hello, my name is... [700ms silence] ...John."
                                              ↑
                                    Split тут (правильно!)
```

### Як Kyutai реалізує Semantic VAD

```
┌─────────────────────────────────────────────┐
│          Kyutai Semantic VAD                │
├─────────────────────────────────────────────┤
│                                             │
│  Audio → Mimi Codec → Tokens                │
│                          │                  │
│                          ▼                  │
│              ┌───────────────────┐          │
│              │ Transformer       │          │
│              │ + Punctuation     │          │
│              │   Prediction      │          │
│              └─────────┬─────────┘          │
│                        │                    │
│                        ▼                    │
│              ┌───────────────────┐          │
│              │ VAD Classifier    │          │
│              │ - Speech          │          │
│              │ - Silence         │          │
│              │ - Endpoint (!)    │ ← New!   │
│              └───────────────────┘          │
│                                             │
└─────────────────────────────────────────────┘
```

### Результати

| Метрика | Traditional VAD | Semantic VAD |
|---------|-----------------|--------------|
| Avg latency | 700ms | 327ms |
| CER degradation | baseline | мінімальна |
| Reduction | - | 53.3% |

**Джерело:** [Semantic VAD Paper (Interspeech 2023)](https://www.isca-archive.org/interspeech_2023/shi23c_interspeech.pdf)

### Застосування для s2t

Повноцінний Semantic VAD потребує:
- ASR модель (Whisper)
- Punctuation prediction
- Більше обчислень

**Спрощений підхід для s2t:**
1. Використовувати Silero VAD probability
2. Враховувати "мовленнєвий контекст" (чи була пауза раніше)
3. Адаптивні thresholds

---

## Архітектура покращення

### Поточна архітектура

```
┌─────────────────────────────────────────────────────────────┐
│                    CURRENT PIPELINE                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Microphone → AudioRecorder → RingBuffer                   │
│                                   │                         │
│                                   ▼                         │
│                          ┌─────────────────┐               │
│                          │   WebRTC VAD    │               │
│                          │   (30ms frames) │               │
│                          └────────┬────────┘               │
│                                   │                         │
│                                   ▼                         │
│                          true/false decision                │
│                                   │                         │
│                                   ▼                         │
│                          Segment extraction                 │
│                                   │                         │
│                                   ▼                         │
│                             Whisper STT                     │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Запропонована архітектура (Варіант A - Silero Only)

```
┌─────────────────────────────────────────────────────────────┐
│                    IMPROVED PIPELINE (A)                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Microphone → AudioRecorder → RingBuffer                   │
│                                   │                         │
│                                   ▼                         │
│                          ┌─────────────────┐               │
│                          │   Silero VAD    │               │
│                          │   (512 samples  │               │
│                          │    @ 16kHz)     │               │
│                          └────────┬────────┘               │
│                                   │                         │
│                                   ▼                         │
│                          probability (0.0-1.0)              │
│                                   │                         │
│                                   ▼                         │
│                    ┌──────────────────────────┐            │
│                    │  Smart Segmentation      │            │
│                    │  - Adaptive threshold    │            │
│                    │  - Hysteresis            │            │
│                    │  - Min/max duration      │            │
│                    └─────────────┬────────────┘            │
│                                  │                          │
│                                  ▼                          │
│                            Whisper STT                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Запропонована архітектура (Варіант B - Denoise + VAD)

```
┌─────────────────────────────────────────────────────────────┐
│                    IMPROVED PIPELINE (B)                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Microphone → AudioRecorder                                │
│                    │                                        │
│                    ▼                                        │
│           ┌─────────────────┐                              │
│           │   nnnoiseless   │  ← Noise suppression         │
│           │   (Resample to  │                              │
│           │    48kHz first) │                              │
│           └────────┬────────┘                              │
│                    │                                        │
│                    ▼ (denoised audio)                      │
│           ┌─────────────────┐                              │
│           │   Resample to   │                              │
│           │     16kHz       │                              │
│           └────────┬────────┘                              │
│                    │                                        │
│            ┌───────┴───────┐                               │
│            │               │                                │
│            ▼               ▼                                │
│      RingBuffer      Silero VAD                            │
│            │               │                                │
│            │               ▼                                │
│            │         probability                            │
│            │               │                                │
│            └───────┬───────┘                               │
│                    │                                        │
│                    ▼                                        │
│          Smart Segmentation                                │
│                    │                                        │
│                    ▼                                        │
│              Whisper STT                                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Порівняння варіантів

| Аспект | A (Silero only) | B (Denoise + VAD) |
|--------|-----------------|-------------------|
| Складність | Низька | Середня |
| CPU usage | +~5% | +~15% |
| Якість VAD | Значно краще | Ще краще |
| Якість STT | Без змін | Покращена |
| Latency | +~5ms | +~15ms |
| Dependencies | +1 crate | +2 crates |

**Рекомендація:** Почати з варіанту A, потім додати B за потреби.

---

## Rust crates та інтеграція

### Silero VAD Crates

| Crate | Version | Notes |
|-------|---------|-------|
| `voice_activity_detector` | 0.2.1 | Silero V5, простий API |
| `silero-vad-rs` | latest | Streaming support, GPU |
| `silero-vad-rust` | 6.2.0 | Bundled ONNX models |
| `vad-silero-rs` | latest | Lightweight |

**Рекомендація:** `voice_activity_detector` - найпростіша інтеграція

### Приклад інтеграції Silero VAD

```rust
use voice_activity_detector::{VoiceActivityDetector, LabeledAudio};

// Створення VAD
let vad = VoiceActivityDetector::builder()
    .sample_rate(16000)
    .chunk_size(512)  // ~32ms @ 16kHz
    .build()?;

// Отримання probability
let probability: f32 = vad.predict(&samples)?;

// Або з labels
let labeled: LabeledAudio = vad.label_audio(
    samples,
    0.5,   // threshold
    0.2,   // negative_speech_threshold
    100,   // redemption_frames (hysteresis)
    50,    // min_speech_frames
    5,     // pre_speech_padding
    5      // post_speech_padding
)?;
```

### nnnoiseless Integration

```rust
use nnnoiseless::DenoiseState;

// Denoise працює на 48kHz
const DENOISE_SAMPLE_RATE: u32 = 48000;
const FRAME_SIZE: usize = 480;  // 10ms @ 48kHz

let mut denoise = DenoiseState::new();

fn process_audio(input: &[f32]) -> Vec<f32> {
    let mut output = vec![0.0f32; input.len()];

    for (in_chunk, out_chunk) in input.chunks(FRAME_SIZE)
                                      .zip(output.chunks_mut(FRAME_SIZE)) {
        if in_chunk.len() == FRAME_SIZE {
            denoise.process_frame(out_chunk, in_chunk);
        }
    }

    output
}
```

### Sample Rate Conversion

```
Audio Pipeline з різними sample rates:

Microphone (device rate, e.g. 44.1kHz)
    │
    ▼ Resample (rubato)
    │
    ├──→ 48kHz → nnnoiseless → denoised
    │                            │
    │                            ▼ Resample
    │                          16kHz
    │                            │
    └──────────────────────────→├──→ Silero VAD (16kHz)
                                │
                                └──→ Whisper (16kHz)
```

### Новий VoiceDetection implementation

```rust
use voice_activity_detector::VoiceActivityDetector as SileroVad;

pub struct EnhancedVAD {
    silero: SileroVad,
    threshold: f32,
    hysteresis: HysteresisState,
    config: VadConfig,
}

pub struct VadConfig {
    pub speech_threshold: f32,      // 0.5 default
    pub silence_threshold: f32,     // 0.3 default
    pub min_speech_ms: u32,         // 200ms
    pub min_silence_ms: u32,        // 500ms
    pub pre_padding_ms: u32,        // 100ms
    pub post_padding_ms: u32,       // 200ms
}

impl VoiceDetection for EnhancedVAD {
    fn is_speech(&self, samples: &[f32]) -> Result<bool> {
        let prob = self.silero.predict(samples)?;
        Ok(self.hysteresis.update(prob, self.config.speech_threshold))
    }

    fn detect_speech_end(&self, samples: &[f32]) -> Result<bool> {
        // Використовувати probability + hysteresis
        // для визначення кінця мовлення
    }

    fn reset(&self) {
        self.hysteresis.reset();
    }
}
```

---

## План імплементації

### Phase 1: Silero VAD Integration (Варіант A)

**Тривалість:** 1-2 дні

1. Додати dependency:
   ```toml
   [dependencies]
   voice_activity_detector = "0.2"
   ```

2. Створити `src/vad_silero.rs`:
   - Implement `VoiceDetection` trait
   - Додати hysteresis logic
   - Конфігурація через config.toml

3. Оновити `src/continuous.rs`:
   - Використовувати нову VAD реалізацію
   - Зберегти fallback на WebRTC

4. Тестування:
   - Unit tests
   - Порівняння з WebRTC на тестових записах

### Phase 2: nnnoiseless Integration (Варіант B)

**Тривалість:** 2-3 дні

1. Додати dependency:
   ```toml
   [dependencies]
   nnnoiseless = "0.5"
   ```

2. Створити `src/denoise.rs`:
   - Wrapper для nnnoiseless
   - Sample rate conversion (rubato)

3. Інтегрувати в audio pipeline:
   - `AudioRecorder` → `Denoiser` → `RingBuffer`

4. Тестування:
   - Бенчмарки CPU usage
   - A/B тест якості STT

### Phase 3: Config & UI

1. Додати налаштування в config.toml:
   ```toml
   [vad]
   engine = "silero"  # або "webrtc"
   threshold = 0.5
   denoise = true
   ```

2. UI індикатор probability (optional)

### Checklist

- [ ] Phase 1: Silero VAD integration
  - [ ] Add voice_activity_detector crate
  - [ ] Implement SileroVAD struct
  - [ ] Implement VoiceDetection trait
  - [ ] Add hysteresis logic
  - [ ] Update config.toml schema
  - [ ] Unit tests
  - [ ] Integration with continuous.rs

- [ ] Phase 2: nnnoiseless integration
  - [ ] Add nnnoiseless crate
  - [ ] Create Denoiser wrapper
  - [ ] Sample rate conversion
  - [ ] Pipeline integration
  - [ ] Benchmark CPU usage

- [ ] Phase 3: Configuration & Polish
  - [ ] Config options for VAD engine
  - [ ] Fallback mechanism
  - [ ] Documentation

---

## Джерела

- [Silero VAD GitHub](https://github.com/snakers4/silero-vad)
- [Silero VAD Quality Metrics](https://github.com/snakers4/silero-vad/wiki/Quality-Metrics)
- [voice_activity_detector crate](https://docs.rs/voice_activity_detector)
- [silero-vad-rs crate](https://docs.rs/silero-vad-rs)
- [nnnoiseless crate](https://docs.rs/nnnoiseless)
- [RNNoise Demo](https://jmvalin.ca/demo/rnnoise/)
- [Semantic VAD Paper](https://www.isca-archive.org/interspeech_2023/shi23c_interspeech.pdf)
- [WebRTC VAD vs Silero Benchmark](https://picovoice.ai/docs/benchmark/vad/)
- [kalosm-sound (denoise + VAD combo)](https://docs.rs/kalosm-sound)
