use anyhow::{Context, Result};
use async_channel::Receiver;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::core::{calculate_rms, to_mono, RecordingCore, WHISPER_SAMPLE_RATE};

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
    core: RecordingCore,
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            core: RecordingCore::new(),
        }
    }

    /// Get a reference to the shared samples buffer.
    /// Used by ContinuousRecorder to read accumulated samples.
    pub fn samples(&self) -> &Arc<Mutex<Vec<f32>>> {
        &self.core.samples
    }

    /// Get current audio amplitude (0.0 - 1.0 range, normalized RMS)
    pub fn get_amplitude(&self) -> f32 {
        self.core.get_amplitude()
    }

    pub fn start_recording(&self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("Не знайдено мікрофон")?;

        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        let handles = self.core.prepare_recording();

        let samples = handles.samples;
        let is_recording = handles.is_recording.clone();
        let is_recording_for_loop = handles.is_recording;
        let current_amplitude = handles.current_amplitude;
        let completion_tx = handles.completion_tx;

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
        self.core.stop()
    }
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

// === Trait Implementation ===

use crate::domain::traits::AudioRecording;

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
        self.core.is_recording()
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
    fn test_audio_recorder_default() {
        let recorder = AudioRecorder::default();
        assert!(!recorder.core.is_recording());
        assert!(recorder.core.samples.lock().unwrap().is_empty());
    }

    #[test]
    fn test_audio_recorder_amplitude_initially_zero() {
        let recorder = AudioRecorder::new();
        assert_eq!(recorder.get_amplitude(), 0.0);
    }

    #[test]
    fn test_audio_recorder_not_recording_initially() {
        let recorder = AudioRecorder::new();
        assert!(!recorder.core.is_recording());
    }

    #[test]
    fn test_audio_recording_trait_amplitude() {
        use crate::domain::traits::AudioRecording;
        let recorder = AudioRecorder::new();
        // Trait method should match direct method
        assert_eq!(
            AudioRecording::amplitude(&recorder),
            recorder.get_amplitude()
        );
    }

    #[test]
    fn test_audio_recording_trait_is_recording() {
        use crate::domain::traits::AudioRecording;
        let recorder = AudioRecorder::new();
        assert!(!AudioRecording::is_recording(&recorder));
    }

    #[test]
    fn test_stop_resets_amplitude() {
        let recorder = AudioRecorder::new();
        // Manually set amplitude to non-zero via prepare + handles
        let handles = recorder.core.prepare_recording();
        handles
            .current_amplitude
            .store(0.5_f32.to_bits(), Ordering::Relaxed);
        assert!(recorder.get_amplitude() > 0.0);

        // Stop should reset amplitude
        recorder.stop_recording();
        assert_eq!(recorder.get_amplitude(), 0.0);
    }
}
