use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_model: String,
    pub language: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_model: "ggml-base.bin".to_string(),
            language: "uk".to_string(),
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
