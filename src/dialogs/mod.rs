//! Modal dialog modules.
//!
//! These dialogs provide UI for managing application resources:
//! - History browser and export
//! - Whisper model download and management
//! - Application settings

pub mod history;
pub mod model;
pub mod settings;

pub use history::show_history_dialog;
pub use model::show_model_dialog;
pub use settings::show_settings_dialog;
