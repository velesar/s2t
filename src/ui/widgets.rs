use crate::config::Config;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, LevelBar, Orientation, ScrolledWindow, Spinner, TextView,
};
use std::sync::{Arc, Mutex};

/// All widgets needed by the main UI orchestrator for signal wiring and context creation.
pub struct MainWidgets {
    pub main_box: GtkBox,
    pub status_label: Label,
    pub spinner: Spinner,
    pub timer_label: Label,
    pub mode_combo: gtk4::ComboBoxText,
    pub level_bar: LevelBar,
    pub level_bars_box: GtkBox,
    pub vad_indicator: Label,
    pub segment_row: GtkBox,
    pub segment_indicators_box: GtkBox,
    pub mic_level_bar: LevelBar,
    pub loopback_level_bar: LevelBar,
    pub result_text_view: TextView,
    pub record_button: Button,
    pub copy_button: Button,
    pub models_button: Button,
    pub history_button: Button,
    pub settings_button: Button,
}

/// Creates all main window widgets and assembles the layout.
/// Signal wiring is left to the caller (build_ui).
pub fn build_main_widgets(config: &Arc<Mutex<Config>>) -> MainWidgets {
    let main_box = GtkBox::new(Orientation::Vertical, 12);
    main_box.set_margin_top(20);
    main_box.set_margin_bottom(20);
    main_box.set_margin_start(20);
    main_box.set_margin_end(20);

    // Status row with label and spinner
    let status_box = GtkBox::new(Orientation::Horizontal, 8);
    status_box.set_halign(Align::Center);

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
    mode_combo.set_active(Some(0));
    mode_combo.set_halign(Align::Start);

    let current_mode = {
        let cfg = config.lock().unwrap();
        if cfg.recording_mode == "conference" {
            mode_combo.set_active(Some(1));
        }
        cfg.recording_mode.clone()
    };

    let mode_row = GtkBox::new(Orientation::Horizontal, 8);
    let mode_label = Label::new(Some("Режим:"));
    mode_label.set_halign(Align::Start);
    mode_row.append(&mode_label);
    mode_row.append(&mode_combo);
    mode_row.set_halign(Align::Start);

    // Dictation audio level indicator
    let level_bar = LevelBar::new();
    level_bar.set_min_value(0.0);
    level_bar.set_max_value(1.0);
    level_bar.set_value(0.0);
    level_bar.set_visible(false);
    level_bar.set_size_request(200, -1);

    // VAD indicator for continuous mode
    let vad_indicator = Label::new(Some(""));
    vad_indicator.set_visible(false);
    vad_indicator.set_halign(Align::Start);

    // Segment progress indicators for continuous mode
    let segment_row = GtkBox::new(Orientation::Horizontal, 8);
    segment_row.set_halign(Align::Start);

    let segment_label = Label::new(Some("Сегменти:"));
    segment_row.append(&segment_label);

    let segment_indicators_box = GtkBox::new(Orientation::Horizontal, 4);
    segment_indicators_box.set_halign(Align::Start);

    let segment_scroll = ScrolledWindow::new();
    segment_scroll.set_policy(gtk4::PolicyType::Automatic, gtk4::PolicyType::Never);
    segment_scroll.set_hexpand(true);
    segment_scroll.set_child(Some(&segment_indicators_box));

    segment_row.append(&segment_scroll);
    segment_row.set_hexpand(true);
    segment_row.set_visible(false);

    // CSS for segment indicator styling
    load_segment_css();

    // Conference mode: mic + loopback level bars
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
    mic_label.set_halign(Align::Start);
    level_bars_box.append(&mic_label);
    level_bars_box.append(&mic_level_bar);
    let loopback_label = Label::new(Some("Системний аудіо:"));
    loopback_label.set_halign(Align::Start);
    loopback_label.set_margin_top(6);
    level_bars_box.append(&loopback_label);
    level_bars_box.append(&loopback_level_bar);
    level_bars_box.set_visible(false);

    // Editable result display
    let result_text_view = gtk4::TextView::new();
    result_text_view.set_wrap_mode(gtk4::WrapMode::Word);
    result_text_view.set_editable(true);
    result_text_view.set_cursor_visible(true);
    result_text_view.set_vexpand(true);

    let result_scrolled = ScrolledWindow::new();
    result_scrolled.set_min_content_height(100);
    result_scrolled.set_child(Some(&result_text_view));

    // Record button
    let record_button = Button::with_label("Почати запис");
    record_button.add_css_class("suggested-action");
    record_button.add_css_class("pill");

    // Set initial visibility based on saved mode
    level_bar.set_visible(current_mode != "conference");
    level_bars_box.set_visible(current_mode == "conference");

    // Action buttons (signal wiring done by caller)
    let copy_button = Button::with_label("Копіювати");
    let models_button = Button::with_label("Моделі");
    let history_button = Button::with_label("Історія");
    let settings_button = Button::with_label("Налаштування");

    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(Align::Center);
    button_box.append(&record_button);
    button_box.append(&copy_button);
    button_box.append(&models_button);
    button_box.append(&history_button);
    button_box.append(&settings_button);

    // Assemble layout
    main_box.append(&status_box);
    main_box.append(&mode_row);
    main_box.append(&timer_label);
    main_box.append(&level_bar);
    main_box.append(&vad_indicator);
    main_box.append(&segment_row);
    main_box.append(&level_bars_box);
    main_box.append(&result_scrolled);
    main_box.append(&button_box);

    MainWidgets {
        main_box,
        status_label,
        spinner,
        timer_label,
        mode_combo,
        level_bar,
        level_bars_box,
        vad_indicator,
        segment_row,
        segment_indicators_box,
        mic_level_bar,
        loopback_level_bar,
        result_text_view,
        record_button,
        copy_button,
        models_button,
        history_button,
        settings_button,
    }
}

fn load_segment_css() {
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
}
