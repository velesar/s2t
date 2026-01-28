use crate::continuous::{AudioSegment, ContinuousRecorder};
use crate::whisper::WhisperSTT;
use gtk4::prelude::*;
use gtk4::{glib, Box as GtkBox, Button, Label, LevelBar, TextView, Spinner};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::state::AppState;

/// Segment indicator symbols
const SEGMENT_PROCESSING: &str = "◐";  // U+25D0 Half circle - processing
const SEGMENT_COMPLETED: &str = "●";   // U+25CF Black circle - completed

// Thread-local counters for tracking segment completion
// These are used to wait for all transcriptions to complete before reading final text
thread_local! {
    static SEGMENTS_SENT: Cell<usize> = const { Cell::new(0) };
    static SEGMENTS_COMPLETED: Cell<usize> = const { Cell::new(0) };
    static PROCESSING_CANCELLED: Cell<bool> = const { Cell::new(false) };
}

/// Start continuous recording with automatic segmentation
pub fn handle_start(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    level_bar: &LevelBar,
    vad_indicator: &Label,
    segment_indicators_box: &GtkBox,
    segment_row: &GtkBox,
    continuous_recorder: &Arc<ContinuousRecorder>,
    whisper: &Arc<Mutex<Option<WhisperSTT>>>,
    config: &Arc<Mutex<crate::config::Config>>,
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

    // Create channel for segments
    let (segment_tx, segment_rx) = async_channel::unbounded::<AudioSegment>();

    match continuous_recorder.start_continuous(segment_tx) {
        Ok(()) => {
            app_state.set(AppState::Recording);
            recording_start_time.set(Some(Instant::now()));

            button.set_label("Зупинити запис");
            button.remove_css_class("suggested-action");
            button.add_css_class("destructive-action");
            status_label.set_text("Неперервний запис...");
            let buffer = result_text_view.buffer();
            buffer.set_text("");

            // Show timer, level bar, VAD indicator, and segment indicators
            timer_label.set_text("00:00");
            timer_label.set_visible(true);
            level_bar.set_value(0.0);
            level_bar.set_visible(true);
            vad_indicator.set_text("🔇 Тиша");
            vad_indicator.set_visible(true);

            // Clear and show segment indicators
            while let Some(child) = segment_indicators_box.first_child() {
                segment_indicators_box.remove(&child);
            }
            segment_row.set_visible(true);

            // Reset segment completion counters
            SEGMENTS_SENT.with(|c| c.set(0));
            SEGMENTS_COMPLETED.with(|c| c.set(0));

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

            // Start level bar and VAD indicator update loop
            let level_bar_clone = level_bar.clone();
            let vad_indicator_clone = vad_indicator.clone();
            let continuous_recorder_clone = continuous_recorder.clone();
            let app_state_clone = app_state.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                if app_state_clone.get() != AppState::Recording {
                    return glib::ControlFlow::Break;
                }
                let amplitude = continuous_recorder_clone.get_amplitude();
                level_bar_clone.set_value(amplitude as f64);

                // Update VAD indicator
                let is_speech = continuous_recorder_clone.is_speech_detected();
                if is_speech {
                    vad_indicator_clone.set_text("🔊 Говорить");
                } else {
                    vad_indicator_clone.set_text("🔇 Тиша");
                }

                glib::ControlFlow::Continue
            });

            // Start parallel segment processing with ordered results
            let result_text_view_clone = result_text_view.clone();
            let status_label_clone = status_label.clone();
            let whisper_clone = whisper.clone();
            let language = {
                let cfg = config.lock().unwrap();
                cfg.language.clone()
            };

            // Channel for transcription results: (segment_id, text)
            let (result_tx, result_rx) = async_channel::unbounded::<(usize, String)>();

            // Shared storage for segment indicator labels (keyed by segment_id)
            let segment_labels: Rc<RefCell<HashMap<usize, Label>>> = Rc::new(RefCell::new(HashMap::new()));

            // Spawn segment receiver that launches parallel transcriptions
            let whisper_for_segments = whisper_clone.clone();
            let language_for_segments = language.clone();
            let result_tx_for_segments = result_tx.clone();
            let status_label_for_segments = status_label_clone.clone();
            let segment_indicators_box_clone = segment_indicators_box.clone();
            let segment_labels_for_receiver = segment_labels.clone();

            glib::spawn_future_local(async move {
                while let Ok(segment) = segment_rx.recv().await {
                    let segment_id = segment.segment_id;
                    let segment_samples = segment.samples.clone();
                    let whisper = whisper_for_segments.clone();
                    let lang = language_for_segments.clone();
                    let tx = result_tx_for_segments.clone();

                    // Track segment as sent for transcription
                    SEGMENTS_SENT.with(|c| c.set(c.get() + 1));

                    // Calculate segment duration from timing (uses start_time and end_time fields)
                    let duration_secs = segment.end_time.duration_since(segment.start_time).as_secs_f32();
                    let duration_text = format!("{:.1}s", duration_secs);

                    // Create indicator label for this segment (starts as processing)
                    let indicator = Label::new(Some(&format!("{} {}", SEGMENT_PROCESSING, duration_text)));
                    indicator.add_css_class("segment-processing");
                    segment_indicators_box_clone.append(&indicator);
                    segment_labels_for_receiver.borrow_mut().insert(segment_id, indicator);

                    // Update status to show segment being processed
                    status_label_for_segments.set_text(&format!("Сегмент {}...", segment_id));

                    // Launch transcription WITHOUT waiting for result (parallel processing)
                    std::thread::spawn(move || {
                        let w = whisper.lock().unwrap();
                        if let Some(ref whisper) = *w {
                            let result = whisper.transcribe(&segment_samples, Some(&lang));
                            let text = result.unwrap_or_default();
                            if text.is_empty() {
                                eprintln!("Сегмент {} повернув порожній результат ({} семплів)",
                                    segment_id, segment_samples.len());
                            }
                            let _ = tx.send_blocking((segment_id, text));
                        } else {
                            eprintln!("Модель не завантажено для сегменту {}", segment_id);
                            let _ = tx.send_blocking((segment_id, String::new()));
                        }
                    });
                }
            });

            // Process results in order using BTreeMap for ordering
            let segment_labels_for_results = segment_labels.clone();
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
                        let duration = current_text
                            .split_whitespace()
                            .last()
                            .unwrap_or("");
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
                            let buffer = result_text_view_clone.buffer();
                            buffer.set_text(&accumulated_text);
                        }
                        next_segment_id += 1;
                    }

                    // Update status with progress
                    status_label_clone.set_text(&format!("Транскрибовано: {} сегментів", completed_count));
                }
            });
        }
        Err(e) => {
            status_label.set_text(&format!("Помилка: {}", e));
        }
    }
}

/// Stop continuous recording
pub fn handle_stop(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    level_bar: &LevelBar,
    vad_indicator: &Label,
    segment_indicators_box: &GtkBox,
    segment_row: &GtkBox,
    spinner: &Spinner,
    continuous_recorder: &Arc<ContinuousRecorder>,
    _whisper: &Arc<Mutex<Option<WhisperSTT>>>,
    config: &Arc<Mutex<crate::config::Config>>,
    history: &Arc<Mutex<crate::history::History>>,
    app_state: &Rc<Cell<AppState>>,
    recording_start_time: &Rc<Cell<Option<Instant>>>,
) {
    app_state.set(AppState::Processing);
    recording_start_time.set(None);

    // Reset cancel flag
    PROCESSING_CANCELLED.with(|c| c.set(false));

    // Show cancel button instead of disabling
    button.set_label("Скасувати очікування");
    button.remove_css_class("destructive-action");
    button.add_css_class("warning");
    button.set_sensitive(true);

    // Connect cancel handler
    let cancel_handler_id = button.connect_clicked(|_| {
        PROCESSING_CANCELLED.with(|c| c.set(true));
    });

    status_label.set_text("Завершення обробки сегментів...");
    timer_label.set_visible(false);
    level_bar.set_visible(false);
    vad_indicator.set_visible(false);
    // Keep segment_row visible to show progress during processing
    spinner.set_visible(true);
    spinner.start();

    let (final_samples, completion_rx) = continuous_recorder.stop_continuous();

    // Calculate duration
    let duration_secs = final_samples.len() as f32 / 16000.0;

    let history = history.clone();
    let status_label = status_label.clone();
    let result_text_view = result_text_view.clone();
    let button = button.clone();
    let spinner = spinner.clone();
    let segment_row = segment_row.clone();
    let segment_indicators_box = segment_indicators_box.clone();
    let app_state = app_state.clone();
    let language = {
        let cfg = config.lock().unwrap();
        cfg.language.clone()
    };

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
            status_label.set_text(&format!("Обробка сегментів: {}/{}...", completed, sent));

            glib::timeout_future(poll_interval).await;
        }

        // Disconnect cancel handler
        button.disconnect(cancel_handler_id);
        button.remove_css_class("warning");

        // NOW get the final accumulated text from result_text_view
        // (after all segments have been processed)
        let buffer = result_text_view.buffer();
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        let final_text = buffer.text(&start, &end, false).to_string();

        // Note: We no longer do fallback transcription of final_samples
        // because with Fix 1, remaining audio is sent as a final segment
        // before the channel is closed. The result processor handles it.

        if !final_text.is_empty() {
            if was_cancelled {
                let sent = SEGMENTS_SENT.with(|c| c.get());
                let completed = SEGMENTS_COMPLETED.with(|c| c.get());
                status_label.set_text(&format!("Скасовано (оброблено {}/{})", completed, sent));
            } else {
                status_label.set_text("Готово!");
            }
            // Don't overwrite buffer - it already has the correct content
            // from the result processor

            // Save to history (even if cancelled - save what we have)
            let entry = crate::history::HistoryEntry::new(
                final_text.clone(),
                duration_secs,
                language,
            );
            let mut h = history.lock().unwrap();
            h.add(entry);
            if let Err(e) = crate::history::save_history(&h) {
                eprintln!("Помилка збереження історії: {}", e);
            }
        } else {
            if was_cancelled {
                status_label.set_text("Скасовано (нічого не оброблено)");
            } else {
                status_label.set_text("Не вдалося розпізнати мову");
            }
        }

        // Transition back to Idle state
        app_state.set(AppState::Idle);
        spinner.stop();
        spinner.set_visible(false);

        // Hide and clear segment indicators
        segment_row.set_visible(false);
        while let Some(child) = segment_indicators_box.first_child() {
            segment_indicators_box.remove(&child);
        }

        button.set_label("Почати запис");
        button.add_css_class("suggested-action");
        button.set_sensitive(true);
    });
}
