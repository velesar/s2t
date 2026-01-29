//! Continuous mode recording handler.
//!
//! This module handles start/stop for continuous recording mode with
//! automatic segmentation, parallel transcription, and ordered results.

use crate::context::AppContext;
use crate::history::{save_history, HistoryEntry};
use crate::traits::{HistoryRepository, Transcription, UIStateUpdater};
use crate::types::AudioSegment;
use gtk4::prelude::*;
use gtk4::{glib, Label};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;

use super::state::{ContinuousUI, RecordingContext};

/// Segment indicator symbols
const SEGMENT_PROCESSING: &str = "◐"; // U+25D0 Half circle - processing
const SEGMENT_COMPLETED: &str = "●"; // U+25CF Black circle - completed

const SAMPLE_RATE: usize = 16000;

// Thread-local counters for tracking segment completion
// These are used to wait for all transcriptions to complete before reading final text
thread_local! {
    static SEGMENTS_SENT: Cell<usize> = const { Cell::new(0) };
    static SEGMENTS_COMPLETED: Cell<usize> = const { Cell::new(0) };
    static PROCESSING_CANCELLED: Cell<bool> = const { Cell::new(false) };
}

/// Start continuous recording with automatic segmentation
pub fn handle_start(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &ContinuousUI) {
    // Check if model is loaded
    if !ctx.is_model_loaded() {
        ui.base
            .set_status("Модель не завантажено. Натисніть 'Моделі'.");
        return;
    }

    // Create channel for segments
    let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();

    match ctx.audio.start_continuous(segment_tx) {
        Ok(()) => {
            rec.start_recording();

            ui.base.set_recording("Неперервний запис...");
            ui.show_recording_ui();

            // Reset segment completion counters
            SEGMENTS_SENT.with(|c| c.set(0));
            SEGMENTS_COMPLETED.with(|c| c.set(0));

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

            // Start level bar and VAD indicator update loop
            let ctx_clone = ctx.clone();
            let rec_clone = rec.clone();
            let ui_clone = ui.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                if !rec_clone.is_recording() {
                    return glib::ControlFlow::Break;
                }
                let amplitude = ctx_clone.audio.get_continuous_amplitude();
                ui_clone.update_level(amplitude as f64);

                let is_speech = ctx_clone.audio.is_speech_detected();
                ui_clone.update_vad_indicator(is_speech);

                glib::ControlFlow::Continue
            });

            // Start parallel segment processing with ordered results
            let ctx_clone = ctx.clone();
            let ui_clone = ui.clone();
            let language = ctx.language();

            // Channel for transcription results: (segment_id, text)
            let (result_tx, result_rx) = async_channel::unbounded::<(usize, String)>();

            // Shared storage for segment indicator labels (keyed by segment_id)
            let segment_labels: Rc<RefCell<HashMap<usize, Label>>> =
                Rc::new(RefCell::new(HashMap::new()));

            // Spawn segment receiver that launches parallel transcriptions
            let ctx_for_segments = ctx_clone.clone();
            let language_for_segments = language.clone();
            let result_tx_for_segments = result_tx.clone();
            let ui_for_segments = ui_clone.clone();
            let segment_labels_for_receiver = segment_labels.clone();

            glib::spawn_future_local(async move {
                while let Ok(segment) = segment_rx.recv().await {
                    let segment_id = segment.segment_id;
                    let segment_samples = segment.samples.clone();
                    let ctx = ctx_for_segments.clone();
                    let lang = language_for_segments.clone();
                    let tx = result_tx_for_segments.clone();

                    // Track segment as sent for transcription
                    SEGMENTS_SENT.with(|c| c.set(c.get() + 1));

                    // Calculate segment duration from timing
                    let duration_secs = segment
                        .end_time
                        .duration_since(segment.start_time)
                        .as_secs_f32();
                    let duration_text = format!("{:.1}s", duration_secs);

                    // Create indicator label for this segment (starts as processing)
                    let indicator =
                        Label::new(Some(&format!("{} {}", SEGMENT_PROCESSING, duration_text)));
                    indicator.add_css_class("segment-processing");
                    ui_for_segments.segment_indicators_box.append(&indicator);
                    segment_labels_for_receiver
                        .borrow_mut()
                        .insert(segment_id, indicator);

                    // Update status to show segment being processed
                    ui_for_segments
                        .base
                        .set_status(&format!("Сегмент {}...", segment_id));

                    // Launch transcription WITHOUT waiting for result (parallel processing)
                    std::thread::spawn(move || {
                        let ts = ctx.transcription.lock().unwrap();
                        let result = ts.transcribe(&segment_samples, &lang);
                        let text = result.unwrap_or_default();
                        if text.is_empty() {
                            eprintln!(
                                "Сегмент {} повернув порожній результат ({} семплів)",
                                segment_id,
                                segment_samples.len()
                            );
                        }
                        let _ = tx.send_blocking((segment_id, text));
                    });
                }
            });

            // Process results in order using BTreeMap for ordering
            let segment_labels_for_results = segment_labels.clone();
            let ui_for_results = ui_clone.clone();
            glib::spawn_future_local(async move {
                let mut accumulated_text = String::new();
                let mut next_segment_id: usize = 1;
                let mut pending_results: BTreeMap<usize, String> = BTreeMap::new();
                let mut completed_count: usize = 0;

                while let Ok((segment_id, text)) = result_rx.recv().await {
                    completed_count += 1;

                    // Track segment as completed for synchronization
                    SEGMENTS_COMPLETED.with(|c| c.set(c.get() + 1));

                    // Mark segment indicator as completed (preserve duration)
                    if let Some(label) = segment_labels_for_results.borrow().get(&segment_id) {
                        let current_text = label.text();
                        let duration = current_text.split_whitespace().last().unwrap_or("");
                        label.set_label(&format!("{} {}", SEGMENT_COMPLETED, duration));
                        label.remove_css_class("segment-processing");
                        label.add_css_class("segment-completed");
                    }

                    // Store result in pending map
                    pending_results.insert(segment_id, text);

                    // Flush all consecutive ready results in order
                    while let Some(text) = pending_results.remove(&next_segment_id) {
                        if !text.is_empty() {
                            accumulated_text.push_str(&text);
                            accumulated_text.push(' ');

                            // Update UI immediately with accumulated text
                            ui_for_results.base.set_result_text(&accumulated_text);
                        }
                        next_segment_id += 1;
                    }

                    // Update status with progress
                    ui_for_results
                        .base
                        .set_status(&format!("Транскрибовано: {} сегментів", completed_count));
                }
            });
        }
        Err(e) => {
            ui.base.set_status(&format!("Помилка: {}", e));
        }
    }
}

/// Stop continuous recording
pub fn handle_stop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &ContinuousUI) {
    rec.start_processing();

    // Reset cancel flag
    PROCESSING_CANCELLED.with(|c| c.set(false));

    // Show cancel button instead of disabling
    ui.base.button.set_label("Скасувати очікування");
    ui.base.button.remove_css_class("destructive-action");
    ui.base.button.add_css_class("warning");
    ui.base.button.set_sensitive(true);

    // Connect cancel handler
    let cancel_handler_id = ui.base.button.connect_clicked(|_| {
        PROCESSING_CANCELLED.with(|c| c.set(true));
    });

    ui.base
        .set_status("Завершення обробки сегментів...");
    ui.base.timer_label.set_visible(false);
    ui.hide_recording_ui();
    // Keep segment_row visible to show progress during processing
    ui.segment_row.set_visible(true);
    ui.base.spinner.set_visible(true);
    ui.base.spinner.start();

    let (final_samples, completion_rx) = ctx.audio.stop_continuous();

    // Calculate duration
    let duration_secs = final_samples.len() as f32 / SAMPLE_RATE as f32;

    let ctx = ctx.clone();
    let rec = rec.clone();
    let ui = ui.clone();
    let language = ctx.language();

    glib::spawn_future_local(async move {
        // Move cancel_handler_id into async block
        let cancel_handler_id = cancel_handler_id;

        // Wait for recording to finish
        if let Some(rx) = completion_rx {
            let _ = rx.recv().await;
        }

        // Wait for all transcriptions to complete
        // Poll until segments_completed >= segments_sent (no timeout - user can cancel)
        let poll_interval = std::time::Duration::from_millis(100);
        let mut was_cancelled = false;

        loop {
            let sent = SEGMENTS_SENT.with(|c| c.get());
            let completed = SEGMENTS_COMPLETED.with(|c| c.get());

            if completed >= sent && sent > 0 {
                // All segments processed
                break;
            }

            // Check if user cancelled
            if PROCESSING_CANCELLED.with(|c| c.get()) {
                was_cancelled = true;
                eprintln!("Processing cancelled by user: {}/{}", completed, sent);
                break;
            }

            // Update status with progress
            ui.base
                .set_status(&format!("Обробка сегментів: {}/{}...", completed, sent));

            glib::timeout_future(poll_interval).await;
        }

        // Disconnect cancel handler
        ui.base.button.disconnect(cancel_handler_id);
        ui.base.button.remove_css_class("warning");

        // Get the final accumulated text from result_text_view
        let final_text = ui.base.get_result_text();

        if !final_text.is_empty() {
            if was_cancelled {
                let sent = SEGMENTS_SENT.with(|c| c.get());
                let completed = SEGMENTS_COMPLETED.with(|c| c.get());
                ui.base
                    .set_status(&format!("Скасовано (оброблено {}/{})", completed, sent));
            } else {
                ui.base.set_status("Готово!");
            }

            // Save to history (even if cancelled - save what we have)
            let entry = HistoryEntry::new(final_text, duration_secs, language);
            let mut h = ctx.history.lock().unwrap();
            h.add(entry);
            if let Err(e) = save_history(&h) {
                eprintln!("Помилка збереження історії: {}", e);
            }
        } else if was_cancelled {
            ui.base
                .set_status("Скасовано (нічого не оброблено)");
        } else {
            ui.base.set_status("Не вдалося розпізнати мову");
        }

        // Transition back to Idle state
        rec.finish();
        ui.base.set_idle();

        // Hide and clear segment indicators
        ui.segment_row.set_visible(false);
        ui.clear_segment_indicators();
    });
}
