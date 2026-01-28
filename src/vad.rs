use anyhow::Result;
use std::sync::{Arc, Mutex};
use webrtc_vad::{Vad, VadMode};

const SAMPLE_RATE_HZ: u32 = 16000;
const FRAME_SIZE_MS: u32 = 30; // 30ms frames for VAD
const FRAME_SIZE_SAMPLES: usize = (SAMPLE_RATE_HZ as usize * FRAME_SIZE_MS as usize) / 1000;

/// Voice Activity Detection for segmenting audio
pub struct VoiceActivityDetector {
    vad: Arc<Mutex<Vad>>,
    silence_threshold_ms: u32,
    min_speech_duration_ms: u32,
}

impl VoiceActivityDetector {
    /// Create a new VAD instance
    pub fn new() -> Result<Self> {
        Self::with_thresholds(1000, 500)
    }

    /// Create a new VAD instance with custom thresholds
    pub fn with_thresholds(silence_threshold_ms: u32, min_speech_duration_ms: u32) -> Result<Self> {
        use webrtc_vad::SampleRate;
        // SampleRate enum variants: Rate8kHz, Rate16kHz, Rate32kHz, Rate48kHz
        // VadMode::Aggressive is less sensitive to background noise than Quality
        let vad = Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, VadMode::Aggressive);

        Ok(Self {
            vad: Arc::new(Mutex::new(vad)),
            silence_threshold_ms,
            min_speech_duration_ms,
        })
    }

    /// Set silence threshold (ms) - how long silence before ending segment
    pub fn set_silence_threshold(&mut self, ms: u32) {
        self.silence_threshold_ms = ms;
    }

    /// Set minimum speech duration (ms) - minimum speech length for a valid segment
    pub fn set_min_speech_duration(&mut self, ms: u32) {
        self.min_speech_duration_ms = ms;
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

        let mut vad = self.vad.lock().unwrap();
        let frame = &i16_samples[..FRAME_SIZE_SAMPLES.min(i16_samples.len())];
        let result = vad.is_voice_segment(frame).map_err(|_| anyhow::anyhow!("Invalid frame length"))?;

        Ok(result)
    }

    /// Detect segments in audio stream
    /// Returns vector of (start_idx, end_idx) tuples for speech segments
    pub fn detect_segments(&self, samples: &[f32]) -> Result<Vec<(usize, usize)>> {
        let mut segments = Vec::new();
        let mut in_speech = false;
        let mut speech_start = 0;
        let mut silence_duration = 0;
        let mut speech_duration = 0;

        let silence_frames = (self.silence_threshold_ms * SAMPLE_RATE_HZ / 1000) as usize;
        let min_speech_frames = (self.min_speech_duration_ms * SAMPLE_RATE_HZ / 1000) as usize;

        // Process audio in frames
        for (i, chunk) in samples.chunks(FRAME_SIZE_SAMPLES).enumerate() {
            let frame_start = i * FRAME_SIZE_SAMPLES;
            let is_speech_frame = self.is_speech(chunk)?;

            if is_speech_frame {
                if !in_speech {
                    // Start of new speech segment
                    in_speech = true;
                    speech_start = frame_start;
                    speech_duration = 0;
                    silence_duration = 0;
                } else {
                    speech_duration += chunk.len();
                }
            } else {
                if in_speech {
                    silence_duration += chunk.len();

                    // Check if silence is long enough to end segment
                    if silence_duration >= silence_frames {
                        // End of speech segment
                        if speech_duration >= min_speech_frames {
                            segments.push((speech_start, frame_start));
                        }
                        in_speech = false;
                        silence_duration = 0;
                        speech_duration = 0; // Reset for next segment
                    }
                }
            }
        }

        // Handle segment that continues to end of audio
        if in_speech && speech_duration >= min_speech_frames {
            segments.push((speech_start, samples.len()));
        }

        Ok(segments)
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

impl Default for VoiceActivityDetector {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback: create with default settings even if VAD init fails
            // This allows code to compile but VAD won't work
            panic!("Failed to initialize VAD")
        })
    }
}
