//! Sortformer diarization and TDT STT model download and management.

use super::DownloadProgress;
use crate::infrastructure::models::{
    delete_sortformer_model, delete_tdt_model, download_sortformer_model, download_tdt_model, format_size,
    get_sortformer_model_info, get_tdt_model_info, get_tdt_total_size, is_sortformer_model_downloaded,
    is_tdt_model_downloaded,
};
use gtk4::prelude::*;
use gtk4::{glib, Align, Box as GtkBox, Button, Label, Orientation, ProgressBar};

pub fn create_sortformer_row() -> GtkBox {
    let model_info = get_sortformer_model_info();
    let is_downloaded = is_sortformer_model_downloaded();

    let content_box = GtkBox::new(Orientation::Vertical, 6);
    content_box.add_css_class("card");
    content_box.set_margin_top(6);
    content_box.set_margin_bottom(6);

    let top_row = GtkBox::new(Orientation::Horizontal, 12);

    let name_label = Label::new(Some(&format!(
        "{} ({})",
        model_info.display_name,
        format_size(model_info.size_bytes)
    )));
    name_label.set_hexpand(true);
    name_label.set_halign(Align::Start);
    name_label.add_css_class("heading");
    top_row.append(&name_label);

    let status_label = Label::new(Some(if is_downloaded { "Завантажено" } else { "" }));
    status_label.add_css_class("dim-label");
    top_row.append(&status_label);

    content_box.append(&top_row);

    let desc_label = Label::new(Some(&model_info.description));
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

    let download_button = Button::with_label("Завантажити");
    download_button.set_sensitive(!is_downloaded);

    let delete_button = Button::with_label("Видалити");
    delete_button.add_css_class("destructive-action");
    delete_button.set_sensitive(is_downloaded);

    // Download button handler
    let download_button_clone = download_button.clone();
    let delete_button_clone = delete_button.clone();
    let status_label_clone = status_label.clone();
    let progress_bar_clone = progress_bar.clone();

    download_button.connect_clicked(move |_| {
        let download_button = download_button_clone.clone();
        let delete_button = delete_button_clone.clone();
        let status_label = status_label_clone.clone();
        let progress_bar = progress_bar_clone.clone();

        download_button.set_sensitive(false);
        progress_bar.set_visible(true);
        progress_bar.set_fraction(0.0);
        progress_bar.set_text(Some("Починаємо..."));
        status_label.set_text("Завантаження...");

        let (tx, rx) = async_channel::bounded::<DownloadProgress>(100);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let tx_clone = tx.clone();
            let result = rt.block_on(download_sortformer_model(move |downloaded, total| {
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
                        delete_button.set_sensitive(true);
                        break;
                    }
                    DownloadProgress::Error(e) => {
                        progress_bar.set_visible(false);
                        status_label.set_text(&format!("Помилка: {}", e));
                        download_button.set_sensitive(true);
                        break;
                    }
                }
            }
        });
    });

    // Delete button handler
    let download_button_clone = download_button.clone();
    let status_label_clone = status_label.clone();

    delete_button.connect_clicked(move |btn| {
        if let Err(e) = delete_sortformer_model() {
            eprintln!("Помилка видалення Sortformer: {}", e);
            return;
        }

        status_label_clone.set_text("");
        download_button_clone.set_sensitive(true);
        btn.set_sensitive(false);
    });

    button_box.append(&download_button);
    button_box.append(&delete_button);
    content_box.append(&button_box);

    content_box
}

/// Create a row for Parakeet TDT v3 model download/management.
pub fn create_tdt_row() -> GtkBox {
    let total_size = get_tdt_total_size();
    let is_downloaded = is_tdt_model_downloaded();
    let _model_info = get_tdt_model_info();

    let content_box = GtkBox::new(Orientation::Vertical, 6);
    content_box.add_css_class("card");
    content_box.set_margin_top(6);
    content_box.set_margin_bottom(6);

    let top_row = GtkBox::new(Orientation::Horizontal, 12);

    let name_label = Label::new(Some(&format!("Parakeet TDT v3 (25 мов) ({})", format_size(total_size))));
    name_label.set_hexpand(true);
    name_label.set_halign(Align::Start);
    name_label.add_css_class("heading");
    top_row.append(&name_label);

    let status_label = Label::new(Some(if is_downloaded { "Завантажено" } else { "" }));
    status_label.add_css_class("dim-label");
    top_row.append(&status_label);

    content_box.append(&top_row);

    let desc_label = Label::new(Some("NVIDIA TDT для 25 мов (WER 6.79% uk). Включає пунктуацію."));
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

    let download_button = Button::with_label("Завантажити");
    download_button.set_sensitive(!is_downloaded);

    let delete_button = Button::with_label("Видалити");
    delete_button.add_css_class("destructive-action");
    delete_button.set_sensitive(is_downloaded);

    // Download button handler
    let download_button_clone = download_button.clone();
    let delete_button_clone = delete_button.clone();
    let status_label_clone = status_label.clone();
    let progress_bar_clone = progress_bar.clone();

    download_button.connect_clicked(move |_| {
        let download_button = download_button_clone.clone();
        let delete_button = delete_button_clone.clone();
        let status_label = status_label_clone.clone();
        let progress_bar = progress_bar_clone.clone();

        download_button.set_sensitive(false);
        progress_bar.set_visible(true);
        progress_bar.set_fraction(0.0);
        progress_bar.set_text(Some("Починаємо..."));
        status_label.set_text("Завантаження...");

        let (tx, rx) = async_channel::bounded::<DownloadProgress>(100);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let tx_clone = tx.clone();
            let result = rt.block_on(download_tdt_model(move |downloaded, total| {
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
                        delete_button.set_sensitive(true);
                        break;
                    }
                    DownloadProgress::Error(e) => {
                        progress_bar.set_visible(false);
                        status_label.set_text(&format!("Помилка: {}", e));
                        download_button.set_sensitive(true);
                        break;
                    }
                }
            }
        });
    });

    // Delete button handler
    let download_button_clone = download_button.clone();
    let status_label_clone = status_label.clone();

    delete_button.connect_clicked(move |btn| {
        if let Err(e) = delete_tdt_model() {
            eprintln!("Помилка видалення TDT: {}", e);
            return;
        }

        status_label_clone.set_text("");
        download_button_clone.set_sensitive(true);
        btn.set_sensitive(false);
    });

    button_box.append(&download_button);
    button_box.append(&delete_button);
    content_box.append(&button_box);

    content_box
}
