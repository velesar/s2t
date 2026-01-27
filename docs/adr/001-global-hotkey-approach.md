# ADR-001: Global Hotkey Implementation Approach

## Status
Accepted

## Context
The application needs to support global hotkeys that work system-wide, allowing users to start/stop voice recording from any application without switching to the voice dictation window. This is a common feature in productivity applications and significantly improves user experience.

On Linux, there are multiple approaches to implementing global hotkeys:
1. Using a dedicated Rust crate like `global-hotkey`
2. Using D-Bus integration with the desktop environment
3. Using X11-specific APIs (not suitable for Wayland)

The application currently runs on Fedora Linux and should support both X11 and Wayland display servers, as Wayland is becoming the default on modern Linux distributions.

## Decision
**Decision: Use `global-hotkey` crate** with D-Bus integration for better Wayland support.

The implementation uses:
- `global-hotkey` crate for X11 support
- D-Bus integration via desktop environment keybindings for Wayland
- Settings dialog UI for hotkey configuration (implemented in `src/settings_dialog.rs`)
- Hotkey management in `src/hotkeys.rs`

### Option 1: global-hotkey crate
Use the `global-hotkey` Rust crate (or similar alternatives like `rdev`, `device_query`).

**Pros:**
- Simple, high-level API
- Cross-platform support (Linux, Windows, macOS)
- Actively maintained Rust ecosystem crate
- Minimal code changes required

**Cons:**
- May have limitations on Wayland (many global hotkey crates rely on X11)
- Additional dependency
- May require X11 fallback on Wayland systems

### Option 2: D-Bus Integration
Use D-Bus to register hotkeys through the desktop environment's keybinding service.

**Pros:**
- Native Linux integration
- Better Wayland support (works with GNOME, KDE keybinding services)
- No additional Rust dependencies (use existing `dbus` or `zbus` crates)
- Follows Linux desktop standards

**Cons:**
- More complex implementation
- Linux-specific (not cross-platform)
- Requires understanding D-Bus API
- May need different approaches for different desktop environments (GNOME vs KDE)

### Option 3: Hybrid Approach
Use `global-hotkey` crate with X11 backend, and D-Bus for Wayland.

**Pros:**
- Best compatibility across display servers
- Leverages strengths of both approaches

**Cons:**
- Most complex implementation
- Requires runtime detection of display server
- More code to maintain

## Consequences

### Positive (if using global-hotkey)
- Faster implementation
- Simpler codebase
- Potential for future cross-platform support

### Negative (if using global-hotkey)
- May not work reliably on Wayland
- Dependency on external crate that may have limitations

### Positive (if using D-Bus)
- Native Linux integration
- Reliable on both X11 and Wayland
- Follows desktop environment standards

### Negative (if using D-Bus)
- More complex implementation
- Linux-only solution
- Requires desktop environment support

## Recommendation
**Recommended approach**: Start with `global-hotkey` crate and test on Wayland. If it works reliably, use it. If not, implement D-Bus integration as a fallback or primary method for Wayland systems.

**Alternative**: If the application is Linux-only and Wayland support is critical, implement D-Bus integration directly.

## Related Files
- [docs/backlog/global-hotkeys.md](../backlog/global-hotkeys.md) - Feature description
- [src/config.rs](../../src/config.rs) - Config struct where hotkey setting is stored
- [src/main.rs](../../src/main.rs) - Main application loop where hotkey handler is registered
- [src/hotkeys.rs](../../src/hotkeys.rs) - Hotkey implementation
- [src/settings_dialog.rs](../../src/settings_dialog.rs) - Settings UI for hotkey configuration
