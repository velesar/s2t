//! Voice Activity Detection module.
//!
//! Provides multiple VAD implementations:
//! - WebRTC VAD: Fast, energy-based, good for quiet environments
//! - Silero VAD: Neural network-based, more accurate in noisy environments
//!
//! Use `create_vad()` factory function to create the appropriate detector
//! based on configuration.

mod silero;
mod webrtc;

pub use silero::SileroVoiceDetector;
pub use webrtc::WebRtcVoiceDetector;

use crate::domain::traits::VoiceDetection;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// VAD engine selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VadEngine {
    /// WebRTC-based VAD (fast, energy-based)
    #[default]
    WebRTC,
    /// Silero VAD (neural network, more accurate)
    Silero,
}

impl VadEngine {
    /// Parse VAD engine from string.
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "silero" => VadEngine::Silero,
            _ => VadEngine::WebRTC,
        }
    }

    /// Convert to string representation.
    #[cfg(test)]
    pub fn as_str(&self) -> &'static str {
        match self {
            VadEngine::WebRTC => "webrtc",
            VadEngine::Silero => "silero",
        }
    }
}

/// Configuration for VAD creation.
pub struct VadConfig {
    pub engine: VadEngine,
    pub silence_threshold_ms: u32,
    pub min_speech_ms: u32,
    pub silero_threshold: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            engine: VadEngine::WebRTC,
            silence_threshold_ms: 1000,
            min_speech_ms: 500,
            silero_threshold: 0.5,
        }
    }
}

/// Create a VAD instance based on configuration.
///
/// # Arguments
/// * `config` - VAD configuration specifying engine and thresholds
///
/// # Returns
/// A boxed trait object implementing VoiceDetection.
/// Note: The returned object is NOT Send/Sync - create in the thread where it will be used.
pub fn create_vad(config: &VadConfig) -> Result<Box<dyn VoiceDetection>> {
    match config.engine {
        VadEngine::WebRTC => {
            let vad = WebRtcVoiceDetector::with_thresholds(config.silence_threshold_ms, config.min_speech_ms)?;
            Ok(Box::new(vad))
        }
        VadEngine::Silero => {
            let vad = SileroVoiceDetector::with_thresholds(
                config.silero_threshold,
                config.silence_threshold_ms,
                config.min_speech_ms,
            )?;
            Ok(Box::new(vad))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_engine_parse() {
        assert_eq!(VadEngine::parse("webrtc"), VadEngine::WebRTC);
        assert_eq!(VadEngine::parse("WebRTC"), VadEngine::WebRTC);
        assert_eq!(VadEngine::parse("silero"), VadEngine::Silero);
        assert_eq!(VadEngine::parse("Silero"), VadEngine::Silero);
        assert_eq!(VadEngine::parse("unknown"), VadEngine::WebRTC);
    }

    #[test]
    fn test_vad_engine_as_str() {
        assert_eq!(VadEngine::WebRTC.as_str(), "webrtc");
        assert_eq!(VadEngine::Silero.as_str(), "silero");
    }

    #[test]
    fn test_vad_config_default() {
        let config = VadConfig::default();
        assert_eq!(config.engine, VadEngine::WebRTC);
        assert_eq!(config.silence_threshold_ms, 1000);
        assert_eq!(config.min_speech_ms, 500);
        assert_eq!(config.silero_threshold, 0.5);
    }

    #[test]
    fn test_create_vad_webrtc() {
        let config = VadConfig {
            engine: VadEngine::WebRTC,
            ..Default::default()
        };
        let vad = create_vad(&config);
        assert!(vad.is_ok());
    }

    #[test]
    fn test_create_vad_silero() {
        let config = VadConfig {
            engine: VadEngine::Silero,
            ..Default::default()
        };
        let vad = create_vad(&config);
        assert!(vad.is_ok());
    }

    #[test]
    fn test_create_vad_silence_detection() {
        let config = VadConfig::default();
        let vad = create_vad(&config).unwrap();

        let silence = vec![0.0f32; 480];
        let result = vad.is_speech(&silence).unwrap();
        assert!(!result);
    }
}
