//! Conference file recording mode handler.
//!
//! This module handles recording audio from mic + system loopback
//! and saving to a WAV file WITHOUT transcription.
//! Use this mode when you want to record a meeting for later processing.

use crate::app::context::AppContext;
use crate::domain::traits::UIStateUpdater;
use crate::infrastructure::recordings::{
    ensure_recordings_dir, generate_recording_filename, recording_path, save_recording,
};
use crate::ui::shared;
use gtk4::glib;
use std::sync::Arc;

use super::state::{ConferenceUI, RecordingContext};

/// Start conference file recording (mic + loopback)
pub fn handle_start(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &ConferenceUI) {
    match ctx.audio.start_conference() {
        Ok(()) => {
            rec.start_recording();

            ui.base.set_recording("Запис у файл...");
            ui.show_level_bars();

            shared::start_timer_loop(rec, &ui.base);
            shared::start_conference_level_loop(ctx, rec, ui);
        }
        Err(e) => {
            ui.base.set_status(&format!("Помилка: {}", e));
        }
    }
}

/// Stop conference file recording and save to WAV file
pub fn handle_stop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &ConferenceUI) {
    rec.start_processing();

    // Update UI for processing state
    ui.base.set_processing("Збереження...");
    ui.hide_level_bars();

    let recording = ctx.audio.stop_conference();

    // Calculate duration
    let duration_secs = recording.duration_secs();
    let duration_mins = (duration_secs / 60.0).floor() as u32;
    let duration_remaining_secs = (duration_secs % 60.0).floor() as u32;

    // Ensure recordings directory exists
    if let Err(e) = ensure_recordings_dir() {
        eprintln!("Помилка створення директорії записів: {}", e);
        ui.base.set_status(&format!("Помилка: {}", e));
        rec.finish();
        ui.base.set_idle();
        return;
    }

    let rec = rec.clone();
    let ui = ui.clone();

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

        match save_recording(
            &recording.mic_samples,
            &recording.loopback_samples,
            &file_path,
        ) {
            Ok(()) => {
                let status = format!(
                    "Збережено {:02}:{:02} -> {}",
                    duration_mins,
                    duration_remaining_secs,
                    file_path.display()
                );
                ui.base.set_status(&status);
                ui.base
                    .set_result_text(&format!("Файл: {}", file_path.display()));
            }
            Err(e) => {
                ui.base.set_status(&format!("Помилка збереження: {}", e));
            }
        }

        // Transition back to Idle state
        rec.finish();
        ui.base.set_idle();
    });
}
