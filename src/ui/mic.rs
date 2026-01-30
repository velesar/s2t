//! Microphone recording handler (dictation + segmented modes).
//!
//! Handles start/stop for both plain dictation and continuous (segmented)
//! recording. The difference is configuration: segmented mode activates
//! the segmentation monitor on top of the shared mic recorder.

use crate::app::context::AppContext;
use crate::domain::traits::{HistoryRepository, Transcription, UIStateUpdater};
use crate::domain::types::AudioSegment;
use crate::history::{save_history, HistoryEntry};
use crate::ui::shared::{self, maybe_denoise};
use gtk4::prelude::*;
use gtk4::{glib, Label};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;

use super::state::{MicUI, RecordingContext};

const WHISPER_SAMPLE_RATE: usize = 16000;
const MIN_RECORDING_SAMPLES: usize = WHISPER_SAMPLE_RATE; // 1 second

/// Segment indicator symbols
const SEGMENT_PROCESSING: &str = "◐";
const SEGMENT_COMPLETED: &str = "●";

// Thread-local counters for tracking segment completion
thread_local! {
    static SEGMENTS_SENT: Cell<usize> = const { Cell::new(0) };
    static SEGMENTS_COMPLETED: Cell<usize> = const { Cell::new(0) };
    static PROCESSING_CANCELLED: Cell<bool> = const { Cell::new(false) };
}

/// Start microphone recording (dictation or segmented depending on config).
pub fn handle_start(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &MicUI) {
    // Check if model is loaded
    if !ctx.is_model_loaded() {
        ui.base
            .set_status("Модель не завантажено. Натисніть 'Моделі'.");
        return;
    }

    let use_segmentation = ctx.continuous_mode();

    match ctx.audio.start_mic() {
        Ok(()) => {
            rec.start_recording();

            if use_segmentation {
                ui.base.set_recording("Неперервний запис...");
                ui.show_level_bar();
                ui.show_segmentation_ui();

                // Reset segment completion counters
                SEGMENTS_SENT.with(|c| c.set(0));
                SEGMENTS_COMPLETED.with(|c| c.set(0));

                // Start segmentation monitor
                let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();
                if let Err(e) = ctx.audio.start_segmentation(segment_tx) {
                    ui.base.set_status(&format!("Помилка: {}", e));
                    return;
                }

                spawn_segment_pipeline(ctx, ui, segment_rx);
            } else {
                ui.base.set_recording("Запис...");
                ui.show_level_bar();
            }

            shared::start_timer_loop(rec, &ui.base);
            start_level_loop(ctx, rec, ui);

            if use_segmentation {
                start_vad_loop(ctx, rec, ui);
            }
        }
        Err(e) => {
            ui.base.set_status(&format!("Помилка: {}", e));
        }
    }
}

/// Stop microphone recording.
pub fn handle_stop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &MicUI) {
    let use_segmentation = ctx.continuous_mode();

    rec.start_processing();

    if use_segmentation {
        handle_segmented_stop(ctx, rec, ui);
    } else {
        handle_simple_stop(ctx, rec, ui);
    }
}

// === Private helpers ===

fn start_level_loop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &MicUI) {
    let ctx_clone = ctx.clone();
    let rec_clone = rec.clone();
    let ui_clone = ui.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if !rec_clone.is_recording() {
            return glib::ControlFlow::Break;
        }
        let amplitude = ctx_clone.audio.mic_amplitude();
        ui_clone.update_level(amplitude as f64);
        glib::ControlFlow::Continue
    });
}

fn start_vad_loop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &MicUI) {
    let ctx_clone = ctx.clone();
    let rec_clone = rec.clone();
    let ui_clone = ui.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        if !rec_clone.is_recording() {
            return glib::ControlFlow::Break;
        }
        let is_speech = ctx_clone.audio.is_speech_detected();
        ui_clone.update_vad_indicator(is_speech);
        glib::ControlFlow::Continue
    });
}

/// Spawn the parallel segment transcription pipeline.
fn spawn_segment_pipeline(
    ctx: &Arc<AppContext>,
    ui: &MicUI,
    segment_rx: async_channel::Receiver<AudioSegment>,
) {
    let language = ctx.language();
    let denoise_enabled = ctx.denoise_enabled();

    // Channel for transcription results: (segment_id, Result<text>)
    let (result_tx, result_rx) =
        async_channel::unbounded::<(usize, Result<String, String>)>();

    // Shared storage for segment indicator labels
    let segment_labels: Rc<RefCell<HashMap<usize, Label>>> =
        Rc::new(RefCell::new(HashMap::new()));

    // Spawn segment receiver that launches parallel transcriptions
    let ctx_for_segments = ctx.clone();
    let language_for_segments = language.clone();
    let result_tx_for_segments = result_tx.clone();
    let ui_for_segments = ui.clone();
    let segment_labels_for_receiver = segment_labels.clone();

    glib::spawn_future_local(async move {
        while let Ok(segment) = segment_rx.recv().await {
            let segment_id = segment.segment_id;
            let segment_samples = segment.samples.clone();
            let ctx = ctx_for_segments.clone();
            let lang = language_for_segments.clone();
            let tx = result_tx_for_segments.clone();

            SEGMENTS_SENT.with(|c| c.set(c.get() + 1));

            let duration_secs = segment
                .end_time
                .duration_since(segment.start_time)
                .as_secs_f32();
            let duration_text = format!("{:.1}s", duration_secs);

            let indicator =
                Label::new(Some(&format!("{} {}", SEGMENT_PROCESSING, duration_text)));
            indicator.add_css_class("segment-processing");
            ui_for_segments.segment_indicators_box.append(&indicator);
            segment_labels_for_receiver
                .borrow_mut()
                .insert(segment_id, indicator);

            ui_for_segments
                .base
                .set_status(&format!("Сегмент {}...", segment_id));

            std::thread::spawn(move || {
                let segment_samples = maybe_denoise(&segment_samples, denoise_enabled);
                let ts = ctx.transcription.lock();
                let result = ts
                    .transcribe(&segment_samples, &lang)
                    .map_err(|e| e.to_string());
                if let Ok(ref text) = result {
                    if text.is_empty() {
                        eprintln!(
                            "Сегмент {} повернув порожній результат ({} семплів)",
                            segment_id,
                            segment_samples.len()
                        );
                    }
                }
                let _ = tx.send_blocking((segment_id, result));
            });
        }
    });

    // Process results in order using BTreeMap
    let segment_labels_for_results = segment_labels.clone();
    let ui_for_results = ui.clone();
    glib::spawn_future_local(async move {
        let mut accumulated_text = String::new();
        let mut next_segment_id: usize = 1;
        let mut pending_results: BTreeMap<usize, Result<String, String>> = BTreeMap::new();
        let mut completed_count: usize = 0;
        let mut failed_count: usize = 0;

        while let Ok((segment_id, result)) = result_rx.recv().await {
            completed_count += 1;
            SEGMENTS_COMPLETED.with(|c| c.set(c.get() + 1));

            let is_error = result.is_err();

            if let Some(label) = segment_labels_for_results.borrow().get(&segment_id) {
                let current_text = label.text();
                let duration = current_text.split_whitespace().last().unwrap_or("");
                if is_error {
                    label.set_label(&format!("✗ {}", duration));
                    label.remove_css_class("segment-processing");
                    label.add_css_class("segment-error");
                } else {
                    label.set_label(&format!("{} {}", SEGMENT_COMPLETED, duration));
                    label.remove_css_class("segment-processing");
                    label.add_css_class("segment-completed");
                }
            }

            pending_results.insert(segment_id, result);

            while let Some(result) = pending_results.remove(&next_segment_id) {
                match result {
                    Ok(text) if !text.is_empty() => {
                        accumulated_text.push_str(&text);
                        accumulated_text.push(' ');
                        ui_for_results.base.set_result_text(&accumulated_text);
                    }
                    Err(ref err) => {
                        failed_count += 1;
                        eprintln!(
                            "Помилка транскрипції сегменту {}: {}",
                            next_segment_id, err
                        );
                    }
                    _ => {} // Ok but empty — already logged by the worker thread
                }
                next_segment_id += 1;
            }

            let status = if failed_count > 0 {
                format!(
                    "Транскрибовано: {} сегментів ({} з помилками)",
                    completed_count, failed_count
                )
            } else {
                format!("Транскрибовано: {} сегментів", completed_count)
            };
            ui_for_results.base.set_status(&status);
        }
    });
}

/// Handle stop for plain dictation (no segmentation).
fn handle_simple_stop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &MicUI) {
    ui.base.set_processing("Обробка...");
    ui.hide_level_bar();

    let (samples, completion_rx) = ctx.audio.stop_mic();

    let duration_secs = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;
    let duration_mins = (duration_secs / 60.0).floor() as u32;
    let duration_remaining_secs = (duration_secs % 60.0).floor() as u32;
    ui.base.set_status(&format!(
        "Обробка запису {:02}:{:02}...",
        duration_mins, duration_remaining_secs
    ));

    let ctx = ctx.clone();
    let rec = rec.clone();
    let ui = ui.clone();
    let language = ctx.language();
    let denoise_enabled = ctx.denoise_enabled();

    glib::spawn_future_local(async move {
        if let Some(rx) = completion_rx {
            let _ = rx.recv().await;
        }

        let (tx, rx) = async_channel::bounded::<anyhow::Result<String>>(1);

        let ctx_for_thread = ctx.clone();
        let language_for_thread = language.clone();
        std::thread::spawn(move || {
            let result = if samples.len() < MIN_RECORDING_SAMPLES {
                Err(anyhow::anyhow!("Запис закороткий"))
            } else {
                let samples = maybe_denoise(&samples, denoise_enabled);
                let ts = ctx_for_thread.transcription.lock();
                ts.transcribe(&samples, &language_for_thread)
            };
            let _ = tx.send_blocking(result);
        });

        if let Ok(result) = rx.recv().await {
            match result {
                Ok(text) => {
                    if text.is_empty() {
                        ui.base.set_status("Не вдалося розпізнати мову");
                    } else {
                        shared::handle_post_transcription(
                            &ctx,
                            &ui.base,
                            &text,
                            &language,
                            duration_secs,
                            None,
                            vec![],
                        )
                        .await;
                    }
                }
                Err(e) => {
                    ui.base.set_status(&format!("Помилка: {}", e));
                }
            }
        }

        rec.finish();
        ui.base.set_idle();
    });
}

/// Handle stop for segmented (continuous) mode.
fn handle_segmented_stop(ctx: &Arc<AppContext>, rec: &RecordingContext, ui: &MicUI) {
    // Reset cancel flag
    PROCESSING_CANCELLED.with(|c| c.set(false));

    // Show cancel button instead of disabling
    ui.base.button.set_label("Скасувати очікування");
    ui.base.button.remove_css_class("destructive-action");
    ui.base.button.add_css_class("warning");
    ui.base.button.set_sensitive(true);

    let cancel_handler_id = ui.base.button.connect_clicked(|_| {
        PROCESSING_CANCELLED.with(|c| c.set(true));
    });

    ui.base.set_status("Завершення обробки сегментів...");
    ui.base.timer_label.set_visible(false);
    ui.hide_level_bar();
    ui.hide_segmentation_ui();
    // Keep segment_row visible to show progress during processing
    ui.segment_row.set_visible(true);
    ui.base.spinner.set_visible(true);
    ui.base.spinner.start();

    // Critical ordering: stop segmentation BEFORE stopping mic
    ctx.audio.stop_segmentation();
    let (final_samples, completion_rx) = ctx.audio.stop_mic();

    let duration_secs = final_samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;

    let ctx = ctx.clone();
    let rec = rec.clone();
    let ui = ui.clone();
    let language = ctx.language();

    glib::spawn_future_local(async move {
        let cancel_handler_id = cancel_handler_id;

        if let Some(rx) = completion_rx {
            let _ = rx.recv().await;
        }

        // Wait for all transcriptions to complete (with 5-minute safety timeout)
        let poll_interval = std::time::Duration::from_millis(100);
        let poll_timeout = std::time::Duration::from_secs(5 * 60);
        let poll_start = std::time::Instant::now();
        let mut was_cancelled = false;
        let mut was_timed_out = false;

        loop {
            let sent = SEGMENTS_SENT.with(|c| c.get());
            let completed = SEGMENTS_COMPLETED.with(|c| c.get());

            if completed >= sent && sent > 0 {
                break;
            }

            if PROCESSING_CANCELLED.with(|c| c.get()) {
                was_cancelled = true;
                eprintln!("Processing cancelled by user: {}/{}", completed, sent);
                break;
            }

            if poll_start.elapsed() >= poll_timeout {
                was_timed_out = true;
                eprintln!(
                    "Segment processing timed out after {:?}: {}/{} completed",
                    poll_timeout, completed, sent
                );
                break;
            }

            ui.base
                .set_status(&format!("Обробка сегментів: {}/{}...", completed, sent));

            glib::timeout_future(poll_interval).await;
        }

        // Disconnect cancel handler
        ui.base.button.disconnect(cancel_handler_id);
        ui.base.button.remove_css_class("warning");

        let final_text = ui.base.get_result_text();

        if !final_text.is_empty() {
            if was_cancelled {
                let sent = SEGMENTS_SENT.with(|c| c.get());
                let completed = SEGMENTS_COMPLETED.with(|c| c.get());
                ui.base
                    .set_status(&format!("Скасовано (оброблено {}/{})", completed, sent));
            } else if was_timed_out {
                let sent = SEGMENTS_SENT.with(|c| c.get());
                let completed = SEGMENTS_COMPLETED.with(|c| c.get());
                ui.base.set_status(&format!(
                    "Тайм-аут обробки (оброблено {}/{})",
                    completed, sent
                ));
            } else {
                ui.base.set_status("Готово!");
            }

            let entry = HistoryEntry::new(final_text, duration_secs, language);
            let mut h = ctx.history.lock();
            h.add(entry);
            if let Err(e) = save_history(&h) {
                eprintln!("Помилка збереження історії: {}", e);
            }
        } else if was_cancelled {
            ui.base.set_status("Скасовано (нічого не оброблено)");
        } else if was_timed_out {
            ui.base
                .set_status("Тайм-аут обробки (нічого не оброблено)");
        } else {
            ui.base.set_status("Не вдалося розпізнати мову");
        }

        rec.finish();
        ui.base.set_idle();

        ui.segment_row.set_visible(false);
        ui.clear_segment_indicators();
    });
}
