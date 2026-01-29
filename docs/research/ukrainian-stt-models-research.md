# Українські STT моделі - Глибоке дослідження

**Дата:** 2026-01-29
**Статус:** Complete
**Джерело:** [egorsmkv/speech-recognition-uk](https://github.com/egorsmkv/speech-recognition-uk)

## Зміст

1. [Огляд екосистеми](#огляд-екосистеми)
2. [Порівняльна таблиця моделей](#порівняльна-таблиця-моделей)
3. [Детальний аналіз моделей](#детальний-аналіз-моделей)
4. [Сумісність з whisper-rs](#сумісність-з-whisper-rs)
5. [Рекомендації для s2t](#рекомендації-для-s2t)
6. [План інтеграції](#план-інтеграції)
7. [NeMo Implementation Notes](#nemo-implementation-notes)
8. [W2V-BERT v1 через Candle](#w2v-bert-v1-через-candle---дослідження-доцільності)
9. [Вимоги до апаратного забезпечення](#вимоги-до-апаратного-забезпечення)

---

## Огляд екосистеми

Репозиторій [speech-recognition-uk](https://github.com/egorsmkv/speech-recognition-uk) збирає ресурси для українського STT. Основні архітектури:

| Архітектура | Представники | Характеристика |
|-------------|--------------|----------------|
| **Wav2Vec2-BERT** | w2v-bert-uk, w2v-bert-uk-v2.1 | Найкраща якість, великі моделі |
| **FastConformer** | NVIDIA, theodotus | Пунктуація, streaming |
| **Citrinet** | NVIDIA | Streaming, transfer learning |
| **Whisper** | OpenAI + fine-tuned | Універсальні, whisper.cpp |
| **Moonshine** | UsefulSensors | Легкі, edge devices |
| **VOSK** | Yehor | Офлайн, легкі (archived) |

### Ключові ресурси

- **Hugging Face Hub:** [speech-uk](https://huggingface.co/speech-uk) - колекція моделей
- **Датасети:** Common Voice 10-17, openstt-uk (~1200 годин)
- **Спільнота:** [Telegram](https://t.me/speech_recognition_uk)

---

## Порівняльна таблиця моделей

### Бенчмарк на Common Voice 10 (test set)

| Модель | WER | CER | Параметри | Формат | Ліцензія |
|--------|-----|-----|-----------|--------|----------|
| **NVIDIA Citrinet-1024** | 3.52-5.02% | - | 141M | NeMo | CC-BY-4.0 |
| **FastConformer Hybrid** | 4-7.1% | - | 120M | NeMo | MIT |
| **Parakeet TDT v3** | 5.1-6.8%* | - | 600M | ONNX | CC-BY-4.0 |
| **W2V-BERT uk v1** | 6.6% | 1.34% | 600M | Transformers | Apache 2.0 |
| **Whisper large-v2 uk** | ~13.72% | - | 1.5B | GGUF | Apache 2.0 |
| **W2V-BERT uk v2.1** | 17.34% | 3.33% | 600M | Transformers | Apache 2.0 |
| **Moonshine tiny uk** | 18.25%** | - | 27M | Transformers | MIT |
| **Whisper small uk** | 27% | - | 244M | GGUF | Apache 2.0 |
| **Whisper base (vanilla)** | 35-40% | - | 74M | GGUF | MIT |
| **VOSK uk** | - | - | 73-345MB | Kaldi | Apache 2.0 |

*Parakeet TDT v3 WER: 5.10% (CoVoST), 6.79% (FLEURS)
**Moonshine WER на FLEURS; на CV17: 26.11%

### Візуалізація WER vs Розмір

```
WER (%)
  40 ┤
     │ ○ Whisper base (vanilla)
  35 ┤
     │
  30 ┤
     │     ○ Whisper small uk
  25 ┤        ○ Moonshine tiny uk (CV17)
     │
  20 ┤           ○ Moonshine tiny uk (FLEURS)
     │              ○ W2V-BERT v2.1
  15 ┤
     │                 ○ Whisper large-v2 uk
  10 ┤
     │
   5 ┤                       ○ FastConformer
     │                    ○ Citrinet  ○ W2V-BERT v1
   0 ┼──────────────────────────────────────────────
     27M   74M  120M  244M  600M  1.5B
                    Parameters
```

---

## Детальний аналіз моделей

### 1. Wav2Vec2-BERT (Найкраща якість)

#### [Yehor/w2v-bert-uk](https://huggingface.co/Yehor/w2v-bert-uk) (v1)

| Характеристика | Значення |
|----------------|----------|
| WER | 6.6% |
| CER | 1.34% |
| Word Accuracy | 93.4% |
| Параметри | 600M |
| Sample Rate | 16 kHz |
| Base Model | facebook/w2v-bert-2.0 |
| Training Data | Common Voice 10.0 |

**Використання:**
```python
from transformers import AutoModelForCTC, Wav2Vec2BertProcessor

model = AutoModelForCTC.from_pretrained('Yehor/w2v-bert-uk')
processor = Wav2Vec2BertProcessor.from_pretrained('Yehor/w2v-bert-uk')
```

**Переваги:**
- Найкращий WER серед доступних моделей
- Добре працює з чистим мовленням

**Недоліки:**
- Великий розмір (600M)
- Потребує Transformers (не whisper-rs)
- Немає пунктуації

#### [Yehor/w2v-bert-uk-v2.1](https://huggingface.co/Yehor/w2v-bert-uk-v2.1)

| Характеристика | Значення |
|----------------|----------|
| WER | 17.34% |
| CER | 3.33% |
| Training Data | openstt-uk (більший датасет) |

**Примітка:** Вищий WER через більш різноманітний training data (openstt-uk). Краще генералізує на реальних даних.

---

### 2. FastConformer (Пунктуація + Streaming)

#### [theodotus/stt_ua_fastconformer_hybrid_large_pc](https://huggingface.co/theodotus/stt_ua_fastconformer_hybrid_large_pc)

| Характеристика | Значення |
|----------------|----------|
| WER | 7.1% |
| Параметри | 120M |
| Архітектура | FastConformer Hybrid (Transducer-CTC) |
| Features | Punctuation & Capitalization |

**Використання (NeMo):**
```python
import nemo.collections.asr as nemo_asr

model = nemo_asr.models.EncDecHybridRNNTCTCModel.from_pretrained(
    "theodotus/stt_ua_fastconformer_hybrid_large_pc"
)
```

**Переваги:**
- Автоматична пунктуація та капіталізація
- Streaming-ready
- Менший розмір ніж W2V-BERT

**Недоліки:**
- Потребує NeMo framework
- Не сумісний з whisper-rs

---

### 3. NVIDIA Citrinet (Найнижчий WER)

#### [nvidia/stt_uk_citrinet_1024_gamma_0_25](https://huggingface.co/nvidia/stt_uk_citrinet_1024_gamma_0_25)

| Характеристика | Значення |
|----------------|----------|
| WER (CV10) | 5.02% |
| WER (CV9) | 3.75% |
| WER (CV8) | 3.52% |
| Параметри | 141M |
| Архітектура | Streaming Citrinet-1024 (CTC) |
| Transfer | Fine-tuned from Russian model |

**Переваги:**
- Найнижчий WER
- Streaming support
- NVIDIA Riva compatible

**Недоліки:**
- Потребує NeMo
- Transfer from Russian може впливати на акцент
- Тільки lowercase output

---

### 4. Whisper Fine-tuned (Сумісний з whisper-rs)

#### [Yehor/whisper-small-ukrainian](https://huggingface.co/Yehor/whisper-small-ukrainian)

| Характеристика | Значення |
|----------------|----------|
| WER | 27% |
| Base Model | openai/whisper-small |
| Training | Common Voice + custom data |

#### [mitchelldehaven/whisper-large-v2-uk](https://huggingface.co/mitchelldehaven/whisper-large-v2-uk)

| Характеристика | Значення |
|----------------|----------|
| WER | ~13.72% |
| Base Model | openai/whisper-large-v2 |

#### [Yehor/whisper-large-v3-turbo-quantized-uk](https://huggingface.co/Yehor/whisper-large-v3-turbo-quantized-uk)

| Характеристика | Значення |
|----------------|----------|
| Параметри | 0.2B (quantized) |
| Base Model | openai/whisper-large-v3-turbo |
| Format | vLLM optimized |

**Конвертація для whisper.cpp:**
```bash
# Потрібно конвертувати в GGUF/GGML формат
python convert-pt-to-ggml.py \
    --model whisper-large-v2-uk \
    --output ggml-large-v2-uk.bin
```

---

### 5. Moonshine (Edge Devices)

#### [UsefulSensors/moonshine-tiny-uk](https://huggingface.co/UsefulSensors/moonshine-tiny-uk)

| Характеристика | Значення |
|----------------|----------|
| WER (FLEURS) | 18.25% |
| WER (CV17) | 26.11% |
| Параметри | 27M |
| Training Data | 19,600 годин |

**Порівняння з Whisper Tiny:**

| Модель | Params | FLEURS WER | CV17 WER |
|--------|--------|------------|----------|
| Whisper Tiny | 39M | 63.83% | 67.07% |
| Moonshine Tiny UK | 27M | 18.25% | 26.11% |

**Переваги:**
- 5-15x швидше за Whisper
- Найменший розмір
- Оптимізовано для edge

**Недоліки:**
- Схильний до галюцинацій
- Repetitive output на коротких аудіо
- Не сумісний з whisper-rs

---

### 6. VOSK (Archived)

#### [Yehor/vosk-uk](https://huggingface.co/Yehor/vosk-uk)

| Варіант | Розмір |
|---------|--------|
| uk_v3_dynamic_nano | 73 MB |
| uk_v3_dynamic_small | 133 MB |
| uk_v3_dynamic | 345 MB |

**Статус:** Archived. Рекомендується w2v-bert-uk-v2.1.

---

## Сумісність з whisper-rs

### Поточна реалізація s2t

```rust
// src/whisper.rs
use whisper_rs::{WhisperContext, WhisperContextParameters};

pub fn new(model_path: &str) -> Result<Self> {
    let ctx = WhisperContext::new_with_params(
        model_path,
        WhisperContextParameters::default()
    )?;
    // ...
}
```

### Матриця сумісності

| Модель | whisper-rs | parakeet-rs | Transformers | NeMo | ONNX |
|--------|------------|-------------|--------------|------|------|
| Whisper (vanilla) | ✅ | ❌ | ✅ | ❌ | ✅ |
| Whisper fine-tuned uk | ⚠️* | ❌ | ✅ | ❌ | ⚠️* |
| **Parakeet TDT v3** | ❌ | **✅** | ❌ | ✅ | ✅ |
| W2V-BERT | ❌ | ❌ | ✅ | ❌ | ⚠️ |
| FastConformer | ❌ | ❌ | ❌ | ✅ | ✅ |
| Citrinet | ❌ | ❌ | ❌ | ✅ | ✅ |
| Moonshine | ❌ | ❌ | ✅ | ❌ | ✅ |
| VOSK | ❌ | ❌ | ❌ | ❌ | ❌ (Kaldi) |

*Потребує конвертації в GGUF/GGML формат

### Шляхи інтеграції кращих моделей

#### Варіант A: Залишитись на whisper-rs

```
Переваги: Мінімальні зміни, Rust native
Недоліки: Обмежений вибір моделей

Кращий вибір: Whisper large-v2 uk (конвертований в GGML)
WER: ~13.72%
```

#### Варіант B: Інтегрувати ONNX Runtime

```rust
// Додати ort (ONNX Runtime)
use ort::{Environment, Session, Value};

// Підтримує: W2V-BERT, FastConformer, Moonshine
// WER: 6.6% - 18.25%
```

#### Варіант C: Інтегрувати candle

```rust
// Використати candle (ML framework для Rust)
use candle_core::{Device, Tensor};
use candle_transformers::models::whisper;

// Підтримує: Whisper, можливо Moonshine
// Native Rust, без Python
```

---

## Рекомендації для s2t

### За критерієм якості (WER)

| Пріоритет | Модель | WER | Інтеграція |
|-----------|--------|-----|------------|
| 1 | NVIDIA Citrinet | 3.52% | ONNX (складно) |
| 2 | W2V-BERT v1 | 6.6% | ONNX |
| 3 | FastConformer | 7.1% | ONNX |
| 4 | Whisper large-v2 uk | 13.72% | whisper-rs |

### За критерієм простоти інтеграції

| Пріоритет | Модель | WER | Зміни |
|-----------|--------|-----|-------|
| 1 | Whisper large-v2 uk | 13.72% | Тільки модель |
| 2 | Whisper small uk | 27% | Тільки модель |
| 3 | Moonshine tiny uk | 18.25% | ONNX backend |

### За критерієм швидкості

| Пріоритет | Модель | WER | Розмір | RTF* |
|-----------|--------|-----|--------|------|
| 1 | Moonshine tiny uk | 18.25% | 27M | 0.1x |
| 2 | Whisper base | 35% | 74M | 0.3x |
| 3 | Whisper small uk | 27% | 244M | 0.5x |

*Real-Time Factor (менше = швидше)

### Оптимальний вибір для s2t

```
┌─────────────────────────────────────────────────────────────┐
│                    РЕКОМЕНДАЦІЯ (оновлено 2026-01-29)        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  НАЙКРАЩИЙ ВИБІР: Parakeet TDT v3 через parakeet-rs         │
│  → WER: 5-7% (краще ніж Whisper large-v2 uk!)               │
│  → Punctuation + capitalization + word timestamps           │
│  → parakeet-rs вже в проекті (Sortformer)                   │
│  → Зміни: додати TDT feature, новий backend                 │
│                                                             │
│  Альтернатива (мінімальні зміни):                           │
│  → Whisper large-v2 uk (GGML конвертований)                 │
│  → WER: ~13.72% (vs 35-40% vanilla)                         │
│  → Зміни: тільки замінити модель                            │
│                                                             │
│  Для edge devices:                                          │
│  → Moonshine tiny uk                                        │
│  → WER: 18.25%, але 5-15x швидше                            │
│  → Зміни: ONNX + custom preprocessing                       │
│                                                             │
│  Див. docs/research/parakeet-rs-models-research.md          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## План інтеграції

### Phase 1: Whisper Fine-tuned (Мінімальні зміни)

1. Завантажити [whisper-large-v2-uk](https://huggingface.co/mitchelldehaven/whisper-large-v2-uk)
2. Конвертувати в GGML:
   ```bash
   git clone https://github.com/ggerganov/whisper.cpp
   cd whisper.cpp
   python models/convert-pt-to-ggml.py \
       mitchelldehaven/whisper-large-v2-uk \
       . \
       ggml-large-v2-uk.bin
   ```
3. Оновити config.rs:
   ```rust
   pub default_model: "ggml-large-v2-uk.bin"
   ```

### Phase 2: ONNX Backend (Опціонально)

1. Додати залежності:
   ```toml
   [dependencies]
   ort = "2.0"
   ndarray = "0.16"
   ```

2. Створити абстракцію:
   ```rust
   pub trait SpeechToText {
       fn transcribe(&self, samples: &[f32]) -> Result<String>;
   }

   pub struct WhisperBackend { /* whisper-rs */ }
   pub struct OnnxBackend { /* ort */ }
   ```

3. Інтегрувати W2V-BERT:
   - Експортувати в ONNX
   - Завантажити processor (tokenizer)
   - Реалізувати preprocessing

### Phase 3: Multi-model Support

```rust
// config.toml
[transcription]
backend = "whisper"  # "whisper" | "onnx" | "moonshine"
model = "ggml-large-v2-uk.bin"

# Для ONNX backend
[onnx]
model = "w2v-bert-uk-v1.onnx"
processor = "w2v-bert-uk-processor"
```

---

## Додаткові ресурси

### Датасети для тестування

| Датасет | Розмір | Опис |
|---------|--------|------|
| [cv10-uk-testset-clean](https://huggingface.co/datasets/Yehor/cv10-uk-testset-clean) | ~4 год | Очищений тестовий набір |
| [openstt-uk](https://huggingface.co/datasets/Yehor/openstt-uk) | ~1200 год | Великий training датасет |
| [FLEURS](https://huggingface.co/datasets/google/fleurs) | - | Multilingual benchmark |

### Інструменти

| Інструмент | Призначення |
|------------|-------------|
| [kenlm-uk](https://huggingface.co/Yehor/kenlm-uk) | Language model для покращення |
| [punctuation model](https://huggingface.co/theodotus/punctuation-uk) | Post-processing |
| [NeMo Forced Aligner](https://github.com/NVIDIA/NeMo) | Alignment для training |

---

## Висновки

1. **Найкращий WER:** NVIDIA Citrinet (3.52%), але потребує NeMo
2. **Найкращий для whisper-rs:** Whisper large-v2 uk (~13.72%)
3. **Найкращий баланс:** W2V-BERT v1 (6.6%) через ONNX
4. **Найшвидший:** Moonshine tiny uk (27M params, 5-15x faster)

Для s2t рекомендується почати з Phase 1 (Whisper fine-tuned) для швидкого покращення якості з мінімальними змінами.

---

## NeMo Implementation Notes

### Огляд NeMo Framework

NVIDIA NeMo - це Python-based framework для ASR, TTS та LLM. **Офіційних Rust bindings немає.**

### Шляхи інтеграції NeMo моделей в Rust

#### Варіант 1: ONNX Export + Rust ONNX Runtime

```python
# Експорт моделі в ONNX (Python)
import nemo.collections.asr as nemo_asr
from pathlib import Path

model = nemo_asr.models.ASRModel.from_pretrained(
    "nvidia/stt_uk_citrinet_1024_gamma_0_25"
)
model.export(str(Path("model.onnx")))
```

```rust
// Inference в Rust (ort crate)
use ort::{Session, Environment, Value};

let session = Session::builder()?
    .with_model_from_file("model.onnx")?;
```

**Обмеження:**
- Streaming моделі потребують `model.set_export_config({'cache_support': 'True'})` перед експортом
- Preprocessor та decoder можуть не експортуватись автоматично
- Деякі користувачі повідомляють про проблеми з FastConformer Hybrid експортом

#### Варіант 2: sherpa-onnx

[sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) - C++/Python бібліотека для ASR з ONNX, підтримує NeMo моделі.

**Rust bindings:** Немає офіційних, але можливо через FFI до C++ API.

#### Варіант 3: parakeet-rs

[parakeet-rs](https://github.com/jason-ni/parakeet-rs) - Rust бібліотека для NVIDIA Parakeet моделей.

| Характеристика | Значення |
|----------------|----------|
| Підтримувані моделі | Parakeet-tdt-0.6B v2 (EN), v3 (25 мов) |
| Українська | Не підтримується напряму |
| Ліцензія | Apache-2.0 |

**Примітка:** Потенційно можна адаптувати для українських NeMo моделей.

#### Варіант 4: Python subprocess (простий fallback)

```rust
use std::process::Command;

fn transcribe_via_nemo(audio_path: &str) -> Result<String> {
    let output = Command::new("python")
        .args(["-m", "nemo_asr", "--model", "nvidia/stt_uk_citrinet_1024_gamma_0_25", audio_path])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

**Недоліки:** Повільно, потребує Python runtime.

### Рекомендація для NeMo

```
Пріоритет                    Підхід
─────────────────────────────────────────────
1. Найпростіший             ONNX export + ort
2. Найшвидший (якщо працює) sherpa-onnx через FFI
3. Fallback                 Python subprocess
```

### Приклад ONNX Pipeline для Citrinet

```
┌──────────────────────────────────────────────────────────────┐
│                    ONNX INFERENCE PIPELINE                    │
├──────────────────────────────────────────────────────────────┤
│                                                              │
│  Audio (WAV 16kHz)                                           │
│       │                                                      │
│       ▼                                                      │
│  ┌─────────────────────────────────────┐                    │
│  │   Preprocessing (Rust)              │                    │
│  │   - Resample to 16kHz               │                    │
│  │   - Compute Mel-filterbanks         │                    │
│  │   - Normalize                        │                    │
│  └────────────────┬────────────────────┘                    │
│                   │                                          │
│                   ▼                                          │
│  ┌─────────────────────────────────────┐                    │
│  │   ONNX Model (ort crate)            │                    │
│  │   - Encoder: Citrinet-1024          │                    │
│  │   - CTC Decoder                     │                    │
│  └────────────────┬────────────────────┘                    │
│                   │                                          │
│                   ▼                                          │
│  ┌─────────────────────────────────────┐                    │
│  │   CTC Decoding (Rust)               │                    │
│  │   - Greedy decode                   │                    │
│  │   - Optional: KenLM Language Model  │                    │
│  └────────────────┬────────────────────┘                    │
│                   │                                          │
│                   ▼                                          │
│  Text Output                                                 │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### Складність інтеграції

| Модель | ONNX Export | Preprocessing | Decoding | Загальна складність |
|--------|-------------|---------------|----------|---------------------|
| Citrinet | Простий | Mel-filterbanks | CTC Greedy | Середня |
| FastConformer CTC | Простий | Mel-filterbanks | CTC Greedy | Середня |
| FastConformer Hybrid | Проблеми | Mel-filterbanks | Transducer | Висока |

---

## W2V-BERT v1 через Candle - Дослідження доцільності

### Поточний стан Candle

[Candle](https://github.com/huggingface/candle) - мінімалістичний ML framework для Rust від HuggingFace.

#### Підтримувані аудіо моделі

| Модель | Статус | Примітки |
|--------|--------|----------|
| Whisper | Повна підтримка | Всі розміри, WASM demo |
| EnCodec | Підтримується | Audio compression |
| MetaVoice | Підтримується | TTS |
| Parler-TTS | Підтримується | TTS |
| **Wav2Vec2** | Не реалізовано | - |
| **Wav2Vec2-BERT** | Не реалізовано | - |
| HuBERT | Не реалізовано | - |

### Реалізація W2V-BERT в Candle

**Verdict: Потребує значної роботи (~2-4 тижні для досвідченого розробника)**

#### Що потрібно реалізувати

1. **Conformer encoder layers** - Candle має базові blocks, але не Conformer
2. **Wav2Vec2-BERT specific attention** - Causal depthwise convolutions
3. **CTC head** - Потрібно для ASR output
4. **Preprocessing** - Mel spectrogram замість raw waveform

#### Архітектура W2V-BERT

```
Input: Mel Spectrogram (80 dim)
        │
        ▼
┌───────────────────────────────┐
│  Convolutional Feature Encoder │ ← Потрібно реалізувати
│  (DepthwiseConv1d + GroupNorm) │
└───────────────────────────────┘
        │
        ▼
┌───────────────────────────────┐
│  Conformer Layers (x24)       │ ← Частково є в candle
│  - Multi-head Attention       │   (потрібна адаптація)
│  - Convolution Module         │
│  - FFN                        │
└───────────────────────────────┘
        │
        ▼
┌───────────────────────────────┐
│  CTC Linear Head              │ ← Просто Linear layer
│  (hidden_size → vocab_size)   │
└───────────────────────────────┘
        │
        ▼
Output: Logits → CTC Decode → Text
```

#### Оцінка зусиль

| Компонент | Оцінка часу | Складність |
|-----------|-------------|------------|
| Conformer blocks | 3-5 днів | Висока |
| Conv Feature Encoder | 1-2 дні | Середня |
| CTC decoding | 1 день | Низька |
| Weight loading | 2-3 дні | Середня |
| Testing & debugging | 3-5 днів | - |
| **Загалом** | **10-16 днів** | - |

### Альтернатива: ONNX Runtime (ort)

**Verdict: Значно простіше (~2-3 дні)**

```rust
// Cargo.toml
[dependencies]
ort = "2.0"
ndarray = "0.16"

// Код
use ort::{Session, Value};
use ndarray::Array2;

fn transcribe(audio: &[f32]) -> Result<String> {
    // 1. Preprocessing (mel spectrogram)
    let mel = compute_mel_spectrogram(audio)?;

    // 2. ONNX inference
    let session = Session::builder()?
        .with_model_from_file("w2v-bert-uk.onnx")?;

    let input = Value::from_array(mel)?;
    let outputs = session.run(ort::inputs![input]?)?;

    // 3. CTC decode
    let logits = outputs[0].extract_tensor::<f32>()?;
    let text = ctc_greedy_decode(logits)?;

    Ok(text)
}
```

### Рекомендація для Candle

```
┌─────────────────────────────────────────────────────────────┐
│                    РЕКОМЕНДАЦІЯ                              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  НЕ РЕКОМЕНДУЄТЬСЯ: Реалізовувати W2V-BERT в Candle        │
│     - Занадто багато роботи для одної моделі               │
│     - Candle roadmap не включає wav2vec2                   │
│                                                             │
│  РЕКОМЕНДУЄТЬСЯ: Використовувати ONNX Runtime (ort)        │
│     - Значно швидша інтеграція                             │
│     - Підтримує всі W2V-BERT варіанти                      │
│     - Можливість використати й інші моделі (NeMo)          │
│                                                             │
│  АЛЬТЕРНАТИВА: Залишитись на whisper-rs                    │
│     - Candle Whisper можна розглянути в майбутньому        │
│     - Менше змін до поточної архітектури                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Вимоги до апаратного забезпечення

### Зведена таблиця

| Модель | Параметри | VRAM (GPU) | RAM (CPU) | RTF* (CPU) | RTF* (GPU) |
|--------|-----------|------------|-----------|------------|------------|
| **Moonshine tiny** | 27M | ~200 MB | 0.5-1 GB | 0.1-0.2x | 0.02x |
| **Whisper tiny** | 39M | ~300 MB | 1-2 GB | 0.3x | 0.05x |
| **Whisper base** | 74M | ~500 MB | 1-2 GB | 0.5x | 0.1x |
| **Whisper small** | 244M | ~1 GB | 2-4 GB | 1-2x | 0.2x |
| **FastConformer** | 120M | ~400 MB | 1-2 GB | 0.3-0.5x | 0.1x |
| **Citrinet-1024** | 141M | ~500 MB | 1-2 GB | 0.3-0.5x | 0.1x |
| **W2V-BERT v1** | 600M | ~1.5 GB | 3-4 GB | 1-3x | 0.2-0.3x |
| **Whisper medium** | 769M | ~2 GB | 4-6 GB | 2-4x | 0.3x |
| **Whisper large-v2** | 1.5B | ~3 GB | 6-10 GB | 4-8x | 0.5x |
| **Whisper large-v3** | 1.5B | ~3 GB | 6-10 GB | 4-8x | 0.5x |

*RTF = Real-Time Factor (менше = швидше; 1x = real-time)

### Мінімальні системні вимоги

#### Для CPU-only (без GPU)

| Рівень | RAM | CPU | Рекомендовані моделі |
|--------|-----|-----|---------------------|
| **Мінімальний** | 4 GB | 2 cores | Moonshine tiny, Whisper tiny |
| **Рекомендований** | 8 GB | 4 cores | Whisper base/small, FastConformer |
| **Оптимальний** | 16 GB | 8 cores | Whisper medium, W2V-BERT |
| **Продуктивний** | 32 GB | 12+ cores | Whisper large-v2/v3 |

#### Для GPU

| GPU | VRAM | Рекомендовані моделі |
|-----|------|---------------------|
| GTX 1650 | 4 GB | Всі до W2V-BERT |
| RTX 3060 | 8 GB | Всі моделі |
| RTX 3080/4080 | 10-16 GB | Всі моделі + batch processing |

### Детальний аналіз по моделях

#### Moonshine tiny (27M)

```
Оптимізований для edge devices:
- Raspberry Pi 4 (4GB): Працює
- Raspberry Pi 5 (8GB): Комфортно
- Мобільні телефони: Можливо

Формат: ONNX рекомендується для embedded
Пам'ять: ~190 MB модель + ~100-300 MB runtime
```

#### W2V-BERT v1 (600M)

```
VRAM (FP16):  600M × 2 bytes = 1.2 GB + 20% overhead = ~1.5 GB
RAM (FP32):   600M × 4 bytes = 2.4 GB + overhead = ~3-4 GB

Квантизація:
- INT8: ~800 MB (50% reduction)
- INT4: ~400 MB (75% reduction)

Мінімальний GPU: GTX 1060 6GB / RTX 3050
Мінімальний CPU: 8GB RAM, 4 cores
```

#### Whisper large-v2 (1.5B)

```
VRAM (FP16):  1.5B × 2 bytes = 3 GB + overhead = ~3.5-4 GB
RAM (FP32):   1.5B × 4 bytes = 6 GB + overhead = ~8-10 GB

Для whisper-rs (whisper.cpp):
- GGML Q8_0: ~1.5 GB
- GGML Q4_0: ~0.8 GB

Мінімальний GPU: GTX 1080 8GB / RTX 3060
Мінімальний CPU: 16GB RAM, 8 cores (повільно, ~4-8x real-time)
```

#### FastConformer (120M)

```
Компактна модель з гарним WER:
- VRAM: ~400 MB
- RAM: ~1-2 GB
- Streaming support: Так

Ідеально для:
- Embedded systems
- Low-latency applications
- Пунктуація + капіталізація
```

### Порівняння швидкості на типовому hardware

#### Desktop: Intel i7-12700K + RTX 3080

| Модель | CPU (8 threads) | GPU |
|--------|-----------------|-----|
| Moonshine tiny | 15x faster | 50x faster |
| Whisper base | 3x faster | 15x faster |
| W2V-BERT v1 | 0.5x (slower) | 5x faster |
| Whisper large-v2 | 0.2x (slower) | 3x faster |

#### Laptop: Intel i5-1235U (no GPU)

| Модель | CPU (4 threads) | Практичність |
|--------|-----------------|--------------|
| Moonshine tiny | 3x faster | Рекомендовано |
| Whisper base | 0.8x (near real-time) | Прийнятно |
| W2V-BERT v1 | 0.2x (5x slower) | Повільно |
| Whisper large-v2 | 0.1x (10x slower) | Непрактично |

### Рекомендації по вибору

```
┌─────────────────────────────────────────────────────────────┐
│                   ВИБІР МОДЕЛІ ПО HARDWARE                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Raspberry Pi / Embedded:                                   │
│  → Moonshine tiny uk (27M, ONNX)                           │
│                                                             │
│  Ноутбук без GPU (8GB RAM):                                │
│  → Whisper small uk (244M, GGML Q8)                        │
│  → FastConformer (120M, ONNX)                              │
│                                                             │
│  Desktop без GPU (16GB+ RAM):                              │
│  → Whisper medium/large (GGML Q4/Q8)                       │
│  → W2V-BERT v1 (600M, ONNX INT8)                           │
│                                                             │
│  Desktop з GPU (4GB+ VRAM):                                │
│  → W2V-BERT v1 (600M) - найкращий WER                      │
│  → Whisper large-v2 uk - якщо потрібен whisper-rs          │
│                                                             │
│  Сервер (GPU cluster):                                     │
│  → FastConformer Hybrid (streaming + P&C)                  │
│  → NeMo Parakeet (multi-language)                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Оптимізації для слабкого hardware

1. **Quantization:**
   ```
   FP32 → FP16: 50% memory reduction, ~same speed
   FP16 → INT8: 50% memory reduction, 10-30% faster
   INT8 → INT4: 50% memory reduction, may lose quality
   ```

2. **Streaming/Chunked processing:**
   - Замість 30s chunks використовувати 5-10s
   - VAD-based сегментація

3. **Model distillation:**
   - whisper-large-v3-turbo: 809M замість 1.5B, ~same quality

4. **Hardware acceleration:**
   - OpenBLAS/MKL для CPU
   - CUDA/cuDNN для NVIDIA
   - CoreML для Apple Silicon

---

## Джерела

- [GitHub: speech-recognition-uk](https://github.com/egorsmkv/speech-recognition-uk)
- [HuggingFace: Yehor/w2v-bert-uk](https://huggingface.co/Yehor/w2v-bert-uk)
- [HuggingFace: Yehor/w2v-bert-uk-v2.1](https://huggingface.co/Yehor/w2v-bert-uk-v2.1)
- [HuggingFace: theodotus/stt_ua_fastconformer_hybrid_large_pc](https://huggingface.co/theodotus/stt_ua_fastconformer_hybrid_large_pc)
- [HuggingFace: nvidia/stt_uk_citrinet_1024_gamma_0_25](https://huggingface.co/nvidia/stt_uk_citrinet_1024_gamma_0_25)
- [HuggingFace: UsefulSensors/moonshine-tiny-uk](https://huggingface.co/UsefulSensors/moonshine-tiny-uk)
- [GitHub: whisper-ukrainian](https://github.com/egorsmkv/whisper-ukrainian)
- [arXiv: Moonshine Paper](https://arxiv.org/abs/2509.02523)
- [GitHub: NVIDIA NeMo](https://github.com/NVIDIA-NeMo/NeMo)
- [GitHub: HuggingFace Candle](https://github.com/huggingface/candle)
- [GitHub: sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx)
- [GitHub: parakeet-rs](https://github.com/jason-ni/parakeet-rs)
- [NVIDIA NeMo Export Docs](https://docs.nvidia.com/nemo-framework/user-guide/24.09/nemotoolkit/core/export.html)
