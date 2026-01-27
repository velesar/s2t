use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const WHISPER_SAMPLE_RATE: u32 = 16000;

pub struct AudioRecorder {
    samples: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_recording(&self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("Не знайдено мікрофон")?;

        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        self.samples.lock().unwrap().clear();
        self.is_recording.store(true, Ordering::SeqCst);

        let samples = self.samples.clone();
        let is_recording = self.is_recording.clone();
        let is_recording_for_loop = self.is_recording.clone();

        let resample_ratio = WHISPER_SAMPLE_RATE as f32 / sample_rate as f32;

        thread::spawn(move || {
            let stream = device
                .build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !is_recording.load(Ordering::SeqCst) {
                            return;
                        }

                        // Convert to mono if stereo
                        let mono: Vec<f32> = if channels > 1 {
                            data.chunks(channels)
                                .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
                                .collect()
                        } else {
                            data.to_vec()
                        };

                        // Simple linear resampling to 16kHz
                        let resampled: Vec<f32> =
                            (0..((mono.len() as f32 * resample_ratio) as usize))
                                .map(|i| {
                                    let src_idx = (i as f32 / resample_ratio) as usize;
                                    mono.get(src_idx).copied().unwrap_or(0.0)
                                })
                                .collect();

                        samples.lock().unwrap().extend(resampled);
                    },
                    |err| eprintln!("Помилка запису: {}", err),
                    None,
                )
                .unwrap();

            stream.play().unwrap();

            while is_recording_for_loop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }
        });

        Ok(())
    }

    pub fn stop_recording(&self) -> Vec<f32> {
        self.is_recording.store(false, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(200));
        self.samples.lock().unwrap().clone()
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}
