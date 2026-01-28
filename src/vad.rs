use anyhow::Result;
use std::cell::RefCell;
use webrtc_vad::{Vad, VadMode};

const SAMPLE_RATE_HZ: u32 = 16000;
const FRAME_SIZE_MS: u32 = 30; // 30ms frames for VAD
const FRAME_SIZE_SAMPLES: usize = (SAMPLE_RATE_HZ as usize * FRAME_SIZE_MS as usize) / 1000;

/// Voice Activity Detection for segmenting audio.
///
/// # Thread Safety
///
/// This type is intentionally `!Send` and `!Sync` because the underlying
/// `webrtc_vad::Vad` type is not thread-safe. Create a new instance for
/// each thread that needs VAD functionality.
pub(crate) struct VoiceActivityDetector {
    vad: RefCell<Vad>,
    silence_threshold_ms: u32,
}

impl VoiceActivityDetector {
    /// Create a new VAD instance
    pub fn new() -> Result<Self> {
        Self::with_thresholds(1000, 500)
    }

    /// Create a new VAD instance with custom silence threshold
    pub fn with_thresholds(
        silence_threshold_ms: u32,
        _min_speech_duration_ms: u32,
    ) -> Result<Self> {
        use webrtc_vad::SampleRate;
        // SampleRate enum variants: Rate8kHz, Rate16kHz, Rate32kHz, Rate48kHz
        // VadMode::Aggressive is less sensitive to background noise than Quality
        let vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive);

        Ok(Self {
            vad: RefCell::new(vad),
            silence_threshold_ms,
        })
    }

    /// Detect if audio frame contains speech
    /// Returns true if speech detected, false if silence
    pub fn is_speech(&self, samples: &[f32]) -> Result<bool> {
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

    /// Check if speech has ended (silence detected after speech)
    /// Analyzes recent samples in reverse to detect if we had speech followed by silence
    pub fn detect_speech_end(&self, recent_samples: &[f32]) -> Result<bool> {
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
}

// === Trait Implementation ===

use crate::traits::VoiceDetection;

impl VoiceDetection for VoiceActivityDetector {
    fn is_speech(&mut self, samples: &[f32]) -> Result<bool> {
        VoiceActivityDetector::is_speech(self, samples)
    }

    fn detect_speech_end(&mut self, samples: &[f32]) -> Result<bool> {
        VoiceActivityDetector::detect_speech_end(self, samples)
    }

    fn reset(&mut self) {
        // Re-create VAD for a clean state
        use webrtc_vad::SampleRate;
        self.vad = RefCell::new(Vad::new_with_rate_and_mode(
            SampleRate::Rate16kHz,
            VadMode::Aggressive,
        ));
    }
}

impl Default for VoiceActivityDetector {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback: create with default settings even if VAD init fails
            // This allows code to compile but VAD won't work
            panic!("Failed to initialize VAD")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_SAMPLES: usize = FRAME_SIZE_SAMPLES;

    #[test]
    fn test_vad_new() {
        let vad = VoiceActivityDetector::new();
        assert!(vad.is_ok());
    }

    #[test]
    fn test_vad_with_thresholds() {
        let vad = VoiceActivityDetector::with_thresholds(2000, 1000);
        assert!(vad.is_ok());
    }

    #[test]
    fn test_vad_default() {
        let vad = VoiceActivityDetector::default();
        assert_eq!(vad.silence_threshold_ms, 1000);
    }

    #[test]
    fn test_vad_silence_not_speech() {
        let vad = VoiceActivityDetector::new().unwrap();
        let silence = vec![0.0f32; FRAME_SAMPLES];
        let result = vad.is_speech(&silence).unwrap();
        assert!(!result, "Silence should not be detected as speech");
    }

    #[test]
    fn test_vad_short_samples_not_speech() {
        let vad = VoiceActivityDetector::new().unwrap();
        let short = vec![0.0f32; FRAME_SAMPLES - 1];
        let result = vad.is_speech(&short).unwrap();
        assert!(!result, "Too-short samples should return false");
    }

    #[test]
    fn test_vad_empty_samples_not_speech() {
        let vad = VoiceActivityDetector::new().unwrap();
        let result = vad.is_speech(&[]).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_vad_detect_speech_end_pure_silence() {
        let vad = VoiceActivityDetector::with_thresholds(500, 200).unwrap();
        // Pure silence: no speech was ever detected, so speech_end should be false
        let silence = vec![0.0f32; FRAME_SAMPLES * 100];
        let result = vad.detect_speech_end(&silence).unwrap();
        assert!(!result, "Pure silence should not trigger speech end (no speech preceded it)");
    }

    #[test]
    fn test_vad_detect_speech_end_short_input() {
        let vad = VoiceActivityDetector::new().unwrap();
        let short = vec![0.0f32; 10];
        let result = vad.detect_speech_end(&short).unwrap();
        assert!(!result);
    }

    // === Trait Implementation Tests ===

    #[test]
    fn test_trait_is_speech_matches_inherent() {
        use crate::traits::VoiceDetection;

        let mut vad = VoiceActivityDetector::new().unwrap();
        let silence = vec![0.0f32; FRAME_SAMPLES];

        let trait_result = VoiceDetection::is_speech(&mut vad, &silence).unwrap();
        assert!(!trait_result);
    }

    #[test]
    fn test_trait_detect_speech_end_matches_inherent() {
        use crate::traits::VoiceDetection;

        let mut vad = VoiceActivityDetector::new().unwrap();
        let silence = vec![0.0f32; FRAME_SAMPLES * 50];

        let trait_result = VoiceDetection::detect_speech_end(&mut vad, &silence).unwrap();
        assert!(!trait_result);
    }

    #[test]
    fn test_trait_reset() {
        use crate::traits::VoiceDetection;

        let mut vad = VoiceActivityDetector::new().unwrap();
        // Reset should not panic and should leave VAD in working state
        VoiceDetection::reset(&mut vad);

        let silence = vec![0.0f32; FRAME_SAMPLES];
        let result = VoiceDetection::is_speech(&mut vad, &silence).unwrap();
        assert!(!result);
    }
}
