use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_model: String,
    pub language: String,
    #[serde(default = "default_history_max_entries")]
    pub history_max_entries: usize,
    #[serde(default = "default_history_max_age_days")]
    pub history_max_age_days: i64,
    #[serde(default = "default_auto_copy")]
    pub auto_copy: bool,
    #[serde(default = "default_hotkey_enabled")]
    pub hotkey_enabled: bool,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_auto_paste")]
    pub auto_paste: bool,
    #[serde(default = "default_recording_mode")]
    pub recording_mode: String,
    #[serde(default = "default_diarization_method")]
    pub diarization_method: String,
    #[serde(default)]
    pub sortformer_model_path: Option<String>,
    #[serde(default = "default_continuous_mode")]
    pub continuous_mode: bool,
    #[serde(default = "default_segment_interval_secs")]
    pub segment_interval_secs: u32,
    #[serde(default = "default_use_vad")]
    pub use_vad: bool,
    #[serde(default = "default_vad_silence_threshold_ms")]
    pub vad_silence_threshold_ms: u32,
    #[serde(default = "default_vad_min_speech_ms")]
    pub vad_min_speech_ms: u32,
    #[serde(default = "default_denoise_enabled")]
    pub denoise_enabled: bool,
    #[serde(default = "default_vad_engine")]
    pub vad_engine: String,
    #[serde(default = "default_silero_threshold")]
    pub silero_threshold: f32,
}

fn default_diarization_method() -> String {
    "channel".to_string() // "channel" or "sortformer"
}

fn default_continuous_mode() -> bool {
    false
}

fn default_segment_interval_secs() -> u32 {
    5 // 5 seconds for more responsive feedback
}

fn default_use_vad() -> bool {
    true
}

fn default_vad_silence_threshold_ms() -> u32 {
    1000 // 1 second of silence to trigger segment
}

fn default_vad_min_speech_ms() -> u32 {
    500 // Minimum 500ms of speech for a valid segment
}

fn default_denoise_enabled() -> bool {
    false // Disabled by default for backward compatibility
}

fn default_vad_engine() -> String {
    "webrtc".to_string() // WebRTC VAD by default for backward compatibility
}

fn default_silero_threshold() -> f32 {
    0.5 // Default speech probability threshold for Silero VAD
}

fn default_history_max_entries() -> usize {
    500
}

fn default_history_max_age_days() -> i64 {
    90
}

fn default_auto_copy() -> bool {
    false
}

fn default_hotkey_enabled() -> bool {
    false
}

fn default_hotkey() -> String {
    "Control+Shift+Space".to_string()
}

fn default_auto_paste() -> bool {
    false
}

fn default_recording_mode() -> String {
    "dictation".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_model: "ggml-base.bin".to_string(),
            language: "uk".to_string(),
            history_max_entries: default_history_max_entries(),
            history_max_age_days: default_history_max_age_days(),
            auto_copy: default_auto_copy(),
            hotkey_enabled: default_hotkey_enabled(),
            hotkey: default_hotkey(),
            auto_paste: default_auto_paste(),
            recording_mode: default_recording_mode(),
            diarization_method: default_diarization_method(),
            sortformer_model_path: None,
            continuous_mode: default_continuous_mode(),
            segment_interval_secs: default_segment_interval_secs(),
            use_vad: default_use_vad(),
            vad_silence_threshold_ms: default_vad_silence_threshold_ms(),
            vad_min_speech_ms: default_vad_min_speech_ms(),
            denoise_enabled: default_denoise_enabled(),
            vad_engine: default_vad_engine(),
            silero_threshold: default_silero_threshold(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voice-dictation")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn models_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("whisper")
}

pub fn recordings_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voice-dictation")
        .join("recordings")
}

pub fn sortformer_models_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voice-dictation")
        .join("sortformer")
}

pub fn load_config() -> Result<Config> {
    let path = config_path();

    if !path.exists() {
        return Ok(Config::default());
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Не вдалося прочитати конфіг: {}", path.display()))?;

    toml::from_str(&content).with_context(|| "Не вдалося розпарсити конфіг")
}

pub fn save_config(config: &Config) -> Result<()> {
    let dir = config_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("Не вдалося створити директорію: {}", dir.display()))?;

    let path = config_path();
    let content = toml::to_string_pretty(config).context("Не вдалося серіалізувати конфіг")?;

    fs::write(&path, content)
        .with_context(|| format!("Не вдалося записати конфіг: {}", path.display()))?;

    Ok(())
}

// === Trait Implementation ===

use crate::traits::ConfigProvider;

impl ConfigProvider for Config {
    fn language(&self) -> String {
        self.language.clone()
    }

    fn default_model(&self) -> String {
        self.default_model.clone()
    }

    fn auto_copy(&self) -> bool {
        self.auto_copy
    }

    fn auto_paste(&self) -> bool {
        self.auto_paste
    }

    fn continuous_mode(&self) -> bool {
        self.continuous_mode
    }

    fn recording_mode(&self) -> String {
        self.recording_mode.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.default_model, "ggml-base.bin");
        assert_eq!(config.language, "uk");
    }

    #[test]
    fn test_config_serialization() {
        let config = Config {
            default_model: "ggml-tiny.bin".to_string(),
            language: "en".to_string(),
            history_max_entries: 500,
            history_max_age_days: 90,
            auto_copy: false,
            hotkey_enabled: false,
            hotkey: "Control+Shift+Space".to_string(),
            ..Default::default()
        };

        let toml_str = toml::to_string(&config).unwrap();
        assert!(toml_str.contains("ggml-tiny.bin"));
        assert!(toml_str.contains("en"));

        let parsed: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.default_model, config.default_model);
        assert_eq!(parsed.language, config.language);
    }

    #[test]
    fn test_config_dir_not_empty() {
        let dir = config_dir();
        assert!(dir.to_string_lossy().contains("voice-dictation"));
    }

    #[test]
    fn test_models_dir_not_empty() {
        let dir = models_dir();
        assert!(dir.to_string_lossy().contains("whisper"));
    }

    #[test]
    fn test_config_path_is_toml() {
        let path = config_path();
        assert!(path.to_string_lossy().ends_with("config.toml"));
    }

    // === Trait Implementation Tests ===

    #[test]
    fn test_trait_language_matches_field() {
        use crate::traits::ConfigProvider;
        let config = Config::default();
        assert_eq!(ConfigProvider::language(&config), config.language);
    }

    #[test]
    fn test_trait_default_model_matches_field() {
        use crate::traits::ConfigProvider;
        let config = Config::default();
        assert_eq!(ConfigProvider::default_model(&config), config.default_model);
    }

    #[test]
    fn test_trait_auto_copy_matches_field() {
        use crate::traits::ConfigProvider;
        let config = Config::default();
        assert_eq!(ConfigProvider::auto_copy(&config), config.auto_copy);
    }

    #[test]
    fn test_trait_recording_mode_matches_field() {
        use crate::traits::ConfigProvider;
        let config = Config::default();
        assert_eq!(ConfigProvider::recording_mode(&config), config.recording_mode);
    }
}
