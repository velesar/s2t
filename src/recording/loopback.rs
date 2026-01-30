use anyhow::{Context, Result};
use async_channel::Receiver;
use std::sync::atomic::Ordering;
use std::thread;

use super::core::{calculate_rms, RecordingCore, WHISPER_SAMPLE_RATE};

pub(crate) struct LoopbackRecorder {
    core: RecordingCore,
}

impl LoopbackRecorder {
    pub fn new() -> Self {
        Self {
            core: RecordingCore::new(),
        }
    }

    /// Get current audio amplitude (0.0 - 1.0 range, normalized RMS)
    pub fn get_amplitude(&self) -> f32 {
        self.core.get_amplitude()
    }

    /// Start recording from PipeWire loopback (system audio monitor)
    /// Note: This is a simplified MVP implementation
    /// For production, proper PipeWire API integration is needed
    pub fn start_loopback(&self) -> Result<()> {
        // For MVP, we'll use a fallback: try to use parec command-line tool
        // This is simpler and works on both PipeWire and PulseAudio
        // TODO: Implement proper PipeWire API integration

        let handles = self.core.prepare_recording();

        let samples = handles.samples;
        let is_recording_for_loop = handles.is_recording;
        let current_amplitude = handles.current_amplitude;
        let completion_tx = handles.completion_tx;

        // Try to get default monitor source using pactl
        let monitor_source = std::process::Command::new("pactl")
            .args(["list", "sources", "short"])
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .find(|line| line.contains(".monitor"))
                    .and_then(|line| line.split_whitespace().nth(1))
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "@DEFAULT_SOURCE@".to_string());

        // Use parec to capture from monitor source
        // This works on both PipeWire and PulseAudio
        let mut child = std::process::Command::new("parec")
            .arg("--format=s16le")
            .arg(format!("--rate={}", WHISPER_SAMPLE_RATE))
            .arg("--channels=1")
            .arg("--device")
            .arg(&monitor_source)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .context("Не вдалося запустити parec. Переконайтеся, що встановлено pulseaudio-utils (sudo dnf install pulseaudio-utils)")?;

        let stdout = child
            .stdout
            .take()
            .context("Не вдалося отримати stdout від parec")?;
        let mut reader = std::io::BufReader::new(stdout);

        // Spawn thread to read audio data
        thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            while is_recording_for_loop.load(Ordering::SeqCst) {
                if let Ok(bytes_read) = std::io::Read::read(&mut reader, &mut buffer) {
                    if bytes_read == 0 {
                        break;
                    }

                    // Convert i16 samples to f32
                    let i16_samples: Vec<i16> = buffer[..bytes_read]
                        .chunks(2)
                        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                        .collect();

                    let f32_samples: Vec<f32> = i16_samples
                        .iter()
                        .map(|&sample| sample as f32 / 32768.0)
                        .collect();

                    // Calculate RMS amplitude
                    let amplitude = calculate_rms(&f32_samples);
                    current_amplitude.store(amplitude.to_bits(), Ordering::Relaxed);

                    // Store samples
                    samples.lock().unwrap().extend(&f32_samples);
                } else {
                    break;
                }
            }

            // Kill parec process
            let _ = child.kill();

            // Signal completion
            let _ = completion_tx.send_blocking(());
        });

        Ok(())
    }

    pub fn stop_loopback(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.core.stop()
    }
}

impl Default for LoopbackRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// === Trait Implementation ===

use crate::domain::traits::AudioRecording;

impl AudioRecording for LoopbackRecorder {
    fn start(&self) -> Result<()> {
        self.start_loopback()
    }

    fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.stop_loopback()
    }

    fn amplitude(&self) -> f32 {
        self.get_amplitude()
    }

    fn is_recording(&self) -> bool {
        self.core.is_recording()
    }
}
