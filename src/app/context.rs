//! Application context bundling all services and shared state.
//!
//! This module provides the `AppContext` struct which serves as the central
//! point for dependency injection, breaking cyclic dependencies between
//! main.rs, ui/, whisper.rs, and config.rs.

use crate::app::channels::UIChannels;
use crate::app::config::Config;
use crate::transcription::diarization::DiarizationEngine;
use crate::history::History;
use crate::recording::service::AudioService;
use crate::recording::segmentation::SegmentationConfig;
use crate::transcription::TranscriptionService;
use crate::domain::traits::{ConfigProvider, Transcription};
use crate::vad::VadEngine;
use anyhow::Result;
use std::sync::Arc;
use parking_lot::Mutex;

/// Central application context bundling all services and shared state.
///
/// `AppContext` serves as the single source of truth for shared application
/// state and services. It breaks cyclic dependencies by providing a unified
/// interface that can be passed to UI components.
///
/// # Usage
///
/// ```ignore
/// let ctx = AppContext::new(config, history, transcription, diarization_engine)?;
/// build_ui(app, ctx.clone());
/// ```
pub struct AppContext {
    /// Audio recording service (dictation, conference, continuous modes)
    pub audio: Arc<AudioService>,

    /// Transcription service (Whisper + diarization)
    pub transcription: Arc<Mutex<TranscriptionService>>,

    /// Application configuration
    pub config: Arc<Mutex<Config>>,

    /// Transcription history
    pub history: Arc<Mutex<History>>,

    /// Diarization engine (for conference mode)
    pub diarization: Arc<Mutex<DiarizationEngine>>,

    /// UI communication channels (tray, hotkeys, dialogs)
    pub channels: Arc<UIChannels>,
}

impl AppContext {
    /// Create a new `AppContext` with all services.
    ///
    /// The `AudioService` is created internally using configuration from `config`.
    pub fn new(
        config: Arc<Mutex<Config>>,
        history: Arc<Mutex<History>>,
        transcription: TranscriptionService,
        diarization: DiarizationEngine,
    ) -> Result<Self> {
        let seg_config = {
            let cfg = config.lock();
            SegmentationConfig {
                use_vad: cfg.use_vad,
                segment_interval_secs: cfg.segment_interval_secs,
                vad_silence_threshold_ms: cfg.vad_silence_threshold_ms,
                vad_min_speech_ms: cfg.vad_min_speech_ms,
                vad_engine: VadEngine::parse(&cfg.vad_engine),
                silero_threshold: cfg.silero_threshold,
                max_segment_secs: cfg.max_segment_secs,
            }
        };

        let audio =
            AudioService::new(seg_config).unwrap_or_else(|_| AudioService::new_default());

        Ok(Self {
            audio: Arc::new(audio),
            transcription: Arc::new(Mutex::new(transcription)),
            config,
            history,
            diarization: Arc::new(Mutex::new(diarization)),
            channels: Arc::new(UIChannels::new()),
        })
    }

    // === Config convenience methods ===
    // These use the ConfigProvider trait for polymorphism

    /// Get current language setting
    pub fn language(&self) -> String {
        ConfigProvider::language(&*self.config.lock())
    }

    /// Check if continuous mode is enabled
    pub fn continuous_mode(&self) -> bool {
        ConfigProvider::continuous_mode(&*self.config.lock())
    }

    /// Check if auto-copy is enabled
    pub fn auto_copy(&self) -> bool {
        ConfigProvider::auto_copy(&*self.config.lock())
    }

    /// Check if auto-paste is enabled
    pub fn auto_paste(&self) -> bool {
        ConfigProvider::auto_paste(&*self.config.lock())
    }

    /// Get diarization method
    pub fn diarization_method(&self) -> String {
        self.config.lock().diarization_method.clone()
    }

    /// Check if denoising is enabled
    pub fn denoise_enabled(&self) -> bool {
        self.config.lock().denoise_enabled
    }

    // === Transcription convenience methods ===

    /// Check if a Whisper model is loaded
    pub fn is_model_loaded(&self) -> bool {
        self.transcription.lock().is_loaded()
    }

    /// Create an `AppContext` for testing without requiring real hardware.
    ///
    /// Accepts pre-built services so tests can inject mocks for audio,
    /// transcription, and diarization. All fields are set directly —
    /// no CPAL devices, Whisper models, or Sortformer weights needed.
    #[cfg(test)]
    pub fn for_testing(
        config: Arc<Mutex<Config>>,
        history: Arc<Mutex<History>>,
        audio: Arc<AudioService>,
        transcription: Arc<Mutex<TranscriptionService>>,
    ) -> Self {
        Self {
            audio,
            transcription,
            config,
            history,
            diarization: Arc::new(Mutex::new(DiarizationEngine::default())),
            channels: Arc::new(UIChannels::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::mocks::MockAudioRecorder;

    #[test]
    fn test_for_testing_creates_valid_context() {
        let config = Arc::new(Mutex::new(Config::default()));
        let history = Arc::new(Mutex::new(History::default()));
        let mock = Arc::new(MockAudioRecorder::new());
        let samples = mock.samples_buffer();
        let audio = Arc::new(AudioService::with_recorder(mock, samples, None).unwrap());
        let transcription = Arc::new(Mutex::new(TranscriptionService::new()));

        let ctx = AppContext::for_testing(config, history, audio, transcription);

        assert!(!ctx.is_model_loaded());
        assert_eq!(ctx.language(), "uk");
    }

    #[test]
    fn test_for_testing_config_accessors() {
        let mut cfg = Config::default();
        cfg.auto_copy = true;
        cfg.auto_paste = false;
        cfg.continuous_mode = true;
        cfg.denoise_enabled = true;
        cfg.diarization_method = "sortformer".to_string();

        let config = Arc::new(Mutex::new(cfg));
        let history = Arc::new(Mutex::new(History::default()));
        let mock = Arc::new(MockAudioRecorder::new());
        let samples = mock.samples_buffer();
        let audio = Arc::new(AudioService::with_recorder(mock, samples, None).unwrap());
        let transcription = Arc::new(Mutex::new(TranscriptionService::new()));

        let ctx = AppContext::for_testing(config, history, audio, transcription);

        assert!(ctx.auto_copy());
        assert!(!ctx.auto_paste());
        assert!(ctx.continuous_mode());
        assert!(ctx.denoise_enabled());
        assert_eq!(ctx.diarization_method(), "sortformer");
    }

    #[test]
    fn test_for_testing_channels_work() {
        let config = Arc::new(Mutex::new(Config::default()));
        let history = Arc::new(Mutex::new(History::default()));
        let mock = Arc::new(MockAudioRecorder::new());
        let samples = mock.samples_buffer();
        let audio = Arc::new(AudioService::with_recorder(mock, samples, None).unwrap());
        let transcription = Arc::new(Mutex::new(TranscriptionService::new()));

        let ctx = AppContext::for_testing(config, history, audio, transcription);

        // Verify channels are functional
        let tx = ctx.channels.toggle_recording_tx().clone();
        let rx = ctx.channels.toggle_recording_rx().clone();
        tx.send_blocking(()).unwrap();
        rx.recv_blocking().unwrap();
    }

    #[test]
    fn test_for_testing_audio_service() {
        let config = Arc::new(Mutex::new(Config::default()));
        let history = Arc::new(Mutex::new(History::default()));
        let mock = Arc::new(MockAudioRecorder::with_samples(vec![0.1, 0.2, 0.3]));
        let samples = mock.samples_buffer();
        let audio = Arc::new(AudioService::with_recorder(mock, samples, None).unwrap());
        let transcription = Arc::new(Mutex::new(TranscriptionService::new()));

        let ctx = AppContext::for_testing(config, history, audio, transcription);

        ctx.audio.start_mic().unwrap();
        let (recorded, _) = ctx.audio.stop_mic();
        assert_eq!(recorded, vec![0.1, 0.2, 0.3]);
    }
}
