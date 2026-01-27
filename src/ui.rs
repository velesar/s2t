use crate::audio::AudioRecorder;
use crate::config::Config;
use crate::model_dialog::show_model_dialog;
use crate::whisper::WhisperSTT;
use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow, Box as GtkBox, Button, Label, Orientation};
use std::sync::{Arc, Mutex};
use std::thread;

const MIN_RECORDING_SAMPLES: usize = 16000; // 1 second at 16kHz

pub fn build_ui(app: &Application, whisper: Arc<Mutex<Option<WhisperSTT>>>, config: Arc<Mutex<Config>>) {
    let recorder = Arc::new(AudioRecorder::new());

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Голосова диктовка")
        .default_width(500)
        .default_height(300)
        .build();

    let main_box = GtkBox::new(Orientation::Vertical, 12);
    main_box.set_margin_top(20);
    main_box.set_margin_bottom(20);
    main_box.set_margin_start(20);
    main_box.set_margin_end(20);

    let status_label = Label::new(Some("Натисніть кнопку для запису"));
    status_label.add_css_class("title-2");

    let result_label = Label::new(Some(""));
    result_label.set_wrap(true);
    result_label.set_selectable(true);
    result_label.set_vexpand(true);
    result_label.set_valign(gtk4::Align::Start);

    let record_button = Button::with_label("Почати запис");
    record_button.add_css_class("suggested-action");
    record_button.add_css_class("pill");

    let config_for_ui = config.clone();
    setup_record_button(&record_button, &status_label, &result_label, recorder, whisper.clone(), config_for_ui);

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

    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(gtk4::Align::Center);
    button_box.append(&record_button);
    button_box.append(&copy_button);
    button_box.append(&models_button);

    main_box.append(&status_label);
    main_box.append(&result_label);
    main_box.append(&button_box);

    window.set_child(Some(&main_box));

    window.connect_close_request(|window| {
        window.hide();
        glib::Propagation::Stop
    });

    window.present();
}

fn setup_record_button(
    button: &Button,
    status_label: &Label,
    result_label: &Label,
    recorder: Arc<AudioRecorder>,
    whisper: Arc<Mutex<Option<WhisperSTT>>>,
    config: Arc<Mutex<Config>>,
) {
    let recorder_clone = recorder.clone();
    let status_label_clone = status_label.clone();
    let result_label_clone = result_label.clone();
    let button_clone = button.clone();

    button.connect_clicked(move |_| {
        if recorder_clone.is_recording() {
            handle_stop_recording(
                &button_clone,
                &status_label_clone,
                &result_label_clone,
                &recorder_clone,
                &whisper,
                &config,
            );
        } else {
            handle_start_recording(
                &button_clone,
                &status_label_clone,
                &result_label_clone,
                &recorder_clone,
                &whisper,
            );
        }
    });
}

fn handle_start_recording(
    button: &Button,
    status_label: &Label,
    result_label: &Label,
    recorder: &Arc<AudioRecorder>,
    whisper: &Arc<Mutex<Option<WhisperSTT>>>,
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
            button.set_label("Зупинити запис");
            button.remove_css_class("suggested-action");
            button.add_css_class("destructive-action");
            status_label.set_text("Запис...");
            result_label.set_text("");
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
    recorder: &Arc<AudioRecorder>,
    whisper: &Arc<Mutex<Option<WhisperSTT>>>,
    config: &Arc<Mutex<Config>>,
) {
    button.set_label("Почати запис");
    button.remove_css_class("destructive-action");
    button.add_css_class("suggested-action");
    status_label.set_text("Обробка...");

    let samples = recorder.stop_recording();
    let whisper = whisper.clone();
    let status_label = status_label.clone();
    let result_label = result_label.clone();
    let language = {
        let cfg = config.lock().unwrap();
        cfg.language.clone()
    };

    let (tx, rx) = async_channel::bounded::<anyhow::Result<String>>(1);

    thread::spawn(move || {
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

    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            match result {
                Ok(text) => {
                    if text.is_empty() {
                        status_label.set_text("Не вдалося розпізнати мову");
                    } else {
                        status_label.set_text("Готово!");
                        result_label.set_text(&text);
                    }
                }
                Err(e) => {
                    status_label.set_text(&format!("Помилка: {}", e));
                }
            }
        }
    });
}

fn setup_copy_button(button: &Button, result_label: &Label) {
    let result_label_clone = result_label.clone();

    button.connect_clicked(move |_| {
        if let Some(display) = gtk4::gdk::Display::default() {
            let clipboard = display.clipboard();
            let text = result_label_clone.text();
            if !text.is_empty() {
                clipboard.set_text(&text);
            }
        }
    });
}
