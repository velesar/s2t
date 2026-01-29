//! History list population and row creation.

use crate::types::SharedHistory;
use chrono::{DateTime, Utc};
use gtk4::prelude::*;
use gtk4::{glib, Align, Box as GtkBox, Button, Label, ListBox, ListBoxRow, Orientation};
use std::cell::RefCell;
use std::rc::Rc;

/// Context for creating history rows, reducing parameter count
struct HistoryRowContext {
    history: SharedHistory,
    list_box: ListBox,
    search_query: Rc<RefCell<String>>,
}

pub fn populate_list(
    list_box: &ListBox,
    history: SharedHistory,
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

    let row_ctx = HistoryRowContext {
        history: history.clone(),
        list_box: list_box.clone(),
        search_query: search_query.clone(),
    };

    for entry in entries {
        let row = create_history_row(
            &entry.id,
            &entry.text,
            &entry.formatted_timestamp(),
            &entry.formatted_duration(),
            &entry.preview(),
            &row_ctx,
        );
        list_box.append(&row);
    }
}

fn create_history_row(
    id: &str,
    full_text: &str,
    timestamp: &str,
    duration: &str,
    preview: &str,
    ctx: &HistoryRowContext,
) -> ListBoxRow {
    let history = ctx.history.clone();
    let list_box = ctx.list_box.clone();
    let search_query = ctx.search_query.clone();
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
            if let Err(e) = h.save() {
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
