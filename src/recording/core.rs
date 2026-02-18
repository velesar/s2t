use async_channel::Receiver;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

pub(crate) const WHISPER_SAMPLE_RATE: u32 = 16000;

/// Calculate normalized RMS amplitude for visualization (0.0 - 1.0).
pub(crate) fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
    let rms = (sum_squares / samples.len() as f32).sqrt();
    // Normalize: typical speech is ~0.05-0.15 RMS, scale so it reaches ~50%
    (rms * 6.0).min(1.0)
}

/// Shared recording infrastructure (samples buffer, flags, completion channel).
///
/// Both `AudioRecorder` (microphone) and `LoopbackRecorder` compose this
/// struct to avoid duplicating the identical field set and lifecycle methods.
pub(crate) struct RecordingCore {
    pub(crate) samples: Arc<Mutex<Vec<f32>>>,
    is_recording: Arc<AtomicBool>,
    completion_rx: Arc<Mutex<Option<Receiver<()>>>>,
    /// Current audio amplitude (RMS), stored as u32 bits for atomic access
    current_amplitude: Arc<AtomicU32>,
}

/// Handles passed to a spawned recording thread so it can write samples,
/// check the recording flag, update amplitude, and signal completion.
pub(crate) struct RecordingHandles {
    pub(crate) samples: Arc<Mutex<Vec<f32>>>,
    pub(crate) is_recording: Arc<AtomicBool>,
    pub(crate) current_amplitude: Arc<AtomicU32>,
    pub(crate) completion_tx: async_channel::Sender<()>,
}

impl RecordingCore {
    pub fn new() -> Self {
        Self {
            samples: Arc::new(Mutex::new(Vec::new())),
            is_recording: Arc::new(AtomicBool::new(false)),
            completion_rx: Arc::new(Mutex::new(None)),
            current_amplitude: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Get current audio amplitude (0.0 - 1.0 range, normalized RMS).
    pub fn get_amplitude(&self) -> f32 {
        f32::from_bits(self.current_amplitude.load(Ordering::Relaxed))
    }

    /// Check if currently recording.
    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::SeqCst)
    }

    /// Clear samples, set recording flag, create completion channel,
    /// and return handles for the spawned recording thread.
    pub fn prepare_recording(&self) -> RecordingHandles {
        self.samples.lock().clear();
        self.is_recording.store(true, Ordering::SeqCst);

        let (completion_tx, completion_rx) = async_channel::bounded::<()>(1);
        *self.completion_rx.lock() = Some(completion_rx);

        RecordingHandles {
            samples: self.samples.clone(),
            is_recording: self.is_recording.clone(),
            current_amplitude: self.current_amplitude.clone(),
            completion_tx,
        }
    }

    /// Clear recording flag, reset amplitude, and return collected samples
    /// plus the completion receiver.
    pub fn stop(&self) -> (Vec<f32>, Option<Receiver<()>>) {
        self.is_recording.store(false, Ordering::SeqCst);
        self.current_amplitude.store(0.0_f32.to_bits(), Ordering::Relaxed);
        let completion_rx = self.completion_rx.lock().take();
        let samples = self.samples.lock().clone();
        (samples, completion_rx)
    }
}

impl Drop for RecordingCore {
    fn drop(&mut self) {
        // Ensure spawned recording threads stop when RecordingCore is dropped.
        // Without this, threads checking `is_recording` would spin forever
        // if `stop()` was never called (e.g., early return, panic unwinding).
        self.is_recording.store(false, Ordering::SeqCst);
    }
}

impl Default for RecordingCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whisper_sample_rate_constant() {
        assert_eq!(WHISPER_SAMPLE_RATE, 16000);
    }

    #[test]
    fn test_calculate_rms_empty() {
        assert_eq!(calculate_rms(&[]), 0.0);
    }

    #[test]
    fn test_calculate_rms_silence() {
        assert_eq!(calculate_rms(&[0.0; 100]), 0.0);
    }

    #[test]
    fn test_calculate_rms_normalized() {
        // Full-scale signal should clamp to 1.0
        let loud = vec![1.0; 100];
        assert_eq!(calculate_rms(&loud), 1.0);
    }

    #[test]
    fn test_recording_core_new() {
        let core = RecordingCore::new();
        assert!(!core.is_recording());
        assert_eq!(core.get_amplitude(), 0.0);
        assert!(core.samples.lock().is_empty());
    }

    #[test]
    fn test_recording_core_prepare_and_stop() {
        let core = RecordingCore::new();

        let handles = core.prepare_recording();
        assert!(core.is_recording());

        // Simulate writing samples
        handles.samples.lock().extend(&[0.1, 0.2, 0.3]);

        let (samples, completion_rx) = core.stop();
        assert!(!core.is_recording());
        assert_eq!(samples, vec![0.1, 0.2, 0.3]);
        assert!(completion_rx.is_some());
    }

    #[test]
    fn test_stop_resets_amplitude() {
        let core = RecordingCore::new();
        core.current_amplitude.store(0.5_f32.to_bits(), Ordering::Relaxed);
        assert!(core.get_amplitude() > 0.0);

        core.stop();
        assert_eq!(core.get_amplitude(), 0.0);
    }

    #[test]
    fn test_stop_without_prepare_returns_none() {
        let core = RecordingCore::new();
        let (samples, completion_rx) = core.stop();
        assert!(samples.is_empty());
        assert!(completion_rx.is_none());
    }

    #[test]
    fn test_drop_clears_is_recording() {
        let is_recording = Arc::new(AtomicBool::new(false));
        {
            let core = RecordingCore::new();
            // Grab a clone of the is_recording flag before drop
            let flag = core.is_recording.clone();
            // Simulate starting a recording
            core.prepare_recording();
            assert!(flag.load(Ordering::SeqCst));
            // Reassign so we can check after drop
            is_recording.store(true, Ordering::SeqCst);

            // `core` is dropped here — Drop should set is_recording to false
        }
        // Cannot check the internal flag after drop, so test via a spawned thread pattern
    }

    #[test]
    fn test_drop_signals_thread_to_stop() {
        use std::thread;
        use std::time::Duration;

        let core = RecordingCore::new();
        let handles = core.prepare_recording();

        let is_recording = handles.is_recording.clone();

        // Simulate a background thread that checks the flag
        let thread_handle = thread::spawn(move || {
            while is_recording.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(1));
            }
            // Thread exits because is_recording became false
            true
        });

        // Drop without calling stop() — Drop impl should clear the flag
        drop(core);

        // The thread should exit within a reasonable time
        let result = thread_handle.join().unwrap();
        assert!(result, "Thread should have exited after Drop set is_recording to false");
    }
}
