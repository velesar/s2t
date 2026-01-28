use anyhow::{Context, Result};
use chrono::Utc;
use hound::{WavSpec, WavWriter};
use std::fs;
use std::path::{Path, PathBuf};

const SAMPLE_RATE: u32 = 16000;

/// Save stereo WAV file with mic (left) and loopback (right) channels
pub fn save_recording(
    mic_samples: &[f32],
    loopback_samples: &[f32],
    output_path: &Path,
) -> Result<()> {
    // Ensure directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Не вдалося створити директорію: {}", parent.display()))?;
    }

    // Create WAV writer
    let spec = WavSpec {
        channels: 2, // Stereo: left = mic, right = loopback
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = WavWriter::create(output_path, spec)
        .with_context(|| format!("Не вдалося створити WAV файл: {}", output_path.display()))?;

    // Interleave samples: [mic, loopback, mic, loopback, ...]
    let max_len = mic_samples.len().max(loopback_samples.len());

    for i in 0..max_len {
        let mic_sample = mic_samples.get(i).copied().unwrap_or(0.0);
        let loopback_sample = loopback_samples.get(i).copied().unwrap_or(0.0);

        writer
            .write_sample(mic_sample)
            .context("Не вдалося записати зразок мікрофона")?;
        writer
            .write_sample(loopback_sample)
            .context("Не вдалося записати зразок системного аудіо")?;
    }

    writer
        .finalize()
        .context("Не вдалося завершити запис WAV файлу")?;

    Ok(())
}

/// Generate filename for recording based on current timestamp
pub fn generate_recording_filename() -> String {
    let now = Utc::now();
    format!(
        "conference_{}.wav",
        now.format("%Y-%m-%d_%H-%M-%S")
    )
}

/// Get full path for a recording file
pub fn recording_path(filename: &str) -> PathBuf {
    crate::config::recordings_dir().join(filename)
}

/// Ensure recordings directory exists
pub fn ensure_recordings_dir() -> Result<()> {
    let dir = crate::config::recordings_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("Не вдалося створити директорію записів: {}", dir.display()))?;
    Ok(())
}
