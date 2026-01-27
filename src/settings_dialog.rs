use crate::config::{save_config, Config};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, ComboBoxText, Label, Orientation, SpinButton,
    Window,
};
use std::sync::{Arc, Mutex};

pub fn show_settings_dialog(parent: &impl IsA<Window>, config: Arc<Mutex<Config>>) {
    let dialog = Window::builder()
        .title("Налаштування")
        .modal(true)
        .transient_for(parent)
        .default_width(400)
        .default_height(450)
        .build();

    let main_box = GtkBox::new(Orientation::Vertical, 12);
    main_box.set_margin_top(20);
    main_box.set_margin_bottom(20);
    main_box.set_margin_start(20);
    main_box.set_margin_end(20);

    // Language selection
    let language_label = Label::new(Some("Мова розпізнавання:"));
    language_label.set_halign(Align::Start);
    main_box.append(&language_label);

    // Language codes and display names
    let languages: Vec<(&str, &str)> = vec![
        ("uk", "Українська"),
        ("en", "English"),
        ("ru", "Русский"),
        ("de", "Deutsch"),
        ("fr", "Français"),
        ("es", "Español"),
        ("pl", "Polski"),
        ("it", "Italiano"),
        ("pt", "Português"),
        ("ja", "日本語"),
        ("zh", "中文"),
        ("ko", "한국어"),
    ];

    let language_combo = ComboBoxText::with_entry();
    // Add languages to combo (format: "code - Name")
    for (code, name) in &languages {
        language_combo.append_text(&format!("{} - {}", code, name));
    }
    language_combo.set_halign(Align::Start);

    // Load current language
    let current_language = {
        let cfg = config.lock().unwrap();
        cfg.language.clone()
    };

    // Try to find and select current language in combo
    let mut found = false;
    for (i, (code, _)) in languages.iter().enumerate() {
        if *code == current_language {
            language_combo.set_active(Some(i as u32));
            found = true;
            break;
        }
    }

    // If not found, set as entry text (for custom language codes)
    if !found {
        language_combo.set_active(None);
        if let Some(entry) = language_combo.child() {
            if let Some(entry_text) = entry.downcast_ref::<gtk4::Entry>() {
                entry_text.set_text(&current_language);
            }
        }
    }

    main_box.append(&language_combo);

    // Auto-copy checkbox
    let auto_copy_check = CheckButton::with_label("Автоматично копіювати результат");
    let auto_copy_enabled = {
        let cfg = config.lock().unwrap();
        cfg.auto_copy
    };
    auto_copy_check.set_active(auto_copy_enabled);
    auto_copy_check.set_halign(Align::Start);
    main_box.append(&auto_copy_check);

    // Hotkey settings section
    let hotkey_label = Label::new(Some("Гарячі клавіші:"));
    hotkey_label.set_halign(Align::Start);
    hotkey_label.set_margin_top(12);
    main_box.append(&hotkey_label);

    // Hotkey enabled checkbox
    let hotkey_enabled_check = CheckButton::with_label("Увімкнути глобальні гарячі клавіші");
    let hotkey_enabled = {
        let cfg = config.lock().unwrap();
        cfg.hotkey_enabled
    };
    hotkey_enabled_check.set_active(hotkey_enabled);
    hotkey_enabled_check.set_halign(Align::Start);
    main_box.append(&hotkey_enabled_check);

    // Hotkey entry
    let hotkey_entry_label = Label::new(Some("Комбінація клавіш (напр. Control+Shift+Space):"));
    hotkey_entry_label.set_halign(Align::Start);
    hotkey_entry_label.set_margin_top(6);
    main_box.append(&hotkey_entry_label);

    let hotkey_entry = gtk4::Entry::new();
    let current_hotkey = {
        let cfg = config.lock().unwrap();
        cfg.hotkey.clone()
    };
    hotkey_entry.set_text(&current_hotkey);
    hotkey_entry.set_halign(Align::Start);
    hotkey_entry.set_sensitive(hotkey_enabled);
    main_box.append(&hotkey_entry);

    // Enable/disable hotkey entry based on checkbox
    let hotkey_entry_clone = hotkey_entry.clone();
    hotkey_enabled_check.connect_toggled(move |check| {
        hotkey_entry_clone.set_sensitive(check.is_active());
    });

    // History settings section
    let history_label = Label::new(Some("Налаштування історії:"));
    history_label.set_halign(Align::Start);
    history_label.set_margin_top(12);
    main_box.append(&history_label);

    // Max entries
    let max_entries_label = Label::new(Some("Максимум записів:"));
    max_entries_label.set_halign(Align::Start);
    main_box.append(&max_entries_label);

    let max_entries_spin = SpinButton::new(
        Some(&gtk4::Adjustment::new(500.0, 10.0, 10000.0, 1.0, 10.0, 0.0)),
        1.0,
        0,
    );
    let current_max_entries = {
        let cfg = config.lock().unwrap();
        cfg.history_max_entries as f64
    };
    max_entries_spin.set_value(current_max_entries);
    max_entries_spin.set_halign(Align::Start);
    main_box.append(&max_entries_spin);

    // Max age days
    let max_age_label = Label::new(Some("Максимальний вік (дні):"));
    max_age_label.set_halign(Align::Start);
    max_age_label.set_margin_top(6);
    main_box.append(&max_age_label);

    let max_age_spin = SpinButton::new(
        Some(&gtk4::Adjustment::new(90.0, 1.0, 3650.0, 1.0, 10.0, 0.0)),
        1.0,
        0,
    );
    let current_max_age = {
        let cfg = config.lock().unwrap();
        cfg.history_max_age_days as f64
    };
    max_age_spin.set_value(current_max_age);
    max_age_spin.set_halign(Align::Start);
    main_box.append(&max_age_spin);

    // Button box
    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(Align::End);
    button_box.set_margin_top(20);

    let cancel_button = Button::with_label("Скасувати");
    let dialog_weak = dialog.downgrade();
    cancel_button.connect_clicked(move |_| {
        if let Some(dialog) = dialog_weak.upgrade() {
            dialog.close();
        }
    });
    button_box.append(&cancel_button);

    let save_button = Button::with_label("Зберегти");
    save_button.add_css_class("suggested-action");
    let dialog_weak = dialog.downgrade();
    let config_clone = config.clone();
    let language_combo_clone = language_combo.clone();
    let languages_clone = languages.clone();
    let auto_copy_check_clone = auto_copy_check.clone();
    let hotkey_enabled_check_clone = hotkey_enabled_check.clone();
    let hotkey_entry_clone = hotkey_entry.clone();
    let max_entries_spin_clone = max_entries_spin.clone();
    let max_age_spin_clone = max_age_spin.clone();

    save_button.connect_clicked(move |_| {
        // Get language from combo
        let language = if let Some(active) = language_combo_clone.active() {
            // Get language code from the languages list
            if let Some((code, _)) = languages_clone.get(active as usize) {
                (*code).to_string()
            } else {
                "uk".to_string()
            }
        } else if let Some(entry) = language_combo_clone.child() {
            if let Some(entry_text) = entry.downcast_ref::<gtk4::Entry>() {
                let text = entry_text.text().to_string();
                // Extract language code if in "code - Name" format
                if let Some(code) = text.split(" - ").next() {
                    code.to_string()
                } else {
                    text
                }
            } else {
                "uk".to_string()
            }
        } else {
            "uk".to_string()
        };

        // Get hotkey
        let hotkey = hotkey_entry_clone.text().to_string();

        // Update config
        let mut cfg = config_clone.lock().unwrap();
        cfg.language = language;
        cfg.auto_copy = auto_copy_check_clone.is_active();
        cfg.hotkey_enabled = hotkey_enabled_check_clone.is_active();
        cfg.hotkey = hotkey;
        cfg.history_max_entries = max_entries_spin_clone.value() as usize;
        cfg.history_max_age_days = max_age_spin_clone.value() as i64;

        // Save config
        if let Err(e) = save_config(&cfg) {
            eprintln!("Помилка збереження конфігу: {}", e);
        } else {
            // Close dialog on success
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.close();
            }
        }
    });
    button_box.append(&save_button);

    main_box.append(&button_box);
    dialog.set_child(Some(&main_box));

    dialog.present();
}
