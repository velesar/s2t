//! Conference file recording mode handler.
//!
//! This module handles recording audio from mic + system loopback
//! and saving to a WAV file WITHOUT transcription.
//! Use this mode when you want to record a meeting for later processing.

use crate::context::AppContext;
use crate::recordings::{
    ensure_recordings_dir, generate_recording_filename, recording_path, save_recording,
};
use crate::traits::UIStateUpdater;
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

        match save_recording(&recording.mic_samples, &recording.loopback_samples, &file_path) {
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
