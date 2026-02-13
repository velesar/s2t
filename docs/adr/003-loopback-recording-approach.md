# ADR-003: Loopback Recording Implementation Approach

## Status
**Accepted** (Updated 2026-01-28)

## Test Results Summary (2026-01-28)

### ❌ Option 1 (CPAL + Monitor Sources) — DOES NOT WORK

**Tested on:** Fedora 41 (PipeWire)

**Finding:** CPAL does NOT see PipeWire/PulseAudio monitor sources as input devices.

```
# Available via pactl:
alsa_output.pci-0000_00_1f.3.analog-stereo.monitor  ← EXISTS

# CPAL sees only:
pipewire, default, sysdefault:CARD=PCH, front:CARD=PCH,DEV=0...  ← NO .monitor
```

### ✅ Recommended: Option 3 (PipeWire Direct API)

The `pipewire` crate (v0.9.2) provides direct access to monitor sources via Stream API.

## Context
Для функції запису конференцій потрібно записувати одночасно:
1. **Input**: Мікрофон користувача (вже реалізовано через cpal)
2. **Output**: Системний аудіо (звук з навушників/динаміків) - loopback recording

На Linux це складна задача, оскільки:
- Різні аудіо системи: ALSA, PulseAudio, PipeWire
- CPAL (поточна бібліотека) не має нативної підтримки loopback на Linux
- Потрібно синхронізувати два аудіо потоки

## Research Findings

### 1. CPAL Limitations ⚠️ CONFIRMED

- **Issue #906**: Відкритий запит на loopback support для Linux
- **Поточний стан**: CPAL підтримує loopback на Windows і macOS, але не на Linux
- **Причина**: Складність абстракції різних Linux аудіо систем (ALSA, PulseAudio, PipeWire)
- **PR #938**: PipeWire implementation для CPAL — ще не merged (станом на 2026-01)

**✅ Протестовано 2026-01-28 на Fedora 41:**
```rust
// CPAL enumeration test
let host = cpal::default_host();
for device in host.input_devices()? {
    println!("{}", device.name()?);
}
// Output: pipewire, default, sysdefault:CARD=PCH, front:CARD=PCH,DEV=0...
// ❌ NO monitor sources visible!
```

**Висновок:** Option 1 (CPAL + monitor sources) **не працює** на сучасних Linux системах.

### 2. PulseAudio Approach
**Monitor Sources** - віртуальні input пристрої, які записують вихід конкретного sink.

**Як це працює:**
- Кожен PulseAudio sink має відповідний monitor source
- Monitor source має назву формату `sink_name.monitor`
- Можна записувати з monitor source як зі звичайного input пристрою

**Приклад:**
```bash
# Перелік monitor sources
pactl list sources | grep -A 10 "monitor"

# Запис з monitor source
parec -d alsa_output.pci-0000_00_1f.3.analog-stereo.monitor | oggenc -o output.ogg -
```

**Переваги:**
- Нативна підтримка в PulseAudio
- Не потребує додаткових модулів
- Працює з будь-яким sink

**Недоліки:**
- Потрібно знати назву sink (може змінюватися)
- CPAL може не бачити monitor sources як input devices

### 3. PipeWire Approach ⭐ RECOMMENDED

**Стан підтримки:**
- PipeWire замінює PulseAudio на сучасних дистрибутивах (Fedora 34+, Ubuntu 22.04+)
- CPAL має PR #938 для PipeWire support, але ще не merged
- **`pipewire` crate (v0.9.2)** — офіційні Rust bindings

**Можливості:**
- Прямий доступ до PipeWire Stream API
- Можна створити capture stream з `MEDIA_CATEGORY = "Capture"`
- Підтримка monitor streams
- Loopback module для routing audio

**Rust Crates (досліджено 2026-01-28):**

| Crate | Version | Status | Notes |
|-------|---------|--------|-------|
| `pipewire` | 0.9.2 | ✅ Stable | Official bindings, recommended |
| `pipewire-native` | 0.1.x | ⚠️ Experimental | Pure Rust, unstable API |

**Залежності:**
```toml
pipewire = "0.9"
```

**Приклад capture stream:**
```rust
use pipewire as pw;

let props = pw::properties! {
    *pw::keys::MEDIA_TYPE => "Audio",
    *pw::keys::MEDIA_CATEGORY => "Capture",
    *pw::keys::MEDIA_ROLE => "Music",
};

let stream = pw::stream::Stream::new(&core, "loopback-capture", props)?;
// Connect to monitor source...
```

**CLI test:**
```bash
# Створення loopback device (PipeWire 0.3.25+)
pw-loopback --capture-props="media.class=Audio/Source"

# List available sources (includes monitors)
pactl list sources short
# → alsa_output.pci-0000_00_1f.3.analog-stereo.monitor  ← TARGET
```

### 4. ALSA Approach
**snd-aloop module:**
- Kernel module для створення віртуальних loopback пристроїв
- Більш низькорівневий підхід
- Потребує налаштування конфігурації ALSA

**Недоліки:**
- Складніше в налаштуванні
- Менш гнучкий, ніж PulseAudio/PipeWire
- Може не працювати з PipeWire

## Decision Options

### Option 1: CPAL + PulseAudio Monitor Sources ❌ DOES NOT WORK
**Підхід:**
- Використовувати CPAL для мікрофона (як зараз)
- Для loopback: використовувати PulseAudio monitor sources через CPAL
- Перевіряти, чи CPAL бачить monitor sources як input devices

**Реалізація:**
```rust
// Enumerate all input devices
let devices = host.input_devices()?;
for device in devices {
    let name = device.name()?;
    // Look for monitor sources (name contains ".monitor")
    if name.contains(".monitor") {
        // Use this device for loopback recording
    }
}
```

**⚠️ Test Result (2026-01-28, Fedora 41):**
- CPAL **does NOT** see `.monitor` sources
- Only ALSA devices visible: `pipewire`, `default`, `sysdefault:CARD=PCH`...
- Monitor source exists in PipeWire but not exposed to CPAL

**Висновок:** ❌ **Відхилено** — не працює на сучасних Linux системах з PipeWire.

### Option 2: Direct PulseAudio API
**Підхід:**
- Використовувати `libpulse` bindings для Rust (`libpulse-simple` або `pulse-binding-rs`)
- Прямий доступ до PulseAudio API
- Окремий потік для loopback recording

**Залежності:**
```toml
pulse-binding-rs = "0.1"  # або
libpulse-simple = "0.1"
```

**Переваги:**
- Повний контроль над PulseAudio
- Гарантована підтримка monitor sources
- Можна створювати null sinks для ізоляції

**Недоліки:**
- Додаткова залежність
- Більше коду для підтримки
- PulseAudio-specific (не працює на PipeWire без compatibility)

### Option 3: PipeWire Direct API ⭐ RECOMMENDED
**Підхід:**
- Використовувати PipeWire Rust bindings напряму
- Створювати capture streams для системного аудіо
- CPAL залишається для мікрофона (працює)

**Залежності:**
```toml
pipewire = "0.9"  # latest stable (2026-01)
```

**Переваги:**
- ✅ Майбутнє-орієнтований підхід (PipeWire - стандарт Linux audio)
- ✅ Потужний API з багатьма можливостями
- ✅ Працює на сучасних дистрибутивах (Fedora 34+, Ubuntu 22.04+)
- ✅ Прямий доступ до monitor sources
- ✅ Офіційні bindings з хорошою документацією

**Недоліки:**
- ⚠️ Додаткова залежність (libpipewire-dev)
- ⚠️ Може не працювати на старих системах з pure PulseAudio
- ⚠️ Потребує системних бібліотек PipeWire

**Реалізація:**
```rust
use pipewire as pw;
use pw::stream::{Stream, StreamFlags};

// Create capture stream for monitor source
let props = pw::properties! {
    *pw::keys::MEDIA_TYPE => "Audio",
    *pw::keys::MEDIA_CATEGORY => "Capture",
    *pw::keys::MEDIA_ROLE => "Music",
    *pw::keys::NODE_TARGET => "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor",
};

let stream = Stream::new(&core, "conference-loopback", props)?;
stream.connect(
    pw::spa::Direction::Input,
    None,
    StreamFlags::AUTOCONNECT | StreamFlags::MAP_BUFFERS,
    &mut [],
)?;
```

**Висновок:** ✅ **Рекомендовано** для production на сучасних Linux системах.

### Option 4: Hybrid Approach (Recommended for Production)
**Підхід:**
- Використовувати CPAL для мікрофона
- Детектувати аудіо систему (PulseAudio vs PipeWire)
- Використовувати відповідний API для loopback:
  - PulseAudio: `pulse-binding-rs` або monitor sources через CPAL
  - PipeWire: PipeWire bindings або через PulseAudio compatibility layer

**Реалізація:**
```rust
enum AudioBackend {
    PulseAudio,
    PipeWire,
    ALSA,
}

fn detect_backend() -> AudioBackend {
    // Check for PipeWire
    if std::path::Path::new("/usr/bin/pw-cli").exists() {
        return AudioBackend::PipeWire;
    }
    // Check for PulseAudio
    if std::path::Path::new("/usr/bin/pactl").exists() {
        return AudioBackend::PulseAudio;
    }
    AudioBackend::ALSA
}
```

**Переваги:**
- Максимальна сумісність
- Працює на різних системах
- Можна використовувати найкращий підхід для кожної системи

**Недоліки:**
- Найскладніша реалізація
- Потрібна підтримка кількох API
- Більше коду

## Recommended Decision (Updated 2026-01-28)

### ✅ ACCEPTED: Option 3 — PipeWire Direct API

**Обґрунтування:**
- ❌ Option 1 (CPAL + monitor) **не працює** — протестовано на Fedora 41
- ✅ PipeWire — стандарт для сучасних Linux дистрибутивів
- ✅ `pipewire` crate (v0.9.2) — стабільний, добре документований
- ✅ Прямий доступ до monitor sources
- ✅ CPAL залишається для мікрофона (працює)

**Залежності:**
```toml
[dependencies]
pipewire = "0.9"

[target.'cfg(target_os = "linux")'.dependencies]
pipewire = "0.9"
```

**System requirements:**
```bash
# Fedora
sudo dnf install pipewire-devel

# Ubuntu/Debian
sudo apt install libpipewire-0.3-dev
```

### Fallback Strategy

Для старих систем з pure PulseAudio (без PipeWire):
1. Детектувати наявність PipeWire
2. Якщо немає — використовувати Option 2 (pulse-binding-rs)
3. Показати warning користувачу

```rust
fn has_pipewire() -> bool {
    std::process::Command::new("pw-cli")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

## Implementation Plan (Updated 2026-01-28)

### Phase 1: PipeWire PoC ✅ CURRENT
1. ~~Тестувати CPAL з monitor sources~~ ❌ Не працює
2. Додати `pipewire = "0.9"` залежність
3. Створити `LoopbackRecorder` модуль для capture з monitor source
4. Тестувати запис системного аудіо на Fedora 41

### Phase 2: ConferenceRecorder
1. Створити `ConferenceRecorder` struct:
   - CPAL для мікрофона (існуючий код)
   - PipeWire для loopback (новий)
2. Синхронізувати два потоки (timestamps)
3. Зберігати в окремі буфери для diarization

### Phase 3: Integration
1. Інтегрувати з `ConferenceTranscriber` (ADR-004)
2. Додати UI для вибору режиму запису
3. Зберігати аудіо файли на диск

### Phase 4: Fallback (Optional)
1. Детектувати наявність PipeWire
2. Якщо немає — fallback на `pulse-binding-rs`
3. Показати warning для unsupported систем

### Code Structure
```
src/
├── audio.rs              # Existing mic recording (CPAL)
├── loopback.rs           # NEW: PipeWire loopback capture
├── conference_recorder.rs # NEW: Dual recording (mic + loopback)
└── ...
```

## Testing Requirements (Updated 2026-01-28)

### Completed ✅
1. **Fedora 41 (PipeWire)**: CPAL не бачить monitor sources
2. **Monitor source available**: `alsa_output.pci-0000_00_1f.3.analog-stereo.monitor`

### TODO
1. **PipeWire capture test**: Записати системний аудіо через `pipewire` crate
2. **Dual recording test**: Одночасний запис мікрофона (CPAL) + loopback (PipeWire)
3. **Синхронізація**: Перевірити timestamp alignment між потоками
4. **Різні конфігурації**: Кілька sinks, HDMI audio, USB audio
5. **Ubuntu 22.04+**: Тестувати на іншому дистрибутиві з PipeWire

### Edge Cases
1. Немає системного аудіо (тиша)
2. Зміна audio sink під час запису
3. Headphones vs speakers

## Consequences

### Positive
- ✅ Можливість записувати системний аудіо
- ✅ Підтримка запису конференцій
- ✅ PipeWire — стандарт для сучасних Linux
- ✅ Офіційні Rust bindings з активною підтримкою

### Negative
- ⚠️ Додаткова залежність (pipewire crate + system lib)
- ⚠️ Не працює на старих системах без PipeWire
- ⚠️ Потрібне тестування на різних конфігураціях
- ⚠️ Можливі проблеми з синхронізацією потоків

### Risks
- ⚠️ PipeWire API може змінитися (minor versions)
- ⚠️ Різні версії PipeWire на різних дистрибутивах

## Related Files
- [docs/research/loopback-recording-test.md](../research/loopback-recording-test.md) - Test results
- [docs/adr/004-speaker-diarization-approach.md](004-speaker-diarization-approach.md) - Speaker diarization ADR
- [src/audio.rs](../../src/audio.rs) - Current audio recording implementation
- [Cargo.toml](../../Cargo.toml) - Dependencies

## References

### PipeWire (Recommended)
- [pipewire crate on crates.io](https://crates.io/crates/pipewire) - v0.9.2
- [pipewire-rs documentation](https://pipewire.pages.freedesktop.org/pipewire-rs/pipewire/)
- [PipeWire Rust Tutorial](https://acalustra.com/playing-with-pipewire-audio-streams-and-rust.html)
- [PipeWire Loopback Module](https://docs.pipewire.org/page_module_loopback.html)
- [PipeWire Workshop 2025](https://www.collabora.com/news-and-blog/blog/2025/07/03/pipewire-workshop-2025-updates-video-transport-rust-bluetooth/)

### CPAL
- [CPAL GitHub Issue #906](https://github.com/RustAudio/cpal/issues/906) - Loopback support request
- [CPAL PR #938](https://github.com/RustAudio/cpal/pull/938) - PipeWire implementation (not merged)

### Other
- [PulseAudio Monitor Sources](https://wiki.ubuntu.com/record_system_sound)
- [ALSA Loopback Module](https://wiki.debian.org/audio-loopback)
