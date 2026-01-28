use anyhow::{Context, Result};
use async_channel::Receiver;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const WHISPER_SAMPLE_RATE: u32 = 16000;

/// Convert multi-channel audio to mono by averaging channels.
fn to_mono(data: &[f32], channels: usize) -> Vec<f32> {
    if channels > 1 {
        data.chunks(channels)
            .map(|chunk| chunk.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        data.to_vec()
    }
}

/// Calculate normalized RMS amplitude for visualization (0.0 - 1.0).
fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum_squares / samples.len() as f32).sqrt();
    // Normalize: typical speech is ~0.05-0.15 RMS, scale so it reaches ~50%
    (rms * 6.0).min(1.0)
}

/// Create a high-quality sinc resampler for converting to 16kHz.
fn create_resampler(sample_rate: u32) -> Result<SincFixedIn<f32>> {
    let resample_ratio = WHISPER_SAMPLE_RATE as f64 / sample_rate as f64;
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    SincFixedIn::<f32>::new(
        resample_ratio,
        2.0, // max relative ratio (safety margin)
        params,
        1024, // chunk size
        1,    // mono channel
    )
    .context("Не вдалося створити ресемплер")
}

pub(crate) struct AudioRecorder {
    pub(crate) samples: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
    completion_rx: Arc<Mutex<Option<Receiver<()>>>>,
    /// Current audio amplitude (RMS), stored as u32 bits for atomic access
    current_amplitude: Arc<AtomicU32>,
}

impl AudioRecorder {
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
        let current_amplitude = self.current_amplitude.clone();

        let resampler = Arc::new(Mutex::new(create_resampler(sample_rate)?));

        thread::spawn(move || {
            let resampler = resampler.clone();
            let current_amplitude = current_amplitude.clone();

            let stream = device
                .build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        if !is_recording.load(Ordering::SeqCst) {
                            return;
                        }

                        let mono = to_mono(data, channels);

                        let amplitude = calculate_rms(&mono);
                        current_amplitude.store(amplitude.to_bits(), Ordering::Relaxed);

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
                                    let output_len = (chunk.len() as f64
                                        * resampler.output_frames_next() as f64
                                        / input_frames as f64)
                                        as usize;
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
        self.current_amplitude
            .store(0.0_f32.to_bits(), Ordering::Relaxed);
        let completion_rx = self.completion_rx.lock().unwrap().take();
        let samples = self.samples.lock().unwrap().clone();
        (samples, completion_rx)
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// === Trait Implementation ===

use crate::traits::AudioRecording;

impl AudioRecording for AudioRecorder {
    fn start(&self) -> Result<()> {
        self.start_recording()
    }

    fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.stop_recording()
    }

    fn amplitude(&self) -> f32 {
        self.get_amplitude()
    }

    fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_audio_recorder_default() {
        let recorder = AudioRecorder::default();
        assert!(!recorder.is_recording.load(Ordering::SeqCst));
        assert!(recorder.samples.lock().unwrap().is_empty());
    }

    #[test]
    fn test_audio_recorder_amplitude_initially_zero() {
        let recorder = AudioRecorder::new();
        assert_eq!(recorder.get_amplitude(), 0.0);
    }

    #[test]
    fn test_audio_recorder_not_recording_initially() {
        let recorder = AudioRecorder::new();
        assert!(!recorder.is_recording.load(Ordering::SeqCst));
    }

    #[test]
    fn test_audio_recording_trait_amplitude() {
        use crate::traits::AudioRecording;
        let recorder = AudioRecorder::new();
        // Trait method should match direct method
        assert_eq!(
            AudioRecording::amplitude(&recorder),
            recorder.get_amplitude()
        );
    }

    #[test]
    fn test_audio_recording_trait_is_recording() {
        use crate::traits::AudioRecording;
        let recorder = AudioRecorder::new();
        assert!(!AudioRecording::is_recording(&recorder));
    }

    #[test]
    fn test_stop_resets_amplitude() {
        let recorder = AudioRecorder::new();
        // Manually set amplitude to non-zero
        recorder
            .current_amplitude
            .store(0.5_f32.to_bits(), Ordering::Relaxed);
        assert!(recorder.get_amplitude() > 0.0);

        // Stop should reset amplitude
        recorder.stop_recording();
        assert_eq!(recorder.get_amplitude(), 0.0);
    }
}
