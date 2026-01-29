use async_channel::{Receiver, Sender};

/// Bundled UI communication channels for tray, hotkey, and dialog interactions
pub struct UIChannels {
    pub open_models: (Sender<()>, Receiver<()>),
    pub open_history: (Sender<()>, Receiver<()>),
    pub open_settings: (Sender<()>, Receiver<()>),
    pub toggle_recording: (Sender<()>, Receiver<()>),
    pub reload_hotkeys: (Sender<()>, Receiver<()>),
}

impl UIChannels {
    /// Create a new set of UI channels with bounded capacity of 1
    pub fn new() -> Self {
        Self {
            open_models: async_channel::bounded(1),
            open_history: async_channel::bounded(1),
            open_settings: async_channel::bounded(1),
            toggle_recording: async_channel::bounded(1),
            reload_hotkeys: async_channel::bounded(1),
        }
    }

    /// Get sender for open_models channel
    pub fn open_models_tx(&self) -> &Sender<()> {
        &self.open_models.0
    }

    /// Get receiver for open_models channel
    pub fn open_models_rx(&self) -> &Receiver<()> {
        &self.open_models.1
    }

    /// Get sender for open_history channel
    pub fn open_history_tx(&self) -> &Sender<()> {
        &self.open_history.0
    }

    /// Get receiver for open_history channel
    pub fn open_history_rx(&self) -> &Receiver<()> {
        &self.open_history.1
    }

    /// Get sender for open_settings channel
    pub fn open_settings_tx(&self) -> &Sender<()> {
        &self.open_settings.0
    }

    /// Get receiver for open_settings channel
    pub fn open_settings_rx(&self) -> &Receiver<()> {
        &self.open_settings.1
    }

    /// Get sender for toggle_recording channel
    pub fn toggle_recording_tx(&self) -> &Sender<()> {
        &self.toggle_recording.0
    }

    /// Get receiver for toggle_recording channel
    pub fn toggle_recording_rx(&self) -> &Receiver<()> {
        &self.toggle_recording.1
    }

    /// Get sender for reload_hotkeys channel
    pub fn reload_hotkeys_tx(&self) -> &Sender<()> {
        &self.reload_hotkeys.0
    }

    /// Get receiver for reload_hotkeys channel
    pub fn reload_hotkeys_rx(&self) -> &Receiver<()> {
        &self.reload_hotkeys.1
    }
}

impl Default for UIChannels {
    fn default() -> Self {
        Self::new()
    }
}
