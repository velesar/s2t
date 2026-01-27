# Loopback Recording - Тестування та дослідження

## Мета
Визначити найкращий підхід для запису системного аудіо (loopback) на Linux в Rust.

## Тестові сценарії

### 1. Перевірка доступних пристроїв через CPAL

**Код для тестування:**
```rust
use cpal::traits::{DeviceTrait, HostTrait};

fn list_audio_devices() {
    let host = cpal::default_host();
    
    println!("=== Input Devices ===");
    for device in host.input_devices().unwrap() {
        let name = device.name().unwrap();
        println!("Input: {}", name);
        
        if let Ok(config) = device.default_input_config() {
            println!("  Sample rate: {:?}", config.sample_rate());
            println!("  Channels: {}", config.channels());
        }
    }
    
    println!("\n=== Output Devices ===");
    for device in host.output_devices().unwrap() {
        let name = device.name().unwrap();
        println!("Output: {}", name);
    }
}
```

**Очікуваний результат:**
- Чи бачить CPAL monitor sources як input devices?
- Які назви мають monitor sources?
- Чи можна записувати з них?

### 2. Перевірка PulseAudio monitor sources

**Команди для тестування:**
```bash
# Перелік всіх sources (включно з monitor)
pactl list sources short

# Детальна інформація про monitor source
pactl list sources | grep -A 20 "monitor"

# Спробувати записати з monitor source
parec -d alsa_output.pci-0000_00_1f.3.analog-stereo.monitor --file-format=wav test.wav
```

**Очікуваний результат:**
- Список доступних monitor sources
- Назви форматів для пошуку в CPAL
- Чи працює запис через `parec`

### 3. Перевірка PipeWire

**Команди для тестування:**
```bash
# Перевірка, чи використовується PipeWire
pw-cli info

# Перелік nodes (включно з monitor)
pw-cli list-objects | grep -i monitor

# Створення loopback (якщо потрібно)
pw-loopback --capture-props="media.class=Audio/Source"
```

**Очікуваний результат:**
- Чи працює PipeWire на системі
- Як створити capture stream для системного аудіо
- Чи є monitor nodes доступні

### 4. Тест синхронізації двох потоків

**Підхід:**
- Запустити запис мікрофона та loopback одночасно
- Використати системні таймстемпи для синхронізації
- Порівняти затримки між потоками

**Метрики:**
- Різниця в часі старту потоків
- Дрифт часу під час запису
- Як вирівняти потоки під час злиття

## Результати тестування

### Система 1: Fedora 41 (PipeWire)
- [ ] CPAL бачить monitor sources?
- [ ] Які назви пристроїв?
- [ ] Чи працює запис?

### Система 2: Ubuntu 22.04 (PipeWire)
- [ ] CPAL бачить monitor sources?
- [ ] Які назви пристроїв?
- [ ] Чи працює запис?

### Система 3: Ubuntu 20.04 (PulseAudio)
- [ ] CPAL бачить monitor sources?
- [ ] Які назви пристроїв?
- [ ] Чи працює запис?

## Рекомендації після тестування

1. **Якщо CPAL бачить monitor sources:**
   - Використовувати Option 1 (CPAL + monitor sources)
   - Найпростіша реалізація

2. **Якщо CPAL НЕ бачить monitor sources:**
   - Використовувати Option 2 (PulseAudio API) або Option 3 (PipeWire API)
   - Потрібна додаткова залежність

3. **Якщо різні результати на різних системах:**
   - Використовувати Option 4 (Hybrid approach)
   - Детектувати систему та використовувати відповідний API

## Наступні кроки

1. Запустити тести на різних системах
2. Записати результати в цей документ
3. Оновити ADR-003 з результатами
4. Реалізувати обраний підхід
