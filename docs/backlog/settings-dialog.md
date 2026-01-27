# Діалог налаштувань

## User Story
As a user, I want a graphical interface to edit application settings, so that I don't need to manually edit the config.toml file.

## Acceptance Criteria
- [ ] Settings dialog accessible from main window (e.g., "Налаштування" button)
- [ ] Dialog displays current configuration values
- [ ] User can change language selection (dropdown with supported languages)
- [ ] User can adjust history limits (max entries and max age in days)
- [ ] User can toggle auto-copy feature
- [ ] User can configure global hotkey (when hotkey feature is implemented)
- [ ] Changes are saved to config.toml when "Save" or "Apply" is clicked
- [ ] Changes take effect immediately (or after restart if needed)
- [ ] Dialog follows GTK4 design patterns consistent with existing dialogs
- [ ] Dialog is modal and transient to main window

## Technical Details

### Implementation Approach
Create a new module `src/settings_dialog.rs` following the pattern of existing dialogs:
- Similar structure to [src/model_dialog.rs](src/model_dialog.rs) and [src/history_dialog.rs](src/history_dialog.rs)
- Use GTK4 widgets: `Window`, `Box`, `Entry`, `SpinButton`, `Switch`, `ComboBoxText`, `Button`
- Modal dialog with "Save" and "Cancel" buttons

### Dialog Fields

1. **Language Selection**
   - `ComboBoxText` with available Whisper language codes
   - Current value from `config.language`
   - Supported languages: uk, en, and others supported by Whisper

2. **History Max Entries**
   - `SpinButton` with range 0-10000
   - Current value from `config.history_max_entries`

3. **History Max Age (days)**
   - `SpinButton` with range 1-365
   - Current value from `config.history_max_age_days`

4. **Auto Copy Toggle**
   - `Switch` widget
   - Current value from `config.auto_copy`

5. **Global Hotkey** (when implemented)
   - `Entry` widget for hotkey string
   - Current value from `config.hotkey`
   - Format: "Ctrl+Shift+D" or similar

### Code Structure
```rust
pub fn show_settings_dialog(
    parent: &impl IsA<Window>,
    config: Arc<Mutex<Config>>,
) {
    let dialog = Window::builder()
        .title("Налаштування")
        .modal(true)
        .transient_for(parent)
        .default_width(400)
        .default_height(500)
        .build();

    // Create form fields with current config values
    // Add Save/Cancel buttons
    // Connect Save button to save_config()
}
```

### Integration Points
- Add "Налаштування" button to main UI in [src/ui.rs](src/ui.rs) next to "Моделі" and "Історія" buttons
- Use existing `save_config()` function from [src/config.rs](src/config.rs:64-76)
- Load current config values when dialog opens
- Validate input before saving (e.g., hotkey format, numeric ranges)

### UI Layout
```
┌─────────────────────────────┐
│ Налаштування                 │
├─────────────────────────────┤
│ Мова: [Dropdown ▼]          │
│                              │
│ Макс. записів історії: [###]│
│                              │
│ Макс. вік історії (дні): [##]│
│                              │
│ Автоматичне копіювання: [✓] │
│                              │
│ Глобальна гаряча клавіша:   │
│ [Entry field____________]    │
│                              │
│         [Скасувати] [Зберегти]│
└─────────────────────────────┘
```

## Configuration
No new config fields needed - dialog edits existing `Config` struct fields:
- `language: String`
- `history_max_entries: usize`
- `history_max_age_days: i64`
- `auto_copy: bool`
- `hotkey: Option<String>` (when hotkey feature is implemented)

## Dependencies
- Auto-copy feature (for toggle in dialog)
- Global hotkeys feature (for hotkey configuration field)
- No external dependencies (uses existing GTK4 widgets)

## Priority
P1 - Implemented

## Status
✅ **Implemented** - Full settings dialog with all fields

## Related Files
- [src/settings_dialog.rs](../../src/settings_dialog.rs) - Settings dialog implementation
- [src/config.rs](../../src/config.rs) - Config struct and save_config function
- [src/ui.rs](../../src/ui.rs) - Main UI with settings button
- [src/model_dialog.rs](../../src/model_dialog.rs) - Reference implementation pattern
- [src/history_dialog.rs](../../src/history_dialog.rs) - Reference implementation pattern
