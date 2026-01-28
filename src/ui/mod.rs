pub mod conference;
pub mod continuous;
pub mod recording;
pub mod state;

use state::{AppState, ConferenceUI, ContinuousUI, DictationUI, RecordingContext, UIContext};

use crate::context::AppContext;
use crate::history_dialog::show_history_dialog;
use crate::model_dialog::show_model_dialog;
use crate::settings_dialog::show_settings_dialog;
use crate::whisper::WhisperSTT;
use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box as GtkBox, Button, Label, LevelBar, Orientation, Spinner, TextView};
use std::sync::{Arc, Mutex};

pub fn build_ui(app: &Application, ctx: Arc<AppContext>) {
    // Extract legacy references from AppContext for components not yet migrated
    let config = ctx.config_arc();
    let history = ctx.history_arc();

    // Get channels from AppContext
    let open_models_rx = ctx.channels.open_models_rx().clone();
    let open_history_rx = ctx.channels.open_history_rx().clone();
    let open_settings_rx = ctx.channels.open_settings_rx().clone();
    let toggle_recording_rx = ctx.channels.toggle_recording_rx().clone();
    let reload_hotkeys_tx = ctx.channels.reload_hotkeys_tx().clone();

    // Create legacy whisper Arc from TranscriptionService for model dialog
    // TODO: Migrate model_dialog to use ctx.transcription directly
    let whisper: Arc<Mutex<Option<WhisperSTT>>> = {
        let ts = ctx.transcription.lock().unwrap();
        if ts.is_loaded() {
            let cfg = config.lock().unwrap();
            let model_path = crate::models::get_model_path(&cfg.default_model);
            drop(cfg);
            if model_path.exists() {
                match WhisperSTT::new(model_path.to_str().unwrap_or_default()) {
                    Ok(w) => Arc::new(Mutex::new(Some(w))),
                    Err(_) => Arc::new(Mutex::new(None)),
                }
            } else {
                Arc::new(Mutex::new(None))
            }
        } else {
            Arc::new(Mutex::new(None))
        }
    };

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

    // VAD indicator for continuous mode
    let vad_indicator = Label::new(Some(""));
    vad_indicator.set_visible(false);
    vad_indicator.set_halign(gtk4::Align::Start);

    // Segment progress indicators for continuous mode
    let segment_row = GtkBox::new(Orientation::Horizontal, 8);
    segment_row.set_halign(gtk4::Align::Start);

    let segment_label = Label::new(Some("Сегменти:"));
    segment_row.append(&segment_label);

    let segment_indicators_box = GtkBox::new(Orientation::Horizontal, 4);
    segment_indicators_box.set_halign(gtk4::Align::Start);

    let segment_scroll = gtk4::ScrolledWindow::new();
    segment_scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    segment_scroll.set_hexpand(true);  // Expand to fill available space
    segment_scroll.set_child(Some(&segment_indicators_box));

    segment_row.append(&segment_scroll);
    segment_row.set_hexpand(true);  // Row also expands
    segment_row.set_visible(false);

    // Load CSS for segment indicator styling
    let css_provider = gtk4::CssProvider::new();
    css_provider.load_from_data(
        r#"
        .segment-processing {
            color: #f0a000;
            font-size: 16px;
        }
        .segment-completed {
            color: #00aa00;
            font-size: 16px;
        }
        "#,
    );
    gtk4::style_context_add_provider_for_display(
        &gtk4::gdk::Display::default().expect("Could not get default display"),
        &css_provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

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

    // Create context structs for recording modes
    let rec_ctx = RecordingContext::new();
    let ui_ctx = UIContext::new(
        record_button.clone(),
        status_label.clone(),
        result_text_view.clone(),
        timer_label.clone(),
        spinner.clone(),
    );
    let dictation_ui = DictationUI::new(ui_ctx.clone(), level_bar.clone());
    let conference_ui = ConferenceUI::new(ui_ctx.clone(), mic_level_bar.clone(), loopback_level_bar.clone());
    let continuous_ui = ContinuousUI::new(
        ui_ctx,
        level_bar.clone(),
        vad_indicator.clone(),
        segment_indicators_box.clone(),
        segment_row.clone(),
    );

    setup_record_button(
        ctx.clone(),
        rec_ctx.clone(),
        dictation_ui.clone(),
        conference_ui.clone(),
        continuous_ui.clone(),
        mode_combo.clone(),
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
    main_box.append(&vad_indicator);
    main_box.append(&segment_row);
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
    let ctx_for_hotkey = ctx.clone();
    let rec_ctx_for_hotkey = rec_ctx.clone();
    let dictation_ui_for_hotkey = dictation_ui.clone();
    let conference_ui_for_hotkey = conference_ui.clone();
    let continuous_ui_for_hotkey = continuous_ui.clone();
    let mode_combo_for_hotkey = mode_combo.clone();
    glib::spawn_future_local(async move {
        while toggle_recording_rx.recv().await.is_ok() {
            let is_conference = mode_combo_for_hotkey.active() == Some(1);
            let is_continuous = ctx_for_hotkey.continuous_mode();

            match rec_ctx_for_hotkey.state.get() {
                AppState::Idle => {
                    if is_conference {
                        conference::handle_start(&ctx_for_hotkey, &rec_ctx_for_hotkey, &conference_ui_for_hotkey);
                    } else if is_continuous {
                        continuous::handle_start(&ctx_for_hotkey, &rec_ctx_for_hotkey, &continuous_ui_for_hotkey);
                    } else {
                        recording::handle_start(&ctx_for_hotkey, &rec_ctx_for_hotkey, &dictation_ui_for_hotkey);
                    }
                }
                AppState::Recording => {
                    if is_conference {
                        conference::handle_stop(&ctx_for_hotkey, &rec_ctx_for_hotkey, &conference_ui_for_hotkey);
                    } else if is_continuous {
                        continuous::handle_stop(&ctx_for_hotkey, &rec_ctx_for_hotkey, &continuous_ui_for_hotkey);
                    } else {
                        recording::handle_stop(&ctx_for_hotkey, &rec_ctx_for_hotkey, &dictation_ui_for_hotkey);
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
    ctx: Arc<AppContext>,
    rec_ctx: RecordingContext,
    dictation_ui: DictationUI,
    conference_ui: ConferenceUI,
    continuous_ui: ContinuousUI,
    mode_combo: gtk4::ComboBoxText,
) {
    let mode_combo_clone = mode_combo.clone();

    // Clone context structs for the closure
    let ctx_clone = ctx.clone();
    let rec_ctx_clone = rec_ctx.clone();
    let dictation_ui_clone = dictation_ui.clone();
    let conference_ui_clone = conference_ui.clone();
    let continuous_ui_clone = continuous_ui.clone();

    dictation_ui.base.button.connect_clicked(move |_| {
        let is_conference = mode_combo_clone.active() == Some(1);
        let is_continuous = ctx_clone.continuous_mode();

        match rec_ctx_clone.state.get() {
            AppState::Idle => {
                if is_conference {
                    conference::handle_start(&ctx_clone, &rec_ctx_clone, &conference_ui_clone);
                } else if is_continuous {
                    continuous::handle_start(&ctx_clone, &rec_ctx_clone, &continuous_ui_clone);
                } else {
                    recording::handle_start(&ctx_clone, &rec_ctx_clone, &dictation_ui_clone);
                }
            }
            AppState::Recording => {
                if is_conference {
                    conference::handle_stop(&ctx_clone, &rec_ctx_clone, &conference_ui_clone);
                } else if ctx_clone.continuous_mode() {
                    continuous::handle_stop(&ctx_clone, &rec_ctx_clone, &continuous_ui_clone);
                } else {
                    recording::handle_stop(&ctx_clone, &rec_ctx_clone, &dictation_ui_clone);
                }
            }
            AppState::Processing => {
                // Ignore clicks while processing
            }
        }
    });
}

pub(crate) fn copy_to_clipboard(text: &str) {
    if let Some(display) = gtk4::gdk::Display::default() {
        let clipboard = display.clipboard();
        if !text.is_empty() {
            clipboard.set_text(text);
        }
    }
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
