use crate::config::{save_config, Config};
use crate::models::{
    delete_model, download_model, format_size, get_available_models, is_model_downloaded,
};
use crate::whisper::WhisperSTT;
use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation, ProgressBar,
    ScrolledWindow, SelectionMode, Window,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

struct RowWidgets {
    indicator: Label,
    set_default_button: Button,
    delete_button: Button,
}

type RowWidgetsMap = Rc<RefCell<HashMap<String, RowWidgets>>>;

pub fn show_model_dialog(
    parent: &impl IsA<Window>,
    config: Arc<Mutex<Config>>,
    whisper: Arc<Mutex<Option<WhisperSTT>>>,
) {
    let dialog = Window::builder()
        .title("Керування моделями")
        .modal(true)
        .transient_for(parent)
        .default_width(500)
        .default_height(400)
        .build();

    let main_box = GtkBox::new(Orientation::Vertical, 0);

    let scrolled = ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::None);
    list_box.add_css_class("boxed-list");
    list_box.set_margin_top(12);
    list_box.set_margin_bottom(12);
    list_box.set_margin_start(12);
    list_box.set_margin_end(12);

    let download_states: Rc<RefCell<HashMap<String, DownloadState>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let row_widgets: RowWidgetsMap = Rc::new(RefCell::new(HashMap::new()));

    for model in get_available_models() {
        let row = create_model_row(
            &model.filename,
            &model.display_name,
            model.size_bytes,
            &model.description,
            config.clone(),
            whisper.clone(),
            download_states.clone(),
            row_widgets.clone(),
        );
        list_box.append(&row);
    }

    scrolled.set_child(Some(&list_box));
    main_box.append(&scrolled);

    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(Align::End);
    button_box.set_margin_top(12);
    button_box.set_margin_bottom(12);
    button_box.set_margin_start(12);
    button_box.set_margin_end(12);

    let close_button = Button::with_label("Закрити");
    let dialog_weak = dialog.downgrade();
    close_button.connect_clicked(move |_| {
        if let Some(dialog) = dialog_weak.upgrade() {
            dialog.close();
        }
    });
    button_box.append(&close_button);

    main_box.append(&button_box);
    dialog.set_child(Some(&main_box));

    dialog.present();
}

#[derive(Clone)]
enum DownloadState {
    Idle,
    Downloading,
}

fn create_model_row(
    filename: &str,
    display_name: &str,
    size_bytes: u64,
    description: &str,
    config: Arc<Mutex<Config>>,
    whisper: Arc<Mutex<Option<WhisperSTT>>>,
    download_states: Rc<RefCell<HashMap<String, DownloadState>>>,
    row_widgets: RowWidgetsMap,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_activatable(false);

    let content_box = GtkBox::new(Orientation::Vertical, 6);
    content_box.set_margin_top(12);
    content_box.set_margin_bottom(12);
    content_box.set_margin_start(12);
    content_box.set_margin_end(12);

    let top_row = GtkBox::new(Orientation::Horizontal, 12);

    let is_downloaded = is_model_downloaded(filename);
    let is_default = {
        let cfg = config.lock().unwrap();
        cfg.default_model == filename
    };

    let name_box = GtkBox::new(Orientation::Horizontal, 8);

    let default_indicator = Label::new(Some(if is_default { "[*]" } else { "[ ]" }));
    default_indicator.add_css_class("monospace");
    name_box.append(&default_indicator);

    let name_label = Label::new(Some(&format!("{} ({})", display_name, format_size(size_bytes))));
    name_label.set_hexpand(true);
    name_label.set_halign(Align::Start);
    name_label.add_css_class("heading");
    name_box.append(&name_label);

    top_row.append(&name_box);

    let status_label = Label::new(Some(if is_downloaded {
        "Завантажено"
    } else {
        ""
    }));
    status_label.add_css_class("dim-label");
    top_row.append(&status_label);

    content_box.append(&top_row);

    let desc_label = Label::new(Some(description));
    desc_label.set_halign(Align::Start);
    desc_label.add_css_class("dim-label");
    content_box.append(&desc_label);

    let progress_bar = ProgressBar::new();
    progress_bar.set_visible(false);
    progress_bar.set_show_text(true);
    content_box.append(&progress_bar);

    let button_box = GtkBox::new(Orientation::Horizontal, 8);
    button_box.set_halign(Align::End);
    button_box.set_margin_top(6);

    let set_default_button = Button::with_label("За замовч.");
    set_default_button.set_sensitive(is_downloaded && !is_default);

    let download_button = Button::with_label("Завантажити");
    download_button.set_sensitive(!is_downloaded);

    let delete_button = Button::with_label("Видалити");
    delete_button.add_css_class("destructive-action");
    delete_button.set_sensitive(is_downloaded && !is_default);

    // Register this row's widgets for cross-row updates
    {
        let mut widgets = row_widgets.borrow_mut();
        widgets.insert(
            filename.to_string(),
            RowWidgets {
                indicator: default_indicator.clone(),
                set_default_button: set_default_button.clone(),
                delete_button: delete_button.clone(),
            },
        );
    }

    let filename_owned = filename.to_string();
    let config_clone = config.clone();
    let whisper_clone = whisper.clone();
    let row_widgets_clone = row_widgets.clone();

    set_default_button.connect_clicked(move |_| {
        let mut cfg = config_clone.lock().unwrap();
        cfg.default_model = filename_owned.clone();
        if let Err(e) = save_config(&cfg) {
            eprintln!("Помилка збереження конфігу: {}", e);
            return;
        }

        let model_path = crate::models::get_model_path(&filename_owned);
        match WhisperSTT::new(model_path.to_str().unwrap_or_default()) {
            Ok(new_whisper) => {
                let mut w = whisper_clone.lock().unwrap();
                *w = Some(new_whisper);
                println!("Модель завантажено: {}", filename_owned);
            }
            Err(e) => {
                eprintln!("Помилка завантаження моделі: {}", e);
            }
        }

        // Update all row indicators
        let widgets = row_widgets_clone.borrow();
        for (fname, rw) in widgets.iter() {
            if fname == &filename_owned {
                rw.indicator.set_text("[*]");
                rw.set_default_button.set_sensitive(false);
                rw.delete_button.set_sensitive(false);
            } else {
                rw.indicator.set_text("[ ]");
                // Only enable set_default if model is downloaded
                let is_downloaded = is_model_downloaded(fname);
                rw.set_default_button.set_sensitive(is_downloaded);
                rw.delete_button.set_sensitive(is_downloaded);
            }
        }
    });

    let filename_owned = filename.to_string();
    let download_button_clone = download_button.clone();
    let set_default_button_clone = set_default_button.clone();
    let delete_button_clone = delete_button.clone();
    let status_label_clone = status_label.clone();
    let progress_bar_clone = progress_bar.clone();
    let download_states_clone = download_states.clone();

    download_button.connect_clicked(move |_| {
        let filename = filename_owned.clone();
        let download_button = download_button_clone.clone();
        let set_default_button = set_default_button_clone.clone();
        let delete_button = delete_button_clone.clone();
        let status_label = status_label_clone.clone();
        let progress_bar = progress_bar_clone.clone();
        let download_states = download_states_clone.clone();

        {
            let mut states = download_states.borrow_mut();
            if matches!(states.get(&filename), Some(DownloadState::Downloading)) {
                return;
            }
            states.insert(filename.clone(), DownloadState::Downloading);
        }

        download_button.set_sensitive(false);
        progress_bar.set_visible(true);
        progress_bar.set_fraction(0.0);
        progress_bar.set_text(Some("Починаємо..."));
        status_label.set_text("Завантаження...");

        let (tx, rx) = async_channel::bounded::<DownloadProgress>(100);

        let filename_for_thread = filename.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let tx_clone = tx.clone();
            let result = rt.block_on(download_model(&filename_for_thread, move |downloaded, total| {
                let _ = tx_clone.send_blocking(DownloadProgress::Progress(downloaded, total));
            }));

            match result {
                Ok(()) => {
                    let _ = tx.send_blocking(DownloadProgress::Done);
                }
                Err(e) => {
                    let _ = tx.send_blocking(DownloadProgress::Error(e.to_string()));
                }
            }
        });

        let filename_for_ui = filename.clone();
        let download_states_for_ui = download_states.clone();
        glib::spawn_future_local(async move {
            while let Ok(progress) = rx.recv().await {
                match progress {
                    DownloadProgress::Progress(downloaded, total) => {
                        if total > 0 {
                            let fraction = downloaded as f64 / total as f64;
                            progress_bar.set_fraction(fraction);
                            progress_bar.set_text(Some(&format!(
                                "{} / {} ({:.0}%)",
                                format_size(downloaded),
                                format_size(total),
                                fraction * 100.0
                            )));
                        }
                    }
                    DownloadProgress::Done => {
                        progress_bar.set_visible(false);
                        status_label.set_text("Завантажено");
                        set_default_button.set_sensitive(true);
                        delete_button.set_sensitive(true);
                        download_states_for_ui
                            .borrow_mut()
                            .insert(filename_for_ui.clone(), DownloadState::Idle);
                        break;
                    }
                    DownloadProgress::Error(e) => {
                        progress_bar.set_visible(false);
                        status_label.set_text(&format!("Помилка: {}", e));
                        download_button.set_sensitive(true);
                        download_states_for_ui
                            .borrow_mut()
                            .insert(filename_for_ui.clone(), DownloadState::Idle);
                        break;
                    }
                }
            }
        });
    });

    let filename_owned = filename.to_string();
    let download_button_clone = download_button.clone();
    let set_default_button_clone = set_default_button.clone();
    let status_label_clone = status_label.clone();
    let config_clone = config.clone();

    delete_button.connect_clicked(move |btn| {
        let is_default = {
            let cfg = config_clone.lock().unwrap();
            cfg.default_model == filename_owned
        };

        if is_default {
            eprintln!("Не можна видалити модель за замовчуванням");
            return;
        }

        if let Err(e) = delete_model(&filename_owned) {
            eprintln!("Помилка видалення: {}", e);
            return;
        }

        status_label_clone.set_text("");
        download_button_clone.set_sensitive(true);
        set_default_button_clone.set_sensitive(false);
        btn.set_sensitive(false);
    });

    button_box.append(&set_default_button);
    button_box.append(&download_button);
    button_box.append(&delete_button);
    content_box.append(&button_box);

    row.set_child(Some(&content_box));
    row
}

enum DownloadProgress {
    Progress(u64, u64),
    Done,
    Error(String),
}
