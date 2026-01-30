use crate::app::config::{save_config, Config};
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, ComboBoxText, Label, Orientation, SpinButton,
    Window,
};
use parking_lot::Mutex;
use std::sync::Arc;

/// Supported languages for the speech recognition dropdown.
const LANGUAGES: &[(&str, &str)] = &[
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

/// All settings widgets whose values are read on save.
struct SettingsWidgets {
    language_combo: ComboBoxText,
    backend_combo: ComboBoxText,
    mode_combo: ComboBoxText,
    diarization_combo: ComboBoxText,
    auto_copy_check: CheckButton,
    auto_paste_check: CheckButton,
    continuous_check: CheckButton,
    vad_check: CheckButton,
    denoise_check: CheckButton,
    hotkey_enabled_check: CheckButton,
    hotkey_entry: gtk4::Entry,
    max_entries_spin: SpinButton,
    max_age_spin: SpinButton,
}

impl SettingsWidgets {
    /// Read all widget values and write them into the config.
    fn apply_to_config(&self, cfg: &mut Config) {
        cfg.language = self.read_language();
        cfg.stt_backend = combo_to_value(&self.backend_combo, &[("whisper", 0), ("tdt", 1)]);
        cfg.recording_mode =
            combo_to_value(&self.mode_combo, &[("dictation", 0), ("conference", 1)]);
        cfg.diarization_method = combo_to_value(
            &self.diarization_combo,
            &[("channel", 0), ("sortformer", 1)],
        );
        cfg.auto_copy = self.auto_copy_check.is_active();
        cfg.auto_paste = self.auto_paste_check.is_active();
        cfg.continuous_mode = self.continuous_check.is_active();
        cfg.use_vad = self.vad_check.is_active();
        cfg.denoise_enabled = self.denoise_check.is_active();
        cfg.hotkey_enabled = self.hotkey_enabled_check.is_active();
        cfg.hotkey = self.hotkey_entry.text().to_string();
        cfg.history_max_entries = self.max_entries_spin.value() as usize;
        cfg.history_max_age_days = self.max_age_spin.value() as i64;
    }

    fn read_language(&self) -> String {
        if let Some(active) = self.language_combo.active() {
            if let Some((code, _)) = LANGUAGES.get(active as usize) {
                return (*code).to_string();
            }
        }
        if let Some(entry) = self.language_combo.child() {
            if let Some(entry_text) = entry.downcast_ref::<gtk4::Entry>() {
                let text = entry_text.text().to_string();
                if let Some(code) = text.split(" - ").next() {
                    return code.to_string();
                }
                return text;
            }
        }
        "uk".to_string()
    }
}

/// Widgets returned by `build_recording_section`.
struct RecordingWidgets {
    mode_combo: ComboBoxText,
    diarization_combo: ComboBoxText,
    auto_copy_check: CheckButton,
    auto_paste_check: CheckButton,
    continuous_check: CheckButton,
    vad_check: CheckButton,
    denoise_check: CheckButton,
}

/// Map a ComboBoxText active index to one of the known string values.
fn combo_to_value(combo: &ComboBoxText, mapping: &[(&str, u32)]) -> String {
    let active = combo.active();
    for (value, index) in mapping {
        if active == Some(*index) {
            return (*value).to_string();
        }
    }
    mapping[0].0.to_string()
}

// ── Section builders ──────────────────────────────────────────────────

fn build_language_section(parent: &GtkBox, cfg: &Config) -> ComboBoxText {
    let label = Label::new(Some("Мова розпізнавання:"));
    label.set_halign(Align::Start);
    parent.append(&label);

    let combo = ComboBoxText::with_entry();
    for (code, name) in LANGUAGES {
        combo.append_text(&format!("{} - {}", code, name));
    }
    combo.set_halign(Align::Start);

    let mut found = false;
    for (i, (code, _)) in LANGUAGES.iter().enumerate() {
        if *code == cfg.language {
            combo.set_active(Some(i as u32));
            found = true;
            break;
        }
    }
    if !found {
        combo.set_active(None);
        if let Some(entry) = combo.child() {
            if let Some(entry_text) = entry.downcast_ref::<gtk4::Entry>() {
                entry_text.set_text(&cfg.language);
            }
        }
    }

    parent.append(&combo);
    combo
}

fn build_backend_section(parent: &GtkBox, cfg: &Config) -> ComboBoxText {
    let label = Label::new(Some("STT Backend:"));
    label.set_halign(Align::Start);
    label.set_margin_top(12);
    parent.append(&label);

    let combo = ComboBoxText::new();
    combo.append_text("Whisper");
    combo.append_text("Parakeet TDT v3");
    if cfg.stt_backend == "tdt" {
        combo.set_active(Some(1));
    } else {
        combo.set_active(Some(0));
    }
    combo.set_halign(Align::Start);

    if !crate::infrastructure::models::is_tdt_model_downloaded() {
        let info = Label::new(Some(
            "(Завантажте модель TDT через меню 'Моделі' для активації)",
        ));
        info.add_css_class("dim-label");
        info.set_halign(Align::Start);
        parent.append(&info);
    }

    parent.append(&combo);
    combo
}

fn build_recording_section(parent: &GtkBox, cfg: &Config) -> RecordingWidgets {
    // Recording mode
    let mode_label = Label::new(Some("Режим запису:"));
    mode_label.set_halign(Align::Start);
    mode_label.set_margin_top(12);
    parent.append(&mode_label);

    let mode_combo = ComboBoxText::new();
    mode_combo.append_text("Диктовка");
    mode_combo.append_text("Конференція");
    if cfg.recording_mode == "conference" {
        mode_combo.set_active(Some(1));
    } else {
        mode_combo.set_active(Some(0));
    }
    mode_combo.set_halign(Align::Start);
    parent.append(&mode_combo);

    // Diarization method
    let diarization_label = Label::new(Some("Метод розпізнавання мовців:"));
    diarization_label.set_halign(Align::Start);
    diarization_label.set_margin_top(12);
    parent.append(&diarization_label);

    let diarization_combo = ComboBoxText::new();
    diarization_combo.append_text("За каналами (2 мовці)");
    diarization_combo.append_text("Sortformer (до 4 мовців)");
    if cfg.diarization_method == "sortformer" {
        diarization_combo.set_active(Some(1));
    } else {
        diarization_combo.set_active(Some(0));
    }
    diarization_combo.set_halign(Align::Start);

    if !crate::infrastructure::models::is_sortformer_model_downloaded() {
        diarization_combo.set_sensitive(false);
        let info = Label::new(Some(
            "(Завантажте модель Sortformer через меню 'Моделі')",
        ));
        info.add_css_class("dim-label");
        info.set_halign(Align::Start);
        parent.append(&info);
    }
    parent.append(&diarization_combo);

    // Auto-copy, auto-paste
    let auto_copy_check = CheckButton::with_label("Автоматично копіювати результат");
    auto_copy_check.set_active(cfg.auto_copy);
    auto_copy_check.set_halign(Align::Start);
    parent.append(&auto_copy_check);

    let auto_paste_check = CheckButton::with_label("Автоматично вставити в активне вікно");
    auto_paste_check.set_active(cfg.auto_paste);
    auto_paste_check.set_halign(Align::Start);
    parent.append(&auto_paste_check);

    // Continuous mode + VAD
    let continuous_check = CheckButton::with_label("Неперервний режим (автоматична сегментація)");
    continuous_check.set_active(cfg.continuous_mode);
    continuous_check.set_halign(Align::Start);
    continuous_check.set_margin_top(12);
    parent.append(&continuous_check);

    let vad_check = CheckButton::with_label("VAD (автовиявлення пауз)");
    vad_check.set_active(cfg.use_vad);
    vad_check.set_sensitive(cfg.continuous_mode);
    vad_check.set_halign(Align::Start);
    vad_check.set_margin_start(20);
    parent.append(&vad_check);

    let vad_check_clone = vad_check.clone();
    continuous_check.connect_toggled(move |check| {
        vad_check_clone.set_sensitive(check.is_active());
    });

    // Denoise
    let denoise_check = CheckButton::with_label("Шумоподавлення (RNNoise)");
    denoise_check.set_active(cfg.denoise_enabled);
    denoise_check.set_halign(Align::Start);
    denoise_check.set_margin_top(12);
    parent.append(&denoise_check);

    RecordingWidgets {
        mode_combo,
        diarization_combo,
        auto_copy_check,
        auto_paste_check,
        continuous_check,
        vad_check,
        denoise_check,
    }
}

fn build_hotkey_section(parent: &GtkBox, cfg: &Config) -> (CheckButton, gtk4::Entry) {
    let label = Label::new(Some("Гарячі клавіші:"));
    label.set_halign(Align::Start);
    label.set_margin_top(12);
    parent.append(&label);

    let enabled_check = CheckButton::with_label("Увімкнути глобальні гарячі клавіші");
    enabled_check.set_active(cfg.hotkey_enabled);
    enabled_check.set_halign(Align::Start);
    parent.append(&enabled_check);

    let entry_label = Label::new(Some("Комбінація клавіш (напр. Control+Shift+Space):"));
    entry_label.set_halign(Align::Start);
    entry_label.set_margin_top(6);
    parent.append(&entry_label);

    let entry = gtk4::Entry::new();
    entry.set_text(&cfg.hotkey);
    entry.set_halign(Align::Start);
    entry.set_sensitive(cfg.hotkey_enabled);
    parent.append(&entry);

    let entry_clone = entry.clone();
    enabled_check.connect_toggled(move |check| {
        entry_clone.set_sensitive(check.is_active());
    });

    (enabled_check, entry)
}

fn build_history_section(parent: &GtkBox, cfg: &Config) -> (SpinButton, SpinButton) {
    let label = Label::new(Some("Налаштування історії:"));
    label.set_halign(Align::Start);
    label.set_margin_top(12);
    parent.append(&label);

    let max_entries_label = Label::new(Some("Максимум записів:"));
    max_entries_label.set_halign(Align::Start);
    parent.append(&max_entries_label);

    let max_entries_spin = SpinButton::new(
        Some(&gtk4::Adjustment::new(
            500.0, 10.0, 10000.0, 1.0, 10.0, 0.0,
        )),
        1.0,
        0,
    );
    max_entries_spin.set_value(cfg.history_max_entries as f64);
    max_entries_spin.set_halign(Align::Start);
    parent.append(&max_entries_spin);

    let max_age_label = Label::new(Some("Максимальний вік (дні):"));
    max_age_label.set_halign(Align::Start);
    max_age_label.set_margin_top(6);
    parent.append(&max_age_label);

    let max_age_spin = SpinButton::new(
        Some(&gtk4::Adjustment::new(
            90.0, 1.0, 3650.0, 1.0, 10.0, 0.0,
        )),
        1.0,
        0,
    );
    max_age_spin.set_value(cfg.history_max_age_days as f64);
    max_age_spin.set_halign(Align::Start);
    parent.append(&max_age_spin);

    (max_entries_spin, max_age_spin)
}

// ── Main dialog ───────────────────────────────────────────────────────

pub fn show_settings_dialog(
    parent: &impl IsA<Window>,
    config: Arc<Mutex<Config>>,
    reload_hotkeys_tx: async_channel::Sender<()>,
) {
    let dialog = Window::builder()
        .title("Налаштування")
        .modal(true)
        .transient_for(parent)
        .default_width(400)
        .default_height(520)
        .build();

    let main_box = GtkBox::new(Orientation::Vertical, 12);
    main_box.set_margin_top(20);
    main_box.set_margin_bottom(20);
    main_box.set_margin_start(20);
    main_box.set_margin_end(20);

    // Snapshot config once for all section builders
    let cfg = config.lock().clone();

    let language_combo = build_language_section(&main_box, &cfg);
    let backend_combo = build_backend_section(&main_box, &cfg);
    let recording = build_recording_section(&main_box, &cfg);
    let (hotkey_enabled_check, hotkey_entry) = build_hotkey_section(&main_box, &cfg);
    let (max_entries_spin, max_age_spin) = build_history_section(&main_box, &cfg);

    // Buttons
    let button_box = GtkBox::new(Orientation::Horizontal, 12);
    button_box.set_halign(Align::End);
    button_box.set_margin_top(20);

    let cancel_button = Button::with_label("Скасувати");
    let dialog_weak = dialog.downgrade();
    cancel_button.connect_clicked(move |_| {
        if let Some(d) = dialog_weak.upgrade() {
            d.close();
        }
    });
    button_box.append(&cancel_button);

    let save_button = Button::with_label("Зберегти");
    save_button.add_css_class("suggested-action");

    let widgets = SettingsWidgets {
        language_combo,
        backend_combo,
        mode_combo: recording.mode_combo,
        diarization_combo: recording.diarization_combo,
        auto_copy_check: recording.auto_copy_check,
        auto_paste_check: recording.auto_paste_check,
        continuous_check: recording.continuous_check,
        vad_check: recording.vad_check,
        denoise_check: recording.denoise_check,
        hotkey_enabled_check,
        hotkey_entry,
        max_entries_spin,
        max_age_spin,
    };

    let dialog_weak = dialog.downgrade();
    save_button.connect_clicked(move |_| {
        let mut cfg = config.lock();
        widgets.apply_to_config(&mut cfg);

        if let Err(e) = save_config(&cfg) {
            eprintln!("Помилка збереження конфігу: {}", e);
        } else {
            let _ = reload_hotkeys_tx.try_send(());
            if let Some(d) = dialog_weak.upgrade() {
                d.close();
            }
        }
    });
    button_box.append(&save_button);

    main_box.append(&button_box);
    dialog.set_child(Some(&main_box));
    dialog.present();
}
