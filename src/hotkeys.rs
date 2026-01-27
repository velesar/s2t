use crate::config::Config;
use anyhow::{Context, Result};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    current_hotkey: Option<HotKey>,
}

impl HotkeyManager {
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new()
            .context("Не вдалося створити менеджер гарячих клавіш")?;

        Ok(Self {
            manager,
            current_hotkey: None,
        })
    }

    pub fn register_from_config(&mut self, config: &Config) -> Result<()> {
        // Unregister existing hotkey if any
        if let Some(hotkey) = self.current_hotkey.take() {
            let _ = self.manager.unregister(hotkey);
        }

        if !config.hotkey_enabled {
            return Ok(());
        }

        // Parse hotkey string (e.g., "Control+Shift+Space")
        let (modifiers, code) = parse_hotkey(&config.hotkey)
            .context("Не вдалося розпарсити гарячу клавішу")?;

        let hotkey = HotKey::new(Some(modifiers), code);
        self.manager
            .register(hotkey)
            .context("Не вдалося зареєструвати гарячу клавішу")?;

        self.current_hotkey = Some(hotkey);
        Ok(())
    }

    /// Unregister the current hotkey. Called automatically by `register_from_config`
    /// before registering a new hotkey, and by `Drop`. Available for explicit cleanup.
    #[allow(dead_code)]
    pub fn unregister(&mut self) -> Result<()> {
        if let Some(hotkey) = self.current_hotkey.take() {
            self.manager
                .unregister(hotkey)
                .context("Не вдалося скасувати реєстрацію гарячої клавіші")?;
        }
        Ok(())
    }
}

fn parse_hotkey(hotkey_str: &str) -> Result<(Modifiers, Code)> {
    let parts: Vec<&str> = hotkey_str.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        anyhow::bail!("Порожня гаряча клавіша");
    }

    let mut modifiers = Modifiers::empty();
    let code_str = parts.last().unwrap().to_string();

    for part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "control" | "ctrl" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" => modifiers |= Modifiers::ALT,
            "super" | "meta" | "cmd" => modifiers |= Modifiers::META,
            _ => {
                anyhow::bail!("Невідомий модифікатор: {}", part);
            }
        }
    }

    let code = parse_key_code(&code_str)?;

    Ok((modifiers, code))
}

fn parse_key_code(code_str: &str) -> Result<Code> {
    let code_str_lower = code_str.to_lowercase();
    match code_str_lower.as_str() {
        "space" => Ok(Code::Space),
        "enter" | "return" => Ok(Code::Enter),
        "tab" => Ok(Code::Tab),
        "escape" | "esc" => Ok(Code::Escape),
        "backspace" => Ok(Code::Backspace),
        "delete" | "del" => Ok(Code::Delete),
        "up" => Ok(Code::ArrowUp),
        "down" => Ok(Code::ArrowDown),
        "left" => Ok(Code::ArrowLeft),
        "right" => Ok(Code::ArrowRight),
        "home" => Ok(Code::Home),
        "end" => Ok(Code::End),
        "pageup" | "page_up" => Ok(Code::PageUp),
        "pagedown" | "page_down" => Ok(Code::PageDown),
        "f1" => Ok(Code::F1),
        "f2" => Ok(Code::F2),
        "f3" => Ok(Code::F3),
        "f4" => Ok(Code::F4),
        "f5" => Ok(Code::F5),
        "f6" => Ok(Code::F6),
        "f7" => Ok(Code::F7),
        "f8" => Ok(Code::F8),
        "f9" => Ok(Code::F9),
        "f10" => Ok(Code::F10),
        "f11" => Ok(Code::F11),
        "f12" => Ok(Code::F12),
        "a" => Ok(Code::KeyA),
        "b" => Ok(Code::KeyB),
        "c" => Ok(Code::KeyC),
        "d" => Ok(Code::KeyD),
        "e" => Ok(Code::KeyE),
        "f" => Ok(Code::KeyF),
        "g" => Ok(Code::KeyG),
        "h" => Ok(Code::KeyH),
        "i" => Ok(Code::KeyI),
        "j" => Ok(Code::KeyJ),
        "k" => Ok(Code::KeyK),
        "l" => Ok(Code::KeyL),
        "m" => Ok(Code::KeyM),
        "n" => Ok(Code::KeyN),
        "o" => Ok(Code::KeyO),
        "p" => Ok(Code::KeyP),
        "q" => Ok(Code::KeyQ),
        "r" => Ok(Code::KeyR),
        "s" => Ok(Code::KeyS),
        "t" => Ok(Code::KeyT),
        "u" => Ok(Code::KeyU),
        "v" => Ok(Code::KeyV),
        "w" => Ok(Code::KeyW),
        "x" => Ok(Code::KeyX),
        "y" => Ok(Code::KeyY),
        "z" => Ok(Code::KeyZ),
        "0" => Ok(Code::Digit0),
        "1" => Ok(Code::Digit1),
        "2" => Ok(Code::Digit2),
        "3" => Ok(Code::Digit3),
        "4" => Ok(Code::Digit4),
        "5" => Ok(Code::Digit5),
        "6" => Ok(Code::Digit6),
        "7" => Ok(Code::Digit7),
        "8" => Ok(Code::Digit8),
        "9" => Ok(Code::Digit9),
        _ => anyhow::bail!("Невідомий код клавіші: {}", code_str),
    }
}
