# ADR-003: Loopback Recording Implementation Approach

## Status
Proposed

## Context
Для функції запису конференцій потрібно записувати одночасно:
1. **Input**: Мікрофон користувача (вже реалізовано через cpal)
2. **Output**: Системний аудіо (звук з навушників/динаміків) - loopback recording

На Linux це складна задача, оскільки:
- Різні аудіо системи: ALSA, PulseAudio, PipeWire
- CPAL (поточна бібліотека) не має нативної підтримки loopback на Linux
- Потрібно синхронізувати два аудіо потоки

## Research Findings

### 1. CPAL Limitations
- **Issue #906**: Відкритий запит на loopback support для Linux
- **Поточний стан**: CPAL підтримує loopback на Windows і macOS, але не на Linux
- **Причина**: Складність абстракції різних Linux аудіо систем (ALSA, PulseAudio, PipeWire)

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

### 3. PipeWire Approach
**Стан підтримки:**
- PipeWire замінює PulseAudio на сучасних дистрибутивах (Fedora, Ubuntu 22.04+)
- CPAL має PR #692 для PipeWire support, але ще не merged
- PipeWire має власні Rust bindings через upstream проект

**Можливості:**
- Прямий доступ до PipeWire Stream API
- Можна створити capture stream з `MEDIA_CATEGORY = "Capture"`
- Підтримка monitor streams

**Приклад створення loopback:**
```bash
# Створення loopback device (PipeWire 0.3.25+)
pw-loopback --capture-props="media.class=Audio/Source"
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

### Option 1: CPAL + PulseAudio Monitor Sources (Recommended for MVP)
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

**Переваги:**
- Використовує існуючу бібліотеку (cpal)
- Мінімальні зміни в архітектурі
- Працює на системах з PulseAudio

**Недоліки:**
- Може не працювати, якщо CPAL не бачить monitor sources
- Не працює на PipeWire без PulseAudio compatibility layer
- Потрібно тестувати на різних системах

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

### Option 3: PipeWire Direct API
**Підхід:**
- Використовувати PipeWire Rust bindings напряму
- Створювати capture streams для системного аудіо
- CPAL тільки для мікрофона

**Залежності:**
```toml
pipewire = "0.4"  # або через системні бібліотеки
```

**Переваги:**
- Майбутнє-орієнтований підхід (PipeWire - майбутнє Linux audio)
- Потужний API з багатьма можливостями
- Працює на сучасних дистрибутивах

**Недоліки:**
- Складніша реалізація
- Менше документації та прикладів
- Може не працювати на старих системах з PulseAudio

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

## Recommended Decision

**Для MVP (Minimum Viable Product):**
**Option 1** - Спробувати використати CPAL з PulseAudio monitor sources.

**Обґрунтування:**
- Мінімальні зміни в коді
- Використовує існуючу бібліотеку
- Швидка реалізація
- Якщо не працює, легко перейти на Option 2

**Для Production:**
**Option 4** - Hybrid approach з детекцією аудіо системи.

**Обґрунтування:**
- Максимальна сумісність
- Підтримка як PulseAudio, так і PipeWire
- Майбутнє-орієнтований підхід

## Implementation Plan

### Phase 1: Proof of Concept (Option 1)
1. Додати функцію для переліку всіх input devices через CPAL
2. Знайти monitor sources (devices з назвою, що містить ".monitor")
3. Створити окремий `ConferenceRecorder`, який записує з двох джерел
4. Тестувати на системі з PulseAudio

### Phase 2: Fallback (Option 2)
Якщо Option 1 не працює:
1. Додати `pulse-binding-rs` залежність
2. Реалізувати прямий доступ до PulseAudio monitor sources
3. Інтегрувати з існуючим `AudioRecorder`

### Phase 3: Production (Option 4)
1. Додати детекцію аудіо системи
2. Реалізувати підтримку PipeWire (якщо потрібно)
3. Додати fallback на ALSA (якщо потрібно)

## Testing Requirements

1. **PulseAudio systems**: Fedora (стара версія), Ubuntu (до 22.04)
2. **PipeWire systems**: Fedora 34+, Ubuntu 22.04+
3. **Різні конфігурації**: один/кілька sinks, різні назви пристроїв
4. **Синхронізація**: перевірити, що два потоки синхронізовані

## Consequences

### Positive
- ✅ Можливість записувати системний аудіо
- ✅ Підтримка запису конференцій
- ✅ Гнучкість в обранні підходу

### Negative
- ⚠️ Додаткова складність в коді
- ⚠️ Можливі проблеми сумісності на різних системах
- ⚠️ Потрібне тестування на різних конфігураціях
- ⚠️ Можливі проблеми з синхронізацією потоків

## Related Files
- [docs/backlog/conference-recording.md](../backlog/conference-recording.md) - Feature description
- [src/audio.rs](../../src/audio.rs) - Current audio recording implementation
- [Cargo.toml](../../Cargo.toml) - Dependencies

## References
- [CPAL GitHub Issue #906](https://github.com/RustAudio/cpal/issues/906) - Loopback support request
- [PipeWire Rust Guide](https://acalustra.com/playing-with-pipewire-audio-streams-and-rust.html)
- [PulseAudio Monitor Sources](https://wiki.ubuntu.com/record_system_sound)
- [ALSA Loopback Module](https://wiki.debian.org/audio-loopback)
