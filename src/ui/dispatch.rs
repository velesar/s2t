//! Recording mode dispatch.
//!
//! Centralizes the mode selection and start/stop routing logic
//! that was previously duplicated in the button handler and hotkey handler.

use crate::app::context::AppContext;
use std::sync::Arc;

use super::state::{
    AppState, ConferenceUI, ContinuousUI, DictationUI, RecordingContext, RecordingMode,
};
use super::{conference, conference_file, continuous, recording};

/// All mode-specific UI types bundled for dispatch.
#[derive(Clone)]
pub struct ModeUIs {
    pub dictation: DictationUI,
    pub continuous: ContinuousUI,
    pub conference: ConferenceUI,
}

/// Toggle recording: start if idle, stop if recording, ignore if processing.
///
/// This is the single entry point for both the record button and the hotkey.
/// It resolves the current mode from the combo box and config, then dispatches
/// to the appropriate handler.
pub fn toggle_recording(
    ctx: &Arc<AppContext>,
    rec: &RecordingContext,
    uis: &ModeUIs,
    mode_combo: &gtk4::ComboBoxText,
) {
    let mode = RecordingMode::resolve(mode_combo, ctx);

    match rec.state.get() {
        AppState::Idle => start_recording(ctx, rec, uis, mode),
        AppState::Recording => stop_recording(ctx, rec, uis, mode),
        AppState::Processing => {
            // Ignore toggle while processing
        }
    }
}

fn start_recording(
    ctx: &Arc<AppContext>,
    rec: &RecordingContext,
    uis: &ModeUIs,
    mode: RecordingMode,
) {
    match mode {
        RecordingMode::Dictation => recording::handle_start(ctx, rec, &uis.dictation),
        RecordingMode::Continuous => continuous::handle_start(ctx, rec, &uis.continuous),
        RecordingMode::Conference => conference::handle_start(ctx, rec, &uis.conference),
        RecordingMode::ConferenceFile => conference_file::handle_start(ctx, rec, &uis.conference),
    }
}

fn stop_recording(
    ctx: &Arc<AppContext>,
    rec: &RecordingContext,
    uis: &ModeUIs,
    mode: RecordingMode,
) {
    match mode {
        RecordingMode::Dictation => recording::handle_stop(ctx, rec, &uis.dictation),
        RecordingMode::Continuous => continuous::handle_stop(ctx, rec, &uis.continuous),
        RecordingMode::Conference => conference::handle_stop(ctx, rec, &uis.conference),
        RecordingMode::ConferenceFile => conference_file::handle_stop(ctx, rec, &uis.conference),
    }
}
