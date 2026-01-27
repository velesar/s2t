use crate::history::{save_history, History};
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GtkBox, Button, Entry, FileChooserNative, Label, ListBox, ListBoxRow,
    Orientation, ScrolledWindow, SelectionMode, Window,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn show_history_dialog(parent: &impl IsA<Window>, history: Arc<Mutex<History>>) {
    let dialog = Window::builder()
        .title("Історія диктовок")
        .modal(true)
        .transient_for(parent)
        .default_width(550)
        .default_height(500)
        .build();

    let main_box = GtkBox::new(Orientation::Vertical, 0);

    // Filter section
    let filter_box = GtkBox::new(Orientation::Vertical, 6);
    filter_box.set_margin_top(12);
    filter_box.set_margin_bottom(6);
    filter_box.set_margin_start(12);
    filter_box.set_margin_end(12);

    // Search entry
    let search_entry = Entry::builder()
        .placeholder_text("Пошук...")
        .build();
    filter_box.append(&search_entry);

    // Date filter row
    let date_filter_row = GtkBox::new(Orientation::Horizontal, 8);
    date_filter_row.set_margin_top(6);

    let date_from_label = Label::new(Some("Від:"));
    date_filter_row.append(&date_from_label);

    let date_from_entry = Entry::builder()
        .placeholder_text("YYYY-MM-DD")
        .tooltip_text("Дата початку (YYYY-MM-DD) або порожньо")
        .build();
    date_filter_row.append(&date_from_entry);

    let date_to_label = Label::new(Some("До:"));
    date_filter_row.append(&date_to_label);

    let date_to_entry = Entry::builder()
        .placeholder_text("YYYY-MM-DD")
        .tooltip_text("Дата кінця (YYYY-MM-DD) або порожньо")
        .build();
    date_filter_row.append(&date_to_entry);

    date_filter_row.set_hexpand(true);
    filter_box.append(&date_filter_row);

    main_box.append(&filter_box);

    // Scrolled window with list
    let scrolled = ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::None);
    list_box.add_css_class("boxed-list");
    list_box.set_margin_start(12);
    list_box.set_margin_end(12);

    // Placeholder for empty state
    let placeholder = Label::new(Some("Історія порожня"));
    placeholder.add_css_class("dim-label");
    list_box.set_placeholder(Some(&placeholder));

    // Shared state for filtering
    let search_query: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let date_from: Rc<RefCell<Option<DateTime<Utc>>>> = Rc::new(RefCell::new(None));
    let date_to: Rc<RefCell<Option<DateTime<Utc>>>> = Rc::new(RefCell::new(None));

    // Populate list
    populate_list(&list_box, history.clone(), &search_query, &date_from, &date_to);

    scrolled.set_child(Some(&list_box));
    main_box.append(&scrolled);

    // Search and date filtering
    let list_box_for_search = list_box.clone();
    let history_for_search = history.clone();
    let search_query_for_search = search_query.clone();
    let date_from_for_search = date_from.clone();
    let date_to_for_search = date_to.clone();
    search_entry.connect_changed({
        let list_box = list_box_for_search.clone();
        let history = history_for_search.clone();
        let search_query = search_query_for_search.clone();
        let date_from = date_from_for_search.clone();
        let date_to = date_to_for_search.clone();
        move |entry| {
            let query = entry.text().to_string();
            *search_query.borrow_mut() = query;
            populate_list(&list_box, history.clone(), &search_query, &date_from, &date_to);
        }
    });

    // Date filtering - from
    let list_box_for_from = list_box.clone();
    let history_for_from = history.clone();
    let search_query_for_from = search_query.clone();
    let date_from_for_from = date_from.clone();
    let date_to_for_from = date_to.clone();
    date_from_entry.connect_changed({
        let list_box = list_box_for_from.clone();
        let history = history_for_from.clone();
        let search_query = search_query_for_from.clone();
        let date_from = date_from_for_from.clone();
        let date_to = date_to_for_from.clone();
        move |entry| {
            let text = entry.text().to_string();
            *date_from.borrow_mut() = parse_date(&text);
            populate_list(&list_box, history.clone(), &search_query, &date_from, &date_to);
        }
    });

    // Date filtering - to
    let list_box_for_to = list_box.clone();
    let history_for_to = history.clone();
    let search_query_for_to = search_query.clone();
    let date_from_for_to = date_from.clone();
    let date_to_for_to = date_to.clone();
    date_to_entry.connect_changed({
        let list_box = list_box_for_to.clone();
        let history = history_for_to.clone();
        let search_query = search_query_for_to.clone();
        let date_from = date_from_for_to.clone();
        let date_to = date_to_for_to.clone();
        move |entry| {
            let text = entry.text().to_string();
            *date_to.borrow_mut() = parse_date(&text);
            populate_list(&list_box, history.clone(), &search_query, &date_from, &date_to);
        }
    });

    // Bottom button box
    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(Align::End);
    button_box.set_margin_top(12);
    button_box.set_margin_bottom(12);
    button_box.set_margin_start(12);
    button_box.set_margin_end(12);

    // Export button
    let export_button = Button::with_label("Експортувати...");
    let dialog_weak_for_export = dialog.downgrade();
    let history_for_export = history.clone();
    let search_query_for_export = search_query.clone();
    let date_from_for_export = date_from.clone();
    let date_to_for_export = date_to.clone();
    export_button.connect_clicked(move |_| {
        if let Some(dialog) = dialog_weak_for_export.upgrade() {
            export_history(
                &dialog,
                history_for_export.clone(),
                &search_query_for_export,
                &date_from_for_export,
                &date_to_for_export,
            );
        }
    });
    button_box.append(&export_button);

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

fn parse_date(date_str: &str) -> Option<DateTime<Utc>> {
    if date_str.trim().is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(date_str.trim(), "%Y-%m-%d")
        .ok()
        .and_then(|date| {
            date.and_hms_opt(0, 0, 0)
                .map(|dt| {
                    // Convert local naive datetime to UTC
                    // Use Local.from_local_datetime which returns LocalResult
                    match Local.from_local_datetime(&dt) {
                        chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
                        chrono::LocalResult::Ambiguous(dt, _) => Some(dt.with_timezone(&Utc)),
                        chrono::LocalResult::None => None,
                    }
                })
                .flatten()
        })
}

fn populate_list(
    list_box: &ListBox,
    history: Arc<Mutex<History>>,
    search_query: &Rc<RefCell<String>>,
    date_from: &Rc<RefCell<Option<DateTime<Utc>>>>,
    date_to: &Rc<RefCell<Option<DateTime<Utc>>>>,
) {
    // Remove all existing rows
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let history_guard = history.lock().unwrap();
    let query = search_query.borrow();
    let from_date = date_from.borrow();
    let to_date = date_to.borrow();

    // First filter by date
    let date_filtered: Vec<_> = history_guard
        .filter_by_date_range(*from_date, *to_date)
        .into_iter()
        .collect();

    // Then filter by search query
    let entries: Vec<_> = if query.is_empty() {
        date_filtered
    } else {
        date_filtered
            .into_iter()
            .filter(|e| e.text.to_lowercase().contains(&query.to_lowercase()))
            .collect()
    };

    for entry in entries {
        let row = create_history_row(
            &entry.id,
            &entry.text,
            &entry.formatted_timestamp(),
            &entry.formatted_duration(),
            &entry.preview(),
            history.clone(),
            list_box.clone(),
            search_query.clone(),
        );
        list_box.append(&row);
    }
}

fn export_history(
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

                    if let Err(e) = history_guard.export_to_text(&entries, &path) {
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

fn create_history_row(
    id: &str,
    full_text: &str,
    timestamp: &str,
    duration: &str,
    preview: &str,
    history: Arc<Mutex<History>>,
    list_box: ListBox,
    search_query: Rc<RefCell<String>>,
) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_activatable(false);

    let content_box = GtkBox::new(Orientation::Vertical, 6);
    content_box.set_margin_top(12);
    content_box.set_margin_bottom(12);
    content_box.set_margin_start(12);
    content_box.set_margin_end(12);

    // Top row: timestamp and duration
    let top_row = GtkBox::new(Orientation::Horizontal, 12);

    let timestamp_label = Label::new(Some(timestamp));
    timestamp_label.set_hexpand(true);
    timestamp_label.set_halign(Align::Start);
    timestamp_label.add_css_class("dim-label");
    top_row.append(&timestamp_label);

    let duration_label = Label::new(Some(&format!("[{}]", duration)));
    duration_label.add_css_class("monospace");
    duration_label.add_css_class("dim-label");
    top_row.append(&duration_label);

    content_box.append(&top_row);

    // Text preview
    let text_label = Label::new(Some(preview));
    text_label.set_halign(Align::Start);
    text_label.set_wrap(true);
    text_label.set_max_width_chars(60);
    content_box.append(&text_label);

    // Button row
    let button_box = GtkBox::new(Orientation::Horizontal, 8);
    button_box.set_halign(Align::End);
    button_box.set_margin_top(6);

    let copy_button = Button::with_label("Копіювати");
    let full_text_owned = full_text.to_string();
    copy_button.connect_clicked(move |_| {
        if let Some(display) = gtk4::gdk::Display::default() {
            let clipboard = display.clipboard();
            clipboard.set_text(&full_text_owned);
        }
    });

    let delete_button = Button::with_label("Видалити");
    delete_button.add_css_class("destructive-action");
    let id_owned = id.to_string();
    let history_for_delete = history.clone();
    let list_box_for_delete = list_box.clone();
    let search_query_for_delete = search_query.clone();
    let row_weak = row.downgrade();
    delete_button.connect_clicked(move |_| {
        {
            let mut h = history_for_delete.lock().unwrap();
            h.remove(&id_owned);
            if let Err(e) = save_history(&h) {
                eprintln!("Помилка збереження історії: {}", e);
            }
        }
        // Remove this row from the list
        if let Some(row) = row_weak.upgrade() {
            list_box_for_delete.remove(&row);
        }
        // Refresh to update placeholder visibility
        glib::idle_add_local_once({
            let list_box = list_box_for_delete.clone();
            let history = history_for_delete.clone();
            let search_query = search_query_for_delete.clone();
            // We need to get date filters from somewhere - for now, use None
            let date_from: Rc<RefCell<Option<DateTime<Utc>>>> = Rc::new(RefCell::new(None));
            let date_to: Rc<RefCell<Option<DateTime<Utc>>>> = Rc::new(RefCell::new(None));
            move || {
                populate_list(&list_box, history, &search_query, &date_from, &date_to);
            }
        });
    });

    button_box.append(&copy_button);
    button_box.append(&delete_button);
    content_box.append(&button_box);

    row.set_child(Some(&content_box));
    row
}
