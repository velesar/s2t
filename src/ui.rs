use crate::audio::AudioRecorder;
use crate::config::Config;
use crate::conference_recorder::ConferenceRecorder;
use crate::history::{save_history, History, HistoryEntry};
use crate::history_dialog::show_history_dialog;
use crate::model_dialog::show_model_dialog;
use crate::recordings::{ensure_recordings_dir, generate_recording_filename, recording_path, save_recording};
use crate::settings_dialog::show_settings_dialog;
use crate::whisper::WhisperSTT;
use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box as GtkBox, Button, Label, LevelBar, Orientation, Spinner, TextView};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
enum AppState {
    Idle,
    Recording,
    Processing,
}

const WHISPER_SAMPLE_RATE: usize = 16000;
const MIN_RECORDING_SAMPLES: usize = WHISPER_SAMPLE_RATE; // 1 second

pub fn build_ui(
    app: &Application,
    whisper: Arc<Mutex<Option<WhisperSTT>>>,
    config: Arc<Mutex<Config>>,
    history: Arc<Mutex<History>>,
    diarization_engine: Arc<Mutex<crate::diarization::DiarizationEngine>>,
    open_models_rx: async_channel::Receiver<()>,
    open_history_rx: async_channel::Receiver<()>,
    open_settings_rx: async_channel::Receiver<()>,
    toggle_recording_rx: async_channel::Receiver<()>,
    reload_hotkeys_tx: async_channel::Sender<()>,
) {
    let recorder = Arc::new(AudioRecorder::new());
    let conference_recorder = Arc::new(ConferenceRecorder::new());

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Голосова диктовка")
        .default_width(500)
        .default_height(300)
        .build();

    // Note: Window positioning in GTK4 is handled by the window manager.
    // Direct position setting is not supported, especially on Wayland.
    // The window will be positioned by the window manager according to its policies.

    let main_box = GtkBox::new(Orientation::Vertical, 12);
    main_box.set_margin_top(20);
    main_box.set_margin_bottom(20);
    main_box.set_margin_start(20);
    main_box.set_margin_end(20);

    // Status row with label and spinner
    let status_box = GtkBox::new(Orientation::Horizontal, 8);
    status_box.set_halign(gtk4::Align::Center);

    let status_label = Label::new(Some("Натисніть кнопку для запису"));
    status_label.add_css_class("title-2");

    let spinner = Spinner::new();
    spinner.set_visible(false);

    status_box.append(&status_label);
    status_box.append(&spinner);

    // Timer label for recording duration
    let timer_label = Label::new(Some(""));
    timer_label.add_css_class("monospace");
    timer_label.set_visible(false);

    // Mode selector
    let mode_combo = gtk4::ComboBoxText::new();
    mode_combo.append_text("Диктовка");
    mode_combo.append_text("Конференція");
    mode_combo.set_active(Some(0)); // Default to dictation
    mode_combo.set_halign(gtk4::Align::Start);
    
    // Load current mode from config
    let current_mode = {
        let cfg = config.lock().unwrap();
        if cfg.recording_mode == "conference" {
            mode_combo.set_active(Some(1));
        }
        cfg.recording_mode.clone()
    };

    // Audio level indicator (for dictation mode)
    let level_bar = LevelBar::new();
    level_bar.set_min_value(0.0);
    level_bar.set_max_value(1.0);
    level_bar.set_value(0.0);
    level_bar.set_visible(false);
    level_bar.set_size_request(200, -1);

    // Conference mode: two level bars (mic + loopback)
    let mic_level_bar = LevelBar::new();
    mic_level_bar.set_min_value(0.0);
    mic_level_bar.set_max_value(1.0);
    mic_level_bar.set_value(0.0);
    mic_level_bar.set_visible(false);
    mic_level_bar.set_size_request(200, -1);

    let loopback_level_bar = LevelBar::new();
    loopback_level_bar.set_min_value(0.0);
    loopback_level_bar.set_max_value(1.0);
    loopback_level_bar.set_value(0.0);
    loopback_level_bar.set_visible(false);
    loopback_level_bar.set_size_request(200, -1);

    let level_bars_box = GtkBox::new(Orientation::Vertical, 4);
    let mic_label = Label::new(Some("Мікрофон:"));
    mic_label.set_halign(gtk4::Align::Start);
    level_bars_box.append(&mic_label);
    level_bars_box.append(&mic_level_bar);
    let loopback_label = Label::new(Some("Системний аудіо:"));
    loopback_label.set_halign(gtk4::Align::Start);
    loopback_label.set_margin_top(6);
    level_bars_box.append(&loopback_label);
    level_bars_box.append(&loopback_level_bar);
    level_bars_box.set_visible(false);

    // Use TextView for editable result display
    let result_text_view = gtk4::TextView::new();
    result_text_view.set_wrap_mode(gtk4::WrapMode::Word);
    result_text_view.set_editable(true);
    result_text_view.set_cursor_visible(true);
    result_text_view.set_vexpand(true);
    
    let result_scrolled = gtk4::ScrolledWindow::new();
    result_scrolled.set_min_content_height(100);
    result_scrolled.set_child(Some(&result_text_view));

    let record_button = Button::with_label("Почати запис");
    record_button.add_css_class("suggested-action");
    record_button.add_css_class("pill");

    // Shared application state
    let app_state = Rc::new(Cell::new(AppState::Idle));
    let recording_start_time: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));

    // Update UI based on mode selection
    let level_bar_clone = level_bar.clone();
    let level_bars_box_clone = level_bars_box.clone();
    let config_for_mode = config.clone();
    mode_combo.connect_changed(move |combo| {
        let is_conference = combo.active() == Some(1);
        level_bar_clone.set_visible(!is_conference);
        level_bars_box_clone.set_visible(is_conference);
        
        // Save mode to config
        let mut cfg = config_for_mode.lock().unwrap();
        cfg.recording_mode = if is_conference {
            "conference".to_string()
        } else {
            "dictation".to_string()
        };
        if let Err(e) = crate::config::save_config(&cfg) {
            eprintln!("Помилка збереження режиму: {}", e);
        }
    });

    // Set initial visibility
    level_bar.set_visible(current_mode != "conference");
    level_bars_box.set_visible(current_mode == "conference");

    let config_for_ui = config.clone();
    let history_for_ui = history.clone();
    let diarization_engine_for_ui = diarization_engine.clone();
    let recorder_for_button = recorder.clone();
    let conference_recorder_for_button = conference_recorder.clone();
    let app_state_for_button = app_state.clone();
    let recording_start_time_for_button = recording_start_time.clone();
    let mode_combo_for_button = mode_combo.clone();
    let level_bar_for_button = level_bar.clone();
    let mic_level_bar_for_button = mic_level_bar.clone();
    let loopback_level_bar_for_button = loopback_level_bar.clone();
    setup_record_button(
        &record_button,
        &status_label,
        &result_text_view,
        &timer_label,
        &level_bar_for_button,
        &mic_level_bar_for_button,
        &loopback_level_bar_for_button,
        &spinner,
        recorder_for_button,
        conference_recorder_for_button,
        mode_combo_for_button,
        whisper.clone(),
        config_for_ui,
        history_for_ui,
        diarization_engine_for_ui,
        app_state_for_button,
        recording_start_time_for_button,
    );

    let copy_button = Button::with_label("Копіювати");
    setup_copy_button(&copy_button, &result_text_view);

    let models_button = Button::with_label("Моделі");
    let window_weak = window.downgrade();
    let config_for_models = config.clone();
    let whisper_for_models = whisper.clone();
    models_button.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            show_model_dialog(&window, config_for_models.clone(), whisper_for_models.clone());
        }
    });

    let history_button = Button::with_label("Історія");
    let window_weak = window.downgrade();
    let history_for_button = history.clone();
    history_button.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            show_history_dialog(&window, history_for_button.clone());
        }
    });

    let settings_button = Button::with_label("Налаштування");
    let window_weak = window.downgrade();
    let config_for_settings = config.clone();
    let reload_hotkeys_tx_for_settings = reload_hotkeys_tx.clone();
    settings_button.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            show_settings_dialog(&window, config_for_settings.clone(), reload_hotkeys_tx_for_settings.clone());
        }
    });

    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(gtk4::Align::Center);
    button_box.append(&record_button);
    button_box.append(&copy_button);
    button_box.append(&models_button);
    button_box.append(&history_button);
    button_box.append(&settings_button);

    main_box.append(&status_box);
    
    // Mode selector row
    let mode_row = GtkBox::new(Orientation::Horizontal, 8);
    let mode_label = Label::new(Some("Режим:"));
    mode_label.set_halign(gtk4::Align::Start);
    mode_row.append(&mode_label);
    mode_row.append(&mode_combo);
    mode_row.set_halign(gtk4::Align::Start);
    main_box.append(&mode_row);
    
    main_box.append(&timer_label);
    main_box.append(&level_bar);
    main_box.append(&level_bars_box);
    main_box.append(&result_scrolled);
    main_box.append(&button_box);

    window.set_child(Some(&main_box));

    window.connect_close_request(|window| {
        window.hide();
        glib::Propagation::Stop
    });

    // Listen for "open models dialog" signal from tray
    let window_for_models = window.downgrade();
    let config_for_tray = config.clone();
    let whisper_for_tray = whisper.clone();
    glib::spawn_future_local(async move {
        while open_models_rx.recv().await.is_ok() {
            if let Some(window) = window_for_models.upgrade() {
                show_model_dialog(&window, config_for_tray.clone(), whisper_for_tray.clone());
            }
        }
    });

    // Listen for "open history dialog" signal from tray
    let window_for_history = window.downgrade();
    let history_for_tray = history.clone();
    glib::spawn_future_local(async move {
        while open_history_rx.recv().await.is_ok() {
            if let Some(window) = window_for_history.upgrade() {
                show_history_dialog(&window, history_for_tray.clone());
            }
        }
    });

    // Listen for "open settings dialog" signal from tray
    let window_for_settings = window.downgrade();
    let config_for_tray = config.clone();
    let reload_hotkeys_tx_for_tray = reload_hotkeys_tx.clone();
    glib::spawn_future_local(async move {
        while open_settings_rx.recv().await.is_ok() {
            if let Some(window) = window_for_settings.upgrade() {
                show_settings_dialog(&window, config_for_tray.clone(), reload_hotkeys_tx_for_tray.clone());
            }
        }
    });

    // Listen for hotkey toggle recording signal
    let record_button_for_hotkey = record_button.clone();
    let status_label_for_hotkey = status_label.clone();
    let result_text_view_for_hotkey = result_text_view.clone();
    let timer_label_for_hotkey = timer_label.clone();
    let level_bar_for_hotkey = level_bar.clone();
    let mic_level_bar_for_hotkey = mic_level_bar.clone();
    let loopback_level_bar_for_hotkey = loopback_level_bar.clone();
    let spinner_for_hotkey = spinner.clone();
    let recorder_for_hotkey = recorder.clone();
    let conference_recorder_for_hotkey = conference_recorder.clone();
    let diarization_engine_for_hotkey = diarization_engine.clone();
    let mode_combo_for_hotkey = mode_combo.clone();
    let whisper_for_hotkey = whisper.clone();
    let config_for_hotkey = config.clone();
    let history_for_hotkey = history.clone();
    let app_state_for_hotkey = app_state.clone();
    let recording_start_time_for_hotkey = recording_start_time.clone();
    glib::spawn_future_local(async move {
        while toggle_recording_rx.recv().await.is_ok() {
            let is_conference = mode_combo_for_hotkey.active() == Some(1);
            
            match app_state_for_hotkey.get() {
                AppState::Idle => {
                    if is_conference {
                        handle_start_conference(
                            &record_button_for_hotkey,
                            &status_label_for_hotkey,
                            &result_text_view_for_hotkey,
                            &timer_label_for_hotkey,
                            &mic_level_bar_for_hotkey,
                            &loopback_level_bar_for_hotkey,
                            &conference_recorder_for_hotkey,
                            &whisper_for_hotkey,
                            &app_state_for_hotkey,
                            &recording_start_time_for_hotkey,
                        );
                    } else {
                        handle_start_recording(
                            &record_button_for_hotkey,
                            &status_label_for_hotkey,
                            &result_text_view_for_hotkey,
                            &timer_label_for_hotkey,
                            &level_bar_for_hotkey,
                            &recorder_for_hotkey,
                            &whisper_for_hotkey,
                            &app_state_for_hotkey,
                            &recording_start_time_for_hotkey,
                        );
                    }
                }
                AppState::Recording => {
                    if is_conference {
                        handle_stop_conference(
                            &record_button_for_hotkey,
                            &status_label_for_hotkey,
                            &result_text_view_for_hotkey,
                            &timer_label_for_hotkey,
                            &mic_level_bar_for_hotkey,
                            &loopback_level_bar_for_hotkey,
                            &spinner_for_hotkey,
                            &conference_recorder_for_hotkey,
                            &whisper_for_hotkey,
                            &config_for_hotkey,
                            &history_for_hotkey,
                            &diarization_engine_for_hotkey,
                            &app_state_for_hotkey,
                            &recording_start_time_for_hotkey,
                        );
                    } else {
                        handle_stop_recording(
                            &record_button_for_hotkey,
                            &status_label_for_hotkey,
                            &result_text_view_for_hotkey,
                            &timer_label_for_hotkey,
                            &level_bar_for_hotkey,
                            &spinner_for_hotkey,
                            &recorder_for_hotkey,
                            &whisper_for_hotkey,
                            &config_for_hotkey,
                            &history_for_hotkey,
                            &app_state_for_hotkey,
                            &recording_start_time_for_hotkey,
                        );
                    }
                }
                AppState::Processing => {
                    // Ignore hotkey while processing
                }
            }
        }
    });

    window.present();
}

fn setup_record_button(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    level_bar: &LevelBar,
    mic_level_bar: &LevelBar,
    loopback_level_bar: &LevelBar,
    spinner: &Spinner,
    recorder: Arc<AudioRecorder>,
    conference_recorder: Arc<ConferenceRecorder>,
    mode_combo: gtk4::ComboBoxText,
    whisper: Arc<Mutex<Option<WhisperSTT>>>,
    config: Arc<Mutex<Config>>,
    history: Arc<Mutex<History>>,
    diarization_engine: Arc<Mutex<crate::diarization::DiarizationEngine>>,
    app_state: Rc<Cell<AppState>>,
    recording_start_time: Rc<Cell<Option<Instant>>>,
) {
    let recorder_clone = recorder.clone();
    let conference_recorder_clone = conference_recorder.clone();
    let diarization_engine_clone = diarization_engine.clone();
    let mode_combo_clone = mode_combo.clone();
    let status_label_clone = status_label.clone();
    let result_text_view_clone = result_text_view.clone();
    let timer_label_clone = timer_label.clone();
    let level_bar_clone = level_bar.clone();
    let mic_level_bar_clone = mic_level_bar.clone();
    let loopback_level_bar_clone = loopback_level_bar.clone();
    let spinner_clone = spinner.clone();
    let button_clone = button.clone();
    let app_state_clone = app_state.clone();
    let recording_start_time_clone = recording_start_time.clone();

    button.connect_clicked(move |_| {
        let is_conference = mode_combo_clone.active() == Some(1);
        
        match app_state_clone.get() {
            AppState::Idle => {
                if is_conference {
                    handle_start_conference(
                        &button_clone,
                        &status_label_clone,
                        &result_text_view_clone,
                        &timer_label_clone,
                        &mic_level_bar_clone,
                        &loopback_level_bar_clone,
                        &conference_recorder_clone,
                        &whisper,
                        &app_state_clone,
                        &recording_start_time_clone,
                    );
                } else {
                    handle_start_recording(
                        &button_clone,
                        &status_label_clone,
                        &result_text_view_clone,
                        &timer_label_clone,
                        &level_bar_clone,
                        &recorder_clone,
                        &whisper,
                        &app_state_clone,
                        &recording_start_time_clone,
                    );
                }
            }
            AppState::Recording => {
                if is_conference {
                    handle_stop_conference(
                        &button_clone,
                        &status_label_clone,
                        &result_text_view_clone,
                        &timer_label_clone,
                        &mic_level_bar_clone,
                        &loopback_level_bar_clone,
                        &spinner_clone,
                        &conference_recorder_clone,
                        &whisper,
                        &config,
                        &history,
                        &diarization_engine_clone,
                        &app_state_clone,
                        &recording_start_time_clone,
                    );
                } else {
                    handle_stop_recording(
                        &button_clone,
                        &status_label_clone,
                        &result_text_view_clone,
                        &timer_label_clone,
                        &level_bar_clone,
                        &spinner_clone,
                        &recorder_clone,
                        &whisper,
                        &config,
                        &history,
                        &app_state_clone,
                        &recording_start_time_clone,
                    );
                }
            }
            AppState::Processing => {
                // Ignore clicks while processing
            }
        }
    });
}

fn handle_start_recording(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    level_bar: &LevelBar,
    recorder: &Arc<AudioRecorder>,
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

    match recorder.start_recording() {
        Ok(()) => {
            app_state.set(AppState::Recording);
            recording_start_time.set(Some(Instant::now()));

            button.set_label("Зупинити запис");
            button.remove_css_class("suggested-action");
            button.add_css_class("destructive-action");
            status_label.set_text("Запис...");
            let buffer = result_text_view.buffer();
            buffer.set_text("");

            // Show timer and level bar
            timer_label.set_text("00:00");
            timer_label.set_visible(true);
            level_bar.set_value(0.0);
            level_bar.set_visible(true);

            // Start timer update loop (1 second interval)
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

            // Start level bar update loop (faster interval for smooth visualization)
            let level_bar_clone = level_bar.clone();
            let recorder_clone = recorder.clone();
            let app_state_clone = app_state.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                if app_state_clone.get() != AppState::Recording {
                    return glib::ControlFlow::Break;
                }
                let amplitude = recorder_clone.get_amplitude();
                level_bar_clone.set_value(amplitude as f64);
                glib::ControlFlow::Continue
            });
        }
        Err(e) => {
            status_label.set_text(&format!("Помилка: {}", e));
        }
    }
}

fn handle_stop_recording(
    button: &Button,
    status_label: &Label,
    result_text_view: &TextView,
    timer_label: &Label,
    level_bar: &LevelBar,
    spinner: &Spinner,
    recorder: &Arc<AudioRecorder>,
    whisper: &Arc<Mutex<Option<WhisperSTT>>>,
    config: &Arc<Mutex<Config>>,
    history: &Arc<Mutex<History>>,
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
    level_bar.set_visible(false);
    spinner.set_visible(true);
    spinner.start();

    let (samples, completion_rx) = recorder.stop_recording();

    // Calculate and display recording duration
    let duration_secs = samples.len() as f32 / WHISPER_SAMPLE_RATE as f32;
    let duration_mins = (duration_secs / 60.0).floor() as u32;
    let duration_remaining_secs = (duration_secs % 60.0).floor() as u32;
    status_label.set_text(&format!(
        "Обробка запису {:02}:{:02}...",
        duration_mins, duration_remaining_secs
    ));

    let whisper = whisper.clone();
    let history = history.clone();
    let config_for_auto_copy = config.clone();
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
        // Wait for recording thread to finish (non-blocking for GTK)
        if let Some(rx) = completion_rx {
            let _ = rx.recv().await;
        }

        // Now transcribe in a separate thread
        let (tx, rx) = async_channel::bounded::<anyhow::Result<String>>(1);

        std::thread::spawn(move || {
            let result = if samples.len() < MIN_RECORDING_SAMPLES {
                Err(anyhow::anyhow!("Запис закороткий"))
            } else {
                let w = whisper.lock().unwrap();
                if let Some(ref whisper) = *w {
                    whisper.transcribe(&samples, Some(&language))
                } else {
                    Err(anyhow::anyhow!("Модель не завантажено"))
                }
            };
            let _ = tx.send_blocking(result);
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

                        // Copy to clipboard if auto-copy or auto-paste is enabled
                        // (auto-paste requires clipboard to work)
                        // Note: This copies the original transcription text immediately
                        // User can edit the text in the TextView and manually copy if needed
                        if auto_copy_enabled || auto_paste_enabled {
                            copy_to_clipboard(&text);
                        }

                        // Auto-paste if enabled (simulates Ctrl+V)
                        if auto_paste_enabled {
                            // Small delay to ensure clipboard is ready
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if let Err(e) = crate::paste::paste_from_clipboard() {
                                eprintln!("Помилка автоматичної вставки: {}", e);
                                status_label.set_text(&format!("Готово! (помилка вставки: {})", e));
                            }
                        }

                        // Save to history - save original transcription text
                        // User can edit in the UI and copy manually if they want the edited version
                        let entry = HistoryEntry::new(
                            text,
                            duration_secs,
                            language_for_history.clone(),
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

fn copy_to_clipboard(text: &str) {
    if let Some(display) = gtk4::gdk::Display::default() {
        let clipboard = display.clipboard();
        if !text.is_empty() {
            clipboard.set_text(text);
        }
    }
}

fn handle_start_conference(
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

fn handle_stop_conference(
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
                            copy_to_clipboard(&text);
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

fn setup_copy_button(button: &Button, result_text_view: &TextView) {
    let result_text_view_clone = result_text_view.clone();

    button.connect_clicked(move |_| {
        let buffer = result_text_view_clone.buffer();
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        let text = buffer.text(&start, &end, false).to_string();
        copy_to_clipboard(&text);
    });
}
