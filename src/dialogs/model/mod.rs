//! Model management dialog.
//!
//! Provides UI for downloading, managing, and selecting Whisper speech
//! recognition models and Sortformer diarization models.

mod download;
mod list;

use crate::config::Config;
use crate::models::get_available_models;
use crate::services::TranscriptionService;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Label, ListBox, Orientation, ScrolledWindow, SelectionMode,
    Separator, Window,
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

/// Context for creating model rows, reducing parameter count
struct ModelRowContext {
    config: Arc<Mutex<Config>>,
    transcription: Arc<Mutex<TranscriptionService>>,
    download_states: Rc<RefCell<HashMap<String, DownloadState>>>,
    row_widgets: RowWidgetsMap,
}

#[derive(Clone)]
enum DownloadState {
    Idle,
    Downloading,
}

enum DownloadProgress {
    Progress(u64, u64),
    Done,
    Error(String),
}

pub fn show_model_dialog(
    parent: &impl IsA<Window>,
    config: Arc<Mutex<Config>>,
    transcription: Arc<Mutex<TranscriptionService>>,
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

    let model_ctx = ModelRowContext {
        config: config.clone(),
        transcription: transcription.clone(),
        download_states: download_states.clone(),
        row_widgets: row_widgets.clone(),
    };

    for model in get_available_models() {
        let row = list::create_model_row(
            &model.filename,
            &model.display_name,
            model.size_bytes,
            &model.description,
            &model_ctx,
        );
        list_box.append(&row);
    }

    scrolled.set_child(Some(&list_box));
    main_box.append(&scrolled);

    // Add separator and Sortformer section
    let separator = Separator::new(Orientation::Horizontal);
    separator.set_margin_top(12);
    separator.set_margin_bottom(6);
    separator.set_margin_start(12);
    separator.set_margin_end(12);
    main_box.append(&separator);

    let diarization_header = Label::new(Some("Розпізнавання мовців"));
    diarization_header.add_css_class("heading");
    diarization_header.set_halign(Align::Start);
    diarization_header.set_margin_start(12);
    diarization_header.set_margin_bottom(6);
    main_box.append(&diarization_header);

    // Sortformer model row
    let sortformer_row = download::create_sortformer_row();
    sortformer_row.set_margin_start(12);
    sortformer_row.set_margin_end(12);
    sortformer_row.set_margin_bottom(12);
    main_box.append(&sortformer_row);

    // TDT section separator
    let tdt_separator = Separator::new(Orientation::Horizontal);
    tdt_separator.set_margin_top(6);
    tdt_separator.set_margin_bottom(6);
    tdt_separator.set_margin_start(12);
    tdt_separator.set_margin_end(12);
    main_box.append(&tdt_separator);

    let tdt_header = Label::new(Some("Альтернативний STT Backend"));
    tdt_header.add_css_class("heading");
    tdt_header.set_halign(Align::Start);
    tdt_header.set_margin_start(12);
    tdt_header.set_margin_bottom(6);
    main_box.append(&tdt_header);

    // TDT model row
    let tdt_row = download::create_tdt_row();
    tdt_row.set_margin_start(12);
    tdt_row.set_margin_end(12);
    tdt_row.set_margin_bottom(12);
    main_box.append(&tdt_row);

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
