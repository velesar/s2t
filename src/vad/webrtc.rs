//! WebRTC-based Voice Activity Detection.
//!
//! Uses the webrtc-vad crate for energy-based VAD.
//! Fast and lightweight, works well in quiet environments.

use crate::domain::traits::VoiceDetection;
use anyhow::Result;
use std::cell::RefCell;
use webrtc_vad::{Vad, VadMode};

const SAMPLE_RATE_HZ: u32 = 16000;
const FRAME_SIZE_MS: u32 = 30; // 30ms frames for VAD
const FRAME_SIZE_SAMPLES: usize = (SAMPLE_RATE_HZ as usize * FRAME_SIZE_MS as usize) / 1000;

/// WebRTC-based Voice Activity Detector.
///
/// # Thread Safety
///
/// This type is intentionally `!Send` and `!Sync` because the underlying
/// `webrtc_vad::Vad` type is not thread-safe. Create a new instance for
/// each thread that needs VAD functionality.
pub struct WebRtcVoiceDetector {
    vad: RefCell<Vad>,
    silence_threshold_ms: u32,
}

impl WebRtcVoiceDetector {
    /// Create a new VAD instance with default thresholds.
    pub fn new() -> Result<Self> {
        Self::with_thresholds(1000, 500)
    }

    /// Create a new VAD instance with custom thresholds.
    ///
    /// # Arguments
    /// * `silence_threshold_ms` - Duration of silence to trigger speech end
    /// * `_min_speech_duration_ms` - Minimum speech duration (currently unused)
    pub fn with_thresholds(
        silence_threshold_ms: u32,
        _min_speech_duration_ms: u32,
    ) -> Result<Self> {
        use webrtc_vad::SampleRate;
        let vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive);

        Ok(Self {
            vad: RefCell::new(vad),
            silence_threshold_ms,
        })
    }
}

impl Default for WebRtcVoiceDetector {
    fn default() -> Self {
        Self::new().expect("Failed to initialize WebRTC VAD")
    }
}

impl VoiceDetection for WebRtcVoiceDetector {
    fn is_speech(&self, samples: &[f32]) -> Result<bool> {
        if samples.len() < FRAME_SIZE_SAMPLES {
            return Ok(false);
        }

        // Convert f32 samples to i16 for VAD
        let i16_samples: Vec<i16> = samples
            .iter()
            .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect();

        let mut vad = self.vad.borrow_mut();
        let frame = &i16_samples[..FRAME_SIZE_SAMPLES.min(i16_samples.len())];
        let result = vad
            .is_voice_segment(frame)
            .map_err(|_| anyhow::anyhow!("Invalid frame length"))?;

        Ok(result)
    }

    fn detect_speech_end(&self, recent_samples: &[f32]) -> Result<bool> {
        let silence_needed = (self.silence_threshold_ms * SAMPLE_RATE_HZ / 1000) as usize;
        let mut consecutive_silence = 0;
        let mut had_speech = false;

        // Process frames in reverse order (most recent first)
        for chunk in recent_samples.chunks(FRAME_SIZE_SAMPLES).rev() {
            if chunk.len() < FRAME_SIZE_SAMPLES {
                continue;
            }
            if self.is_speech(chunk)? {
                had_speech = true;
                break;
            }
            consecutive_silence += chunk.len();
        }

        Ok(had_speech && consecutive_silence >= silence_needed)
    }

    fn reset(&self) {
        use webrtc_vad::SampleRate;
        *self.vad.borrow_mut() =
            Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webrtc_vad_new() {
        let vad = WebRtcVoiceDetector::new();
        assert!(vad.is_ok());
    }

    #[test]
    fn test_webrtc_vad_with_thresholds() {
        let vad = WebRtcVoiceDetector::with_thresholds(2000, 1000);
        assert!(vad.is_ok());
    }

    #[test]
    fn test_webrtc_vad_default() {
        let vad = WebRtcVoiceDetector::default();
        assert_eq!(vad.silence_threshold_ms, 1000);
    }

    #[test]
    fn test_webrtc_vad_silence_not_speech() {
        let vad = WebRtcVoiceDetector::new().unwrap();
        let silence = vec![0.0f32; FRAME_SIZE_SAMPLES];
        let result = vad.is_speech(&silence).unwrap();
        assert!(!result, "Silence should not be detected as speech");
    }

    #[test]
    fn test_webrtc_vad_short_samples_not_speech() {
        let vad = WebRtcVoiceDetector::new().unwrap();
        let short = vec![0.0f32; FRAME_SIZE_SAMPLES - 1];
        let result = vad.is_speech(&short).unwrap();
        assert!(!result, "Too-short samples should return false");
    }

    #[test]
    fn test_webrtc_vad_empty_samples_not_speech() {
        let vad = WebRtcVoiceDetector::new().unwrap();
        let result = vad.is_speech(&[]).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_webrtc_vad_detect_speech_end_pure_silence() {
        let vad = WebRtcVoiceDetector::with_thresholds(500, 200).unwrap();
        let silence = vec![0.0f32; FRAME_SIZE_SAMPLES * 100];
        let result = vad.detect_speech_end(&silence).unwrap();
        assert!(
            !result,
            "Pure silence should not trigger speech end (no speech preceded it)"
        );
    }

    #[test]
    fn test_webrtc_vad_detect_speech_end_short_input() {
        let vad = WebRtcVoiceDetector::new().unwrap();
        let short = vec![0.0f32; 10];
        let result = vad.detect_speech_end(&short).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_trait_is_speech_matches_inherent() {
        use crate::domain::traits::VoiceDetection;

        let vad = WebRtcVoiceDetector::new().unwrap();
        let silence = vec![0.0f32; FRAME_SIZE_SAMPLES];

        let trait_result = VoiceDetection::is_speech(&vad, &silence).unwrap();
        assert!(!trait_result);
    }

    #[test]
    fn test_trait_detect_speech_end_matches_inherent() {
        use crate::domain::traits::VoiceDetection;

        let vad = WebRtcVoiceDetector::new().unwrap();
        let silence = vec![0.0f32; FRAME_SIZE_SAMPLES * 50];

        let trait_result = VoiceDetection::detect_speech_end(&vad, &silence).unwrap();
        assert!(!trait_result);
    }

    #[test]
    fn test_trait_reset() {
        use crate::domain::traits::VoiceDetection;

        let vad = WebRtcVoiceDetector::new().unwrap();
        VoiceDetection::reset(&vad);

        let silence = vec![0.0f32; FRAME_SIZE_SAMPLES];
        let result = VoiceDetection::is_speech(&vad, &silence).unwrap();
        assert!(!result);
    }
}
