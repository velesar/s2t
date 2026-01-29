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
use std::sync::{Arc, Mutex};

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
            let cfg = config.lock().unwrap();
            SegmentationConfig {
                use_vad: cfg.use_vad,
                segment_interval_secs: cfg.segment_interval_secs,
                vad_silence_threshold_ms: cfg.vad_silence_threshold_ms,
                vad_min_speech_ms: cfg.vad_min_speech_ms,
                vad_engine: VadEngine::from_str(&cfg.vad_engine),
                silero_threshold: cfg.silero_threshold,
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
        ConfigProvider::language(&*self.config.lock().unwrap())
    }

    /// Check if continuous mode is enabled
    pub fn continuous_mode(&self) -> bool {
        ConfigProvider::continuous_mode(&*self.config.lock().unwrap())
    }

    /// Check if auto-copy is enabled
    pub fn auto_copy(&self) -> bool {
        ConfigProvider::auto_copy(&*self.config.lock().unwrap())
    }

    /// Check if auto-paste is enabled
    pub fn auto_paste(&self) -> bool {
        ConfigProvider::auto_paste(&*self.config.lock().unwrap())
    }

    /// Get diarization method
    pub fn diarization_method(&self) -> String {
        self.config.lock().unwrap().diarization_method.clone()
    }

    // === Transcription convenience methods ===

    /// Check if a Whisper model is loaded
    pub fn is_model_loaded(&self) -> bool {
        self.transcription.lock().unwrap().is_loaded()
    }
}
