# Автоматичне копіювання

## User Story
As a user, I want transcription results to be automatically copied to the clipboard, so that I can immediately paste the text without manually clicking the copy button.

## Acceptance Criteria
- [ ] User can enable/disable auto-copy via config setting `auto_copy = true/false`
- [ ] When enabled, transcription result is automatically copied to clipboard after successful transcription
- [ ] Auto-copy only occurs when transcription is successful (non-empty result)
- [ ] Auto-copy respects the same clipboard API as manual copy button
- [ ] Setting can be changed via settings dialog (when implemented)
- [ ] Setting persists across application restarts

## Technical Details

### Implementation Approach
1. Add `auto_copy: bool` field to `Config` struct in [src/config.rs](src/config.rs)
2. After transcription completes successfully in `handle_stop_recording` function ([src/ui.rs](src/ui.rs:306-418)), check the config value
3. If `auto_copy` is true and text is not empty, call the same clipboard code used in `setup_copy_button` ([src/ui.rs](src/ui.rs:420-432))

### Code Changes
Modify the transcription completion handler in `handle_stop_recording`:
```rust
if let Ok(text) = rx.recv().await {
    match result {
        Ok(text) => {
            if text.is_empty() {
                status_label.set_text("Не вдалося розпізнати мову");
            } else {
                status_label.set_text("Готово!");
                result_label.set_text(&text);

                // Auto-copy if enabled
                let should_auto_copy = {
                    let cfg = config.lock().unwrap();
                    cfg.auto_copy
                };
                if should_auto_copy {
                    if let Some(display) = gtk4::gdk::Display::default() {
                        let clipboard = display.clipboard();
                        clipboard.set_text(&text);
                    }
                }

                // Save to history
                // ... existing code ...
            }
        }
        // ... error handling ...
    }
}
```

### Integration Points
- Reuse existing clipboard code from `setup_copy_button` function
- Check config value after successful transcription
- No UI changes needed (feature is transparent to user)

## Configuration

Add to `Config` struct:
```rust
#[serde(default = "default_auto_copy")]
pub auto_copy: bool,
```

Add default function:
```rust
fn default_auto_copy() -> bool {
    false  // Disabled by default
}
```

Add to `config.toml`:
```toml
auto_copy = true  # Optional, defaults to false
```

## Dependencies
- Settings dialog feature (for GUI configuration)
- No external dependencies required (uses existing GTK clipboard API)

## Priority
P1 - Implemented

## Status
✅ **Implemented** - Auto-copy setting in config and settings dialog

## Related Files
- [src/config.rs](../../src/config.rs) - Config struct and default values
- [src/ui.rs](../../src/ui.rs) - Transcription handler and clipboard code
  - `handle_stop_recording` function - auto-copy logic
  - `copy_to_clipboard` function - clipboard implementation
- [src/settings_dialog.rs](../../src/settings_dialog.rs) - UI toggle for auto-copy
