//! History browser dialog.
//!
//! Provides UI for viewing, searching, filtering, and exporting
//! transcription history.

mod export;
mod list;

use crate::domain::types::SharedHistory;
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, Entry, Label, ListBox, Orientation, ScrolledWindow,
    SelectionMode, Window,
};
use std::cell::RefCell;
use std::rc::Rc;

pub fn show_history_dialog(parent: &impl IsA<Window>, history: SharedHistory) {
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
    let search_entry = Entry::builder().placeholder_text("Пошук...").build();
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
    list::populate_list(
        &list_box,
        history.clone(),
        &search_query,
        &date_from,
        &date_to,
    );

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
            list::populate_list(
                &list_box,
                history.clone(),
                &search_query,
                &date_from,
                &date_to,
            );
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
            list::populate_list(
                &list_box,
                history.clone(),
                &search_query,
                &date_from,
                &date_to,
            );
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
            list::populate_list(
                &list_box,
                history.clone(),
                &search_query,
                &date_from,
                &date_to,
            );
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
            export::export_history(
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
            date.and_hms_opt(0, 0, 0).and_then(|dt| {
                // Convert local naive datetime to UTC
                // Use Local.from_local_datetime which returns LocalResult
                match Local.from_local_datetime(&dt) {
                    chrono::LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
                    chrono::LocalResult::Ambiguous(dt, _) => Some(dt.with_timezone(&Utc)),
                    chrono::LocalResult::None => None,
                }
            })
        })
}
