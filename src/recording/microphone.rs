use anyhow::{Context, Result};
use async_channel::Receiver;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::core::{calculate_rms, RecordingCore, WHISPER_SAMPLE_RATE};

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

/// Convert multi-channel audio to mono into a pre-allocated buffer (zero allocation).
///
/// Returns the number of mono samples written.
fn to_mono_into(data: &[f32], channels: usize, mono_buf: &mut [f32]) -> usize {
    if channels <= 1 {
        let n = data.len().min(mono_buf.len());
        mono_buf[..n].copy_from_slice(&data[..n]);
        n
    } else {
        let n = (data.len() / channels).min(mono_buf.len());
        for (i, chunk) in data.chunks_exact(channels).take(n).enumerate() {
            mono_buf[i] = chunk.iter().sum::<f32>() / channels as f32;
        }
        n
    }
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
        let is_recording_for_consumer = handles.is_recording.clone();
        let is_recording_for_loop = handles.is_recording;
        let current_amplitude = handles.current_amplitude;
        let completion_tx = handles.completion_tx;

        // Lock-free SPSC ring buffer: CPAL callback (producer) → consumer thread.
        // 10 seconds of mono audio at native sample rate provides ample headroom.
        let ring_capacity = sample_rate as usize * 10;
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(ring_capacity);

        // Pre-allocate mono conversion buffer for the CPAL callback.
        // Sized for the largest expected callback frame (typical: 256–4096 samples).
        let max_callback_mono = 8192;
        let mut mono_buf = vec![0.0f32; max_callback_mono];

        thread::spawn(move || {
            // --- Consumer thread: reads from ring buffer, resamples, stores ---
            let consumer_handle = thread::spawn(move || {
                let mut resampler = match create_resampler(sample_rate) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Не вдалося створити ресемплер: {}", e);
                        return;
                    }
                };

                let input_frames = resampler.input_frames_next();
                let mut chunk_buf = vec![0.0f32; input_frames];
                let mut chunk_pos = 0usize;

                while is_recording_for_consumer.load(Ordering::SeqCst) {
                    let available = consumer.slots();
                    if available == 0 {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }

                    if let Ok(read_chunk) = consumer.read_chunk(available.min(4096)) {
                        let (first, second) = read_chunk.as_slices();

                        for slice in [first, second] {
                            for &sample in slice {
                                chunk_buf[chunk_pos] = sample;
                                chunk_pos += 1;

                                if chunk_pos == input_frames {
                                    let input = vec![std::mem::take(&mut chunk_buf)];
                                    if let Ok(output) = resampler.process(&input, None) {
                                        samples.lock().extend(&output[0]);
                                    }
                                    chunk_buf = input.into_iter().next().unwrap();
                                    let next_frames = resampler.input_frames_next();
                                    chunk_buf.resize(next_frames, 0.0);
                                    chunk_pos = 0;
                                }
                            }
                        }

                        read_chunk.commit_all();
                    }
                }

                // Flush remaining partial chunk
                if chunk_pos > 0 {
                    let input_len = chunk_buf.len();
                    chunk_buf[chunk_pos..input_len].fill(0.0);
                    let input = vec![chunk_buf];
                    if let Ok(output) = resampler.process(&input, None) {
                        let output_len = (chunk_pos as f64 * resampler.output_frames_next() as f64
                            / input_len as f64) as usize;
                        samples
                            .lock()
                            .extend(&output[0][..output_len.min(output[0].len())]);
                    }
                }
            });

            // --- CPAL audio callback: real-time safe (no locks, no allocations) ---
            let stream = match device.build_input_stream(
                &config.into(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !is_recording.load(Ordering::SeqCst) {
                        return;
                    }

                    // Convert to mono in pre-allocated buffer (zero allocation)
                    let mono_len = to_mono_into(data, channels, &mut mono_buf);
                    let mono = &mono_buf[..mono_len];

                    // Update amplitude for UI visualization (atomic, lock-free)
                    let amplitude = calculate_rms(mono);
                    current_amplitude.store(amplitude.to_bits(), Ordering::Relaxed);

                    // Write to lock-free SPSC ring buffer (non-blocking)
                    if let Ok(mut write_chunk) = producer.write_chunk(mono_len) {
                        let (first, second) = write_chunk.as_mut_slices();
                        let first_len = first.len();
                        first.copy_from_slice(&mono[..first_len]);
                        if !second.is_empty() {
                            second.copy_from_slice(&mono[first_len..]);
                        }
                        write_chunk.commit_all();
                    }
                    // If ring buffer is full, samples are dropped (preferable to blocking)
                },
                |err| eprintln!("Помилка запису: {}", err),
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Не вдалося створити аудіопотік: {}", e);
                    is_recording_for_loop.store(false, Ordering::SeqCst);
                    let _ = consumer_handle.join();
                    let _ = completion_tx.send_blocking(());
                    return;
                }
            };

            if let Err(e) = stream.play() {
                eprintln!("Не вдалося запустити аудіопотік: {}", e);
                is_recording_for_loop.store(false, Ordering::SeqCst);
                let _ = consumer_handle.join();
                let _ = completion_tx.send_blocking(());
                return;
            }

            while is_recording_for_loop.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }

            // Drop the stream first so no more callbacks fire
            drop(stream);

            // Wait for consumer thread to drain remaining ring buffer samples
            let _ = consumer_handle.join();

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
        assert!(recorder.core.samples.lock().is_empty());
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
        let handles = recorder.core.prepare_recording();
        handles
            .current_amplitude
            .store(0.5_f32.to_bits(), Ordering::Relaxed);
        assert!(recorder.get_amplitude() > 0.0);

        recorder.stop_recording();
        assert_eq!(recorder.get_amplitude(), 0.0);
    }

    #[test]
    fn test_to_mono_into_single_channel() {
        let data = [0.1, 0.2, 0.3];
        let mut buf = [0.0f32; 3];
        let n = to_mono_into(&data, 1, &mut buf);
        assert_eq!(n, 3);
        assert_eq!(&buf[..n], &data);
    }

    #[test]
    fn test_to_mono_into_stereo() {
        let data = [0.2, 0.4, 0.6, 0.8];
        let mut buf = [0.0f32; 2];
        let n = to_mono_into(&data, 2, &mut buf);
        assert_eq!(n, 2);
        assert!((buf[0] - 0.3).abs() < 1e-6);
        assert!((buf[1] - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_to_mono_into_larger_buffer() {
        let data = [0.5, 0.5];
        let mut buf = [0.0f32; 10];
        let n = to_mono_into(&data, 1, &mut buf);
        assert_eq!(n, 2);
        assert_eq!(buf[0], 0.5);
        assert_eq!(buf[1], 0.5);
    }
}
