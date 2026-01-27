use anyhow::{Context, Result};

/// Simulates Ctrl+V paste keystroke to paste clipboard content into active window
/// Uses xdotool for X11 (most common on Linux)
/// Note: On Wayland, this may not work and user may need to manually paste
pub fn paste_from_clipboard() -> Result<()> {
    // Small delay to ensure clipboard is ready
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Use xdotool to simulate Ctrl+V (works on X11)
    let output = std::process::Command::new("xdotool")
        .arg("key")
        .arg("ctrl+v")
        .output()
        .context("Не вдалося виконати xdotool. Переконайтеся, що xdotool встановлено (sudo dnf install xdotool)")?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("xdotool помилка: {}", error_msg);
    }

    Ok(())
}
