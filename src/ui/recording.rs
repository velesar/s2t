use crate::audio::AudioRecorder;
use crate::config::Config;
use crate::history::{save_history, History, HistoryEntry};
use crate::whisper::WhisperSTT;
use gtk4::prelude::*;
use gtk4::{glib, Button, Label, LevelBar, Spinner, TextView};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::state::AppState;

const WHISPER_SAMPLE_RATE: usize = 16000;
const MIN_RECORDING_SAMPLES: usize = WHISPER_SAMPLE_RATE; // 1 second

/// Start dictation recording
pub fn handle_start(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    level_bar: &LevelBar,
    recorder: &Arc<AudioRecorder>,
    whisper: &Arc<Mutex<Option<WhisperSTT>>>,
    app_state: &Rc<Cell<AppState>>,
    recording_start_time: &Rc<Cell<Option<Instant>>>,
) {
    {
        let w = whisper.lock().unwrap();
        if w.is_none() {
            status_label.set_text("Модель не завантажено. Натисніть 'Моделі'.");
            return;
        }
    }

    match recorder.start_recording() {
        Ok(()) => {
            app_state.set(AppState::Recording);
            recording_start_time.set(Some(Instant::now()));

            button.set_label("Зупинити запис");
            button.remove_css_class("suggested-action");
            button.add_css_class("destructive-action");
            status_label.set_text("Запис...");
            let buffer = result_text_view.buffer();
            buffer.set_text("");

            // Show timer and level bar
            timer_label.set_text("00:00");
            timer_label.set_visible(true);
            level_bar.set_value(0.0);
            level_bar.set_visible(true);

            // Start timer update loop (1 second interval)
            let timer_label_clone = timer_label.clone();
            let app_state_clone = app_state.clone();
            let recording_start_time_clone = recording_start_time.clone();
            glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
                if app_state_clone.get() != AppState::Recording {
                    return glib::ControlFlow::Break;
                }
                if let Some(start) = recording_start_time_clone.get() {
                    let elapsed = start.elapsed().as_secs();
                    let minutes = elapsed / 60;
                    let seconds = elapsed % 60;
                    timer_label_clone.set_text(&format!("{:02}:{:02}", minutes, seconds));
                }
                glib::ControlFlow::Continue
            });

            // Start level bar update loop (faster interval for smooth visualization)
            let level_bar_clone = level_bar.clone();
            let recorder_clone = recorder.clone();
            let app_state_clone = app_state.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                if app_state_clone.get() != AppState::Recording {
                    return glib::ControlFlow::Break;
                }
                let amplitude = recorder_clone.get_amplitude();
                level_bar_clone.set_value(amplitude as f64);
                glib::ControlFlow::Continue
            });
        }
        Err(e) => {
            status_label.set_text(&format!("Помилка: {}", e));
        }
    }
}

/// Stop dictation recording and transcribe
pub fn handle_stop(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    level_bar: &LevelBar,
    spinner: &Spinner,
    recorder: &Arc<AudioRecorder>,
    whisper: &Arc<Mutex<Option<WhisperSTT>>>,
    config: &Arc<Mutex<Config>>,
    history: &Arc<Mutex<History>>,
    app_state: &Rc<Cell<AppState>>,
    recording_start_time: &Rc<Cell<Option<Instant>>>,
) {
    // Transition to Processing state
    app_state.set(AppState::Processing);
    recording_start_time.set(None);

    // Update UI for processing state
    button.set_label("Обробка...");
    button.remove_css_class("destructive-action");
    button.remove_css_class("suggested-action");
    button.set_sensitive(false);
    status_label.set_text("Обробка...");
    timer_label.set_visible(false);
    level_bar.set_visible(false);
    spinner.set_visible(true);
    spinner.start();

    let (samples, completion_rx) = recorder.stop_recording();

    // Calculate and display recording duration
    let duration_secs = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;
    let duration_mins = (duration_secs / 60.0).floor() as u32;
    let duration_remaining_secs = (duration_secs % 60.0).floor() as u32;
    status_label.set_text(&format!(
        "Обробка запису {:02}:{:02}...",
        duration_mins, duration_remaining_secs
    ));

    let whisper = whisper.clone();
    let history = history.clone();
    let config_for_auto_copy = config.clone();
    let status_label = status_label.clone();
    let result_text_view = result_text_view.clone();
    let button = button.clone();
    let spinner = spinner.clone();
    let app_state = app_state.clone();
    let language = {
        let cfg = config.lock().unwrap();
        cfg.language.clone()
    };
    let language_for_history = language.clone();

    glib::spawn_future_local(async move {
        // Wait for recording thread to finish (non-blocking for GTK)
        if let Some(rx) = completion_rx {
            let _ = rx.recv().await;
        }

        // Now transcribe in a separate thread
        let (tx, rx) = async_channel::bounded::<anyhow::Result<String>>(1);

        std::thread::spawn(move || {
            let result = if samples.len() < MIN_RECORDING_SAMPLES {
                Err(anyhow::anyhow!("Запис закороткий"))
            } else {
                let w = whisper.lock().unwrap();
                if let Some(ref whisper) = *w {
                    whisper.transcribe(&samples, Some(&language))
                } else {
                    Err(anyhow::anyhow!("Модель не завантажено"))
                }
            };
            let _ = tx.send_blocking(result);
        });

        if let Ok(result) = rx.recv().await {
            match result {
                Ok(text) => {
                    if text.is_empty() {
                        status_label.set_text("Не вдалося розпізнати мову");
                    } else {
                        status_label.set_text("Готово!");
                        let buffer = result_text_view.buffer();
                        buffer.set_text(&text);

                        // Get config values for auto-copy/auto-paste
                        let (auto_copy_enabled, auto_paste_enabled) = {
                            let cfg = config_for_auto_copy.lock().unwrap();
                            (cfg.auto_copy, cfg.auto_paste)
                        };

                        // Copy to clipboard if auto-copy or auto-paste is enabled
                        if auto_copy_enabled || auto_paste_enabled {
                            super::copy_to_clipboard(&text);
                        }

                        // Auto-paste if enabled (simulates Ctrl+V)
                        if auto_paste_enabled {
                            // Small delay to ensure clipboard is ready
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if let Err(e) = crate::paste::paste_from_clipboard() {
                                eprintln!("Помилка автоматичної вставки: {}", e);
                                status_label.set_text(&format!("Готово! (помилка вставки: {})", e));
                            }
                        }

                        // Save to history
                        let entry = HistoryEntry::new(
                            text,
                            duration_secs,
                            language_for_history.clone(),
                        );
                        let mut h = history.lock().unwrap();
                        h.add(entry);
                        if let Err(e) = save_history(&h) {
                            eprintln!("Помилка збереження історії: {}", e);
                        }
                    }
                }
                Err(e) => {
                    status_label.set_text(&format!("Помилка: {}", e));
                }
            }
        }

        // Transition back to Idle state
        app_state.set(AppState::Idle);
        spinner.stop();
        spinner.set_visible(false);
        button.set_label("Почати запис");
        button.add_css_class("suggested-action");
        button.set_sensitive(true);
    });
}
