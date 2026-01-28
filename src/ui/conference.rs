use crate::conference_recorder::ConferenceRecorder;
use crate::config::Config;
use crate::history::{save_history, History, HistoryEntry};
use crate::recordings::{ensure_recordings_dir, generate_recording_filename, recording_path, save_recording};
use crate::whisper::WhisperSTT;
use gtk4::prelude::*;
use gtk4::{glib, Button, Label, LevelBar, Spinner, TextView};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::state::AppState;

/// Start conference recording (mic + loopback)
pub fn handle_start(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    mic_level_bar: &LevelBar,
    loopback_level_bar: &LevelBar,
    conference_recorder: &Arc<ConferenceRecorder>,
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

    match conference_recorder.start_conference() {
        Ok(()) => {
            app_state.set(AppState::Recording);
            recording_start_time.set(Some(Instant::now()));

            button.set_label("Зупинити запис");
            button.remove_css_class("suggested-action");
            button.add_css_class("destructive-action");
            status_label.set_text("Запис конференції...");
            let buffer = result_text_view.buffer();
            buffer.set_text("");

            // Show timer and level bars
            timer_label.set_text("00:00");
            timer_label.set_visible(true);
            mic_level_bar.set_value(0.0);
            mic_level_bar.set_visible(true);
            loopback_level_bar.set_value(0.0);
            loopback_level_bar.set_visible(true);

            // Start timer update loop
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

            // Start level bar update loops for both channels
            let mic_level_bar_clone = mic_level_bar.clone();
            let loopback_level_bar_clone = loopback_level_bar.clone();
            let conference_recorder_clone = conference_recorder.clone();
            let app_state_clone = app_state.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                if app_state_clone.get() != AppState::Recording {
                    return glib::ControlFlow::Break;
                }
                let mic_amplitude = conference_recorder_clone.get_mic_amplitude();
                let loopback_amplitude = conference_recorder_clone.get_loopback_amplitude();
                mic_level_bar_clone.set_value(mic_amplitude as f64);
                loopback_level_bar_clone.set_value(loopback_amplitude as f64);
                glib::ControlFlow::Continue
            });
        }
        Err(e) => {
            status_label.set_text(&format!("Помилка: {}", e));
        }
    }
}

/// Stop conference recording and transcribe with diarization
pub fn handle_stop(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    mic_level_bar: &LevelBar,
    loopback_level_bar: &LevelBar,
    spinner: &Spinner,
    conference_recorder: &Arc<ConferenceRecorder>,
    whisper: &Arc<Mutex<Option<WhisperSTT>>>,
    config: &Arc<Mutex<Config>>,
    history: &Arc<Mutex<History>>,
    diarization_engine: &Arc<Mutex<crate::diarization::DiarizationEngine>>,
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
    mic_level_bar.set_visible(false);
    loopback_level_bar.set_visible(false);
    spinner.set_visible(true);
    spinner.start();

    let (mic_samples, loopback_samples, mic_completion_rx, loopback_completion_rx) =
        conference_recorder.stop_conference();

    // Calculate duration
    let duration_secs = mic_samples.len().max(loopback_samples.len()) as f32 / 16000.0;
    let duration_mins = (duration_secs / 60.0).floor() as u32;
    let duration_remaining_secs = (duration_secs % 60.0).floor() as u32;
    status_label.set_text(&format!(
        "Обробка запису {:02}:{:02}...",
        duration_mins, duration_remaining_secs
    ));

    // Ensure recordings directory exists
    if let Err(e) = ensure_recordings_dir() {
        eprintln!("Помилка створення директорії записів: {}", e);
    }

    let whisper = whisper.clone();
    let history = history.clone();
    let config_for_auto_copy = config.clone();
    let diarization_engine_for_transcribe = diarization_engine.clone();
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
        // Wait for both recording threads to finish
        if let Some(rx) = mic_completion_rx {
            let _ = rx.recv().await;
        }
        if let Some(rx) = loopback_completion_rx {
            let _ = rx.recv().await;
        }

        // Save audio file
        let filename = generate_recording_filename();
        let recording_path = recording_path(&filename);
        if let Err(e) = save_recording(&mic_samples, &loopback_samples, &recording_path) {
            eprintln!("Помилка збереження аудіо файлу: {}", e);
        }

        // Get diarization method from config
        let diarization_method = {
            let cfg = config_for_auto_copy.lock().unwrap();
            cfg.diarization_method.clone()
        };

        // Transcribe with diarization (auto-selects method)
        let (tx, rx) = async_channel::bounded::<anyhow::Result<String>>(1);

        std::thread::spawn(move || {
            let w = whisper.lock().unwrap();
            if let Some(ref whisper) = *w {
                let mut engine_guard = diarization_engine_for_transcribe.lock().unwrap();
                let result = whisper.transcribe_with_auto_diarization(
                    &mic_samples,
                    &loopback_samples,
                    Some(&language),
                    &diarization_method,
                    Some(&mut *engine_guard),
                );
                let _ = tx.send_blocking(result);
            } else {
                let _ = tx.send_blocking(Err(anyhow::anyhow!("Модель не завантажено")));
            }
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

                        // Copy to clipboard if enabled
                        if auto_copy_enabled || auto_paste_enabled {
                            super::copy_to_clipboard(&text);
                        }

                        // Auto-paste if enabled
                        if auto_paste_enabled {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if let Err(e) = crate::paste::paste_from_clipboard() {
                                eprintln!("Помилка автоматичної вставки: {}", e);
                                status_label.set_text(&format!("Готово! (помилка вставки: {})", e));
                            }
                        }

                        // Save to history with recording metadata
                        let speakers = vec!["Ви".to_string(), "Учасник".to_string()];
                        let entry = HistoryEntry::new_with_recording(
                            text,
                            duration_secs,
                            language_for_history.clone(),
                            Some(recording_path.to_string_lossy().to_string()),
                            speakers,
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
