# Kyutai STT - Глибоке дослідження

**Дата:** 2026-01-29
**Статус:** In Progress
**Гіпотеза:** Kyutai STT може забезпечити кращий баланс швидкості та якості для транскрипції завдяки streaming архітектурі.

## Зміст

1. [Огляд](#огляд)
2. [Архітектура](#архітектура)
3. [Моделі та розміри](#моделі-та-розміри)
4. [Як працює стрімінг](#як-працює-стрімінг)
5. [Локальний запуск](#локальний-запуск)
6. [Продуктивність](#продуктивність)
7. [Порівняння з Whisper](#порівняння-з-whisper)
8. [Висновки для s2t](#висновки-для-s2t)

---

## Огляд

**Kyutai STT** - це streaming speech-to-text модель від французької некомерційної лабораторії Kyutai (заснована 2023, підтримується Xavier Niel, Rodolphe Saadé, Eric Schmidt).

**Ключова інновація:** Delayed Streams Modeling (DSM) - техніка, що дозволяє транскрибувати аудіо в реальному часі, не чекаючи завершення запису.

### Основні характеристики

| Характеристика | Значення |
|----------------|----------|
| Ліцензія | CC-BY 4.0 |
| Дата релізу | 2025-06-17 |
| Підтримувані мови | English, French (1B), English only (2.6B) |
| Українська | ❌ Не підтримується |
| Streaming | ✅ Native |
| Rust implementation | ✅ Є (candle) |

---

## Архітектура

### Високорівнева схема

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         KYUTAI STT PIPELINE                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Audio Input (24kHz, mono)                                              │
│       │                                                                 │
│       ▼                                                                 │
│  ┌─────────────────────────────────────────┐                           │
│  │           MIMI CODEC (Encoder)          │                           │
│  │  ┌─────────────────────────────────┐    │                           │
│  │  │  Convolutional Encoder          │    │  Параметри: 96.2M         │
│  │  │  + Transformer layers           │    │  Sample rate: 24kHz       │
│  │  │  + Residual Vector Quantizer    │    │  Frame rate: 12.5 Hz      │
│  │  │    (16 codebooks)               │    │  Bitrate: 1.1 kbps        │
│  │  └─────────────────────────────────┘    │                           │
│  │                                         │                           │
│  │  Output: 32 audio tokens per frame      │                           │
│  │  (80ms chunks → discrete tokens)        │                           │
│  └────────────────┬────────────────────────┘                           │
│                   │                                                     │
│                   ▼                                                     │
│  ┌─────────────────────────────────────────┐                           │
│  │      DECODER-ONLY TRANSFORMER           │                           │
│  │  ┌─────────────────────────────────┐    │  Параметри: ~1B або 2.6B  │
│  │  │  Delayed Streams Modeling       │    │  Hidden size: 2048        │
│  │  │  - Audio stream (from Mimi)     │    │  Layers: 48               │
│  │  │  - Text stream (predictions)    │    │  Attention heads: 32      │
│  │  │                                 │    │  Sliding window: 375      │
│  │  │  Inner Monologue: предсказує    │    │                           │
│  │  │  text tokens як prefix до       │    │                           │
│  │  │  audio tokens                   │    │                           │
│  │  └─────────────────────────────────┘    │                           │
│  │                                         │                           │
│  │  + Semantic VAD (voice detection)       │                           │
│  │  + Word-level timestamps                │                           │
│  └────────────────┬────────────────────────┘                           │
│                   │                                                     │
│                   ▼                                                     │
│  Text Output (streaming, з затримкою 0.5s або 2.5s)                    │
│  + Punctuation & Capitalization                                        │
│  + Timestamps                                                          │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Mimi Codec - Детальніше

**Mimi** - це neural audio codec, що є серцем Kyutai STT:

```
Audio Waveform (24kHz)
       │
       ▼ Downsampling (128x)
┌──────────────────────┐
│  Convolutional       │
│  Encoder             │  → Continuous embeddings
│  + Transformer       │
└──────────┬───────────┘
           │
           ▼ Vector Quantization
┌──────────────────────┐
│  Residual Vector     │
│  Quantizer (RVQ)     │
│  - 16 codebooks      │  → Discrete tokens
│  - 2049 entries each │     (32 per frame)
└──────────┬───────────┘
           │
           ▼
   Audio Tokens @ 12.5 Hz
```

**Ключові особливості Mimi:**

| Параметр | Значення | Порівняння |
|----------|----------|------------|
| Frame rate | 12.5 Hz | EnCodec: 50 Hz, SoundStream: 50 Hz |
| Bitrate | 1.1 kbps | EnCodec: 6-24 kbps |
| Parameters | 96.2M | - |
| Codebooks | 16 | - |
| Latency | 80ms | - |

**Інновації Mimi:**
1. **Semantic distillation** - перший codebook навчається матчити WavLM embeddings (semantic info)
2. **Adversarial training** - GAN-style discriminator для кращої якості
3. **RVQ dropout** - тренування з різною кількістю codebooks для гнучкості

### Decoder Architecture

```python
# Конфігурація з HuggingFace
KyutaiSpeechToTextConfig:
  vocab_size: 4001
  hidden_size: 2048
  num_hidden_layers: 48
  num_attention_heads: 32
  num_key_value_heads: ?  # GQA support
  ffn_dim: 11264
  max_position_embeddings: 750
  sliding_window: 375
  hidden_act: "silu"
  rms_norm_eps: 1e-08
  num_codebooks: 32
  codebook_vocab_size: 2049
```

**Delayed Streams Modeling (DSM):**

На відміну від Whisper (encoder-decoder, потребує всього аудіо):

```
Whisper (Encoder-Decoder):
  [Full Audio] → Encoder → [Encoded] → Decoder → [Text]
                           ↑
                    Needs ALL audio first

Kyutai (Decoder-only with DSM):
  [Audio chunk 1] → Mimi → [tokens] ─┐
  [Audio chunk 2] → Mimi → [tokens] ─┼→ Decoder → [Text chunk 1]
  [Audio chunk 3] → Mimi → [tokens] ─┼→ Decoder → [Text chunk 2]
                                     └→ Decoder → [Text chunk 3]
                                        ↑
                            Streaming! Затримка = 0.5s або 2.5s
```

**"Inner Monologue" technique:**
- Модель спочатку передбачає text tokens
- Потім використовує їх як prefix для audio tokens
- Це покращує лінгвістичну якість

---

## Моделі та розміри

### Доступні моделі

| Модель | Параметри | Мови | Затримка | Розмір | Format |
|--------|-----------|------|----------|--------|--------|
| `stt-1b-en_fr` | ~1B | EN+FR | 0.5s | ~4.8 GB | safetensors |
| `stt-2.6b-en` | ~2.6B | EN | 2.5s | ~5.2 GB* | safetensors |
| `stt-1b-en_fr-mlx` | ~1B | EN+FR | 0.5s | ~2.0 GB | MLX |
| `stt-1b-en_fr-candle` | ~1B | EN+FR | 0.5s | ~4.8 GB | Candle/Rust |

*Estimated based on parameter count

### Файлова структура моделі

```
kyutai/stt-1b-en_fr/ (~4.8 GB total)
├── model.safetensors           # Main decoder weights (~4 GB)
├── mimi-pytorch-e351c8d8@125.safetensors  # Mimi codec (385 MB)
├── tokenizer_en_fr_audio_8000.model       # SentencePiece tokenizer
├── config.json                 # Model configuration
└── README.md
```

### Квантизація

| Format | Доступність | Примітки |
|--------|-------------|----------|
| BF16 (default) | ✅ | Full precision |
| FP32 | ✅ | Transformers format |
| INT8 | ⚠️ | MLX only (Apple) |
| INT4 | ⚠️ | MLX only (Apple) |
| GGUF | ❌ | Не доступно офіційно |

**MLX quantization (Apple Silicon only):**
```bash
# 8-bit quantization для MLX
python -m moshi_mlx.convert --quantize-int8 ...

# 4-bit quantization
python -m moshi_mlx.convert --quantize-int4 ...
```

---

## Як працює стрімінг

### Чому Whisper не може стрімити

```
Whisper Architecture:
┌─────────────────────────────────────────────────────────┐
│  1. Encoder отримує ВСЕ аудіо одразу                    │
│  2. Cross-attention між decoder та encoder              │
│  3. Decoder не може працювати без повного encoder output│
└─────────────────────────────────────────────────────────┘
       ↓
  Результат: Потрібно дочекатися кінця аудіо
```

### Як Kyutai досягає streaming

```
Kyutai Architecture:
┌─────────────────────────────────────────────────────────┐
│  1. Mimi codec працює chunk-by-chunk (80ms)             │
│  2. Decoder-only (no cross-attention)                   │
│  3. Кожен chunk обробляється незалежно                  │
│  4. Causal attention - бачить тільки минулі токени      │
└─────────────────────────────────────────────────────────┘
       ↓
  Результат: Текст з'являється через 0.5-2.5s
```

### Rust implementation - step_pcm loop

```rust
// З stt-rs/src/main.rs
// Обробка аудіо chunks по 1920 семплів (80ms @ 24kHz)

for pcm in pcm.chunks(1920) {
    // 1. Конвертуємо PCM в tensor
    let pcm = Tensor::new(pcm, &self.dev)?.reshape((1, 1, ()))?;

    // 2. Один крок інференсу
    let asr_msgs = self.state.step_pcm(pcm, None, &().into(), |_, _, _| ())?;

    // 3. Обробляємо результати
    for msg in asr_msgs {
        match msg {
            AsrMsg::Word { word, .. } => {
                // Нове слово розпізнано
                print!("{} ", word);
            }
            AsrMsg::EndWord { timestamp } => {
                // Слово завершено, маємо timestamp
            }
            AsrMsg::Step { vad_prob } => {
                // VAD probability для детекції тиші
                if vad_prob < threshold {
                    // Мовлення закінчилось
                }
            }
        }
    }
}
```

### Delay explanation

| Модель | Delay | Причина |
|--------|-------|---------|
| 1B (en_fr) | 0.5s | Менше контексту, швидше |
| 2.6B (en) | 2.5s | Більше контексту, точніше |

**Затримка = час, через який слово з'явиться в output після того як було вимовлене**

---

## Локальний запуск

### Варіант 1: Python (найпростіший)

```bash
# Встановлення
pip install moshi>=0.2.6

# Запуск inference
python -m moshi.run_inference \
    --hf-repo kyutai/stt-1b-en_fr \
    audio_file.mp3
```

**Вимоги:** Python 3.10+, PyTorch, ~8GB RAM

### Варіант 2: Transformers (HuggingFace)

```python
import torch
from transformers import (
    KyutaiSpeechToTextProcessor,
    KyutaiSpeechToTextForConditionalGeneration
)

# Завантаження моделі
model_id = "kyutai/stt-1b-en_fr-trfs"
processor = KyutaiSpeechToTextProcessor.from_pretrained(model_id)
model = KyutaiSpeechToTextForConditionalGeneration.from_pretrained(
    model_id,
    device_map="auto",  # auto GPU/CPU
    torch_dtype="auto"  # auto precision
)

# Inference
inputs = processor(audio_array, sampling_rate=24000, return_tensors="pt")
inputs.to(model.device)
output_tokens = model.generate(**inputs)
text = processor.batch_decode(output_tokens, skip_special_tokens=True)
```

**Вимоги:** transformers >= 4.53.0

### Варіант 3: Rust Server (Production)

```bash
# Встановлення
cargo install --features cuda moshi-server

# Запуск WebSocket сервера
moshi-server worker --config configs/config-stt-en_fr-hf.toml
# Server: ws://localhost:8080/api/asr-streaming

# Тест клієнт
python test_client.py --audio audio_file.wav
```

**Cargo.toml dependencies:**
```toml
[dependencies]
candle-core = "0.9.1"      # ML framework
candle-nn = "0.9.1"
candle-transformers = "0.9.1"
moshi = "0.6.1"            # Streaming protocol
kaudio = "0.2.1"           # Audio processing
sentencepiece = "0.11.3"   # Tokenization
hf-hub = "0.4.3"           # Model download

[features]
cuda = []   # NVIDIA GPU
metal = []  # Apple GPU
```

### Варіант 4: MLX (Apple Silicon)

```bash
# Встановлення
pip install moshi-mlx>=0.2.6

# Запуск
python -m moshi_mlx.run_inference \
    --hf-repo kyutai/stt-1b-en_fr-mlx \
    audio_file.mp3
```

**Підтримка:**
- ✅ MacBook M1/M2/M3/M4
- ✅ iPhone 16 Pro (1B model)
- ✅ 8-bit/4-bit quantization

### Варіант 5: Standalone Rust Binary

```bash
# Clone repo
git clone https://github.com/kyutai-labs/delayed-streams-modeling
cd delayed-streams-modeling/stt-rs

# Build
cargo build --release --features cuda

# Run
./target/release/kyutai-stt-rs \
    --hf-repo kyutai/stt-1b-en_fr-candle \
    audio_file.wav
```

---

## Продуктивність

### GPU Performance

| GPU | Model | Concurrent Streams | RTF |
|-----|-------|-------------------|-----|
| H100 | 2.6B | 400 | Real-time |
| L40S | 1B | 64 | 3x real-time |
| A100 | 1B | ~200* | Real-time |

*Estimated

### CPU Performance (очікувана)

**Офіційних CPU бенчмарків немає!**

Оцінка на основі архітектури:

| CPU | Model | Expected RTF | Notes |
|-----|-------|--------------|-------|
| Apple M4 | 1B (MLX) | ~0.3-0.5x | Hardware acceleration |
| Apple M1 | 1B (MLX) | ~1-2x | Працює |
| Intel i7 | 1B | ~3-5x | Без оптимізації |
| Intel i7 | 1B (quantized) | ~1-3x | Потрібно тестувати |

**Порівняння з Whisper на CPU:**

| Model | Whisper base | Kyutai 1B |
|-------|--------------|-----------|
| Params | 74M | 1000M |
| Expected CPU RTF | 0.3-0.5x | 3-5x (slower!) |

**Висновок:** Kyutai STT оптимізований для GPU throughput, не для CPU inference.

### Memory Requirements

| Model | GPU VRAM | RAM (CPU) |
|-------|----------|-----------|
| 1B (BF16) | ~4 GB | ~8 GB |
| 2.6B (BF16) | ~8 GB | ~16 GB |
| 1B (INT8 MLX) | ~2 GB | ~4 GB |
| 1B (INT4 MLX) | ~1 GB | ~2 GB |

---

## Порівняння з Whisper

### Архітектурні відмінності

| Аспект | Whisper | Kyutai STT |
|--------|---------|------------|
| Архітектура | Encoder-Decoder | Decoder-only |
| Streaming | ❌ Ні | ✅ Native |
| Audio encoding | Mel spectrogram | Mimi codec (neural) |
| Cross-attention | ✅ Є | ❌ Немає |
| Causal | ❌ Bidirectional | ✅ Causal only |
| Frame rate | 50 Hz | 12.5 Hz |

### Якість (WER)

| Dataset | Whisper Large v3 | Kyutai 2.6B |
|---------|------------------|-------------|
| LibriSpeech clean | 2-3% | ~2-3% |
| LibriSpeech other | 5-6% | ~4-5% |
| Average English | 10-12% | 6.4% |

**Kyutai часто краще для English!**

### Швидкість та throughput

| Metric | Whisper Large v3 Turbo | Kyutai 2.6B |
|--------|------------------------|-------------|
| RTFx (batch) | 216 | 88 |
| Latency | Full audio | 2.5s streaming |
| Concurrent streams (H100) | N/A | 400 |

### Мовна підтримка

| Мова | Whisper | Kyutai STT |
|------|---------|------------|
| English | ✅ | ✅ |
| French | ✅ | ✅ (1B only) |
| Ukrainian | ✅ | ❌ |
| Russian | ✅ | ❌ |
| Інші (99) | ✅ | ❌ |

---

## Висновки для s2t

### Переваги Kyutai STT

1. **True streaming** - текст з'являється через 0.5s
2. **Краща якість для EN** - 6.4% WER vs 10-12%
3. **Rust implementation** - native, без FFI
4. **Semantic VAD** - вбудована детекція мовлення
5. **High throughput** - для server deployment

### Недоліки для s2t

1. **❌ Немає Ukrainian** - критично для основного use case
2. **Повільніше на CPU** - оптимізовано для GPU
3. **Більші моделі** - 1B vs 74M параметрів
4. **Більше RAM** - 8GB vs 1GB

### Рекомендації

| Сценарій | Рекомендація |
|----------|--------------|
| Ukrainian transcription | Залишатись на Whisper |
| English-only app | Розглянути Kyutai STT |
| Server with GPU | Kyutai STT для throughput |
| Desktop CPU-only | Whisper (менший, швидший) |

### Що можна запозичити з архітектури

1. **Chunked processing** - реалізувати для Whisper
   ```rust
   // Замість обробки всього аудіо
   // Обробляти сегментами з overlap
   for chunk in audio.chunks(5_seconds) {
       let partial = whisper.transcribe(chunk);
       output.push(partial);
   }
   ```

2. **Semantic VAD** - покращити поточний VAD
   - Silero VAD або подібний
   - Інтеграція з nnnoiseless

3. **WebSocket streaming protocol**
   - Для remote/server deployment
   - Real-time UI updates

---

## Наступні кроки

- [ ] Запустити Kyutai STT локально (Python)
- [ ] Заміряти реальну продуктивність на CPU
- [ ] Порівняти якість на тестових аудіо (EN)
- [ ] Дослідити fine-tuning можливості для UA
- [ ] Імплементувати chunked Whisper processing

---

## Джерела

- [GitHub: delayed-streams-modeling](https://github.com/kyutai-labs/delayed-streams-modeling)
- [HuggingFace: kyutai/stt-1b-en_fr](https://huggingface.co/kyutai/stt-1b-en_fr)
- [HuggingFace: Kyutai STT docs](https://huggingface.co/docs/transformers/model_doc/kyutai_speech_to_text)
- [Moshi Paper (arXiv:2410.00037)](https://arxiv.org/abs/2410.00037)
- [Kyutai Codec Explainer](https://kyutai.org/codec-explainer)
- [Modal: Top Open Source STT 2025](https://modal.com/blog/open-source-stt)
- [HuggingFace: kyutai/mimi](https://huggingface.co/kyutai/mimi)
