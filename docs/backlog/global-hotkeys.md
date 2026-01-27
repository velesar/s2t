# Глобальні гарячі клавіші

## User Story
As a user, I want to start/stop voice recording from any application using a keyboard shortcut, so that I can quickly dictate text without switching to the voice dictation window.

## Acceptance Criteria
- [ ] User can configure a global hotkey combination in config.toml (e.g., `hotkey = "Ctrl+Shift+D"`)
- [ ] Hotkey works system-wide, even when the application window is not focused
- [ ] Pressing the hotkey toggles recording state (start if idle, stop if recording)
- [ ] Hotkey works on both X11 and Wayland display servers
- [ ] If recording is in progress, hotkey stops recording and triggers transcription
- [ ] Hotkey configuration can be changed without restarting the application
- [ ] Application shows visual feedback when hotkey is triggered (notification or tray icon change)

## Technical Details

### Implementation Approach
Two potential approaches need to be evaluated (see ADR-001):

1. **global-hotkey crate**: Rust crate specifically designed for cross-platform global hotkeys
   - Pros: Simple API, cross-platform support
   - Cons: May have limitations on Wayland, dependency overhead

2. **D-Bus integration**: Use Linux D-Bus to register hotkeys through the desktop environment
   - Pros: Native Linux integration, better Wayland support
   - Cons: More complex implementation, Linux-specific

### Integration Points
- Add `hotkey: Option<String>` field to `Config` struct in [src/config.rs](src/config.rs)
- Parse hotkey string format (e.g., "Ctrl+Shift+D") into key combination
- Register hotkey handler that sends action to main application thread
- Connect hotkey handler to existing recording state machine in [src/ui.rs](src/ui.rs)
- Use async channel (similar to `tray_rx` in [src/main.rs](src/main.rs)) to communicate hotkey events
- Hotkey handler should trigger the same logic as clicking the record button

### State Management
The hotkey handler must respect the current `AppState`:
- `Idle` → Start recording (call `handle_start_recording`)
- `Recording` → Stop recording (call `handle_stop_recording`)
- `Processing` → Ignore hotkey (transcription in progress)

## Configuration

Add to `Config` struct:
```rust
pub hotkey: Option<String>,  // e.g., "Ctrl+Shift+D"
```

Add to `config.toml`:
```toml
hotkey = "Ctrl+Shift+D"  # Optional, None if not set
```

Default value: `None` (feature disabled by default)

## Dependencies
- Requires ADR-001 decision on implementation approach
- May require new dependency: `global-hotkey` crate or D-Bus bindings
- Settings dialog feature (to allow GUI configuration of hotkey)

## Priority
P1 - Implemented

## Status
✅ **Implemented** - See `src/hotkeys.rs` and `src/settings_dialog.rs`

## Related Files
- [src/config.rs](../../src/config.rs) - Config struct definition
- [src/ui.rs](../../src/ui.rs) - Recording state machine and button handlers
- [src/main.rs](../../src/main.rs) - Main application loop and event handling
- [src/tray.rs](../../src/tray.rs) - Tray icon integration (similar pattern for hotkeys)
- [src/hotkeys.rs](../../src/hotkeys.rs) - Hotkey implementation
- [src/settings_dialog.rs](../../src/settings_dialog.rs) - Settings UI for hotkey configuration
