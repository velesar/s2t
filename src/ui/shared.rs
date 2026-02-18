//! Shared UI helpers extracted from mic.rs, conference.rs, conference_file.rs.
//!
//! These functions eliminate duplication across recording mode handlers:
//! - Timer update loop (was identical in all 3 modules)
//! - Conference level bar loop (was identical in conference.rs and conference_file.rs)
//! - Post-transcription actions (auto-copy, auto-paste, history save)
//! - Audio denoising wrapper

use crate::app::context::AppContext;
use crate::domain::traits::{HistoryRepository, UIStateUpdater};
use crate::history::{save_history, HistoryEntry};
use crate::recording::denoise::NnnoiselessDenoiser;
use gtk4::glib;
use std::sync::Arc;

use super::state::{ConferenceUI, RecordingContext, UIContext};

/// Start a 1-second timer update loop on the GTK main thread.
///
/// Updates the timer label with elapsed seconds. Stops automatically
/// when recording ends (rec.is_recording() returns false).
pub fn start_timer_loop(rec: &RecordingContext, base: &UIContext) {
    let rec = rec.clone();
    let base = base.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        if !rec.is_recording() {
            return glib::ControlFlow::Break;
        }
        if let Some(secs) = rec.elapsed_secs() {
            base.update_timer(secs);
        }
        glib::ControlFlow::Continue
    });
}

/// Start a 50ms level bar update loop for conference mode (dual mic + loopback).
///
/// Reads amplitudes from AudioService and updates both level bars.
/// Stops when recording ends.
pub fn start_conference_level_loop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &ConferenceUI) {
    let ctx = ctx.clone();
    let rec = rec.clone();
    let ui = ui.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if !rec.is_recording() {
            return glib::ControlFlow::Break;
        }
        let mic_amplitude = ctx.audio.get_mic_amplitude();
        let loopback_amplitude = ctx.audio.get_loopback_amplitude();
        ui.update_levels(mic_amplitude as f64, loopback_amplitude as f64);
        glib::ControlFlow::Continue
    });
}

/// Apply denoising if enabled, returning original samples on failure.
pub fn maybe_denoise(samples: &[f32], enabled: bool) -> Vec<f32> {
    if !enabled {
        return samples.to_vec();
    }
    let denoiser = NnnoiselessDenoiser::new();
    match denoiser.denoise_buffer(samples) {
        Ok(denoised) => denoised,
        Err(e) => {
            eprintln!("Denoising failed, using original: {}", e);
            samples.to_vec()
        }
    }
}

/// Handle post-transcription actions: auto-copy, auto-paste, and history save.
///
/// This is the shared "success path" after transcription produces text.
/// Conference mode passes additional recording metadata via `recording_file`
/// and `speakers`.
pub async fn handle_post_transcription(
    ctx: &Arc<AppContext>,
    base: &UIContext,
    text: &str,
    language: &str,
    duration_secs: f32,
    recording_file: Option<String>,
    speakers: Vec<String>,
) {
    base.set_status("Готово!");
    base.set_result_text(text);

    let auto_copy = ctx.auto_copy();
    let auto_paste = ctx.auto_paste();

    if auto_copy || auto_paste {
        super::copy_to_clipboard(text);
    }

    if auto_paste {
        glib::timeout_future(std::time::Duration::from_millis(100)).await;
        let (paste_tx, paste_rx) = async_channel::bounded::<Option<String>>(1);
        std::thread::spawn(move || {
            let err = crate::infrastructure::paste::paste_from_clipboard()
                .err()
                .map(|e| e.to_string());
            let _ = paste_tx.send_blocking(err);
        });
        if let Ok(Some(err)) = paste_rx.recv().await {
            eprintln!("Помилка автоматичної вставки: {}", err);
            base.set_status(&format!("Готово! (помилка вставки: {})", err));
        }
    }

    if speakers.is_empty() && recording_file.is_none() {
        let entry = HistoryEntry::new(text.to_string(), duration_secs, language.to_string());
        let mut h = ctx.history.lock();
        h.add(entry);
        if let Err(e) = save_history(&h) {
            eprintln!("Помилка збереження історії: {}", e);
        }
    } else {
        let entry = HistoryEntry::new_with_recording(
            text.to_string(),
            duration_secs,
            language.to_string(),
            recording_file,
            speakers,
        );
        let mut h = ctx.history.lock();
        h.add(entry);
        if let Err(e) = save_history(&h) {
            eprintln!("Помилка збереження історії: {}", e);
        }
    }
}
