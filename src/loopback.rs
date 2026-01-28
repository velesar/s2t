use anyhow::{Context, Result};
use async_channel::Receiver;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

const WHISPER_SAMPLE_RATE: u32 = 16000;

pub struct LoopbackRecorder {
    samples: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
    completion_rx: Arc<Mutex<Option<Receiver<()>>>>,
    /// Current audio amplitude (RMS), stored as u32 bits for atomic access
    current_amplitude: Arc<AtomicU32>,
}

impl LoopbackRecorder {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            completion_rx: Arc::new(Mutex::new(None)),
            current_amplitude: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Get current audio amplitude (0.0 - 1.0 range, normalized RMS)
    pub fn get_amplitude(&self) -> f32 {
        f32::from_bits(self.current_amplitude.load(Ordering::Relaxed))
    }

    /// Start recording from PipeWire loopback (system audio monitor)
    /// Note: This is a simplified MVP implementation
    /// For production, proper PipeWire API integration is needed
    pub fn start_loopback(&self) -> Result<()> {
        // For MVP, we'll use a fallback: try to use parec command-line tool
        // This is simpler and works on both PipeWire and PulseAudio
        // TODO: Implement proper PipeWire API integration
        
        self.samples.lock().unwrap().clear();
        self.is_recording.store(true, Ordering::SeqCst);

        // Create completion channel
        let (completion_tx, completion_rx) = async_channel::bounded::<()>(1);
        *self.completion_rx.lock().unwrap() = Some(completion_rx);

        let samples = self.samples.clone();
        let is_recording_for_loop = self.is_recording.clone();
        let current_amplitude = self.current_amplitude.clone();

        // Try to get default monitor source using pactl
        let monitor_source = std::process::Command::new("pactl")
            .args(&["list", "sources", "short"])
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

        let stdout = child.stdout.take().context("Не вдалося отримати stdout від parec")?;
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
                    if !f32_samples.is_empty() {
                        let sum_squares: f32 = f32_samples.iter().map(|s| s * s).sum();
                        let rms = (sum_squares / f32_samples.len() as f32).sqrt();
                        // Use 6.0 multiplier so typical audio reaches ~50% of scale
                        let normalized = (rms * 6.0).min(1.0);
                        current_amplitude.store(normalized.to_bits(), Ordering::Relaxed);
                    }

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
        self.is_recording.store(false, Ordering::SeqCst);
        self.current_amplitude.store(0.0_f32.to_bits(), Ordering::Relaxed);
        let completion_rx = self.completion_rx.lock().unwrap().take();
        let samples = self.samples.lock().unwrap().clone();
        (samples, completion_rx)
    }
}

impl Default for LoopbackRecorder {
    fn default() -> Self {
        Self::new()
    }
}
