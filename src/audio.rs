use anyhow::{Context, Result};
use async_channel::Receiver;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const WHISPER_SAMPLE_RATE: u32 = 16000;

pub struct AudioRecorder {
    samples: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
    completion_rx: Arc<Mutex<Option<Receiver<()>>>>,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            completion_rx: Arc::new(Mutex::new(None)),
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

        // Create completion channel
        let (completion_tx, completion_rx) = async_channel::bounded::<()>(1);
        *self.completion_rx.lock().unwrap() = Some(completion_rx);

        let samples = self.samples.clone();
        let is_recording = self.is_recording.clone();
        let is_recording_for_loop = self.is_recording.clone();

        let resample_ratio = WHISPER_SAMPLE_RATE as f64 / sample_rate as f64;

        // Create high-quality sinc resampler with anti-aliasing
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let resampler = SincFixedIn::<f32>::new(
            resample_ratio,
            2.0, // max relative ratio (safety margin)
            params,
            1024, // chunk size
            1,    // mono channel
        )
        .context("Не вдалося створити ресемплер")?;
        let resampler = Arc::new(Mutex::new(resampler));

        thread::spawn(move || {
            let resampler = resampler.clone();

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

                        // Resample to 16kHz using high-quality sinc interpolation
                        let mut resampler = resampler.lock().unwrap();
                        let input_frames = resampler.input_frames_next();

                        // Process in chunks matching resampler's expected input size
                        for chunk in mono.chunks(input_frames) {
                            if chunk.len() == input_frames {
                                let input = vec![chunk.to_vec()];
                                if let Ok(output) = resampler.process(&input, None) {
                                    samples.lock().unwrap().extend(&output[0]);
                                }
                            } else {
                                // Pad the last chunk if needed
                                let mut padded = chunk.to_vec();
                                padded.resize(input_frames, 0.0);
                                let input = vec![padded];
                                if let Ok(output) = resampler.process(&input, None) {
                                    // Only take proportional output for partial input
                                    let output_len =
                                        (chunk.len() as f64 * resampler.output_frames_next() as f64
                                            / input_frames as f64) as usize;
                                    samples
                                        .lock()
                                        .unwrap()
                                        .extend(&output[0][..output_len.min(output[0].len())]);
                                }
                            }
                        }
                    },
                    |err| eprintln!("Помилка запису: {}", err),
                    None,
                )
                .unwrap();

            stream.play().unwrap();

            while is_recording_for_loop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }

            // Signal completion
            let _ = completion_tx.send_blocking(());
        });

        Ok(())
    }

    pub fn stop_recording(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.is_recording.store(false, Ordering::SeqCst);
        let completion_rx = self.completion_rx.lock().unwrap().take();
        let samples = self.samples.lock().unwrap().clone();
        (samples, completion_rx)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_recorder_new() {
        let recorder = AudioRecorder::new();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_audio_recorder_default() {
        let recorder = AudioRecorder::default();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_audio_recorder_initial_samples_empty() {
        let recorder = AudioRecorder::new();
        let (samples, _) = recorder.stop_recording();
        assert!(samples.is_empty());
    }

    #[test]
    fn test_stop_recording_returns_none_receiver_when_not_started() {
        let recorder = AudioRecorder::new();
        let (_, completion_rx) = recorder.stop_recording();
        assert!(completion_rx.is_none());
    }

    #[test]
    fn test_whisper_sample_rate_constant() {
        assert_eq!(WHISPER_SAMPLE_RATE, 16000);
    }

    #[test]
    fn test_multiple_recorders_independent() {
        let recorder1 = AudioRecorder::new();
        let recorder2 = AudioRecorder::new();

        // Вони мають бути незалежними
        assert!(!recorder1.is_recording());
        assert!(!recorder2.is_recording());
    }
}
