use gtk4::prelude::*;
use gtk4::{Button, Label, LevelBar, Spinner, TextView, Box as GtkBox};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

/// Application state for recording modes
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AppState {
    Idle,
    Recording,
    Processing,
}

/// Recording state shared across UI components
pub struct RecordingContext {
    pub state: Rc<Cell<AppState>>,
    pub start_time: Rc<Cell<Option<Instant>>>,
}

impl RecordingContext {
    pub fn new() -> Self {
        Self {
            state: Rc::new(Cell::new(AppState::Idle)),
            start_time: Rc::new(Cell::new(None)),
        }
    }

    pub fn is_idle(&self) -> bool {
        self.state.get() == AppState::Idle
    }

    pub fn is_recording(&self) -> bool {
        self.state.get() == AppState::Recording
    }

    pub fn is_processing(&self) -> bool {
        self.state.get() == AppState::Processing
    }

    pub fn start_recording(&self) {
        self.state.set(AppState::Recording);
        self.start_time.set(Some(Instant::now()));
    }

    pub fn start_processing(&self) {
        self.state.set(AppState::Processing);
        self.start_time.set(None);
    }

    pub fn finish(&self) {
        self.state.set(AppState::Idle);
        self.start_time.set(None);
    }

    pub fn elapsed_secs(&self) -> Option<u64> {
        self.start_time.get().map(|t| t.elapsed().as_secs())
    }
}

impl Default for RecordingContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for RecordingContext {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
            start_time: Rc::clone(&self.start_time),
        }
    }
}

/// Common UI widgets used across all recording modes
#[derive(Clone)]
pub struct UIContext {
    pub button: Button,
    pub status_label: Label,
    pub result_text_view: TextView,
    pub timer_label: Label,
    pub spinner: Spinner,
}

impl UIContext {
    pub fn new(
        button: Button,
        status_label: Label,
        result_text_view: TextView,
        timer_label: Label,
        spinner: Spinner,
    ) -> Self {
        Self {
            button,
            status_label,
            result_text_view,
            timer_label,
            spinner,
        }
    }

    /// Update UI to recording state
    pub fn set_recording(&self, status_text: &str) {
        self.button.set_label("Зупинити запис");
        self.button.remove_css_class("suggested-action");
        self.button.add_css_class("destructive-action");
        self.status_label.set_text(status_text);
        self.result_text_view.buffer().set_text("");
        self.timer_label.set_text("00:00");
        self.timer_label.set_visible(true);
    }

    /// Update UI to processing state
    pub fn set_processing(&self, status_text: &str) {
        self.button.set_label("Обробка...");
        self.button.remove_css_class("destructive-action");
        self.button.remove_css_class("suggested-action");
        self.button.set_sensitive(false);
        self.status_label.set_text(status_text);
        self.timer_label.set_visible(false);
        self.spinner.set_visible(true);
        self.spinner.start();
    }

    /// Update UI to idle state
    pub fn set_idle(&self) {
        self.button.set_label("Почати запис");
        self.button.add_css_class("suggested-action");
        self.button.remove_css_class("destructive-action");
        self.button.set_sensitive(true);
        self.spinner.stop();
        self.spinner.set_visible(false);
    }

    /// Update timer display
    pub fn update_timer(&self, secs: u64) {
        let minutes = secs / 60;
        let seconds = secs % 60;
        self.timer_label.set_text(&format!("{:02}:{:02}", minutes, seconds));
    }

    /// Get current result text
    pub fn get_result_text(&self) -> String {
        let buffer = self.result_text_view.buffer();
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        buffer.text(&start, &end, false).to_string()
    }

    /// Set result text
    pub fn set_result_text(&self, text: &str) {
        self.result_text_view.buffer().set_text(text);
    }
}

/// Dictation mode specific UI widgets
#[derive(Clone)]
pub struct DictationUI {
    pub base: UIContext,
    pub level_bar: LevelBar,
}

impl DictationUI {
    pub fn new(base: UIContext, level_bar: LevelBar) -> Self {
        Self { base, level_bar }
    }

    pub fn show_level_bar(&self) {
        self.level_bar.set_value(0.0);
        self.level_bar.set_visible(true);
    }

    pub fn hide_level_bar(&self) {
        self.level_bar.set_visible(false);
    }

    pub fn update_level(&self, amplitude: f64) {
        self.level_bar.set_value(amplitude);
    }
}

/// Continuous mode specific UI widgets
#[derive(Clone)]
pub struct ContinuousUI {
    pub base: UIContext,
    pub level_bar: LevelBar,
    pub vad_indicator: Label,
    pub segment_indicators_box: GtkBox,
    pub segment_row: GtkBox,
}

impl ContinuousUI {
    pub fn new(
        base: UIContext,
        level_bar: LevelBar,
        vad_indicator: Label,
        segment_indicators_box: GtkBox,
        segment_row: GtkBox,
    ) -> Self {
        Self {
            base,
            level_bar,
            vad_indicator,
            segment_indicators_box,
            segment_row,
        }
    }

    pub fn show_recording_ui(&self) {
        self.level_bar.set_value(0.0);
        self.level_bar.set_visible(true);
        self.vad_indicator.set_text("🔇 Тиша");
        self.vad_indicator.set_visible(true);
        self.clear_segment_indicators();
        self.segment_row.set_visible(true);
    }

    pub fn hide_recording_ui(&self) {
        self.level_bar.set_visible(false);
        self.vad_indicator.set_visible(false);
    }

    pub fn clear_segment_indicators(&self) {
        while let Some(child) = self.segment_indicators_box.first_child() {
            self.segment_indicators_box.remove(&child);
        }
    }

    pub fn update_level(&self, amplitude: f64) {
        self.level_bar.set_value(amplitude);
    }

    pub fn update_vad_indicator(&self, is_speech: bool) {
        if is_speech {
            self.vad_indicator.set_text("🔊 Говорить");
        } else {
            self.vad_indicator.set_text("🔇 Тиша");
        }
    }
}

/// Conference mode specific UI widgets
#[derive(Clone)]
pub struct ConferenceUI {
    pub base: UIContext,
    pub mic_level_bar: LevelBar,
    pub loopback_level_bar: LevelBar,
}

impl ConferenceUI {
    pub fn new(
        base: UIContext,
        mic_level_bar: LevelBar,
        loopback_level_bar: LevelBar,
    ) -> Self {
        Self {
            base,
            mic_level_bar,
            loopback_level_bar,
        }
    }

    pub fn show_level_bars(&self) {
        self.mic_level_bar.set_value(0.0);
        self.mic_level_bar.set_visible(true);
        self.loopback_level_bar.set_value(0.0);
        self.loopback_level_bar.set_visible(true);
    }

    pub fn hide_level_bars(&self) {
        self.mic_level_bar.set_visible(false);
        self.loopback_level_bar.set_visible(false);
    }

    pub fn update_levels(&self, mic_amplitude: f64, loopback_amplitude: f64) {
        self.mic_level_bar.set_value(mic_amplitude);
        self.loopback_level_bar.set_value(loopback_amplitude);
    }
}
