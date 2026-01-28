//! Dictation mode recording handler.
//!
//! This module handles start/stop recording for single dictation sessions.

use crate::context::AppContext;
use crate::history::{save_history, HistoryEntry};
use gtk4::glib;
use std::sync::Arc;

use super::state::{DictationUI, RecordingContext};

const WHISPER_SAMPLE_RATE: usize = 16000;
const MIN_RECORDING_SAMPLES: usize = WHISPER_SAMPLE_RATE; // 1 second

/// Start dictation recording
pub fn handle_start(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &DictationUI) {
    // Check if model is loaded
    if !ctx.is_model_loaded() {
        ui.base.status_label.set_text("Модель не завантажено. Натисніть 'Моделі'.");
        return;
    }

    match ctx.audio.start_dictation() {
        Ok(()) => {
            rec.start_recording();

            ui.base.set_recording("Запис...");
            ui.show_level_bar();

            // Start timer update loop (1 second interval)
            let rec_clone = rec.clone();
            let ui_clone = ui.clone();
            glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
                if !rec_clone.is_recording() {
                    return glib::ControlFlow::Break;
                }
                if let Some(secs) = rec_clone.elapsed_secs() {
                    ui_clone.base.update_timer(secs);
                }
                glib::ControlFlow::Continue
            });

            // Start level bar update loop (faster interval for smooth visualization)
            let ctx_clone = ctx.clone();
            let rec_clone = rec.clone();
            let ui_clone = ui.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                if !rec_clone.is_recording() {
                    return glib::ControlFlow::Break;
                }
                let amplitude = ctx_clone.audio.get_dictation_amplitude();
                ui_clone.update_level(amplitude as f64);
                glib::ControlFlow::Continue
            });
        }
        Err(e) => {
            ui.base.status_label.set_text(&format!("Помилка: {}", e));
        }
    }
}

/// Stop dictation recording and transcribe
pub fn handle_stop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &DictationUI) {
    rec.start_processing();

    // Update UI for processing state
    ui.base.set_processing("Обробка...");
    ui.hide_level_bar();

    let (samples, completion_rx) = ctx.audio.stop_dictation();

    // Calculate and display recording duration
    let duration_secs = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;
    let duration_mins = (duration_secs / 60.0).floor() as u32;
    let duration_remaining_secs = (duration_secs % 60.0).floor() as u32;
    ui.base.status_label.set_text(&format!(
        "Обробка запису {:02}:{:02}...",
        duration_mins, duration_remaining_secs
    ));

    // Clone what we need for the async block
    let ctx = ctx.clone();
    let rec = rec.clone();
    let ui = ui.clone();
    let language = ctx.language();

    glib::spawn_future_local(async move {
        // Wait for recording thread to finish (non-blocking for GTK)
        if let Some(rx) = completion_rx {
            let _ = rx.recv().await;
        }

        // Now transcribe in a separate thread
        let (tx, rx) = async_channel::bounded::<anyhow::Result<String>>(1);

        let ctx_for_thread = ctx.clone();
        let language_for_thread = language.clone();
        std::thread::spawn(move || {
            let result = if samples.len() < MIN_RECORDING_SAMPLES {
                Err(anyhow::anyhow!("Запис закороткий"))
            } else {
                let ts = ctx_for_thread.transcription.lock().unwrap();
                ts.transcribe(&samples, &language_for_thread)
            };
            let _ = tx.send_blocking(result);
        });

        if let Ok(result) = rx.recv().await {
            match result {
                Ok(text) => {
                    if text.is_empty() {
                        ui.base.status_label.set_text("Не вдалося розпізнати мову");
                    } else {
                        ui.base.status_label.set_text("Готово!");
                        ui.base.set_result_text(&text);

                        // Get config values for auto-copy/auto-paste
                        let auto_copy_enabled = ctx.auto_copy();
                        let auto_paste_enabled = ctx.auto_paste();

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
                                ui.base.status_label.set_text(&format!("Готово! (помилка вставки: {})", e));
                            }
                        }

                        // Save to history
                        let entry = HistoryEntry::new(
                            text,
                            duration_secs,
                            language.clone(),
                        );
                        let mut h = ctx.history.lock().unwrap();
                        h.add(entry);
                        if let Err(e) = save_history(&h) {
                            eprintln!("Помилка збереження історії: {}", e);
                        }
                    }
                }
                Err(e) => {
                    ui.base.status_label.set_text(&format!("Помилка: {}", e));
                }
            }
        }

        // Transition back to Idle state
        rec.finish();
        ui.base.set_idle();
    });
}
