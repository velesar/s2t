# parakeet-rs Models Research

**Дата:** 2026-01-29
**Статус:** Complete
**Джерело:** [altunenes/parakeet-rs](https://github.com/altunenes/parakeet-rs)

## Огляд

`parakeet-rs` — Rust бібліотека для швидкого STT та speaker diarization з підтримкою NVIDIA моделей через ONNX Runtime. Вже використовується в s2t для Sortformer diarization.

## Доступні моделі

### 1. Parakeet TDT 0.6B v3 (Multilingual STT)

**Ключова знахідка:** Підтримує українську мову з WER 6.79%!

| Характеристика | Значення |
|----------------|----------|
| Параметри | 600M |
| Мови | 25 (включно з українською) |
| WER (uk, FLEURS) | **6.79%** |
| WER (uk, CoVoST) | **5.10%** |
| Особливості | Punctuation, capitalization, word timestamps |
| Довжина аудіо | До 24 хв (full attention) або 3 год (local) |
| HuggingFace | [nvidia/parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) |
| ONNX версія | [istupakov/parakeet-tdt-0.6b-v3-onnx](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx) |
| altunenes ONNX | [altunenes/parakeet-rs/tdt](https://huggingface.co/altunenes/parakeet-rs/tree/main/tdt) |

**Файли моделі (altunenes):**

| Файл | Розмір |
|------|--------|
| encoder-model.onnx + .data | 2.5 GB |
| encoder-model.int8.onnx | 652 MB |
| decoder_joint-model.onnx | 72.5 MB |
| decoder_joint-model.int8.onnx | 18.2 MB |
| vocab.txt | 93.9 KB |

**Код для використання:**
```rust
use parakeet_rs::{ParakeetTDT, Transcriber, TimestampMode};

let mut model = ParakeetTDT::from_pretrained("./tdt_model")?;
let result = model.transcribe_samples(
    audio_samples,
    16000,  // sample rate
    1,      // channels
    Some(TimestampMode::Words)
)?;
println!("Text: {}", result.text);
```

---

### 2. Nemotron Speech Streaming 0.6B (English)

| Характеристика | Значення |
|----------------|----------|
| Параметри | 600M |
| Мови | English only |
| Особливості | Cache-aware streaming, punctuation |
| HuggingFace | [nvidia/nemotron-speech-streaming-en-0.6b](https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b) |
| altunenes ONNX | [altunenes/parakeet-rs/nemotron-speech-streaming-en-0.6b](https://huggingface.co/altunenes/parakeet-rs/tree/main/nemotron-speech-streaming-en-0.6b) |

**Примітка:** Тільки англійська, не підходить для українського STT.

---

### 3. Parakeet CTC 0.6B (English)

| Характеристика | Значення |
|----------------|----------|
| Параметри | 600M |
| Мови | English only |
| Особливості | Punctuation, capitalization |
| HuggingFace | [onnx-community/parakeet-ctc-0.6b-ONNX](https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX) |

**Примітка:** Тільки англійська, не підходить для українського STT.

---

### 4. Parakeet EOU 120M (End-of-Utterance Detection)

| Характеристика | Значення |
|----------------|----------|
| Параметри | 120M |
| Призначення | Визначення кінця фрази (real-time) |
| altunenes ONNX | [altunenes/parakeet-rs/realtime_eou_120m-v1-onnx](https://huggingface.co/altunenes/parakeet-rs/tree/main/realtime_eou_120m-v1-onnx) |

**Потенційне застосування:**
- Streaming transcription — визначати коли користувач закінчив говорити
- Покращення UX — швидше відображення результатів
- Інтеграція з continuous mode

---

### 5. Sortformer (Speaker Diarization)

Вже інтегровано в s2t!

| Версія | Розмір | Примітки |
|--------|--------|----------|
| v1 | 514 MB | Original |
| v2 | 492 MB | Streaming |
| v2.1 | 492 MB | Improved streaming |

**Особливості:**
- До 4 мовців
- Streaming diarization
- Callhome/DIHARD3 configs

---

## Порівняння з поточними моделями s2t

### Для українського STT

| Модель | WER (uk) | Розмір | Punctuation | Timestamps | Інтеграція |
|--------|----------|--------|-------------|------------|------------|
| **Parakeet TDT v3** | **5-7%** | 2.5GB / 670MB (int8) | ✅ | ✅ Word-level | parakeet-rs |
| Whisper large-v2 uk | ~13.72% | 3 GB | ❌ | ✅ | whisper-rs |
| W2V-BERT uk v1 | 6.6% | 600M | ❌ | ❌ | ONNX (manual) |
| Whisper small uk | ~27% | 500 MB | ❌ | ✅ | whisper-rs |

### Висновок

**Parakeet TDT v3** — найкраща модель для українського STT:
- Кращий WER ніж Whisper large-v2 uk
- Вбудована пунктуація та капіталізація
- Word-level timestamps
- Вже маємо parakeet-rs в проекті

---

## План інтеграції

### Phase 1: Додати TDT v3 як альтернативний backend

```toml
# Cargo.toml
[features]
tdt = ["parakeet-rs/tdt"]
```

```rust
// src/tdt.rs
pub struct ParakeetSTT {
    model: ParakeetTDT,
}

impl Transcription for ParakeetSTT {
    fn transcribe(&mut self, samples: &[f32], language: Option<&str>) -> Result<String> {
        let result = self.model.transcribe_samples(samples, 16000, 1, None)?;
        Ok(result.text)
    }
}
```

### Phase 2: Автоматичне завантаження моделей

```rust
// src/models.rs
pub fn get_tdt_model_info() -> ModelInfo {
    ModelInfo {
        filename: "parakeet-tdt-0.6b-v3".to_string(),
        display_name: "Parakeet TDT v3 (25 мов)".to_string(),
        size_bytes: 670_000_000, // INT8 version
        description: "NVIDIA Parakeet TDT для 25 мов (WER 6.79% uk)".to_string(),
    }
}

pub async fn download_tdt_model<F>(progress_callback: F) -> Result<()> {
    // Завантажити з altunenes/parakeet-rs/tdt
    // encoder-model.int8.onnx + decoder_joint-model.int8.onnx + vocab.txt
}
```

### Phase 3: EOU для streaming

```rust
// src/eou.rs
pub struct EndOfUtteranceDetector {
    model: ParakeetEOU,
}

impl EndOfUtteranceDetector {
    pub fn is_end_of_utterance(&mut self, samples: &[f32]) -> bool {
        self.model.detect(samples).unwrap_or(false)
    }
}
```

---

## Вимоги до Hardware

### TDT v3 (INT8)

| Hardware | Пам'ять | Швидкість |
|----------|---------|-----------|
| CPU (8 cores) | 2-3 GB RAM | ~0.3-0.5x RTF |
| GPU (4GB VRAM) | 1-2 GB VRAM | ~0.1x RTF |

### TDT v3 (FP32)

| Hardware | Пам'ять | Швидкість |
|----------|---------|-----------|
| CPU (8 cores) | 4-6 GB RAM | ~0.5-1x RTF |
| GPU (8GB+ VRAM) | 3-4 GB VRAM | ~0.1x RTF |

---

## Рекомендації

### Короткострокові (v0.4.0)

1. **Інтегрувати TDT v3** як альтернативний STT backend
2. Використовувати INT8 версію для менших вимог до пам'яті
3. Оновити UI для вибору backend (Whisper / Parakeet TDT)

### Середньострокові (v0.5.0)

1. **Інтегрувати EOU** для покращеного streaming
2. Комбінувати TDT + Sortformer для конференцій з diarization
3. Автоматичне визначення мови (TDT v3 feature)

### Довгострокові

1. Замінити Whisper на TDT v3 як default backend для української
2. Зберегти Whisper для інших мов (якщо потрібно)
3. Дослідити fine-tuning TDT для кращої української підтримки

---

## Джерела

- [parakeet-rs GitHub](https://github.com/altunenes/parakeet-rs)
- [parakeet-rs docs.rs](https://docs.rs/parakeet-rs)
- [altunenes/parakeet-rs HuggingFace](https://huggingface.co/altunenes/parakeet-rs)
- [NVIDIA Parakeet TDT v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3)
- [ONNX Parakeet TDT v3](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx)
