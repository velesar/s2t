use crate::audio::AudioRecorder;
use crate::config::Config;
use crate::history::{save_history, History, HistoryEntry};
use crate::history_dialog::show_history_dialog;
use crate::model_dialog::show_model_dialog;
use crate::paste::paste_from_clipboard;
use crate::settings_dialog::show_settings_dialog;
use crate::whisper::WhisperSTT;
use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box as GtkBox, Button, Label, LevelBar, Orientation, Spinner};
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
    open_models_rx: async_channel::Receiver<()>,
    open_history_rx: async_channel::Receiver<()>,
    open_settings_rx: async_channel::Receiver<()>,
    toggle_recording_rx: async_channel::Receiver<()>,
    reload_hotkeys_tx: async_channel::Sender<()>,
) {
    let recorder = Arc::new(AudioRecorder::new());

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

    // Audio level indicator
    let level_bar = LevelBar::new();
    level_bar.set_min_value(0.0);
    level_bar.set_max_value(1.0);
    level_bar.set_value(0.0);
    level_bar.set_visible(false);
    level_bar.set_size_request(200, -1);

    let result_label = Label::new(Some(""));
    result_label.set_wrap(true);
    result_label.set_selectable(true);
    result_label.set_vexpand(true);
    result_label.set_valign(gtk4::Align::Start);

    let record_button = Button::with_label("Почати запис");
    record_button.add_css_class("suggested-action");
    record_button.add_css_class("pill");

    // Shared application state
    let app_state = Rc::new(Cell::new(AppState::Idle));
    let recording_start_time: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));

    let config_for_ui = config.clone();
    let history_for_ui = history.clone();
    let recorder_for_button = recorder.clone();
    let app_state_for_button = app_state.clone();
    let recording_start_time_for_button = recording_start_time.clone();
    setup_record_button(
        &record_button,
        &status_label,
        &result_label,
        &timer_label,
        &level_bar,
        &spinner,
        recorder_for_button,
        whisper.clone(),
        config_for_ui,
        history_for_ui,
        app_state_for_button,
        recording_start_time_for_button,
    );

    let copy_button = Button::with_label("Копіювати");
    setup_copy_button(&copy_button, &result_label);

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
    main_box.append(&timer_label);
    main_box.append(&level_bar);
    main_box.append(&result_label);
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
    let result_label_for_hotkey = result_label.clone();
    let timer_label_for_hotkey = timer_label.clone();
    let level_bar_for_hotkey = level_bar.clone();
    let spinner_for_hotkey = spinner.clone();
    let recorder_for_hotkey = recorder.clone();
    let whisper_for_hotkey = whisper.clone();
    let config_for_hotkey = config.clone();
    let history_for_hotkey = history.clone();
    let app_state_for_hotkey = app_state.clone();
    let recording_start_time_for_hotkey = recording_start_time.clone();
    glib::spawn_future_local(async move {
        while toggle_recording_rx.recv().await.is_ok() {
            match app_state_for_hotkey.get() {
                AppState::Idle => {
                    handle_start_recording(
                        &record_button_for_hotkey,
                        &status_label_for_hotkey,
                        &result_label_for_hotkey,
                        &timer_label_for_hotkey,
                        &level_bar_for_hotkey,
                        &recorder_for_hotkey,
                        &whisper_for_hotkey,
                        &app_state_for_hotkey,
                        &recording_start_time_for_hotkey,
                    );
                }
                AppState::Recording => {
                    handle_stop_recording(
                        &record_button_for_hotkey,
                        &status_label_for_hotkey,
                        &result_label_for_hotkey,
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
    result_label: &Label,
    timer_label: &Label,
    level_bar: &LevelBar,
    spinner: &Spinner,
    recorder: Arc<AudioRecorder>,
    whisper: Arc<Mutex<Option<WhisperSTT>>>,
    config: Arc<Mutex<Config>>,
    history: Arc<Mutex<History>>,
    app_state: Rc<Cell<AppState>>,
    recording_start_time: Rc<Cell<Option<Instant>>>,
) {
    let recorder_clone = recorder.clone();
    let status_label_clone = status_label.clone();
    let result_label_clone = result_label.clone();
    let timer_label_clone = timer_label.clone();
    let level_bar_clone = level_bar.clone();
    let spinner_clone = spinner.clone();
    let button_clone = button.clone();
    let app_state_clone = app_state.clone();
    let recording_start_time_clone = recording_start_time.clone();

    button.connect_clicked(move |_| {
        match app_state_clone.get() {
            AppState::Idle => {
                handle_start_recording(
                    &button_clone,
                    &status_label_clone,
                    &result_label_clone,
                    &timer_label_clone,
                    &level_bar_clone,
                    &recorder_clone,
                    &whisper,
                    &app_state_clone,
                    &recording_start_time_clone,
                );
            }
            AppState::Recording => {
                handle_stop_recording(
                    &button_clone,
                    &status_label_clone,
                    &result_label_clone,
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
            AppState::Processing => {
                // Ignore clicks while processing
            }
        }
    });
}

fn handle_start_recording(
    button: &Button,
    status_label: &Label,
    result_label: &Label,
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
            result_label.set_text("");

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
    result_label: &Label,
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
    let result_label = result_label.clone();
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
                        result_label.set_text(&text);

                        // Get config values
                        let (auto_copy_enabled, auto_paste_enabled) = {
                            let cfg = config_for_auto_copy.lock().unwrap();
                            (cfg.auto_copy, cfg.auto_paste)
                        };

                        // Copy to clipboard if auto-copy or auto-paste is enabled
                        // (auto-paste requires clipboard to work)
                        if auto_copy_enabled || auto_paste_enabled {
                            copy_to_clipboard(&text);
                        }

                        // Auto-paste if enabled (simulates Ctrl+V)
                        if auto_paste_enabled {
                            // Small delay to ensure clipboard is ready
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            if let Err(e) = paste_from_clipboard() {
                                eprintln!("Помилка автоматичної вставки: {}", e);
                                status_label.set_text(&format!("Готово! (помилка вставки: {})", e));
                            }
                        }

                        // Save to history
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

fn setup_copy_button(button: &Button, result_label: &Label) {
    let result_label_clone = result_label.clone();

    button.connect_clicked(move |_| {
        let text = result_label_clone.text();
        copy_to_clipboard(&text);
    });
}
