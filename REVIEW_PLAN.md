# План ревью коду та архітектури

## Огляд проекту

**Voice Dictation** — Linux застосунок для офлайн розпізнавання мовлення через Whisper.
- **Стек:** Rust, GTK4, CPAL, whisper-rs, KSNI
- **LOC:** ~1200 рядків
- **Тести:** 20 unit-тестів (config, models, audio)

---

## 1. Архітектурне ревью

### 1.1 Модульна структура

| Модуль | Відповідальність | Залежності | Оцінка |
|--------|------------------|------------|--------|
| `main.rs` | Точка входу, ініціалізація | Всі модулі | Перевантажений |
| `audio.rs` | Запис аудіо | cpal, rubato | OK |
| `whisper.rs` | STT транскрипція | whisper-rs | OK |
| `config.rs` | Конфігурація | serde, toml, dirs | OK |
| `models.rs` | Завантаження моделей | reqwest, tokio | OK |
| `ui.rs` | GTK вікно | gtk4 | Потребує рефакторингу |
| `tray.rs` | Системний трей | ksni | OK |
| `model_dialog.rs` | Діалог моделей | gtk4 | Великий, складний |

### 1.2 Питання для обговорення

- [ ] **Розділення `main.rs`**: Винести `find_model_path()` в `models.rs`?
- [ ] **UI рефакторинг**: Розбити `ui.rs` на компоненти?
- [ ] **Error handling**: Уніфікувати повідомлення про помилки
- [ ] **Тестування UI**: Додати integration tests?

---

## 2. Ревью безпеки

### 2.1 Критичні точки

| Область | Ризик | Статус |
|---------|-------|--------|
| Завантаження моделей | MITM, corrupted files | HTTPS, але без checksum |
| Файлова система | Path traversal | Використовуються фіксовані шляхи |
| Конфігурація | Injection | TOML парсер безпечний |
| Аудіо | Privacy | Локальна обробка, OK |

### 2.2 Рекомендації

- [ ] Додати SHA256 checksum для моделей
- [ ] Валідувати розмір завантаженого файлу
- [ ] Логувати помилки без sensitive data

---

## 3. Ревью продуктивності

### 3.1 Потенційні bottlenecks

| Компонент | Проблема | Пріоритет |
|-----------|----------|-----------|
| Ресемплінг | CPU-intensive в callback | Середній |
| Whisper | Блокує thread | Низький (вже async) |
| Model loading | ~2-5 сек для large | Низький |

### 3.2 Рекомендації

- [ ] Профілювання з `perf` або `flamegraph`
- [ ] Розглянути GPU acceleration для whisper
- [ ] Кешування моделі в пам'яті

---

## 4. Ревью коду

### 4.1 Файли для детального ревью

#### Високий пріоритет
- [ ] `audio.rs:70-132` — recording thread та callback
- [ ] `models.rs:88-141` — download_model async flow
- [ ] `ui.rs:157-218` — handle_stop_recording async chain

#### Середній пріоритет
- [ ] `model_dialog.rs` — складна state machine
- [ ] `tray.rs:60-100` — model selection logic

### 4.2 Code smells для перевірки

- [ ] Unwrap() без context — пошук `unwrap()` без `.expect()`
- [ ] Clone() overhead — зайве клонування Arc
- [ ] Mutex contention — перевірити lock ordering
- [ ] Error swallowing — `let _ =` приховує помилки

---

## 5. Тестове покриття

### 5.1 Поточний стан

| Модуль | Тести | Покриття |
|--------|-------|----------|
| `config.rs` | 5 | Базове |
| `models.rs` | 9 | Добре |
| `audio.rs` | 6 | Тільки state |
| `whisper.rs` | 0 | Потребує mock |
| `ui.rs` | 0 | Складно тестувати |
| `tray.rs` | 0 | Складно тестувати |

### 5.2 Рекомендовані тести

- [ ] Integration test: record → transcribe flow (з mock audio)
- [ ] `models.rs`: test download with mock server
- [ ] `config.rs`: test save/load roundtrip з temp dir
- [ ] `whisper.rs`: test з тестовим .bin файлом (якщо є tiny)

---

## 6. Чеклист для ревью

### Перед релізом

- [ ] Всі тести проходять (`cargo test`)
- [ ] Немає warnings (`cargo clippy`)
- [ ] Форматування (`cargo fmt --check`)
- [ ] Release build працює (`cargo build --release`)
- [ ] Manual testing на Fedora

### Документація

- [ ] README з інструкцією встановлення
- [ ] Опис конфігурації
- [ ] Troubleshooting guide

---

## 7. Графік ревью

| Етап | Тривалість | Результат |
|------|------------|-----------|
| 1. Архітектура | 1 день | Architectural Decision Records |
| 2. Безпека | 0.5 дня | Security checklist |
| 3. Code review | 1-2 дні | PR comments / issues |
| 4. Тести | 1 день | Coverage report |
| 5. Документація | 0.5 дня | README, CONTRIBUTING |

---

## 8. Інструменти для ревью

```bash
# Статичний аналіз
cargo clippy -- -W clippy::all

# Форматування
cargo fmt --check

# Тести з покриттям (потребує cargo-tarpaulin)
cargo tarpaulin --out Html

# Залежності з вразливостями
cargo audit

# Невикористаний код
cargo +nightly udeps
```

---

*Створено: 2026-01-27*
*Проект: voice-dictation v0.1.0*
