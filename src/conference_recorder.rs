use crate::audio::AudioRecorder;
use crate::loopback::LoopbackRecorder;
use anyhow::{Context, Result};
use async_channel::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct ConferenceRecorder {
    mic_recorder: Arc<AudioRecorder>,
    loopback_recorder: Arc<LoopbackRecorder>,
    start_time: Arc<Mutex<Option<Instant>>>,
}

impl ConferenceRecorder {
    pub fn new() -> Self {
        Self {
            mic_recorder: Arc::new(AudioRecorder::new()),
            loopback_recorder: Arc::new(LoopbackRecorder::new()),
            start_time: Arc::new(Mutex::new(None)),
        }
    }

    /// Start recording from both microphone and loopback simultaneously
    pub fn start_conference(&self) -> Result<()> {
        let start = Instant::now();
        *self.start_time.lock().unwrap() = Some(start);

        // Start both recorders
        self.mic_recorder
            .start_recording()
            .context("Не вдалося запустити запис мікрофона")?;
        self.loopback_recorder
            .start_loopback()
            .context("Не вдалося запустити запис системного аудіо")?;

        Ok(())
    }

    /// Stop recording and return synchronized samples from both sources
    pub fn stop_conference(
        &self,
    ) -> (
        Vec<f32>,                    // mic_samples
        Vec<f32>,                    // loopback_samples
        Option<Receiver<()>>,        // mic_completion
        Option<Receiver<()>>,        // loopback_completion
    ) {
        let (mic_samples, mic_completion) = self.mic_recorder.stop_recording();
        let (loopback_samples, loopback_completion) = self.loopback_recorder.stop_loopback();

        *self.start_time.lock().unwrap() = None;

        (
            mic_samples,
            loopback_samples,
            mic_completion,
            loopback_completion,
        )
    }

    /// Get amplitude from microphone
    pub fn get_mic_amplitude(&self) -> f32 {
        self.mic_recorder.get_amplitude()
    }

    /// Get amplitude from loopback
    pub fn get_loopback_amplitude(&self) -> f32 {
        self.loopback_recorder.get_amplitude()
    }
}

impl Default for ConferenceRecorder {
    fn default() -> Self {
        Self::new()
    }
}
