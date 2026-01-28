//! History export functionality.

use crate::history::History;
use chrono::{DateTime, Utc};
use gtk4::prelude::*;
use gtk4::{FileChooserNative, Window};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn export_history(
    parent: &Window,
    history: Arc<Mutex<History>>,
    search_query: &Rc<RefCell<String>>,
    date_from: &Rc<RefCell<Option<DateTime<Utc>>>>,
    date_to: &Rc<RefCell<Option<DateTime<Utc>>>>,
) {
    let dialog = FileChooserNative::builder()
        .title("Експортувати історію")
        .action(gtk4::FileChooserAction::Save)
        .modal(true)
        .transient_for(parent)
        .build();

    // Set default filename
    let default_name = format!(
        "voice-dictation-history-{}.txt",
        chrono::Local::now().format("%Y-%m-%d")
    );
    dialog.set_current_name(&default_name);

    let history_for_export = history.clone();
    let search_query_for_export = search_query.clone();
    let date_from_for_export = date_from.clone();
    let date_to_for_export = date_to.clone();

    dialog.connect_response(move |dialog, response| {
        if response == gtk4::ResponseType::Accept {
            if let Some(file) = dialog.file() {
                if let Some(path) = file.path() {
                    let history_guard = history_for_export.lock().unwrap();
                    let query = search_query_for_export.borrow();
                    let from_date = date_from_for_export.borrow();
                    let to_date = date_to_for_export.borrow();

                    // Get filtered entries
                    let date_filtered: Vec<_> = history_guard
                        .filter_by_date_range(*from_date, *to_date)
                        .into_iter()
                        .collect();

                    let entries: Vec<_> = if query.is_empty() {
                        date_filtered
                    } else {
                        date_filtered
                            .into_iter()
                            .filter(|e| e.text.to_lowercase().contains(&query.to_lowercase()))
                            .collect()
                    };

                    if let Err(e) = crate::history::export_to_text(&entries, &path) {
                        eprintln!("Помилка експорту: {}", e);
                        // TODO: Show error dialog
                    }
                }
            }
        }
        dialog.destroy();
    });

    dialog.show();
}
