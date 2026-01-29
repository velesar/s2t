//! Conference mode recording handler.
//!
//! This module handles start/stop recording for conference mode
//! with dual-channel audio (microphone + system loopback) and diarization.

use crate::app::context::AppContext;
use crate::history::{save_history, HistoryEntry};
use crate::domain::traits::{HistoryRepository, UIStateUpdater};
use crate::infrastructure::recordings::{
    ensure_recordings_dir, generate_recording_filename, recording_path, save_recording,
};
use gtk4::glib;
use std::sync::Arc;

use super::state::{ConferenceUI, RecordingContext};

/// Start conference recording (mic + loopback)
pub fn handle_start(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &ConferenceUI) {
    // Check if model is loaded
    if !ctx.is_model_loaded() {
        ui.base
            .set_status("Модель не завантажено. Натисніть 'Моделі'.");
        return;
    }

    match ctx.audio.start_conference() {
        Ok(()) => {
            rec.start_recording();

            ui.base.set_recording("Запис конференції...");
            ui.show_level_bars();

            // Start timer update loop
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

            // Start level bar update loops for both channels
            let ctx_clone = ctx.clone();
            let rec_clone = rec.clone();
            let ui_clone = ui.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                if !rec_clone.is_recording() {
                    return glib::ControlFlow::Break;
                }
                let mic_amplitude = ctx_clone.audio.get_mic_amplitude();
                let loopback_amplitude = ctx_clone.audio.get_loopback_amplitude();
                ui_clone.update_levels(mic_amplitude as f64, loopback_amplitude as f64);
                glib::ControlFlow::Continue
            });
        }
        Err(e) => {
            ui.base.set_status(&format!("Помилка: {}", e));
        }
    }
}

/// Stop conference recording and transcribe with diarization
pub fn handle_stop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &ConferenceUI) {
    rec.start_processing();

    // Update UI for processing state
    ui.base.set_processing("Обробка...");
    ui.hide_level_bars();

    let recording = ctx.audio.stop_conference();

    // Calculate duration using shared type's method
    let duration_secs = recording.duration_secs();
    let duration_mins = (duration_secs / 60.0).floor() as u32;
    let duration_remaining_secs = (duration_secs % 60.0).floor() as u32;
    ui.base.set_status(&format!(
        "Обробка запису {:02}:{:02}...",
        duration_mins, duration_remaining_secs
    ));

    // Ensure recordings directory exists
    if let Err(e) = ensure_recordings_dir() {
        eprintln!("Помилка створення директорії записів: {}", e);
    }

    // Clone what we need for the async block
    let ctx = ctx.clone();
    let rec = rec.clone();
    let ui = ui.clone();
    let language = ctx.language();
    let diarization_method = ctx.diarization_method();

    glib::spawn_future_local(async move {
        // Wait for both recording threads to finish
        if let Some(rx) = recording.mic_completion {
            let _ = rx.recv().await;
        }
        if let Some(rx) = recording.loopback_completion {
            let _ = rx.recv().await;
        }

        // Save audio file
        let filename = generate_recording_filename();
        let file_path = recording_path(&filename);
        if let Err(e) = save_recording(
            &recording.mic_samples,
            &recording.loopback_samples,
            &file_path,
        ) {
            eprintln!("Помилка збереження аудіо файлу: {}", e);
        }

        // Transcribe with diarization
        let (tx, rx) = async_channel::bounded::<anyhow::Result<String>>(1);

        let ctx_for_thread = ctx.clone();
        let mic_samples = recording.mic_samples;
        let loopback_samples = recording.loopback_samples;
        let language_for_thread = language.clone();
        let diarization_method_for_thread = diarization_method.clone();

        std::thread::spawn(move || {
            let ts = ctx_for_thread.transcription.lock().unwrap();
            if let Some(whisper) = ts.whisper() {
                let mut engine_guard = ctx_for_thread.diarization.lock().unwrap();
                let result = whisper.transcribe_with_auto_diarization(
                    &mic_samples,
                    &loopback_samples,
                    Some(&language_for_thread),
                    &diarization_method_for_thread,
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
                        ui.base.set_status("Не вдалося розпізнати мову");
                    } else {
                        ui.base.set_status("Готово!");
                        ui.base.set_result_text(&text);

                        // Get config values for auto-copy/auto-paste
                        let auto_copy_enabled = ctx.auto_copy();
                        let auto_paste_enabled = ctx.auto_paste();

                        // Copy to clipboard if enabled
                        if auto_copy_enabled || auto_paste_enabled {
                            super::copy_to_clipboard(&text);
                        }

                        // Auto-paste if enabled
                        if auto_paste_enabled {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if let Err(e) = crate::infrastructure::paste::paste_from_clipboard() {
                                eprintln!("Помилка автоматичної вставки: {}", e);
                                ui.base
                                    .set_status(&format!("Готово! (помилка вставки: {})", e));
                            }
                        }

                        // Save to history with recording metadata
                        let speakers = vec!["Ви".to_string(), "Учасник".to_string()];
                        let entry = HistoryEntry::new_with_recording(
                            text,
                            duration_secs,
                            language.clone(),
                            Some(file_path.to_string_lossy().to_string()),
                            speakers,
                        );
                        let mut h = ctx.history.lock().unwrap();
                        h.add(entry);
                        if let Err(e) = save_history(&h) {
                            eprintln!("Помилка збереження історії: {}", e);
                        }
                    }
                }
                Err(e) => {
                    ui.base.set_status(&format!("Помилка: {}", e));
                }
            }
        }

        // Transition back to Idle state
        rec.finish();
        ui.base.set_idle();
    });
}
