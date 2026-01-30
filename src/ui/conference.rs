//! Conference mode recording handler.
//!
//! This module handles start/stop recording for conference mode
//! with dual-channel audio (microphone + system loopback) and diarization.

use crate::app::context::AppContext;
use crate::domain::traits::UIStateUpdater;
use crate::infrastructure::recordings::{
    ensure_recordings_dir, generate_recording_filename, recording_path, save_recording,
};
use crate::ui::shared::{self, maybe_denoise};
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

            shared::start_timer_loop(rec, &ui.base);
            shared::start_conference_level_loop(ctx, rec, ui);
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
    let denoise_enabled = ctx.denoise_enabled();

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
            let mic_samples = maybe_denoise(&mic_samples, denoise_enabled);
            let loopback_samples = maybe_denoise(&loopback_samples, denoise_enabled);
            // Lock ordering: diarization before transcription.
            // This ensures consistent ordering across the codebase.
            let mut engine_guard = ctx_for_thread.diarization.lock();
            let ts = ctx_for_thread.transcription.lock();
            let result = ts.transcribe_conference(
                &mic_samples,
                &loopback_samples,
                &language_for_thread,
                &diarization_method_for_thread,
                Some(&mut *engine_guard),
            );
            let _ = tx.send_blocking(result);
        });

        if let Ok(result) = rx.recv().await {
            match result {
                Ok(text) => {
                    if text.is_empty() {
                        ui.base.set_status("Не вдалося розпізнати мову");
                    } else {
                        let speakers = vec!["Ви".to_string(), "Учасник".to_string()];
                        shared::handle_post_transcription(
                            &ctx,
                            &ui.base,
                            &text,
                            &language,
                            duration_secs,
                            Some(file_path.to_string_lossy().to_string()),
                            speakers,
                        )
                        .await;
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
