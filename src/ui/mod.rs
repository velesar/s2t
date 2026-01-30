pub mod conference;
pub mod conference_file;
pub mod mic;
mod dispatch;
pub mod state;
mod widgets;

use dispatch::ModeUIs;
use state::{ConferenceUI, MicUI, RecordingContext, UIContext};
use widgets::build_main_widgets;

use crate::app::context::AppContext;
use crate::dialogs::{show_history_dialog, show_model_dialog, show_settings_dialog};
use crate::domain::traits::Transcription;
use crate::domain::types::SharedHistory;
use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Button, TextView};
use std::sync::Arc;
use parking_lot::Mutex;

pub fn build_ui(app: &Application, ctx: Arc<AppContext>) {
    let config = ctx.config.clone();
    let history: SharedHistory = ctx.history.clone();
    let transcription: Arc<Mutex<dyn Transcription>> = ctx.transcription.clone();

    let open_models_rx = ctx.channels.open_models_rx().clone();
    let open_history_rx = ctx.channels.open_history_rx().clone();
    let open_settings_rx = ctx.channels.open_settings_rx().clone();
    let toggle_recording_rx = ctx.channels.toggle_recording_rx().clone();
    let reload_hotkeys_tx = ctx.channels.reload_hotkeys_tx().clone();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Голосова диктовка")
        .default_width(500)
        .default_height(300)
        .build();

    // Build all widgets and assemble layout
    let w = build_main_widgets(&config);

    // Mode combo change handler: toggle level bar visibility and persist mode
    let level_bar_clone = w.level_bar.clone();
    let level_bars_box_clone = w.level_bars_box.clone();
    let config_for_mode = config.clone();
    w.mode_combo.connect_changed(move |combo| {
        // Conference modes (1 and 2) show dual level bars, dictation (0) shows single
        let is_conference_mode = matches!(combo.active(), Some(1) | Some(2));
        level_bar_clone.set_visible(!is_conference_mode);
        level_bars_box_clone.set_visible(is_conference_mode);

        let mut cfg = config_for_mode.lock();
        cfg.recording_mode = match combo.active() {
            Some(1) => "conference".to_string(),
            Some(2) => "conference_file".to_string(),
            _ => "dictation".to_string(),
        };
        if let Err(e) = crate::app::config::save_config(&cfg) {
            eprintln!("Помилка збереження режиму: {}", e);
        }
    });

    // Create context structs for recording modes
    let rec_ctx = RecordingContext::new();
    let ui_ctx = UIContext::new(
        w.record_button.clone(),
        w.status_label.clone(),
        w.result_text_view.clone(),
        w.timer_label.clone(),
        w.spinner.clone(),
    );
    let mic_ui = MicUI::new(
        ui_ctx.clone(),
        w.level_bar.clone(),
        w.vad_indicator.clone(),
        w.segment_indicators_box.clone(),
        w.segment_row.clone(),
    );
    let conference_ui = ConferenceUI::new(
        ui_ctx,
        w.mic_level_bar.clone(),
        w.loopback_level_bar.clone(),
    );

    let mode_uis = ModeUIs {
        mic: mic_ui.clone(),
        conference: conference_ui.clone(),
    };

    setup_record_button(
        ctx.clone(),
        rec_ctx.clone(),
        mode_uis.clone(),
        w.mode_combo.clone(),
    );

    setup_copy_button(&w.copy_button, &w.result_text_view);

    // Models button
    let window_weak = window.downgrade();
    let config_for_models = config.clone();
    let transcription_for_models = transcription.clone();
    w.models_button.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            show_model_dialog(
                &window,
                config_for_models.clone(),
                transcription_for_models.clone(),
            );
        }
    });

    // History button
    let window_weak = window.downgrade();
    let history_for_button = history.clone();
    w.history_button.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            show_history_dialog(&window, history_for_button.clone());
        }
    });

    // Settings button
    let window_weak = window.downgrade();
    let config_for_settings = config.clone();
    let reload_hotkeys_tx_for_settings = reload_hotkeys_tx.clone();
    w.settings_button.connect_clicked(move |_| {
        if let Some(window) = window_weak.upgrade() {
            show_settings_dialog(
                &window,
                config_for_settings.clone(),
                reload_hotkeys_tx_for_settings.clone(),
            );
        }
    });

    window.set_child(Some(&w.main_box));

    window.connect_close_request(|window| {
        window.hide();
        glib::Propagation::Stop
    });

    // Listen for "open models dialog" signal from tray
    let window_for_models = window.downgrade();
    let config_for_tray = config.clone();
    let transcription_for_tray = transcription.clone();
    glib::spawn_future_local(async move {
        while open_models_rx.recv().await.is_ok() {
            if let Some(window) = window_for_models.upgrade() {
                show_model_dialog(
                    &window,
                    config_for_tray.clone(),
                    transcription_for_tray.clone(),
                );
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
                show_settings_dialog(
                    &window,
                    config_for_tray.clone(),
                    reload_hotkeys_tx_for_tray.clone(),
                );
            }
        }
    });

    // Listen for hotkey toggle recording signal
    let ctx_for_hotkey = ctx.clone();
    let rec_ctx_for_hotkey = rec_ctx.clone();
    let mode_uis_for_hotkey = ModeUIs {
        mic: mic_ui.clone(),
        conference: conference_ui.clone(),
    };
    let mode_combo_for_hotkey = w.mode_combo.clone();
    glib::spawn_future_local(async move {
        while toggle_recording_rx.recv().await.is_ok() {
            dispatch::toggle_recording(
                &ctx_for_hotkey,
                &rec_ctx_for_hotkey,
                &mode_uis_for_hotkey,
                &mode_combo_for_hotkey,
            );
        }
    });

    window.present();
}

fn setup_record_button(
    ctx: Arc<AppContext>,
    rec_ctx: RecordingContext,
    mode_uis: ModeUIs,
    mode_combo: gtk4::ComboBoxText,
) {
    let button = mode_uis.mic.base.button.clone();
    button.connect_clicked(move |_| {
        dispatch::toggle_recording(&ctx, &rec_ctx, &mode_uis, &mode_combo);
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
