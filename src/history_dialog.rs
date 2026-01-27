use crate::history::{save_history, History};
use gtk4::prelude::*;
use gtk4::{
    glib, Align, Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation,
    ScrolledWindow, SelectionMode, Window,
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

    // Search entry
    let search_entry = Entry::builder()
        .placeholder_text("Пошук...")
        .margin_top(12)
        .margin_bottom(12)
        .margin_start(12)
        .margin_end(12)
        .build();

    main_box.append(&search_entry);

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

    // Shared state for search filtering
    let search_query: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    // Populate list
    populate_list(&list_box, history.clone(), &search_query);

    scrolled.set_child(Some(&list_box));
    main_box.append(&scrolled);

    // Search filtering
    let list_box_for_search = list_box.clone();
    let history_for_search = history.clone();
    let search_query_for_search = search_query.clone();
    search_entry.connect_changed(move |entry| {
        let query = entry.text().to_string();
        *search_query_for_search.borrow_mut() = query;
        populate_list(&list_box_for_search, history_for_search.clone(), &search_query_for_search);
    });

    // Bottom button box
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

fn populate_list(list_box: &ListBox, history: Arc<Mutex<History>>, search_query: &Rc<RefCell<String>>) {
    // Remove all existing rows
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let history_guard = history.lock().unwrap();
    let query = search_query.borrow();

    let entries: Vec<_> = if query.is_empty() {
        history_guard.entries.iter().collect()
    } else {
        history_guard.search(&query)
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
            move || {
                populate_list(&list_box, history, &search_query);
            }
        });
    });

    button_box.append(&copy_button);
    button_box.append(&delete_button);
    content_box.append(&button_box);

    row.set_child(Some(&content_box));
    row
}
