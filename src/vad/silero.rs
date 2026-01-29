//! Silero VAD - Neural network-based Voice Activity Detection.
//!
//! Uses the voice_activity_detector crate which bundles the Silero ONNX model.
//! More accurate than WebRTC VAD, especially in noisy environments.

use crate::domain::traits::VoiceDetection;
use anyhow::Result;
use std::cell::RefCell;
use voice_activity_detector::VoiceActivityDetector as SileroVad;

const SAMPLE_RATE_HZ: u32 = 16000;
/// Chunk size for Silero VAD at 16kHz (must be 512 samples per V5 model requirements)
const CHUNK_SIZE: usize = 512;

/// Silero-based Voice Activity Detector.
///
/// Uses a neural network model for more accurate speech detection.
/// Better performance in noisy environments compared to WebRTC VAD.
///
/// # Thread Safety
///
/// This type is intentionally `!Send` and `!Sync` because the underlying
/// model uses RefCell for interior mutability. Create a new instance for
/// each thread that needs VAD functionality.
pub struct SileroVoiceDetector {
    vad: RefCell<SileroVad>,
    threshold: f32,
    silence_threshold_ms: u32,
}

impl SileroVoiceDetector {
    /// Create a new Silero VAD instance with default settings.
    pub fn new() -> Result<Self> {
        Self::with_thresholds(0.5, 1000, 500)
    }

    /// Create a new Silero VAD instance with custom thresholds.
    ///
    /// # Arguments
    /// * `threshold` - Speech probability threshold (0.0-1.0), default 0.5
    /// * `silence_threshold_ms` - Duration of silence to trigger speech end
    /// * `_min_speech_duration_ms` - Minimum speech duration (currently unused)
    pub fn with_thresholds(
        threshold: f32,
        silence_threshold_ms: u32,
        _min_speech_duration_ms: u32,
    ) -> Result<Self> {
        let vad = SileroVad::builder()
            .sample_rate(SAMPLE_RATE_HZ)
            .chunk_size(CHUNK_SIZE)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create Silero VAD: {}", e))?;

        Ok(Self {
            vad: RefCell::new(vad),
            threshold,
            silence_threshold_ms,
        })
    }

    /// Get the configured speech probability threshold.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }
}

impl Default for SileroVoiceDetector {
    fn default() -> Self {
        Self::new().expect("Failed to initialize Silero VAD")
    }
}

impl VoiceDetection for SileroVoiceDetector {
    fn is_speech(&self, samples: &[f32]) -> Result<bool> {
        if samples.is_empty() {
            return Ok(false);
        }

        let mut vad = self.vad.borrow_mut();

        // Process samples in chunks of CHUNK_SIZE
        // Return true if any chunk has probability above threshold
        for chunk in samples.chunks(CHUNK_SIZE) {
            let probability = vad.predict(chunk.iter().copied());
            if probability >= self.threshold {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn detect_speech_end(&self, recent_samples: &[f32]) -> Result<bool> {
        let silence_needed = (self.silence_threshold_ms * SAMPLE_RATE_HZ / 1000) as usize;
        let mut consecutive_silence = 0;
        let mut had_speech = false;

        let mut vad = self.vad.borrow_mut();

        // Process in reverse order (most recent first) in CHUNK_SIZE chunks
        let chunks: Vec<_> = recent_samples.chunks(CHUNK_SIZE).collect();

        for chunk in chunks.iter().rev() {
            if chunk.len() < CHUNK_SIZE {
                // Skip partial chunks at boundaries
                continue;
            }

            let probability = vad.predict(chunk.iter().copied());

            if probability >= self.threshold {
                had_speech = true;
                break;
            }
            consecutive_silence += chunk.len();
        }

        // Check if we had speech and enough silence after it
        Ok(had_speech && consecutive_silence >= silence_needed)
    }

    fn reset(&self) {
        // Reset internal state
        self.vad.borrow_mut().reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silero_vad_new() {
        let vad = SileroVoiceDetector::new();
        assert!(vad.is_ok());
    }

    #[test]
    fn test_silero_vad_with_thresholds() {
        let vad = SileroVoiceDetector::with_thresholds(0.7, 2000, 1000);
        assert!(vad.is_ok());
        assert_eq!(vad.unwrap().threshold(), 0.7);
    }

    #[test]
    fn test_silero_vad_default() {
        let vad = SileroVoiceDetector::default();
        assert_eq!(vad.threshold, 0.5);
        assert_eq!(vad.silence_threshold_ms, 1000);
    }

    #[test]
    fn test_silero_vad_silence_not_speech() {
        let vad = SileroVoiceDetector::new().unwrap();
        // 1 second of silence
        let silence = vec![0.0f32; SAMPLE_RATE_HZ as usize];
        let result = vad.is_speech(&silence).unwrap();
        assert!(!result, "Silence should not be detected as speech");
    }

    #[test]
    fn test_silero_vad_empty_samples() {
        let vad = SileroVoiceDetector::new().unwrap();
        let result = vad.is_speech(&[]).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_silero_vad_detect_speech_end_pure_silence() {
        let vad = SileroVoiceDetector::with_thresholds(0.5, 500, 200).unwrap();
        let silence = vec![0.0f32; SAMPLE_RATE_HZ as usize * 2];
        let result = vad.detect_speech_end(&silence).unwrap();
        assert!(
            !result,
            "Pure silence should not trigger speech end (no speech preceded it)"
        );
    }

    #[test]
    fn test_trait_is_speech() {
        use crate::domain::traits::VoiceDetection;

        let vad = SileroVoiceDetector::new().unwrap();
        let silence = vec![0.0f32; SAMPLE_RATE_HZ as usize];

        let result = VoiceDetection::is_speech(&vad, &silence).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_trait_reset() {
        use crate::domain::traits::VoiceDetection;

        let vad = SileroVoiceDetector::new().unwrap();
        VoiceDetection::reset(&vad);

        let silence = vec![0.0f32; SAMPLE_RATE_HZ as usize];
        let result = VoiceDetection::is_speech(&vad, &silence).unwrap();
        assert!(!result);
    }
}
